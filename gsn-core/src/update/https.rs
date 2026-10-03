//! 最小 HTTPS/1.1 客户端（同步、阻塞），仅供自动更新抓取 npm registry 与 GitHub
//! Release 资产使用。
//!
//! 设计取舍（见 docs 更新机制说明）：
//! - **不引入** reqwest/hyper/native-tls/OpenSSL。复用已在依赖树中的
//!   `rustls(ring)` + `webpki-roots`，无系统 OpenSSL 依赖，跨平台一致。
//! - 每次请求新建连接（更新检查极低频），`Connection: close` 读到 EOF 即结束。
//! - 跟随最多 4 次同协议(https) 3xx 跳转（GitHub Release 下载会 302 到对象存储）。
//! - 强制响应体上限与连接/读超时，避免被异常对端无限拖住（slow-loris 防护）。
//!
//! 本模块不保留任何连接池、不接受明文 http 跳转、不做任何凭据透传。

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use rustls::client::ClientConfig;
use rustls::pki_types::ServerName;
use rustls::{ClientConnection, RootCertStore, StreamOwned};

use super::UpdateError;

/// 一次 HTTP 响应的解码结果。
pub struct HttpResponse {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

fn build_tls_config() -> Result<ClientConfig, UpdateError> {
    // 显式注入 ring provider，不依赖进程级默认 provider（默认未安装时会 panic）。
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Ok(
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|e| UpdateError::Tls(format!("protocol versions: {e}")))?
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}

/// 把主机名解析为候选套接字地址。
///
/// 关键：**必须走 DNS 解析器**（`to_socket_addrs`），不能用
/// `"host:port".parse::<SocketAddr>()`——后者只接受 IP 字面量，对任何域名
/// （registry.npmjs.org / github.com）都会失败。v3.6.1 曾因此让更新器对域名完全不可用。
fn resolve_addrs(host: &str, port: u16) -> Result<Vec<std::net::SocketAddr>, UpdateError> {
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|e| UpdateError::Http(format!("DNS 解析 {host} 失败: {e}")))?
        .collect();
    if addrs.is_empty() {
        return Err(UpdateError::Http(format!("DNS 解析 {host} 未返回任何地址")));
    }
    Ok(addrs)
}

/// 依次尝试解析到的所有地址（IPv4/IPv6 多记录、Happy Eyeballs 的简化版），
/// 返回第一个连通的 TCP 流；全部失败时报最后一个错误。
fn connect_addrs(
    addrs: &[std::net::SocketAddr],
    timeout: Duration,
) -> Result<TcpStream, UpdateError> {
    let mut last_err: Option<std::io::Error> = None;
    for addr in addrs {
        match TcpStream::connect_timeout(addr, timeout) {
            Ok(tcp) => return Ok(tcp),
            Err(e) => last_err = Some(e),
        }
    }
    Err(UpdateError::Http(format!(
        "所有候选地址均不可达: {}",
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "无可用地址".to_string())
    )))
}

/// 解析后的正向代理。
///
/// 仅支持 **http 正向代理承载 `CONNECT` 隧道**（企业网络最常见形态：
/// `http://[user:pass@]proxy.host:3128`）。建立到目标的隧道后，TLS 仍在隧道内
/// 端到端进行，代理只看到 SNI/目标主机名，看不到明文内容。
/// 不支持把 https:// 代理（到代理本身先 TLS）作为本版目标——那种部署极少见，
/// 遇到时显式报错而非静默直连（避免在用户以为走代理时绕过代理）。
#[derive(Debug, PartialEq, Eq)]
struct Proxy {
    host: String,
    port: u16,
    /// 已做 base64 的 `user:pass`，用于 `Proxy-Authorization: Basic`。
    basic_auth: Option<String>,
}

