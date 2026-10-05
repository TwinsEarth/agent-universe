//! EVM JSON-RPC 适配器（手写 HTTPS/1.1 POST，无 reqwest/hyper）。
//!
//! 复用 `src/update/https.rs` 已验证的 TLS/代理思路：
//! - rustls(ring) + webpki-roots 根证书；
//! - 环境变量 `HTTPS_PROXY`/`https_proxy`/`HTTP_PROXY`/`http_proxy` + `NO_PROXY`；
//!   命中则走 `CONNECT` 隧道，TLS 在隧道内端到端。
//! - 每次请求新建连接，`Connection: close`，带超时与响应体上限。
//!
//! 本模块**不持有私钥**；交易字节来自 [`crate::chain::tx`]，地址来自
//! [`crate::chain::keys`]。错误信息绝不回显私钥/tx 敏感载荷（tx hash 除外，它是公开的）。
//!
//! # 诚实性
//!
//! 无真实 RPC/资金时，所有 `eth_sendRawTransaction` 一律标注「未验证」。本模块只保证
//! 请求**序列化正确**与响应**按 JSON-RPC 约定解析**。

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use rustls::client::ClientConfig;
use rustls::pki_types::ServerName;
use rustls::{ClientConnection, RootCertStore, StreamOwned};
use serde_json::Value;

use crate::chain::config::ChainError;

/// 默认请求超时。
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
/// 响应体硬上限（JSON-RPC 响应很小）。
const MAX_RESPONSE_BYTES: usize = 1 << 20; // 1 MiB

// ── 底层 HTTPS POST（自包含，逻辑同源 update/https.rs）──────────────────────

fn build_tls_config() -> Result<ClientConfig, ChainError> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Ok(
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|e| ChainError::Rpc(format!("TLS 协议版本: {e}")))?
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}

fn split_https_url(url: &str) -> Result<(String, u16, String), ChainError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| ChainError::Rpc(format!("RPC URL 必须是 https://，拒绝: {url:?}")))?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| ChainError::Rpc(format!("RPC URL 非法端口: {url:?}")))?,
        ),
        None => (hostport.to_string(), 443),
    };
    if host.is_empty() {
        return Err(ChainError::Rpc(format!("RPC URL 空主机名: {url:?}")));
    }
    Ok((host, port, path.to_string()))
}

fn resolve_addrs(host: &str, port: u16) -> Result<Vec<std::net::SocketAddr>, ChainError> {
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|e| ChainError::Rpc(format!("DNS 解析 {host} 失败: {e}")))?
        .collect();
    if addrs.is_empty() {
        return Err(ChainError::Rpc(format!("DNS 解析 {host} 无地址")));
    }
    Ok(addrs)
}

fn connect_addrs(
    addrs: &[std::net::SocketAddr],
    timeout: Duration,
) -> Result<TcpStream, ChainError> {
    let mut last: Option<std::io::Error> = None;
    for a in addrs {
        match TcpStream::connect_timeout(a, timeout) {
            Ok(t) => return Ok(t),
            Err(e) => last = Some(e),
        }
    }
    Err(ChainError::Rpc(format!(
        "RPC 节点不可达: {}",
        last.map(|e| e.to_string())
            .unwrap_or_else(|| "无地址".into())
    )))
}

