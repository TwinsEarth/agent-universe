use sha2::{Sha256, Digest};

/// 去中心化标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Did(String);

impl Did {
    /// 从公钥创建 DID
    pub fn from_public_key(pubkey: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(pubkey);
        let hash = hasher.finalize();
        let encoded = hex::encode(&hash[..8]);
        Did(format!("did:aip:{}", encoded))
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