/// 标准 Base64 编码（RFC 4648）。代理凭据极短，手写以免为此引入新依赖。
fn base64_encode(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((triple >> 18) & 0x3f) as usize] as char);
        out.push(T[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// 解析 `http://[user:pass@]host:port` 形态的代理地址。端口必填；非法形态显式报错。
fn parse_proxy_url(raw: &str) -> Result<Proxy, UpdateError> {
    let rest = raw
        .strip_prefix("http://")
        .ok_or_else(|| UpdateError::Http(format!("仅支持 http 正向代理(CONNECT)，拒绝: {raw}")))?;
    // 去掉 path/query，只留 [userinfo@]host:port。
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((u, hp)) => (Some(u), hp),
        None => (None, authority),
    };
    let (host, port) = hostport
        .rsplit_once(':')
        .ok_or_else(|| UpdateError::Http(format!("代理地址缺少端口: {raw}")))?;
    let port: u16 = port
        .parse()
        .map_err(|_| UpdateError::Http(format!("代理端口非法: {raw}")))?;
    if host.is_empty() {
        return Err(UpdateError::Http(format!("代理主机名为空: {raw}")));
    }
    let basic_auth = match userinfo {
        Some(u) if !u.is_empty() => {
            if !u.is_ascii() {
                return Err(UpdateError::Http("代理凭据仅支持 ASCII".to_string()));
            }
            Some(base64_encode(u.as_bytes()))
        }
        _ => None,
    };
    Ok(Proxy {
        host: host.to_string(),
        port,
        basic_auth,
    })
}

/// 判断目标主机是否命中 `NO_PROXY`（逗号分隔；支持精确、域后缀、前导点、`*`）。
fn no_proxy_matches(host: &str, no_proxy: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    for rule in no_proxy.split(',') {
        let mut r = rule.trim().to_ascii_lowercase();
        if r.is_empty() {
            continue;
        }
        if r == "*" {
            return true;
        }
        // 归一化前导 "*." 与 "."：`*.corp.example` / `.corp.example` 都按域后缀 `corp.example`。
        if let Some(rest) = r.strip_prefix("*.") {
            r = rest.to_string();
        }
        r = r.trim_start_matches('.').to_string();
        if r.is_empty() {
            continue;
        }
        if h == r || h.ends_with(&format!(".{r}")) {
            return true;
        }
    }
    false
}

/// 依据标准环境变量决定本次连接是否走代理。
///
/// 读取 `HTTPS_PROXY`/`https_proxy`（退而求其次 `HTTP_PROXY`/`http_proxy`），
/// 命中 `NO_PROXY`/`no_proxy` 则直连。代理变量存在但形态非法时**报错**而非直连，
/// 避免在用户明确要求走代理时被悄悄绕过。
fn proxy_from_env(target_host: &str) -> Result<Option<Proxy>, UpdateError> {
    let no_proxy = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .unwrap_or_default();
    if no_proxy_matches(target_host, &no_proxy) {
        return Ok(None);
    }
    let raw = ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .map(|v| v.trim().to_string());
    match raw {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) => parse_proxy_url(&v).map(Some),
    }
}

/// 读取 CONNECT 隧道响应，仅在 2xx 时放行。
fn parse_connect_ok(raw: &[u8]) -> Result<(), UpdateError> {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| UpdateError::Http("代理 CONNECT 响应缺少头部结束标记".to_string()))?;
    let header = std::str::from_utf8(&raw[..end])
        .map_err(|_| UpdateError::Http("代理 CONNECT 响应非 UTF-8".to_string()))?;
    let status_line = header
        .lines()
        .next()
        .ok_or_else(|| UpdateError::Http("代理 CONNECT 响应为空".to_string()))?;
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| UpdateError::Http(format!("代理 CONNECT 状态行非法: {status_line}")))?;
    if (200..300).contains(&code) {
        Ok(())
    } else if code == 407 {
        Err(UpdateError::Http(
            "代理要求认证或凭据被拒(407 Proxy Authentication Required)".to_string(),
        ))
    } else {
        Err(UpdateError::Http(format!(
            "代理 CONNECT 被拒: {status_line}"
        )))
    }
}

