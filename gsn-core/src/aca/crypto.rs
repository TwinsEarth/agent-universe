//! ACA 签名与验签辅助
//!
//! 为 Manifest / Message / Receipt 提供统一的 Ed25519 签名能力。
//!
//! ## 签名载荷（canonical payload）
//! 先把结构序列化为 JSON，在**任意深度**移除 `signature` 字段，再用紧凑（无空白）
//! JSON 字节作为待签名内容。签名与验签使用完全相同的载荷构造，保证可复现。
//!
//! ## 错误面（v2.6.4，GAP §4.2/§4.3）
//! [`canonical_payload`] / [`sign_hex`] / [`verify_hex`] 均返回 `Result`：
//! 序列化失败不再退化成对 4 字节 `null` 签名，而是显式报错。
//!
//! ## 公钥来源
//! DID（`did:aip:{sha256(pubkey)前8字节}`）是公钥的指纹、不可逆，
//! 因此验签需要调用方提供公钥。公钥通常通过 P2P 握手交换或 DHT 解析获得，
//! 并可用 `Did::from_public_key` 校验其与 DID 一致。

use crate::identity::Ed25519Signer;
use serde::Serialize;
use serde_json::Value;

/// 在**任意深度**递归移除 `signature` 键（GAP §4.3）。
///
/// 仅在顶层移除会让嵌套结构的内层 `signature` 被外层签名覆盖；
/// 因此对对象的每个值、数组的每个元素都递归过滤。
fn strip_signatures(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("signature");
            for (_key, child) in map.iter_mut() {
                strip_signatures(child);
            }
        }
        Value::Array(items) => {
            for child in items.iter_mut() {
                strip_signatures(child);
            }
        }
        _ => {}
    }
}

/// 计算结构的规范待签名载荷：序列化为 JSON、任意深度移除 signature 键、紧凑字节。
///
/// 返回 `Result`（GAP §4.2）：序列化失败时返回错误，**不会**退化成对 `null` 签名。
pub fn canonical_payload<T: Serialize>(obj: &T) -> Result<Vec<u8>, String> {
    let mut v = serde_json::to_value(obj).map_err(|e| format!("序列化失败，拒绝签名: {e}"))?;
    strip_signatures(&mut v);
    serde_json::to_vec(&v).map_err(|e| format!("规范序列化失败: {e}"))
}

/// 计算结构的规范**对象**载荷：除 [`canonical_payload`] 的保证外，额外要求根是 JSON 对象
///（GAP §4.2 `RootNotObject`），避免对数组 / 标量根签名造成的歧义。
pub fn canonical_object<T: Serialize>(obj: &T) -> Result<Vec<u8>, String> {
    let mut v = serde_json::to_value(obj).map_err(|e| format!("序列化失败，拒绝签名: {e}"))?;
    if !v.is_object() {
        return Err("规范载荷根必须是 JSON 对象（RootNotObject）".to_string());
    }
    strip_signatures(&mut v);
    serde_json::to_vec(&v).map_err(|e| format!("规范序列化失败: {e}"))
}

/// 用签名器对结构签名，返回 hex 编码（64 字节 → 128 hex 字符）。
///
/// 返回 `Result`：载荷构造失败时不签名、不产生 `null` 签名。
pub fn sign_hex<T: Serialize>(obj: &T, signer: &Ed25519Signer) -> Result<String, String> {
    let payload = canonical_payload(obj)?;
    Ok(hex::encode(signer.sign(&payload)))
}

/// 用公钥验证结构的 hex 签名。
///
/// - `signature`：结构中存的 hex 签名字符串
/// - `pubkey`：签名者 32 字节公钥
///
/// 返回 `Result<bool>`：空签名、hex 解码失败、载荷构造失败均显式处理；
/// 调用方只需要布尔结论时可用 `.unwrap_or(false)`。
pub fn verify_hex<T: Serialize>(obj: &T, signature: &str, pubkey: &[u8]) -> Result<bool, String> {
    if signature.is_empty() {
        return Ok(false);
    }
    let sig = hex::decode(signature).map_err(|e| format!("签名 hex 解码失败: {e}"))?;
    let payload = canonical_payload(obj)?;
    Ok(Ed25519Signer::verify_with_pubkey(pubkey, &payload, &sig))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;
    use serde::Serializer;
    use std::collections::BTreeMap;

    /// 恒失败的 Serialize 类型：用于证明不会退化成签 null（GAP §4.2）
    struct AlwaysFail;
    impl Serialize for AlwaysFail {
        fn serialize<S: Serializer>(&self, _s: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("intentional serialization failure"))
        }
    }

    #[test]
    fn canonical_payload_error_instead_of_null() {
        // 序列化失败 → Err，而非 4 字节 null
        assert!(canonical_payload(&AlwaysFail).is_err());
        assert!(canonical_object(&AlwaysFail).is_err());
    }

    #[test]
    fn canonical_object_rejects_non_object_root() {
        // 数组根 / 标量根 → RootNotObject
        assert!(canonical_object(&vec![1u8, 2, 3]).is_err());
        assert!(canonical_object(&42u8).is_err());
        assert!(canonical_object(&"scalar").is_err());
    }

    #[test]
    fn nested_signature_stripped_at_any_depth() {
        let mut root = BTreeMap::new();
        root.insert("a".to_string(), serde_json::json!(1));
        // 内层对象与内层数组里的对象都带 signature
        let mut inner = BTreeMap::new();
        inner.insert("signature".to_string(), serde_json::json!("should-be-removed"));
        inner.insert("keep".to_string(), serde_json::json!(7));
        root.insert("inner".to_string(), serde_json::to_value(&inner).unwrap());
        root.insert(
            "arr".to_string(),
            serde_json::json!([{"signature": "x", "k": 1}, 2]),
        );

        let bytes = canonical_payload(&root).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("signature"), "嵌套 signature 未被剥离: {text}");
        assert!(!text.contains("should-be-removed"));
        assert!(text.contains("\"keep\":7"));
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let kp = Keypair::from_seed(&{
            let mut s = [0u8; 32];
            s[0] = 7;
            s
        });
        let signer = Ed25519Signer::new(&kp);
        let mut obj = BTreeMap::new();
        obj.insert("name".to_string(), serde_json::json!("t"));
        let sig = sign_hex(&obj, &signer).unwrap();
        let pk = kp.public_key();
        assert!(verify_hex(&obj, &sig, pk).unwrap());
        // 篡改后验签失败
        obj.insert("name".to_string(), serde_json::json!("tampered"));
        assert!(!verify_hex(&obj, &sig, pk).unwrap());
    }
}
