//! 本地 PAC（Proxy Auto-Config）服务（v3.9.14，参照 v2rayN `PacManager`）。
//!
//! 在本地起一个极简 HTTP 服务，返回 PAC 脚本；模板中的 `__PROXY__` 占位符
//! 被替换为 `PROXY 127.0.0.1:{httpPort};DIRECT;`。支持自定义 PAC 路径：
//! 若配置了 `CustomSystemProxyPacPath`，则直接返回该路径文件内容（保持
//! 其内 `__PROXY__` 占位替换逻辑，与 v2rayN 语义一致）。
//!
//! 纯内核（tokio TcpListener + 模板渲染），无外部依赖，可单测渲染逻辑。

use std::net::SocketAddr;

/// PAC 模板：`__PROXY__` 为占位符（对齐 v2rayN PacManager 的替换逻辑）。
pub const PAC_TEMPLATE: &str = r#"function FindProxyForURL(url, host) {
    if (shExpMatch(host, "*.local") || shExpMatch(host, "localhost")) {
        return "DIRECT";
    }
    return "__PROXY__";
}
"#;

/// PAC 服务配置。
#[derive(Clone, Debug)]
pub struct PacConfig {
    /// 本地监听地址（如 `127.0.0.1:1080`）。
    pub listen: SocketAddr,
    /// 填入 PAC 的本地代理端口（SOCKS/HTTP 入口端口）。
    pub proxy_port: u16,
    /// 自定义 PAC 文件路径；Some 时优先读该文件（保留 `__PROXY__` 占位替换）。
    pub custom_pac_path: Option<String>,
}

/// 渲染 PAC 脚本：替换 `__PROXY__` 占位符。
pub fn render_pac(proxy_port: u16) -> String {
    PAC_TEMPLATE.replace(
        "__PROXY__",
        &format!("PROXY 127.0.0.1:{proxy_port};DIRECT;"),
    )
}

/// 带自定义文件的渲染：保留自定义文件原样替换占位符。
pub fn render_pac_from_custom(template: &str, proxy_port: u16) -> String {
    template.replace(
        "__PROXY__",
        &format!("PROXY 127.0.0.1:{proxy_port};DIRECT;"),
    )
}

/// 启动本地 PAC 服务，返回后台任务句柄。
/// 服务持续监听直到任务被取消；每个请求返回渲染后的 PAC 文本。
pub async fn serve_pac(cfg: PacConfig) -> Result<tokio::task::JoinHandle<()>, String> {
    let listener = tokio::net::TcpListener::bind(cfg.listen)
        .await
        .map_err(|e| format!("PAC 服务监听 {} 失败: {e}", cfg.listen))?;

    let proxy_port = cfg.proxy_port;
    let custom_pac_path = cfg.custom_pac_path.clone();

    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _peer)) = listener.accept().await else {
                continue;
            };
            let custom = custom_pac_path.clone();
            tokio::spawn(async move {
                // 读取请求头（足够解析 GET 行即可）。
                let mut buf = [0u8; 4096];
                let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;

                // 渲染 PAC 内容：优先自定义文件，否则内置模板。
                let body = match &custom {
                    Some(path) => match std::fs::read_to_string(path) {
                        Ok(t) => render_pac_from_custom(&t, proxy_port),
                        Err(_) => render_pac(proxy_port),
                    },
                    None => render_pac(proxy_port),
                };

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes()).await;
            });
        }
    });

    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn render_replaces_placeholder() {
        let pac = render_pac(10808);
        assert!(pac.contains("PROXY 127.0.0.1:10808;DIRECT;"));
        assert!(!pac.contains("__PROXY__"));
        assert!(pac.contains("function FindProxyForURL"));
    }

    #[test]
    fn render_preserves_local_bypass() {
        let pac = render_pac(10808);
        assert!(pac.contains("shExpMatch(host, \"*.local\")"));
        assert!(pac.contains("return \"DIRECT\";"));
    }

    #[test]
    fn custom_template_also_replaced() {
        let custom = "return \"__PROXY__\";\n";
        let out = render_pac_from_custom(custom, 9000);
        assert_eq!(out, "return \"PROXY 127.0.0.1:9000;DIRECT;\";\n");
    }

    #[test]
    fn template_has_no_other_placeholders() {
        let pac = render_pac(1);
        // 渲染后不应残留任何占位符。
        assert!(!pac.contains("__"));
    }

    #[tokio::test]
    async fn serve_returns_pac_over_http() {
        // 绑定固定高位端口（serve_pac 内部绑定，返回句柄不含实际地址）。
        let cfg = PacConfig {
            listen: "127.0.0.1:39871".parse().unwrap(),
            proxy_port: 10808,
            custom_pac_path: None,
        };
        let handle = serve_pac(cfg.clone()).await.unwrap();

        // 等一小会儿确保监听就绪。
        tokio::time::sleep(Duration::from_millis(200)).await;

        let addr = cfg.listen;
        let client = reqwest_like_get(addr).await;
        assert!(client.contains("PROXY 127.0.0.1:10808;DIRECT;"));

        handle.abort();
    }

    /// 极简 HTTP GET（无外部依赖，手写 TCP）。
    async fn reqwest_like_get(addr: SocketAddr) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        let req = format!(
            "GET /pac HTTP/1.1\r\nHost: {}\r\nUser-Agent: gsn-test\r\nAccept: */*\r\n\r\n",
            addr
        );
        s.write_all(req.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).await.unwrap();
        String::from_utf8_lossy(&buf).to_string()
    }
}
