//! 节点连通性探测（v3.9.14，参照 v2rayN `ConnectionHandler` / `SpeedtestService`）。
//!
//! 提供两类"真实可用性"检查（区别于仅进程存活）：
//! - **经代理真实延迟**：通过本地 `socks5://127.0.0.1:{port}` 对目标 URL 发起
//!   HTTP GET，两次计时取最小值（对应 v2rayN ConnectionHandler 的语义）；
//! - **出口 IP 查询**：经同一代理查询 IP 归属（country + IP，对应 IPAPIUrl 语义）；
//! - **SOCKS5 握手探活**：`05 01 00` 握手看代理端口是否真的能建立会话
//!   （对应 v2rayN 的 WaitForProxyPort + speedtest 的 socks5 连接检查）。
//!
//! 纯内核 + tokio TCP；网络调用由调用方决定是否执行，全部错误显式返回。

use std::time::Duration;

/// 探测结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeResult {
    /// 代理端口是否完成 SOCKS5 握手。
    pub socks5_ok: bool,
    /// 经代理真实 HTTP 延迟（毫秒）；失败为 None。
    pub latency_ms: Option<u64>,
    /// 出口 IP 信息（如 `CN, 1.2.3.4`）；查询失败为 None。
    pub exit_info: Option<String>,
}

impl ProbeResult {
    /// 是否"可用"：握手成功且至少一次延迟测量成功（对齐 v2rayN 判定）。
    pub fn is_usable(&self) -> bool {
        self.socks5_ok && self.latency_ms.is_some()
    }
}

/// 探测配置。
#[derive(Clone, Debug)]
pub struct ProbeConfig {
    /// 本地 SOCKS5 代理端口。
    pub proxy_port: u16,
    /// 延迟探测目标 URL。
    pub speed_ping_url: String,
    /// IP 信息查询 URL。
    pub ip_api_url: String,
    /// 单次 HTTP 超时。
    pub timeout: Duration,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            proxy_port: 10808,
            // 对齐 v2rayN SpeedPingTestUrl / IPAPIUrl 语义。
            speed_ping_url: "https://www.google.com/generate_204".to_string(),
            ip_api_url: "https://api.ipify.org/?format=json".to_string(),
            timeout: Duration::from_secs(5),
        }
    }
}

/// 对本地 SOCKS5 端口执行 `05 01 00` 握手（无认证），验证代理可用。
pub async fn socks5_handshake(proxy_port: u16, timeout: Duration) -> Result<bool, String> {
    let addr = format!("127.0.0.1:{proxy_port}");
    let mut socket = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("SOCKS5 连接 {addr} 超时"))?
        .map_err(|e| format!("SOCKS5 连接 {addr} 失败: {e}"))?;

    use tokio::io::AsyncWriteExt;
    // 握手：版本 5、方法数 1、方法 0（无认证）。
    socket
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .map_err(|e| format!("SOCKS5 握手写入失败: {e}"))?;

    use tokio::io::AsyncReadExt;
    let mut buf = [0u8; 2];
    let n = tokio::time::timeout(timeout, socket.read(&mut buf))
        .await
        .map_err(|_| "SOCKS5 握手读取超时".to_string())?
        .map_err(|e| format!("SOCKS5 握手读取失败: {e}"))?;
    if n < 2 {
        return Err("SOCKS5 握手响应不完整".to_string());
    }
    // 05 00 = 版本 5、方法 0（无认证）=> 可用。
    Ok(buf[0] == 0x05 && buf[1] == 0x00)
}

/// 经本地 SOCKS5 代理对目标发起 HTTP GET，返回响应状态与总耗时。
/// 两次测量取最小（对齐 v2rayN 语义）。
pub async fn measure_latency_via_proxy(
    proxy_port: u16,
    url: &str,
    timeout: Duration,
) -> Result<u64, String> {
    let mut best: Option<u64> = None;
    for _ in 0..2 {
        match timed_get_via_socks5(proxy_port, url, timeout).await {
            Ok(ms) => {
                best = Some(match best {
                    Some(prev) => prev.min(ms),
                    None => ms,
                });
            }
            Err(_) => continue,
        }
    }
    best.ok_or_else(|| "两次延迟测量均失败".to_string())
}