/// 经正向代理建立到 `host:port` 的 CONNECT 隧道，返回隧道上的明文 TCP 流
/// （调用方随后在其上做端到端 TLS）。
fn connect_via_proxy(
    px: &Proxy,
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<TcpStream, UpdateError> {
    let addrs = resolve_addrs(&px.host, px.port)?;
    let mut tcp = connect_addrs(&addrs, timeout)?;
    let _ = tcp.set_read_timeout(Some(timeout));
    let _ = tcp.set_write_timeout(Some(timeout));

    let mut req = format!(
        "CONNECT {host}:{port} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Proxy-Connection: keep-alive\r\n"
    );
    if let Some(a) = &px.basic_auth {
        req.push_str(&format!("Proxy-Authorization: Basic {a}\r\n"));
    }
    req.push_str("\r\n");
    tcp.write_all(req.as_bytes())
        .map_err(|e| UpdateError::Http(format!("写代理 CONNECT 失败: {e}")))?;
    tcp.flush()
        .map_err(|e| UpdateError::Http(format!("flush 代理 CONNECT 失败: {e}")))?;

    // 读到头部结束即止（CONNECT 成功后代理开始透明转发，不应吞掉后续 TLS 字节）。
    let mut buf = [0u8; 1024];
    let mut raw = Vec::new();
    loop {
        let n = tcp
            .read(&mut buf)
            .map_err(|e| UpdateError::Http(format!("读代理 CONNECT 响应失败: {e}")))?;
        if n == 0 {
            return Err(UpdateError::Http(
                "代理在 CONNECT 后直接关闭连接".to_string(),
            ));
        }
        raw.extend_from_slice(&buf[..n]);
        if raw.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if raw.len() > 8192 {
            return Err(UpdateError::Http("代理 CONNECT 响应头过大".to_string()));
        }
    }
    parse_connect_ok(&raw)?;
    // 进入隧道：读超时仍由后续 TLS 读阶段设置。
    Ok(tcp)
}

/// 建立到目标的明文连接：按环境变量决定直连或经代理 CONNECT。
fn dial(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, UpdateError> {
    match proxy_from_env(host)? {
        Some(px) => connect_via_proxy(&px, host, port, timeout),
        None => {
            let addrs = resolve_addrs(host, port)?;
            connect_addrs(&addrs, timeout)
        }
    }
}

/// 解析 `https://host[:port]/path?query`，返回 (host, port, authority-path)。
fn split_url(url: &str) -> Result<(String, u16, String), UpdateError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| UpdateError::Http(format!("仅支持 https，拒绝: {url}")))?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| UpdateError::Http(format!("非法端口: {url}")))?,
        ),
        None => (hostport.to_string(), 443),
    };
    if host.is_empty() {
        return Err(UpdateError::Http(format!("空主机名: {url}")));
    }
    Ok((host, port, path.to_string()))
}

/// 发起一次 https GET（不跟随跳转）。`max_bytes` 为响应体硬上限。
fn get_once(
    url: &str,
    cfg: &ClientConfig,
    timeout: Duration,
    max_bytes: usize,
) -> Result<HttpResponse, UpdateError> {
    let (host, port, path) = split_url(url)?;
    // 建立明文连接：命中 HTTPS_PROXY 走 CONNECT 隧道，否则 DNS 解析后直连。
    // TLS 在其上端到端进行；绝不把 "host:port" 当 IP 字面量 parse。
    let tcp = dial(&host, port, timeout)?;
    let _ = tcp.set_read_timeout(Some(timeout));
    let _ = tcp.set_write_timeout(Some(timeout));

    let server_name = ServerName::try_from(host.clone())
        .map_err(|e| UpdateError::Tls(format!("非法 SNI {host}: {e}")))?;
    let conn = ClientConnection::new(Arc::new(cfg.clone()), server_name)
        .map_err(|e| UpdateError::Tls(format!("TLS 握手初始化失败: {e}")))?;
    let mut tls = StreamOwned::new(conn, tcp);

    let req = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         User-Agent: agent-universe-updater/{ver}\r\n\
         Accept: application/octet-stream, application/json\r\n\
         Connection: close\r\n\r\n",
        ver = env!("CARGO_PKG_VERSION"),
    );
    tls.write_all(req.as_bytes())
        .map_err(|e| UpdateError::Http(format!("写请求失败: {e}")))?;
    tls.flush()
        .map_err(|e| UpdateError::Http(format!("flush 失败: {e}")))?;

    // 读到 EOF，受 max_bytes 约束。
    let mut raw = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        match tls.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if raw.len().saturating_add(n) > max_bytes {
                    return Err(UpdateError::BodyTooLarge);
                }
                raw.extend_from_slice(&buf[..n]);
            }
            // 对端正常关闭时 std 可能返回 UnexpectedEof（TLS close_notify 缺失），
            // 只要已经拿到完整响应头就容忍；在下面解析阶段判定。
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(UpdateError::Timeout);
            }
            Err(e) => return Err(UpdateError::Http(format!("读响应失败: {e}"))),
        }
    }

    decode_response(&raw)
}

