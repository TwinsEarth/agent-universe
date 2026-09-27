//! 跨实现 / 跨语言身份与签名互验（v2.5.7）。
//!
//! 向量取自上游 gsn-core/tests/cross_lang_signature.rs（固定 seed=[1;32]）。
//! 本测试验证：
//! - 公钥一致；
//! - 本项目新铸造身份使用 did:nau: 前缀，fingerprint 与上游相同；
//! - Did::parse 接受上游 did:aip: 身份（向后兼容）；
//! - 上游 canonical 载荷下，Rust 签名与上游签名逐字节相同；
//! - Rust 能验证上游签名；篡改后验证失败。
//!
//! 同一向量另由 conformance/generate.mjs 用 Node/OpenSSL Ed25519 独立生成
//! （与 Rust 零共享代码），构成真正的跨实现校验而非"自己验自己"。

use gsn_core::aca::crypto::{canonical_payload, sign_hex, verify_hex};
use gsn_core::identity::{Did, Ed25519Signer, Keypair};
use serde_json::json;

const PUBKEY: &str = "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c";
const AIP_DID: &str = "did:aip:34750f98bd59fcfc";
const NAU_DID: &str = "did:nau:34750f98bd59fcfc";
const UPSTREAM_PAYLOAD: &str = r#"{"capabilities":["text-generation","mcp"],"did":"did:aip:34750f98bd59fcfc","name":"CrossLang","stake":100}"#;
const UPSTREAM_SIG: &str = "e14d3f9e8204ea185ea4ba32a8117f262095ab9dac1352e1a7964ce36d3355c288dd6804ffa7daeb5f6eba9f30428f52702a75fd3efbb6a62555a7f24665da0e";

#[test]
fn cross_language_identity_and_signature() {
    let seed = [1u8; 32];
    let keypair = Keypair::from_seed(&seed);
    let pubkey = keypair.public_key();

    // 公钥一致
    assert_eq!(hex::encode(pubkey), PUBKEY);

    // 新铸造身份使用 did:nau:，fingerprint（SHA256 原始公钥前 8 字节）与上游相同
    assert_eq!(Did::from_public_key(pubkey).as_str(), NAU_DID);

    // parse 接受上游 did:aip: 身份（向后兼容）
    let parsed = Did::parse(AIP_DID).expect("上游 did:aip 身份应可解析");
    assert_eq!(parsed.as_str(), AIP_DID);
    assert_eq!(parsed.method(), "aip");

    // 上游 canonical 载荷（aip DID）逐字节一致
    let obj = json!({
        "did": AIP_DID,
        "name": "CrossLang",
        "capabilities": ["text-generation", "mcp"],
        "stake": 100,
        "signature": ""
    });
    assert_eq!(canonical_payload(&obj), UPSTREAM_PAYLOAD.as_bytes());

    // Ed25519 确定性 + canonical 一致 → Rust 签名逐字节命中上游签名
    let signer = Ed25519Signer::new(&keypair);
    let rust_sig = sign_hex(&obj, &signer);
    assert_eq!(rust_sig, UPSTREAM_SIG, "Rust 签名应逐字节命中上游签名");

    // Rust 验证上游产生的签名
    assert!(verify_hex(&obj, UPSTREAM_SIG, pubkey));

    // 篡改 name 后，原签名验证失败
    let tampered = json!({
        "did": AIP_DID,
        "name": "Tampered",
        "capabilities": ["text-generation", "mcp"],
        "stake": 100,
        "signature": UPSTREAM_SIG
    });
    assert!(!verify_hex(&tampered, UPSTREAM_SIG, pubkey));
}

#[test]
fn new_nau_identity_signature_roundtrip() {
    // 新身份（nau）自身的签名/验证往返
    let seed = [2u8; 32];
    let keypair = Keypair::from_seed(&seed);
    let pubkey = keypair.public_key();
    let did = Did::from_public_key(pubkey);
    assert!(did.as_str().starts_with("did:nau:"));

    let obj = json!({
        "did": did.as_str(),
        "name": "NauAgent",
        "capabilities": ["mcp"],
        "stake": 100,
        "signature": ""
    });
    let signer = Ed25519Signer::new(&keypair);
    let sig = sign_hex(&obj, &signer);
    assert!(verify_hex(&obj, &sig, pubkey));
}
