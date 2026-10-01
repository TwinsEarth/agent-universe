//! 插件清单（Manifest）—— 解析、签名载荷、摘要与校验
//!
//! # 清单是签名载荷
//!
//! 版本、能力、依赖、模块摘要都在签名覆盖范围内，改一个字节即失效。
//!
//! # 用 JSON 而非 TOML
//!
//! 签名必须覆盖**规范化**字节，而已有的规范化器是 JSON 的（紧凑、任意深度移除
//! `signature`）。为第二种语法再写一个规范化器，会让「被签的字节」与「被验的字节」
//! 多一处可能分歧。
//!
//! # `deny_unknown_fields` 全开
//!
//! 打错一个键名是**拒绝**，不是被静默忽略的那一行——作者写错的那个键，
//! 正是他以为生效的那个键。

use crate::aca::crypto::canonical_payload;
use crate::identity::{Ed25519Signer, Keypair};
use crate::plugin::capability::Capability;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::tier::Tier;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// 插件清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// 插件基本信息。
    pub plugin: PluginInfo,
    /// 声明的能力。
    #[serde(default)]
    pub capabilities: Capabilities,
    /// 资源限制。
    #[serde(default)]
    pub limits: PluginLimits,
    /// 豁免声明（运行时强制不了的边界，逐条写明理由，签名覆盖）。
    #[serde(default)]
    pub waivers: BTreeMap<String, String>,
    /// 签名信息（被排除在签名载荷外）。
    #[serde(default)]
    pub signature: ManifestSignature,
}

/// 插件基本信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginInfo {
    /// 反向域名命名（决定级别）。
    pub name: String,
    /// 插件版本（语义化）。
    pub version: String,
    /// ABI 版本（用于热兼容判定）。
    pub abi: String,
    /// 入口模块/可执行体相对路径。
    pub entry: String,
    /// 发布者 DID（可空，T3 无 DID）。
    #[serde(default)]
    pub publisher: String,
    /// 入口模块的 SHA-256（绑定载荷，防替换）。
    #[serde(default)]
    pub module_sha256: String,
}

/// 能力声明。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// 申请的能力名（规范字符串，如 `net:dht:write`）。
    #[serde(default)]
    pub grant: Vec<String>,
}

/// 资源限制。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginLimits {
    /// 内存上限（字节）。
    pub memory_bytes: u64,
    /// CPU 时间上限（毫秒）。
    pub cpu_ms: u64,
    /// 磁盘上限（字节）。
    pub disk_bytes: u64,
    /// 最大进程数。
    pub max_processes: u32,
    /// 单次输出上限（字节）。
    pub max_output_bytes: u64,
}

impl Default for PluginLimits {
    fn default() -> Self {
        // 默认值按最严（T3）级别；宿主会按级别收紧或放宽校验。
        PluginLimits {
            memory_bytes: 128 * 1024 * 1024,
            cpu_ms: 10_000,
            disk_bytes: 256 * 1024 * 1024,
            max_processes: 1,
            max_output_bytes: 262_144,
        }
    }
}

/// 签名信息。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestSignature {
    /// 发布者公钥（hex）。
    #[serde(default)]
    pub publisher_key: String,
    /// 清单摘要（hex，sha256(规范载荷)）。
    #[serde(default)]
    pub manifest_digest: String,
    /// 发布者对规范载荷的签名（hex）。
    #[serde(default)]
    pub sig: String,
    /// 官方副签（T1/T2，hex）。
    #[serde(default)]
    pub counter_sig: String,
    /// 副签所用官方公钥（hex）。
    #[serde(default)]
    pub counter_key: String,
}

impl PluginManifest {
    /// 从 JSON 解析清单（拒绝未知字段）。
    pub fn parse(json: &str) -> PluginResult<PluginManifest> {
        serde_json::from_str(json)
            .map_err(|e| PluginError::Manifest(format!("清单 JSON 非法: {e}")))
    }

    /// 由 name 前缀判定级别。
    pub fn tier(&self) -> Tier {
        Tier::from_name(&self.plugin.name)
    }

    /// 计算规范签名载荷：紧凑 JSON、任意深度移除 `signature`。
    pub fn signing_payload(&self) -> PluginResult<Vec<u8>> {
        canonical_payload(self).map_err(PluginError::Manifest)
    }

    /// 计算清单摘要（hex，sha256(规范载荷)）。
    pub fn compute_digest(&self) -> PluginResult<String> {
        let payload = self.signing_payload()?;
        Ok(hex::encode(Sha256::digest(payload)))
    }