/// 手工解析最小 HTTP/1.1 响应：状态行 + 头部 + 正文（支持 Content-Length；
/// 无 Content-Length 时使用读到的全部剩余字节，因为本连接是 close 模式）。
fn decode_response(raw: &[u8]) -> Result<HttpResponse, UpdateError> {
    let header_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| UpdateError::Http("响应缺少头部结束标记".to_string()))?;
    let header_bytes = &raw[..header_end];
    let header_str = std::str::from_utf8(header_bytes)
        .map_err(|_| UpdateError::Http("响应头非 UTF-8".into()))?;
    let mut lines = header_str.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| UpdateError::Http("空响应行".to_string()))?;
    let mut sp = status_line.split_whitespace();
    let _version = sp.next();
    let status: u16 = sp
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| UpdateError::Http(format!("非法状态行: {status_line}")))?;

    let mut content_length: Option<usize> = None;
    let mut location: Option<String> = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-length" => {
                    content_length = v.trim().parse::<usize>().ok();
                }
                "location" => location = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }

    let body = &raw[header_end + 4..];
    let body = match content_length {
        Some(len) => {
            if len > body.len() {
                return Err(UpdateError::Http(format!(
                    "响应被截断: 声明 {len} 字节，实得 {}",
                    body.len()
                )));
            }
            body[..len].to_vec()
        }
        None => body.to_vec(),
    };
    Ok(HttpResponse {
        status,
        location,
        body,
    })
}