fn base64_encode(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for c in input.chunks(3) {
        let b0 = c[0] as u32;
        let b1 = if c.len() > 1 { c[1] as u32 } else { 0 };
        let b2 = if c.len() > 2 { c[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((triple >> 18) & 0x3f) as usize] as char);
        out.push(T[((triple >> 12) & 0x3f) as usize] as char);
        if c.len() > 1 {
            out.push(T[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if c.len() > 2 {
            out.push(T[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[derive(Debug)]
struct Proxy {
    host: String,
    port: u16,
    basic_auth: Option<String>,
}

fn parse_proxy_url(raw: &str) -> Result<Proxy, ChainError> {
    let rest = raw
        .strip_prefix("http://")
        .ok_or_else(|| ChainError::Rpc(format!("仅支持 http 正向代理(CONNECT)，拒绝: {raw:?}")))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((u, hp)) => (Some(u), hp),
        None => (None, authority),
    };
    let (host, port) = hostport
        .rsplit_once(':')
        .ok_or_else(|| ChainError::Rpc(format!("代理缺端口: {raw:?}")))?;
    let port: u16 = port
        .parse()
        .map_err(|_| ChainError::Rpc(format!("代理端口非法: {raw:?}")))?;
    if host.is_empty() {
        return Err(ChainError::Rpc(format!("代理空主机: {raw:?}")));
    }
    let basic_auth = match userinfo {
        Some(u) if !u.is_empty() => Some(base64_encode(u.as_bytes())),
        _ => None,
    };
    Ok(Proxy {
        host: host.to_string(),
        port,
        basic_auth,
    })
}

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
        if let Some(rest) = r.strip_prefix("*.") {
            r = rest.to_string();
        }
        let r = r.trim_start_matches('.').to_string();
        if r.is_empty() {
            continue;
        }
        if h == r || h.ends_with(&format!(".{r}")) {
            return true;
        }
    }
    false
}

fn proxy_from_env(target_host: &str) -> Result<Option<Proxy>, ChainError> {
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

fn connect_via_proxy(
    px: &Proxy,
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<TcpStream, ChainError> {
    let addrs = resolve_addrs(&px.host, px.port)?;
    let mut tcp = connect_addrs(&addrs, timeout)?;
    let _ = tcp.set_read_timeout(Some(timeout));
    let _ = tcp.set_write_timeout(Some(timeout));
    let mut req = format!(
        "CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\nProxy-Connection: keep-alive\r\n"
    );
    if let Some(a) = &px.basic_auth {
        req.push_str(&format!("Proxy-Authorization: Basic {a}\r\n"));
    }
    req.push_str("\r\n");
    tcp.write_all(req.as_bytes())
        .map_err(|e| ChainError::Rpc(format!("写 CONNECT 失败: {e}")))?;
    tcp.flush()
        .map_err(|e| ChainError::Rpc(format!("flush CONNECT 失败: {e}")))?;
    let mut buf = [0u8; 1024];
    let mut raw = Vec::new();
    loop {
        let n = tcp
            .read(&mut buf)
            .map_err(|e| ChainError::Rpc(format!("读 CONNECT 响应失败: {e}")))?;
        if n == 0 {
            return Err(ChainError::Rpc("代理 CONNECT 后关闭".into()));
        }
        raw.extend_from_slice(&buf[..n]);
        if raw.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if raw.len() > 8192 {
            return Err(ChainError::Rpc("CONNECT 响应头过大".into()));
        }
    }
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| ChainError::Rpc("CONNECT 无头部结束".into()))?;
    let status_line = std::str::from_utf8(&raw[..end])
        .map_err(|_| ChainError::Rpc("CONNECT 响应非 UTF-8".into()))?
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ChainError::Rpc(format!("CONNECT 状态行非法: {status_line}")))?;
    if !(200..300).contains(&code) {
        return Err(ChainError::Rpc(format!("代理 CONNECT 被拒: {status_line}")));
    }
    Ok(tcp)
}

fn dial(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, ChainError> {
    match proxy_from_env(host)? {
        Some(px) => connect_via_proxy(&px, host, port, timeout),
        None => {
            let addrs = resolve_addrs(host, port)?;
            connect_addrs(&addrs, timeout)
        }
    }
}

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

fn decode_http_response(raw: &[u8]) -> Result<HttpResponse, ChainError> {
    let header_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| ChainError::Rpc("RPC 响应无头部结束".into()))?;
    let header_str = std::str::from_utf8(&raw[..header_end])
        .map_err(|_| ChainError::Rpc("RPC 响应头非 UTF-8".into()))?;
    let mut lines = header_str.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ChainError::Rpc(format!("RPC 状态行非法: {status_line}")))?;
    let mut content_length: Option<usize> = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_length = v.trim().parse::<usize>().ok();
            }
        }
    }
    let body = &raw[header_end + 4..];
    let body = match content_length {
        Some(len) => {
            if len > body.len() {
                return Err(ChainError::Rpc(format!(
                    "RPC 响应截断: 声明 {len}, 实得 {}",
                    body.len()
                )));
            }
            body[..len].to_vec()
        }
        None => body.to_vec(),
    };
    Ok(HttpResponse { status, body })
}

/// 同步：对 `https://` RPC 端点发一个 JSON-RPC POST。
fn https_post(url: &str, json_body: &str, timeout: Duration) -> Result<HttpResponse, ChainError> {
    let cfg = build_tls_config()?;
    let (host, port, path) = split_https_url(url)?;
    let tcp = dial(&host, port, timeout)?;
    let _ = tcp.set_read_timeout(Some(timeout));
    let _ = tcp.set_write_timeout(Some(timeout));
    let server_name = ServerName::try_from(host.clone())
        .map_err(|e| ChainError::Rpc(format!("非法 SNI {host}: {e}")))?;
    let conn = ClientConnection::new(Arc::new(cfg), server_name)
        .map_err(|e| ChainError::Rpc(format!("TLS 握手失败: {e}")))?;
    let mut tls = StreamOwned::new(conn, tcp);

    let req = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {clen}\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\r\n{body}",
        clen = json_body.len(),
        body = json_body,
    );
    tls.write_all(req.as_bytes())
        .map_err(|e| ChainError::Rpc(format!("写 RPC 请求失败: {e}")))?;
    tls.flush()
        .map_err(|e| ChainError::Rpc(format!("flush RPC 失败: {e}")))?;

    let mut raw = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        match tls.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if raw.len().saturating_add(n) > MAX_RESPONSE_BYTES {
                    return Err(ChainError::Rpc("RPC 响应体过大".into()));
                }
                raw.extend_from_slice(&buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(ChainError::Rpc("RPC 超时".into()));
            }
            Err(e) => return Err(ChainError::Rpc(format!("读 RPC 响应失败: {e}"))),
        }
    }
    decode_http_response(&raw)
}