    /// 解析并校验清单中申请的能力名（非法能力名即拒绝）。
    pub fn requested_capabilities(&self) -> PluginResult<Vec<Capability>> {
        let mut caps = Vec::new();
        for raw in &self.capabilities.grant {
            match Capability::parse(raw) {
                Some(c) => caps.push(c),
                None => {
                    return Err(PluginError::Manifest(format!(
                        "未知能力名 {raw}（不在已知能力集合）"
                    )))
                }
            }
        }
        Ok(caps)
    }

    /// 用发布者密钥对清单签名（填充 signature，不含副签）。
    ///
    /// 用于开发/打包工具；签名后 `signature.manifest_digest` 与 `sig` 被填充。
    pub fn sign_with(&mut self, keypair: &Keypair) -> PluginResult<()> {
        // 先清空旧签名，再计算载荷与摘要。
        self.signature = ManifestSignature::default();
        let payload = self.signing_payload()?;
        let digest = hex::encode(Sha256::digest(&payload));
        let signer = Ed25519Signer::new(keypair);
        let sig = hex::encode(signer.sign(&payload));
        self.signature.publisher_key = hex::encode(keypair.public_key());
        self.signature.manifest_digest = digest;
        self.signature.sig = sig;
        Ok(())
    }

    /// 追加官方副签（T1/T2），用官方密钥对规范载荷副签。
    pub fn counter_sign_with(&mut self, official: &Keypair) -> PluginResult<()> {
        let payload = self.signing_payload()?;
        let signer = Ed25519Signer::new(official);
        self.signature.counter_sig = hex::encode(signer.sign(&payload));
        self.signature.counter_key = hex::encode(official.public_key());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小可用的未签名清单。
    fn sample_manifest(name: &str) -> PluginManifest {
        PluginManifest {
            plugin: PluginInfo {
                name: name.to_string(),
                version: "3.0.0".to_string(),
                abi: "3.0".to_string(),
                entry: "plugin.bin".to_string(),
                publisher: String::new(),
                module_sha256: String::new(),
            },
            capabilities: Capabilities::default(),
            limits: PluginLimits::default(),
            waivers: BTreeMap::new(),
            signature: ManifestSignature::default(),
        }
    }

    #[test]
    fn rejects_unknown_field() {
        let json = r#"{
            "plugin": {"name":"com.twinsearth.sys.x","version":"3.0.0","abi":"3.0","entry":"x"},
            "bogus_field": 1
        }"#;
        assert!(PluginManifest::parse(json).is_err());
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let kp = Keypair::generate();
        let mut m = sample_manifest("com.twinsearth.official.demo");
        m.sign_with(&kp).unwrap();
        let payload = m.signing_payload().unwrap();
        let pk = hex::decode(&m.signature.publisher_key).unwrap();
        let sig = hex::decode(&m.signature.sig).unwrap();
        assert!(Ed25519Signer::verify_with_pubkey(&pk, &payload, &sig));
        // 摘要与重算一致。
        assert_eq!(m.signature.manifest_digest, m.compute_digest().unwrap());
    }

    #[test]
    fn tampering_changes_digest() {
        let kp = Keypair::generate();
        let mut m = sample_manifest("com.twinsearth.official.demo");
        m.sign_with(&kp).unwrap();
        let signed_digest = m.signature.manifest_digest.clone();
        // 篡改一个字段（签名未重算）→ 摘要变化，原签名失效。
        m.plugin.version = "9.9.9".to_string();
        assert_ne!(signed_digest, m.compute_digest().unwrap());
        let payload = m.signing_payload().unwrap();
        let pk = hex::decode(&m.signature.publisher_key).unwrap();
        let sig = hex::decode(&m.signature.sig).unwrap();
        assert!(!Ed25519Signer::verify_with_pubkey(&pk, &payload, &sig));
    }

    #[test]
    fn unknown_capability_name_rejected() {
        let mut m = sample_manifest("com.twinsearth.official.demo");
        m.capabilities.grant = vec!["not:a:cap".to_string()];
        assert!(m.requested_capabilities().is_err());
    }

    #[test]
    fn counter_signature_roundtrip() {
        let dev = Keypair::generate();
        let official = Keypair::generate();
        let mut m = sample_manifest("com.twinsearth.official.demo");
        m.sign_with(&dev).unwrap();
        m.counter_sign_with(&official).unwrap();
        let payload = m.signing_payload().unwrap();
        let ck = hex::decode(&m.signature.counter_key).unwrap();
        let cs = hex::decode(&m.signature.counter_sig).unwrap();
        assert!(Ed25519Signer::verify_with_pubkey(&ck, &payload, &cs));
    }
}