/// 跟随 3xx 跳转的 GET（最多 4 跳，只允许 https）。
pub fn https_get(url: &str, timeout: Duration, max_bytes: usize) -> Result<Vec<u8>, UpdateError> {
    let cfg = build_tls_config()?;
    let mut current = url.to_string();
    for _ in 0..5 {
        let resp = get_once(&current, &cfg, timeout, max_bytes)?;
        match resp.status {
            200 => return Ok(resp.body),
            301 | 302 | 303 | 307 | 308 => {
                let loc = resp.location.ok_or_else(|| {
                    UpdateError::Http(format!("HTTP {} 跳转缺 Location", resp.status))
                })?;
                // 只允许 https，拒绝降级到明文或任何非 http(s) scheme。
                if !loc.starts_with("https://") {
                    return Err(UpdateError::Http(format!("拒绝非 https 跳转: {loc}")));
                }
                current = loc;
            }
            other => {
                return Err(UpdateError::Http(format!("HTTP {other} 来自 {current}")));
            }
        }
    }
    Err(UpdateError::Http("重定向次数过多".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_url_and_rejects_plain_http() {
        let (h, p, path) = split_url("https://registry.npmjs.org/x/y?a=1").unwrap();
        assert_eq!(h, "registry.npmjs.org");
        assert_eq!(p, 443);
        assert_eq!(path, "/x/y?a=1");
        let (h2, p2, _) = split_url("https://example.com:8443/").unwrap();
        assert_eq!(h2, "example.com");
        assert_eq!(p2, 8443);
        assert!(split_url("http://example.com/x").is_err());
    }

    #[test]
    fn decodes_content_length_response() {
        let body = b"{\"ok\":true}";
        let raw = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
            body.len()
        );
        let mut all = raw.into_bytes();
        all.extend_from_slice(body);
        let r = decode_response(&all).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, body);
    }

    #[test]
    fn detects_redirect_location() {
        let raw = b"HTTP/1.1 302 Found\r\nLocation: https://x/y\r\nContent-Length: 0\r\n\r\n";
        let r = decode_response(raw).unwrap();
        assert_eq!(r.status, 302);
        assert_eq!(r.location.as_deref(), Some("https://x/y"));
    }

    /// 回归（v3.6.1 域名不可用缺陷）：主机名必须经 DNS 解析得到地址，
    /// 而不是当作 IP 字面量 parse。旧实现 `"localhost:1".parse::<SocketAddr>()`
    /// 在一般平台返回错误，本测试在旧实现上会失败。
    ///
    /// 只做解析、不发起连接，因此不依赖外网（localhost 由本机解析器提供）。
    #[test]
    fn resolves_hostname_via_dns_not_ip_literal_parse() {
        // 旧实现等价写法，必须对域名失败——以此锚定缺陷确实存在。
        assert!("localhost:1".parse::<std::net::SocketAddr>().is_err());
        // 新实现：localhost 至少能解析出一个环回地址。
        let addrs = resolve_addrs("localhost", 1).expect("localhost 应可解析");
        assert!(!addrs.is_empty());
        assert!(addrs.iter().any(|a| a.ip().is_loopback()));
        // IP 字面量也仍然可解析。
        assert!(!resolve_addrs("127.0.0.1", 443).unwrap().is_empty());
    }

    #[test]
    fn base64_encodes_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn parses_proxy_urls_and_rejects_bad_forms() {
        let p = parse_proxy_url("http://proxy.corp:3128").unwrap();
        assert_eq!(
            p,
            Proxy {
                host: "proxy.corp".into(),
                port: 3128,
                basic_auth: None
            }
        );
        let with_auth = parse_proxy_url("http://agent:secret@10.0.0.1:8080").unwrap();
        assert_eq!(with_auth.host, "10.0.0.1");
        assert_eq!(with_auth.port, 8080);
        assert_eq!(with_auth.basic_auth.as_deref(), Some("YWdlbnQ6c2VjcmV0"));
        // 去 path/query。
        assert_eq!(parse_proxy_url("http://p:1/").unwrap().host, "p");
        // 拒绝 https 代理、无端口、空主机（非法配置必须报错，不能静默直连）。
        assert!(parse_proxy_url("https://p:3128").is_err());
        assert!(parse_proxy_url("http://proxy.corp").is_err());
        assert!(parse_proxy_url("http://:3128").is_err());
    }

    #[test]
    fn evaluates_no_proxy_rules() {
        let rules = "localhost, .corp.example, 10.0.0.1, *.internal";
        assert!(no_proxy_matches("localhost", rules));
        assert!(no_proxy_matches("api.corp.example", rules));
        assert!(no_proxy_matches("corp.example", rules));
        assert!(no_proxy_matches("10.0.0.1", rules));
        assert!(no_proxy_matches("anything.internal", rules));
        assert!(!no_proxy_matches("registry.npmjs.org", rules));
        assert!(!no_proxy_matches("notcorp.example", rules));
        // 大小写不敏感。
        assert!(no_proxy_matches("API.CORP.EXAMPLE", rules));
    }

    #[test]
    fn accepts_only_2xx_connect_response() {
        assert!(parse_connect_ok(b"HTTP/1.1 200 Connection established\r\n\r\n").is_ok());
        assert!(parse_connect_ok(b"HTTP/1.1 200 OK\r\nProxy-A: x\r\n\r\n").is_ok());
        let auth = parse_connect_ok(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n");
        assert!(auth.is_err());
        assert!(parse_connect_ok(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").is_err());
        assert!(parse_connect_ok(b"garbage without headers").is_err());
    }
}
