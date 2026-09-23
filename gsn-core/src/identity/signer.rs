//! Ed25519 密钥对与签名
//!
//! 使用真正的 Ed25519 签名算法（ed25519-dalek）

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;

/// Ed25519 密钥对
#[derive(Debug, Clone)]
pub struct Keypair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl Keypair {
    /// 生成新密钥对（使用 OS 随机数）
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let signing_key = SigningKey::from_bytes(&bytes);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// 从种子（32 字节）恢复密钥对
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(seed);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// 公钥（32 字节）
    pub fn public_key(&self) -> &[u8] {
        self.verifying_key.as_bytes()
    }

    /// 私钥种子（32 字节，可用于持久化）
    pub fn seed(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
}

/// Ed25519 签名器
pub struct Ed25519Signer<'a> {
    keypair: &'a Keypair,
}

impl<'a> Ed25519Signer<'a> {
    pub fn new(keypair: &'a Keypair) -> Self {
        Self { keypair }
    }

    /// 对消息签名，返回 64 字节签名
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.keypair.signing_key.sign(message).to_bytes().to_vec()
    }

    /// 验证签名
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        let sig_bytes: [u8; 64] = match signature.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        self.keypair.verifying_key.verify(message, &signature).is_ok()
    }

    /// 用指定公钥验证签名（用于验证他人签名）
    pub fn verify_with_pubkey(pubkey: &[u8], message: &[u8], signature: &[u8]) -> bool {
        let pk_bytes: [u8; 32] = match pubkey.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let vk = match VerifyingKey::from_bytes(&pk_bytes) {
            Ok(vk) => vk,
            Err(_) => return false,
        };
        let sig_bytes: [u8; 64] = match signature.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        vk.verify(message, &signature).is_ok()
    }
}
