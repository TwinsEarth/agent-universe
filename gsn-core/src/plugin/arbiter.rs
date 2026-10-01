//! 权限仲裁器（Arbiter）—— 签名校验、令牌签发、审批与发布者信任
//!
//! # 仲裁器是信任决策点
//!
//! 加载一个插件前，仲裁器完成**四重校验**并据此签发能力令牌：
//!
//! 1. 清单摘要与重算一致（未被篡改）；
//! 2. 发布者签名对规范载荷有效；
//! 3. T1/T2 官方副签有效，且副签公钥是受信官方根；
//! 4. T3 第三方发布者公钥已被操作者显式信任（默认信任库为空，fail-closed）。
//!
//! 然后按能力矩阵（[`crate::plugin::capability::grant_for`]）签发令牌：
//! 申请到被 `Denied` 的能力即拒绝；`Declarable` 的能力需先经审批。

use crate::identity::Ed25519Signer;
use crate::plugin::capability::{grant_for, Capability, CapabilityToken, Grant};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::manifest::PluginManifest;
use crate::plugin::tier::Tier;
use std::collections::BTreeMap;

/// 权限仲裁器。
#[derive(Debug, Default, Clone)]
pub struct Arbiter {
    /// 受信官方副签公钥（hex）。
    official_roots: Vec<String>,
    /// 操作者显式信任的第三方发布者公钥（hex），默认为空。
    trusted_publishers: Vec<String>,
    /// 已审批的声明式能力（插件名 → 已批准能力）。
    approvals: BTreeMap<String, Vec<Capability>>,
}

impl Arbiter {
    /// 新仲裁器（信任库为空：fail-closed）。
    pub fn new() -> Self {
        Arbiter::default()
    }

    /// 注册官方副签根公钥（hex）。
    pub fn add_official_root(&mut self, key_hex: &str) {
        if !self.official_roots.iter().any(|k| k == key_hex) {
            self.official_roots.push(key_hex.to_string());
        }
    }

    /// 操作者显式信任一个第三方发布者公钥（hex）。
    pub fn trust_publisher(&mut self, key_hex: &str) {
        if !self.trusted_publishers.iter().any(|k| k == key_hex) {
            self.trusted_publishers.push(key_hex.to_string());
        }
    }

    /// 审批一个声明式能力（T1/T2）。
    pub fn approve(&mut self, plugin_name: &str, cap: Capability) {
        let entry = self.approvals.entry(plugin_name.to_string()).or_default();
        if !entry.contains(&cap) {
            entry.push(cap);
        }
    }

    /// 校验清单签名（四重校验）。
    ///
    /// 成功返回级别；失败返回类型化错误。
    pub fn verify_manifest(&self, manifest: &PluginManifest) -> PluginResult<Tier> {
        let name = &manifest.plugin.name;
        let tier = Tier::from_name(name);
        let payload = manifest.signing_payload()?;

        // 1. 摘要重算一致。
        let digest = manifest.compute_digest()?;
        if manifest.signature.manifest_digest != digest {
            return Err(PluginError::Signature(format!(
                "清单摘要不匹配（载荷被篡改）：清单 {} 重算 {digest}",
                manifest.signature.manifest_digest
            )));
        }

        // 发布者公钥/签名。
        let pk = hex::decode(&manifest.signature.publisher_key)
            .map_err(|e| PluginError::Signature(format!("发布者公钥 hex 非法: {e}")))?;
        if manifest.signature.sig.is_empty() {
            return Err(PluginError::Signature("缺少发布者签名".to_string()));
        }
        let sig = hex::decode(&manifest.signature.sig)
            .map_err(|e| PluginError::Signature(format!("发布者签名 hex 非法: {e}")))?;

        // 2. 发布者签名有效。
        if !Ed25519Signer::verify_with_pubkey(&pk, &payload, &sig) {
            return Err(PluginError::Signature(
                "发布者签名对规范载荷无效".to_string(),
            ));
        }

        match tier {
            Tier::Official | Tier::Certified => {
                // 3. 官方副签：存在、有效、且副签公钥是受信根。
                let ck = hex::decode(&manifest.signature.counter_key)
                    .map_err(|e| PluginError::Signature(format!("副签公钥 hex 非法: {e}")))?;
                let cs = hex::decode(&manifest.signature.counter_sig)
                    .map_err(|e| PluginError::Signature(format!("副签 hex 非法: {e}")))?;
                if !Ed25519Signer::verify_with_pubkey(&ck, &payload, &cs) {
                    return Err(PluginError::Signature("官方副签无效".to_string()));
                }
                if !self
                    .official_roots
                    .iter()
                    .any(|root| root == &manifest.signature.counter_key)
                {
                    return Err(PluginError::Trust(
                        "副签公钥不是受信官方根（自造副签不被接受）".to_string(),
                    ));
                }
            }
            Tier::ThirdParty => {
                // 4. 第三方发布者必须被操作者显式信任。
                if !self
                    .trusted_publishers
                    .iter()
                    .any(|k| k == &manifest.signature.publisher_key)
                {
                    return Err(PluginError::Trust(format!(
                        "第三方发布者公钥 {} 未被信任（需操作者显式信任）",
                        manifest.signature.publisher_key
                    )));
                }
            }
            Tier::System => {
                // T0 随内核发布、进程内；签名由宿主构建链路保证。
            }
            Tier::Blacklist => {
                return Err(PluginError::Blacklisted(name.clone()));
            }
        }

        Ok(tier)
    }