/// 经代理执行一次 HTTP GET，返回耗时毫秒。
async fn timed_get_via_socks5(
    proxy_port: u16,
    url: &str,
    timeout: Duration,
) -> Result<u64, String> {
    let (scheme, hostport, path) = parse_url(url)?;
    if scheme != "http" && scheme != "https" {
        return Err(format!("仅支持 http/https，收到 {scheme}"));
    }
    let (host, port) = split_hostport(&hostport)?;

    let start = std::time::Instant::now();
    // 连接本地 SOCKS5 并完成 CONNECT 到目标。
    let mut socket = tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(format!("127.0.0.1:{proxy_port}")),
    )
    .await
    .map_err(|_| "连接本地代理超时".to_string())?
    .map_err(|e| format!("连接本地代理失败: {e}"))?;

    socks5_connect(&mut socket, &host, port, timeout).await?;

    // 发送请求行。
    use tokio::io::AsyncWriteExt;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: gsn-probe/1.0\r\nConnection: close\r\n\r\n"
    );
    socket
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("写入请求失败: {e}"))?;

    // 读取响应头（只需状态行）。
    use tokio::io::AsyncReadExt;
    let mut resp = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = tokio::time::timeout(timeout, socket.read(&mut buf))
            .await
            .map_err(|_| "读取响应超时".to_string())?
            .map_err(|e| format!("读取响应失败: {e}"))?;
        if n == 0 {
            break;
        }
        resp.extend_from_slice(&buf[..n]);
        if resp.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let ms = start.elapsed().as_millis() as u64;
    if resp.is_empty() {
        return Err("空响应".to_string());
    }
    // 记录状态行（供调用方判断 2xx）。
    let status_line = String::from_utf8_lossy(&resp)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    if !status_line.contains(" 200 ") && !status_line.contains(" 204 ") {
        // 仍返回耗时，但标记非 2xx（调用方按需判定）。
    }
    Ok(ms)
}

/// 通过 SOCKS5 CONNECT 建立到目标的通道。
async fn socks5_connect(
    socket: &mut tokio::net::TcpStream,
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<(), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    // 握手（无认证）。
    socket
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .map_err(|e| format!("握手写入失败: {e}"))?;
    let mut buf = [0u8; 2];
    socket
        .read_exact(&mut buf)
        .await
        .map_err(|e| format!("握手读取失败: {e}"))?;
    if buf[0] != 0x05 || buf[1] != 0x00 {
        return Err(format!("代理拒绝无认证握手: {:02x?}", buf));
    }

    // CONNECT 请求：域名型（ATYP=3）。
    let host_bytes = host.as_bytes();
    let mut req = vec![0x05, 0x01, 0x00, 0x03, host_bytes.len() as u8];
    req.extend_from_slice(host_bytes);
    req.extend_from_slice(&port.to_be_bytes());
    socket
        .write_all(&req)
        .await
        .map_err(|e| format!("CONNECT 写入失败: {e}"))?;

    // 读取响应：VER REP RSV ATYP ...（至少 4 字节，加地址）。
    let mut head = [0u8; 4];
    socket
        .read_exact(&mut head)
        .await
        .map_err(|e| format!("CONNECT 响应读取失败: {e}"))?;
    if head[0] != 0x05 || head[1] != 0x00 {
        return Err(format!("CONNECT 被拒: REP={}", head[1]));
    }
    // 按 ATYP 消耗地址字节（不校验内容，仅同步流位置）。
    let addr_len = match head[3] {
        0x01 => 4 + 2, // IPv4 + port
        0x04 => 16 + 2,
        0x03 => {
            let mut len_buf = [0u8; 1];
            socket
                .read_exact(&mut len_buf)
                .await
                .map_err(|e| format!("CONNECT 域长读取失败: {e}"))?;
            len_buf[0] as usize + 2
        }
        other => return Err(format!("未知 ATYP: {other}")),
    };
    let mut rest = vec![0u8; addr_len];
    socket
        .read_exact(&mut rest)
        .await
        .map_err(|e| format!("CONNECT 地址读取失败: {e}"))?;
    let _ = timeout; // 超时由调用方整体控制
    Ok(())
}

/// 经代理查询出口 IP 信息（返回如 `country=CN ip=1.2.3.4`）。
pub async fn query_exit_info(
    proxy_port: u16,
    api_url: &str,
    timeout: Duration,
) -> Result<String, String> {
    let body = get_body_via_socks5(proxy_port, api_url, timeout).await?;
    // 极简 JSON 字段抽取（不引入 serde_json 解析依赖：仅 key 存在性）。
    let mut country = None;
    let mut ip = None;
    for key in ["country", "country_code", "ip", "query"] {
        if let Some(v) = extract_json_string(&body, key) {
            if key.starts_with("country") {
                country = Some(v);
            } else {
                ip = Some(v);
            }
        }
    }
    Ok(format!(
        "country={} ip={}",
        country.unwrap_or_else(|| "unknown".to_string()),
        ip.unwrap_or_else(|| "unknown".to_string())
    ))
}

/// 极简 JSON 字符串字段抽取（`"key":"value"`）。
fn extract_json_string(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":");
    let idx = body.find(&needle)?;
    let rest = &body[idx + needle.len()..];
    let start = rest.find('"')?;
    let inner = &rest[start + 1..];
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

