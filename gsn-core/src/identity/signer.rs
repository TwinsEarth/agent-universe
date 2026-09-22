use sha2::{Sha256, Digest};

/// 模拟密钥对（测试用，生产环境替换为 ed25519）
#[derive(Debug, Clone)]
pub struct Keypair {
    public_key: Vec<u8>,
    secret_key: Vec<u8>,
}

impl Keypair {
    /// 生成新密钥对
    pub fn generate() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut hasher = Sha256::new();
        hasher.update(timestamp.to_le_bytes());
        let hash = hasher.finalize();
        Self {
            public_key: hash.to_vec(),
            secret_key: hash.to_vec(),
        }
    }

    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }
}

/// 签名器（模拟实现）
pub struct Ed25519Signer<'a> {
    keypair: &'a Keypair,
}

impl<'a> Ed25519Signer<'a> {
    pub fn new(keypair: &'a Keypair) -> Self {
        Self { keypair }
    }

    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(&self.keypair.secret_key);
        hasher.update(message);
        hasher.finalize().to_vec()
    }

    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(&self.keypair.secret_key);
        hasher.update(message);
        hasher.finalize().as_slice() == signature
    }
}