    /// 校验能力申请：按级别矩阵决定授予，拒绝越权，声明式需审批。
    ///
    /// 返回最终授予的能力集合（含基础能力）。
    pub fn resolve_granted(
        &self,
        manifest: &PluginManifest,
        tier: Tier,
    ) -> PluginResult<Vec<Capability>> {
        let requested = manifest.requested_capabilities()?;
        let mut granted = Vec::new();

        // 基础能力默认授予。
        for cap in Capability::ALL {
            if cap.is_basic() {
                granted.push(*cap);
            }
        }

        for cap in requested {
            match grant_for(tier, cap) {
                Grant::Granted => {
                    if !granted.contains(&cap) {
                        granted.push(cap);
                    }
                }
                Grant::Declarable => {
                    let approved = self
                        .approvals
                        .get(&manifest.plugin.name)
                        .map(|v| v.contains(&cap))
                        .unwrap_or(false);
                    if !approved {
                        return Err(PluginError::Manifest(format!(
                            "能力 {} 为声明式但插件 {} 未获审批",
                            cap.as_str(),
                            manifest.plugin.name
                        )));
                    }
                    if !granted.contains(&cap) {
                        granted.push(cap);
                    }
                }
                Grant::Denied => {
                    return Err(PluginError::Unauthorized {
                        plugin_id: manifest.plugin.name.clone(),
                        capability: cap.as_str().to_string(),
                    });
                }
            }
        }
        Ok(granted)
    }

    /// 完整流程：校验清单 → 校验能力 → 签发令牌。
    pub fn issue_token(
        &self,
        manifest: &PluginManifest,
        now: u64,
    ) -> PluginResult<CapabilityToken> {
        let tier = self.verify_manifest(manifest)?;
        let granted = self.resolve_granted(manifest, tier)?;
        Ok(CapabilityToken {
            plugin_id: manifest.plugin.name.clone(),
            granted,
            issued_at: now,
            manifest_digest: manifest.compute_digest()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::{Capabilities, ManifestSignature, PluginInfo, PluginLimits};
    use std::collections::BTreeMap;

    fn unsigned(name: &str) -> PluginManifest {
        PluginManifest {
            plugin: PluginInfo {
                name: name.to_string(),
                version: "3.0.0".to_string(),
                abi: "3.0".to_string(),
                entry: "x".to_string(),
                publisher: String::new(),
                module_sha256: String::new(),
            },
            capabilities: Capabilities::default(),
            limits: PluginLimits::default(),
            waivers: BTreeMap::new(),
            signature: ManifestSignature::default(),
        }
    }

    /// 开发者签名 + 官方副签，返回清单。
    fn signed_pack(name: &str, dev: &Keypair, official: Option<&Keypair>) -> PluginManifest {
        let mut m = unsigned(name);
        m.sign_with(dev).unwrap();
        if let Some(off) = official {
            m.counter_sign_with(off).unwrap();
        }
        m
    }

    use crate::identity::Keypair;

    #[test]
    fn official_accepted_with_trusted_root() {
        let dev = Keypair::generate();
        let off = Keypair::generate();
        let m = signed_pack("com.twinsearth.official.x", &dev, Some(&off));
        let mut arb = Arbiter::new();
        // 未加官方根 → 拒绝。
        assert!(arb.verify_manifest(&m).is_err());
        arb.add_official_root(&hex::encode(off.public_key()));
        assert_eq!(arb.verify_manifest(&m).unwrap(), Tier::Official);
    }

    #[test]
    fn certified_requires_counter_signature() {
        let dev = Keypair::generate();
        // 无官方副签。
        let m = signed_pack("com.twinsearth.certified.x", &dev, None);
        let arb = Arbiter::new();
        assert!(arb.verify_manifest(&m).is_err());
    }

    #[test]
    fn third_party_requires_explicit_trust() {
        let dev = Keypair::generate();
        let m = signed_pack("com.example.x", &dev, None);
        let mut arb = Arbiter::new();
        // 未信任 → 拒绝。
        assert!(arb.verify_manifest(&m).is_err());
        arb.trust_publisher(&hex::encode(dev.public_key()));
        assert_eq!(arb.verify_manifest(&m).unwrap(), Tier::ThirdParty);
    }

    #[test]
    fn tampered_manifest_rejected() {
        let dev = Keypair::generate();
        let mut m = signed_pack("com.example.x", &dev, None);
        let mut arb = Arbiter::new();
        arb.trust_publisher(&hex::encode(dev.public_key()));
        // 篡改。
        m.plugin.version = "4.0.0".to_string();
        assert!(arb.verify_manifest(&m).is_err());
    }

    #[test]
    fn third_party_denied_network_capability() {
        let dev = Keypair::generate();
        let mut m = unsigned("com.example.x");
        m.capabilities.grant = vec!["net:dht:write".to_string()];
        m.sign_with(&dev).unwrap();
        let mut arb = Arbiter::new();
        arb.trust_publisher(&hex::encode(dev.public_key()));
        // 签名校验通过，但能力 Denied → 签发失败。
        assert!(arb.issue_token(&m, 1).is_err());
    }

    #[test]
    fn official_declarable_requires_approval() {
        let dev = Keypair::generate();
        let off = Keypair::generate();
        let mut m = unsigned("com.twinsearth.official.x");
        m.capabilities.grant = vec!["net:dht:write".to_string()];
        m.sign_with(&dev).unwrap();
        m.counter_sign_with(&off).unwrap();
        let mut arb = Arbiter::new();
        arb.add_official_root(&hex::encode(off.public_key()));
        // 未审批 → 拒绝。
        assert!(arb.issue_token(&m, 1).is_err());
        arb.approve(&m.plugin.name, Capability::DhtWrite);
        let token = arb.issue_token(&m, 1).unwrap();
        assert!(token.has(Capability::DhtWrite));
        // 基础能力也在。
        assert!(token.has(Capability::MessageSend));
    }
}