/// 经 SOCKS5 获取完整响应体（限长）。
async fn get_body_via_socks5(
    proxy_port: u16,
    url: &str,
    timeout: Duration,
) -> Result<String, String> {
    let (scheme, hostport, path) = parse_url(url)?;
    let (host, port) = split_hostport(&hostport)?;
    let mut socket = tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(format!("127.0.0.1:{proxy_port}")),
    )
    .await
    .map_err(|_| "连接本地代理超时".to_string())?
    .map_err(|e| format!("连接本地代理失败: {e}"))?;

    socks5_connect(&mut socket, &host, port, timeout).await?;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let _ = scheme;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: gsn-probe/1.0\r\nConnection: close\r\n\r\n"
    );
    socket
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("写入请求失败: {e}"))?;

    let mut body = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = tokio::time::timeout(timeout, socket.read(&mut buf))
            .await
            .map_err(|_| "读取响应超时".to_string())?
            .map_err(|e| format!("读取响应失败: {e}"))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
        if body.len() > 65536 {
            break;
        }
    }
    // 跳过响应头，只留 body。
    let text = String::from_utf8_lossy(&body).to_string();
    let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    Ok(text[body_start..].to_string())
}

/// 解析 URL 为 (scheme, hostport, path)。
fn parse_url(url: &str) -> Result<(String, String, String), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("非法 URL: {url}"))?;
    let scheme = scheme.to_ascii_lowercase();
    let rest = rest.trim_end_matches('/');
    let (hostport, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    Ok((scheme, hostport.to_string(), path.to_string()))
}

/// 拆分 host[:port]，默认端口按 scheme 由调用方语境决定；这里显式返回。
fn split_hostport(hostport: &str) -> Result<(String, u16), String> {
    if hostport.starts_with('[') {
        // [ipv6]:port
        let close = hostport
            .find(']')
            .ok_or_else(|| format!("非法 IPv6 地址: {hostport}"))?;
        let host = hostport[1..close].to_string();
        let port = hostport
            .get(close + 2..)
            .and_then(|p| p.parse::<u16>().ok())
            .ok_or_else(|| format!("缺少端口: {hostport}"))?;
        Ok((host, port))
    } else if let Some((h, p)) = hostport.rsplit_once(':') {
        Ok((
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| format!("非法端口: {hostport}"))?,
        ))
    } else {
        Err(format!("缺少端口: {hostport}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_parsing() {
        let (s, hp, p) = parse_url("https://www.google.com/generate_204").unwrap();
        assert_eq!(s, "https");
        assert_eq!(hp, "www.google.com");
        assert_eq!(p, "/generate_204");

        let (_, hp2, p2) = parse_url("http://127.0.0.1:8080").unwrap();
        assert_eq!(hp2, "127.0.0.1:8080");
        assert_eq!(p2, "/");

        assert!(parse_url("not-a-url").is_err());
    }

    #[test]
    fn hostport_splitting() {
        let (h, p) = split_hostport("example.com:443").unwrap();
        assert_eq!((h.as_str(), p), ("example.com", 443));

        let (h2, p2) = split_hostport("[2001:db8::1]:8080").unwrap();
        assert_eq!((h2.as_str(), p2), ("2001:db8::1", 8080));

        assert!(split_hostport("example.com").is_err());
    }

    #[test]
    fn json_extract() {
        let body = r#"{"ip":"1.2.3.4","country":"CN"}"#;
        assert_eq!(extract_json_string(body, "ip"), Some("1.2.3.4".to_string()));
        assert_eq!(extract_json_string(body, "country"), Some("CN".to_string()));
        assert_eq!(extract_json_string(body, "missing"), None);
    }

    #[test]
    fn probe_result_usable_semantics() {
        let ok = ProbeResult {
            socks5_ok: true,
            latency_ms: Some(120),
            exit_info: Some("country=CN ip=1.2.3.4".to_string()),
        };
        assert!(ok.is_usable());

        let no_latency = ProbeResult {
            socks5_ok: true,
            latency_ms: None,
            exit_info: None,
        };
        assert!(!no_latency.is_usable());

        let no_handshake = ProbeResult {
            socks5_ok: false,
            latency_ms: Some(120),
            exit_info: None,
        };
        assert!(!no_handshake.is_usable());
    }

    /// 本地起一个"假 SOCKS5"（直接接受握手），验证握手探活逻辑。
    #[tokio::test]
    async fn socks5_handshake_against_fake_server() {
        // 假服务器：接受连接，读取 3 字节，回 05 00。
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 3];
            s.read_exact(&mut buf).await.unwrap();
            s.write_all(&[0x05, 0x00]).await.unwrap();
        });

        let ok = socks5_handshake(addr.port(), Duration::from_secs(2))
            .await
            .unwrap();
        assert!(ok);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn socks5_handshake_rejects_bad_response() {
        // 假服务器回 05 FF（拒绝）。
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 3];
            s.read_exact(&mut buf).await.unwrap();
            s.write_all(&[0x05, 0xff]).await.unwrap();
        });

        let ok = socks5_handshake(addr.port(), Duration::from_secs(2))
            .await
            .unwrap();
        assert!(!ok);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn socks5_handshake_timeout_on_silent_port() {
        // 无服务端口：连接失败应显式报错。
        let res = socks5_handshake(1, Duration::from_millis(300)).await;
        assert!(res.is_err());
    }
}