// ── JSON-RPC 客户端 ────────────────────────────────────────────────────────

/// 一个 EVM JSON-RPC 客户端（轻量、无连接池）。
#[derive(Debug, Clone)]
pub struct RpcClient {
    url: String,
    timeout: Duration,
}

impl RpcClient {
    pub fn new(url: &str) -> Result<Self, ChainError> {
        // 预检 URL 形态；不实际连接。
        split_https_url(url)?;
        Ok(Self {
            url: url.to_string(),
            timeout: DEFAULT_TIMEOUT,
        })
    }

    /// 同步底层：发 method + params，解析 JSON-RPC result。
    fn call_blocking(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let body_str = serde_json::to_string(&body)
            .map_err(|e| ChainError::Rpc(format!("RPC 请求序列化: {e}")))?;
        let resp = https_post(&self.url, &body_str, self.timeout)?;
        if !(200..300).contains(&resp.status) {
            return Err(ChainError::Rpc(format!("HTTP {} from RPC", resp.status)));
        }
        let v: Value = serde_json::from_slice(&resp.body)
            .map_err(|e| ChainError::Rpc(format!("RPC 响应非 JSON: {e}")))?;
        if let Some(err) = v.get("error") {
            return Err(ChainError::Rpc(format!("RPC 节点错误: {err}")));
        }
        v.get("result")
            .cloned()
            .ok_or_else(|| ChainError::Rpc("RPC 响应缺少 result 字段".into()))
    }

    /// 异步包装：把阻塞的网络调用丢到 blocking 线程池。
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        let client = self.clone();
        let method = method.to_string();
        tokio::task::spawn_blocking(move || client.call_blocking(&method, params))
            .await
            .map_err(|e| ChainError::Rpc(format!("blocking join: {e}")))?
    }
}

/// 把 hex "0x..." 解析成 u64。
pub fn hex_u64(v: &Value) -> Result<u64, ChainError> {
    let s = v
        .as_str()
        .ok_or_else(|| ChainError::Rpc(format!("期望 hex 字符串，得到 {v}")))?;
    let hex = s.strip_prefix("0x").unwrap_or(s);
    if hex.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(hex, 16).map_err(|e| ChainError::Rpc(format!("hex u64 解析 {s}: {e}")))
}

