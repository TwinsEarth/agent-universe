//! ACA 签名与验签辅助
//!
//! 为 Manifest / Message / Receipt 提供统一的 Ed25519 签名能力。
//!
//! ## 签名载荷（canonical payload）
//! 先把结构序列化为 JSON，移除 `signature` 字段，再用紧凑（无空白）JSON
//! 字节作为待签名内容。签名与验签使用完全相同的载荷构造，保证可复现。
//!
//! ## 公钥来源
//! DID（`did:aip:{sha256(pubkey)前8字节}`）是公钥的指纹、不可逆，
//! 因此验签需要调用方提供公钥。公钥通常通过 P2P 握手交换或 DHT 解析获得，
//! 并可用 `Did::from_public_key` 校验其与 DID 一致。

use crate::identity::Ed25519Signer;
use serde::Serialize;
use serde_json::Value;

/// 计算结构的规范待签名载荷：序列化为 JSON、移除 signature 键、紧凑字节
pub fn canonical_payload<T: Serialize>(obj: &T) -> Vec<u8> {
    let mut v = serde_json::to_value(obj).unwrap_or(Value::Null);
    if let Some(map) = v.as_object_mut() {
        map.remove("signature");
    }
    serde_json::to_vec(&v).unwrap_or_default()
}

/// 用签名器对结构签名，返回 hex 编码（64 字节 → 128 hex 字符）
pub fn sign_hex<T: Serialize>(obj: &T, signer: &Ed25519Signer) -> String {
    hex::encode(signer.sign(&canonical_payload(obj)))
}

/// 用公钥验证结构的 hex 签名
///
/// - `signature`：结构中存的 hex 签名字符串
/// - `pubkey`：签名者 32 字节公钥
pub fn verify_hex<T: Serialize>(obj: &T, signature: &str, pubkey: &[u8]) -> bool {
    if signature.is_empty() {
        return false;
    }
    let sig = match hex::decode(signature) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    Ed25519Signer::verify_with_pubkey(pubkey, &canonical_payload(obj), &sig)
}
