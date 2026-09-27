use sha2::{Digest, Sha256};

/// 上游 DID 方法前缀（gsn-core，向后兼容）
pub const METHOD_AIP: &str = "aip";
/// 本项目新身份 DID 方法前缀
pub const METHOD_NAU: &str = "nau";

/// 去中心化标识符
///
/// 身份派生口径（Rust / JS / Python 三端一致）：
/// `fingerprint = hex(SHA-256(原始 32 字节公钥)[..8])`
///
/// - 本项目新铸造身份：`did:nau:<fingerprint>`
/// - 上游身份：`did:aip:<fingerprint>`，`parse` 仍接受并可验证其签名
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Did(String);

impl Did {
    /// 从公钥创建新身份（本项目统一使用 did:nau: 前缀）
    pub fn from_public_key(pubkey: &[u8]) -> Self {
        Did(format!("did:{}:{}", METHOD_NAU, Self::fingerprint(pubkey)))
    }

    /// 公钥指纹：SHA-256(原始 32 字节公钥) 取前 8 字节 hex。
    /// 三端统一口径，保证同一公钥在 Rust/JS/Python 得到同一标识。
    pub fn fingerprint(pubkey: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(pubkey);
        let hash = hasher.finalize();
        hex::encode(&hash[..8])
    }

    /// 解析 DID。
    ///
    /// 接受上游 `did:aip:<id>` 与本项目 `did:nau:<id>`；
    /// 其他方法或格式错误均返回 Err。
    pub fn parse(s: &str) -> Result<Self, String> {
        let rest = s
            .strip_prefix("did:")
            .ok_or_else(|| format!("非法 DID（缺少 did: 前缀）: {s}"))?;
        let (method, identifier) = rest
            .split_once(':')
            .ok_or_else(|| format!("非法 DID（格式应为 did:<method>:<id>）: {s}"))?;
        if identifier.is_empty() {
            return Err(format!("非法 DID（标识部分为空）: {s}"));
        }
        if method != METHOD_AIP && method != METHOD_NAU {
            return Err(format!(
                "不支持的 DID 方法 '{method}'（仅接受 {METHOD_AIP}/{METHOD_NAU}）: {s}"
            ));
        }
        Ok(Did(s.to_string()))
    }

    /// DID 方法（aip / nau）
    pub fn method(&self) -> &str {
        self.0.split(':').nth(1).unwrap_or("")
    }

    /// DID 标识部分（fingerprint）
    pub fn identifier(&self) -> &str {
        self.0.split(':').nth(2).unwrap_or("")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Did {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_identity_uses_nau() {
        let did = Did::from_public_key(&[1u8; 32]);
        assert!(did.as_str().starts_with("did:nau:"));
        assert_eq!(did.method(), "nau");
    }

    #[test]
    fn parse_accepts_upstream_aip() {
        let did = Did::parse("did:aip:34750f98bd59fcfc").unwrap();
        assert_eq!(did.method(), "aip");
        assert_eq!(did.identifier(), "34750f98bd59fcfc");
    }

    #[test]
    fn parse_accepts_nau() {
        let did = Did::parse("did:nau:34750f98bd59fcfc").unwrap();
        assert_eq!(did.method(), "nau");
    }

    #[test]
    fn parse_rejects_unknown_method() {
        assert!(Did::parse("did:au:abcd").is_err());
        assert!(Did::parse("did:foo:abcd").is_err());
    }

    #[test]
    fn parse_rejects_malformed() {
        assert!(Did::parse("naddaip").is_err());
        assert!(Did::parse("did:aip:").is_err());
        assert!(Did::parse("did:aip").is_err());
    }
}
