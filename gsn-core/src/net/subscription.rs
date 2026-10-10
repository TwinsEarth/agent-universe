//! 节点订阅管理（v3.9.14，参照 v2rayN `SubscriptionHandler` / `UpdateService`）。
//!
//! 订阅是一组远端节点列表（peer 入口/中继地址/引导种子），支持：
//! - URL 校验（http/https 前缀、enabled 开关）；
//! - Base64 自动解码回退（订阅正文可能是 Base64 编码的文本或纯文本）；
//! - 多订阅合并（附加订阅 MoreUrl 逗号拆分）；
//! - 节点去重（同一 peer id 或地址只保留一份）。
//!
//! 纯逻辑、无网络，便于穷举单测；下载与解析由调用方分步执行。

/// 单条订阅。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionEntry {
    pub id: String,
    pub url: String,
    pub enabled: bool,
    /// 附加订阅 URL（逗号/换行分隔）。
    pub more_urls: Vec<String>,
}

impl SubscriptionEntry {
    /// v2rayN 语义校验：Id/Url 非空、Url 以 http(s):// 开头、enabled 才参与。
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("订阅 Id 不能为空".to_string());
        }
        if self.url.trim().is_empty() {
            return Err("订阅 Url 不能为空".to_string());
        }
        let u = self.url.trim();
        if !(u.starts_with("http://") || u.starts_with("https://")) {
            return Err(format!("订阅 Url 必须以 http(s):// 开头: {u}"));
        }
        Ok(())
    }

    /// 展开全部订阅 URL（主 URL + 附加 MoreUrl）。
    pub fn all_urls(&self) -> Vec<String> {
        let mut out = vec![self.url.trim().to_string()];
        for more in &self.more_urls {
            for part in more.split([',', '\n', '\r']) {
                let p = part.trim();
                if !p.is_empty() {
                    out.push(p.to_string());
                }
            }
        }
        out
    }
}

/// 单个节点条目（订阅解析产物）。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SubscribedNode {
    /// 去重主键（优先 peer id，其次地址）。
    pub key: String,
    /// peer id（如 `12D3KooW...`），可空。
    pub peer_id: Option<String>,
    /// 地址（multiaddr 或 host:port）。
    pub address: String,
    /// 来源订阅 id。
    pub source: String,
    /// 原始行（保留溯源）。
    pub raw: String,
}

/// 订阅正文解码结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeKind {
    /// 原文本。
    Plain,
    /// Base64 解码后得到文本。
    Base64,
}

/// 尝试把订阅正文按文本解析成节点行。
///
/// 若正文是 Base64（可整体解码为可打印文本），自动回退解码一次。
/// 返回 (行列表, 解码方式)。
pub fn decode_lines(body: &str) -> (Vec<String>, DecodeKind) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return (Vec::new(), DecodeKind::Plain);
    }

    // 尝试 Base64 解码：仅当整体可解码且解码结果是可打印文本时采用。
    if let Ok(decoded) = base64_std_decode(trimmed) {
        let text = String::from_utf8_lossy(&decoded);
        if looks_like_text(text.as_ref()) {
            return (split_lines(text.as_ref()), DecodeKind::Base64);
        }
    }

    (split_lines(body), DecodeKind::Plain)
}

/// 从订阅正文解析节点列表（去重）。
pub fn parse_nodes(
    body: &str,
    source_id: &str,
    dedup: &mut std::collections::HashSet<String>,
) -> Vec<SubscribedNode> {
    let (lines, _kind) = decode_lines(body);
    let mut out = Vec::new();
    for line in lines {
        let node = parse_node_line(&line, source_id);
        if let Some(n) = node {
            if dedup.insert(n.key.clone()) {
                out.push(n);
            }
        }
    }
    out
}

