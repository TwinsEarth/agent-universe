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

/// 判断公钥是否为弱公钥（GAP §4.6）。
///
/// 拒绝：长度不是 32 字节、全零、全 0xFF、或 32 字节完全相同的平凡公钥。
/// Ed25519 低阶点由签名库在验签时另行负责；这里只拦截明显退化的公钥，
/// 避免全零 / 平凡密钥被注册成"有效身份"。
///
/// 注意：本地派生指纹 [`Did::from_public_key`] 不受此谓词影响（保持三端 8 字节指纹兼容）；
/// 铸造 / 注册外部身份时应先调用本函数并拒绝弱公钥。
pub fn is_weak_pubkey(pubkey: &[u8]) -> bool {
    if pubkey.len() != 32 {
        return true;
    }
    if pubkey.iter().all(|&b| b == 0) {
        return true;
    }
    if pubkey.iter().all(|&b| b == 0xff) {
        return true;
    }
    // 32 字节全部相同（[x;32]），显然不是随机密钥
    let first = pubkey[0];
    pubkey.iter().all(|&b| b == first)
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

    #[test]
    fn weak_pubkeys_rejected() {
        assert!(is_weak_pubkey(&[0u8; 32])); // 全零
        assert!(is_weak_pubkey(&[0xffu8; 32])); // 全 0xFF
        assert!(is_weak_pubkey(&[1u8; 32])); // 全相同
        assert!(is_weak_pubkey(&[])); // 空
        assert!(is_weak_pubkey(&[1u8; 16])); // 长度不对
    }

    #[test]
    fn realistic_pubkey_not_weak() {
        // cross_lang 测试使用的上游真实公钥：不能被误判弱
        let pk: [u8; 32] = [
            0x8a, 0x88, 0xe3, 0xdd, 0x74, 0x09, 0xf1, 0x95, 0xfd, 0x52, 0xdb, 0x2d, 0x3c, 0xba,
            0x5d, 0x72, 0xca, 0x67, 0x09, 0xbf, 0x1d, 0x94, 0x12, 0x1b, 0xf3, 0x74, 0x88, 0x01,
            0xb4, 0x0f, 0x6f, 0x5c,
        ];
        assert!(!is_weak_pubkey(&pk));
    }
}
