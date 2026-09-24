//! Python ↔ Rust 跨语言签名互验。
//!
//! 基准值由 Python SDK（aip/crypto.py，固定 seed=[1;32]）生成，
//! 本测试验证：公钥 / DID / canonical 载荷一致，Rust 签名与 Python
//! 签名逐字符相同，Rust 能验证 Python 签名，篡改后验证失败。

use gsn_core::aca::crypto::{canonical_payload, sign_hex, verify_hex};
use gsn_core::identity::{Did, Ed25519Signer, Keypair};
use serde_json::json;

const PY_PUBKEY: &str = "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c";
const PY_DID: &str = "did:aip:34750f98bd59fcfc";
const PY_PAYLOAD: &str = r#"{"capabilities":["text-generation","mcp"],"did":"did:aip:34750f98bd59fcfc","name":"CrossLang","stake":100}"#;
const PY_SIG: &str = "e14d3f9e8204ea185ea4ba32a8117f262095ab9dac1352e1a7964ce36d3355c288dd6804ffa7daeb5f6eba9f30428f52702a75fd3efbb6a62555a7f24665da0e";

#[test]
fn cross_language_identity_and_signature() {
    let seed = [1u8; 32];
    let keypair = Keypair::from_seed(&seed);
    let pubkey = keypair.public_key();

    // 公钥一致
    assert_eq!(hex::encode(pubkey), PY_PUBKEY);
    // DID 一致
    assert_eq!(Did::from_public_key(pubkey).as_str(), PY_DID);

    let obj = json!({
        "did": PY_DID,
        "name": "CrossLang",
        "capabilities": ["text-generation", "mcp"],
        "stake": 100,
        "signature": ""
    });

    // canonical 载荷逐字节一致
    assert_eq!(canonical_payload(&obj), PY_PAYLOAD.as_bytes());

    let signer = Ed25519Signer::new(&keypair);
    let rust_sig = sign_hex(&obj, &signer);
    // Ed25519 确定性 + canonical 一致 → Rust 与 Python 签名相同
    assert_eq!(rust_sig, PY_SIG);

    // Rust 验证 Python 产生的签名
    assert!(verify_hex(&obj, PY_SIG, pubkey));

    // 篡改 name 后，原签名验证失败
    let tampered = json!({
        "did": PY_DID,
        "name": "Tampered",
        "capabilities": ["text-generation", "mcp"],
        "stake": 100,
        "signature": PY_SIG
    });
    assert!(!verify_hex(&tampered, PY_SIG, pubkey));
}
