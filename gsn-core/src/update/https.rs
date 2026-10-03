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
use std::net::TcpStream;
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
    let addr = format!("{host}:{port}");
    let tcp = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|_| UpdateError::Http(format!("无法解析地址 {addr}")))?,
        timeout,
    )
    .map_err(|e| UpdateError::Http(format!("连接 {addr} 失败: {e}")))?;
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
}