/// 把 hex "0x..." 解析成 u128。
pub fn hex_u128(v: &Value) -> Result<u128, ChainError> {
    let s = v
        .as_str()
        .ok_or_else(|| ChainError::Rpc(format!("期望 hex 字符串，得到 {v}")))?;
    let hex = s.strip_prefix("0x").unwrap_or(s);
    if hex.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(hex, 16).map_err(|e| ChainError::Rpc(format!("hex u128 解析 {s}: {e}")))
}

impl RpcClient {
    /// eth_chainId。
    pub async fn chain_id(&self) -> Result<u64, ChainError> {
        let v = self.call("eth_chainId", Value::Array(vec![])).await?;
        hex_u64(&v)
    }

    /// eth_blockNumber。
    pub async fn block_number(&self) -> Result<u64, ChainError> {
        let v = self.call("eth_blockNumber", Value::Array(vec![])).await?;
        hex_u64(&v)
    }

    /// eth_getTransactionCount(addr, "latest") → nonce。
    pub async fn get_nonce(&self, address_hex: &str) -> Result<u64, ChainError> {
        let v = self
            .call(
                "eth_getTransactionCount",
                Value::Array(vec![
                    Value::String(address_hex.to_string()),
                    Value::String("latest".to_string()),
                ]),
            )
            .await?;
        hex_u64(&v)
    }

    /// eth_estimateGas（返回 gas 估算；调用方加安全余量）。
    pub async fn estimate_gas(&self, tx: Value) -> Result<u64, ChainError> {
        let v = self.call("eth_estimateGas", Value::Array(vec![tx])).await?;
        hex_u64(&v)
    }

    /// eth_sendRawTransaction(raw_tx_hex) → tx hash（仅返回节点接受的 hash；
    /// 是否被打包/上链需后续 receipt 查询确认）。
    pub async fn send_raw_transaction(&self, raw_tx_hex: &str) -> Result<String, ChainError> {
        let v = self
            .call(
                "eth_sendRawTransaction",
                Value::Array(vec![Value::String(raw_tx_hex.to_string())]),
            )
            .await?;
        v.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ChainError::Rpc("sendRawTransaction 未返回 hash".into()))
    }

    /// eth_getTransactionReceipt(txhash)。返回 None 表示尚未打包。
    pub async fn get_receipt(&self, txhash: &str) -> Result<Option<Value>, ChainError> {
        let v = self
            .call(
                "eth_getTransactionReceipt",
                Value::Array(vec![Value::String(txhash.to_string())]),
            )
            .await?;
        if v.is_null() {
            Ok(None)
        } else {
            Ok(Some(v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_https_url() {
        let (h, p, path) = split_https_url("https://sepolia.base.org/rpc").unwrap();
        assert_eq!(h, "sepolia.base.org");
        assert_eq!(p, 443);
        assert_eq!(path, "/rpc");
        let (h, p, _) = split_https_url("https://arb1.arbitrum.io:8545/x").unwrap();
        assert_eq!(h, "arb1.arbitrum.io");
        assert_eq!(p, 8545);
        assert!(split_https_url("http://x/y").is_err());
    }

    #[test]
    fn parses_hex_numbers() {
        assert_eq!(hex_u64(&serde_json::json!("0x1")).unwrap(), 1);
        assert_eq!(hex_u64(&serde_json::json!("0x84532")).unwrap(), 0x84532);
        assert_eq!(hex_u64(&serde_json::json!("0x0")).unwrap(), 0);
        assert_eq!(hex_u64(&serde_json::json!("0x")).unwrap(), 0);
        assert_eq!(
            hex_u128(&serde_json::json!("0xde0b6b3a7640000")).unwrap(),
            1_000_000_000_000_000_000
        );
    }

    #[test]
    fn decodes_http_response_with_content_length() {
        let body = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"0x1\"}";
        let raw = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
            body.len()
        );
        let mut all = raw.into_bytes();
        all.extend_from_slice(body);
        let r = decode_http_response(&all).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(&r.body, body);
    }

    #[test]
    fn rejects_non_https_client_construction() {
        assert!(RpcClient::new("http://example.com/rpc").is_err());
        assert!(RpcClient::new("https://rpc.example:8545/").is_ok());
    }

    #[test]
    fn no_proxy_rules_work() {
        assert!(no_proxy_matches("foo.internal", ".internal, localhost"));
        assert!(!no_proxy_matches("evil.com", ".internal"));
    }
}