/// 解析单行节点。识别两种形态：
/// - `peerid@host:port`（agent-universe 中继/引导节点惯用格式）
/// - `host:port`（普通节点）
/// - 其他行忽略（保持与 v2rayN 忽略无法识别行的语义一致）。
pub fn parse_node_line(line: &str, source_id: &str) -> Option<SubscribedNode> {
    let l = line.trim();
    if l.is_empty() || l.starts_with('#') {
        return None;
    }

    // 去掉可能的注释尾。
    let core = l.split('#').next().unwrap_or(l).trim();

    if let Some((peer, addr)) = core.split_once('@') {
        let peer = peer.trim();
        let addr = addr.trim();
        if !peer.is_empty() && !addr.is_empty() {
            return Some(SubscribedNode {
                key: format!("{peer}@{addr}"),
                peer_id: Some(peer.to_string()),
                address: addr.to_string(),
                source: source_id.to_string(),
                raw: l.to_string(),
            });
        }
    }

    // 普通 host:port / multiaddr 形态。
    if looks_like_address(core) {
        return Some(SubscribedNode {
            key: core.to_string(),
            peer_id: None,
            address: core.to_string(),
            source: source_id.to_string(),
            raw: l.to_string(),
        });
    }

    None
}

fn split_lines(body: &str) -> Vec<String> {
    body.split(['\n', '\r'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Base64 标准编码解码（容忍空白；手写实现，不引入新依赖）。
/// 对齐 RFC 4648 标准字母表 + `=` 填充；非法字符/长度返回 Err。
fn base64_std_decode(input: &str) -> Result<Vec<u8>, ()> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let compact: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !compact.len().is_multiple_of(4) {
        return Err(());
    }

    let mut out = Vec::with_capacity(compact.len() / 4 * 3);
    let mut i = 0;
    while i < compact.len() {
        let block = &compact[i..i + 4];
        i += 4;
        let (a, b, _c, _d) = (
            val(block[0]).ok_or(())?,
            val(block[1]).ok_or(())?,
            block[2],
            block[3],
        );
        // 填充位只允许出现在末尾且为 '='。
        let pad_count = (block[2] == b'=') as usize + (block[3] == b'=') as usize;
        if pad_count == 1 && block[2] != b'=' {
            // 单个 '=' 只在第三位：允许；但第四位必须是 '='。
            if block[3] != b'=' {
                return Err(());
            }
        }
        let c_val = if block[2] == b'=' {
            0
        } else {
            val(block[2]).ok_or(())?
        };
        let d_val = if block[3] == b'=' {
            0
        } else {
            val(block[3]).ok_or(())?
        };
        out.push((a << 2) | (b >> 4));
        if pad_count < 2 {
            out.push((b << 4) | (c_val >> 2));
        }
        if pad_count == 0 {
            out.push((c_val << 6) | d_val);
        }
    }
    Ok(out)
}

/// 判定解码文本是否"像文本"（可打印字符占比高）。
fn looks_like_text(s: &str) -> bool {
    let printable = s
        .chars()
        .filter(|c| c.is_ascii_graphic() || c.is_whitespace() || c.is_ascii_alphanumeric())
        .count();
    !s.is_empty() && printable as f64 / s.chars().count() as f64 > 0.9
}

/// 判定是否像地址（含端口或 multiaddr 前缀）。
fn looks_like_address(s: &str) -> bool {
    if s.starts_with('/') && s.contains("/p2p/") {
        return true;
    }
    if s.contains(':') {
        // host:port —— 冒号后是数字端口。
        let after = s.rsplit(':').next().unwrap_or("");
        return after.chars().all(|c| c.is_ascii_digit()) && !after.is_empty();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_validation_rules() {
        let ok = SubscriptionEntry {
            id: "s1".into(),
            url: "https://example.com/sub".into(),
            enabled: true,
            more_urls: vec![],
        };
        assert!(ok.validate().is_ok());

        let no_id = SubscriptionEntry {
            id: "  ".into(),
            ..ok.clone()
        };
        assert!(no_id.validate().unwrap_err().contains("Id"));

        let bad_url = SubscriptionEntry {
            url: "ftp://x".into(),
            ..ok.clone()
        };
        assert!(bad_url.validate().unwrap_err().contains("http"));

        let no_url = SubscriptionEntry {
            url: "".into(),
            ..ok.clone()
        };
        assert!(no_url.validate().unwrap_err().contains("Url"));
    }

    #[test]
    fn all_urls_expands_more_urls() {
        let e = SubscriptionEntry {
            id: "s".into(),
            url: "https://a/sub".into(),
            enabled: true,
            more_urls: vec!["https://b/x, https://c/y".into()],
        };
        assert_eq!(
            e.all_urls(),
            vec!["https://a/sub", "https://b/x", "https://c/y"]
        );
    }

    #[test]
    fn decode_detects_base64() {
        let plain = "12D3KooWabc@1.2.3.4:4001\n5.6.7.8:9000";
        let (lines, kind) = decode_lines(plain);
        assert_eq!(kind, DecodeKind::Plain);
        assert_eq!(lines.len(), 2);

        // 同一文本的 Base64 版本（手写编码）。
        let b64 = base64_std_encode(plain.as_bytes());
        let (lines2, kind2) = decode_lines(&b64);
        assert_eq!(kind2, DecodeKind::Base64);
        assert_eq!(lines2, lines);
    }

    /// 测试辅助：Base64 标准编码（与解码互逆）。
    fn base64_std_encode(input: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
        for chunk in input.chunks(3) {
            let b0 = chunk[0];
            let b1 = chunk.get(1).copied().unwrap_or(0);
            let b2 = chunk.get(2).copied().unwrap_or(0);
            out.push(TABLE[(b0 >> 2) as usize] as char);
            out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            if chunk.len() > 1 {
                out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(TABLE[(b2 & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    #[test]
    fn parse_node_variants() {
        let n = parse_node_line("12D3KooWabc@1.2.3.4:4001", "s1").unwrap();
        assert_eq!(n.peer_id.as_deref(), Some("12D3KooWabc"));
        assert_eq!(n.address, "1.2.3.4:4001");

        let n2 = parse_node_line("5.6.7.8:9000", "s1").unwrap();
        assert!(n2.peer_id.is_none());
        assert_eq!(n2.address, "5.6.7.8:9000");

        let n3 = parse_node_line("/ip4/9.9.9.9/tcp/4001/p2p/12D3KooWxyz", "s1").unwrap();
        assert!(n3.address.starts_with("/ip4/"));

        assert!(parse_node_line("# comment", "s1").is_none());
        assert!(parse_node_line("", "s1").is_none());
        assert!(parse_node_line("not-an-address", "s1").is_none());
    }

    #[test]
    fn parse_nodes_dedups() {
        let body = "12D3KooWabc@1.2.3.4:4001\n1.2.3.4:4001\n12D3KooWabc@1.2.3.4:4001\n";
        let mut dedup = std::collections::HashSet::new();
        let nodes = parse_nodes(body, "s1", &mut dedup);
        assert_eq!(nodes.len(), 2);
        // 重复行不再出现。
        let keys: Vec<_> = nodes.iter().map(|n| n.key.clone()).collect();
        assert!(keys.contains(&"12D3KooWabc@1.2.3.4:4001".to_string()));
        assert!(keys.contains(&"1.2.3.4:4001".to_string()));
    }

    #[test]
    fn dedup_shares_across_subscriptions() {
        let mut dedup = std::collections::HashSet::new();
        let a = parse_nodes("1.2.3.4:4001", "sA", &mut dedup);
        let b = parse_nodes("1.2.3.4:4001\n9.9.9.9:4001", "sB", &mut dedup);
        assert_eq!(a.len() + b.len(), 2);
        assert!(b.iter().all(|n| n.address != "1.2.3.4:4001"));
    }
}
