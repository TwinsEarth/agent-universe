//! 治理签名命令与受权治理集（P0-4 修复版）
//!
//! # 背景（P0-4 漏洞）
//!
//! 旧实现里 `arbitrate` 只要求 `arbitrator` 非空字符串（调用方自报即可），
//! 任何人都能罚没任意账户 100% 质押；`deposit` 无凭证即可对任意账户记账
//! （本地铸币）。根因：特权写操作缺 sender 身份 + 签名 + capability 认证，
//! 仲裁者不在受权治理集。
//!
//! # 修复模型
//!
//! 特权经济写（仲裁罚没 / 信用授予）必须携带 [`SignedGovernanceCommand`]：
//! 治理集成员用自己的 Ed25519 私钥对「capability + 目标 + claim + nonce +
//! 时间窗」签名。`Governance::verify` 依次校验：
//!
//! 1. 治理集非空（为空 = fail-closed，特权路径整体不可用）；
//! 2. sender DID 可解析、标识 == 公钥指纹、公钥非弱；
//! 3. sender 确实在受权治理集，且治理集登记的公钥与信封公钥一致；
//! 4. capability 在受权集合内（`governance:arbitrate` / `governance:credit`）；
//! 5. 时间窗（issued_at 非未来、expires_at 未过期）；
//! 6. 用治理集登记公钥验签；
//! 7. nonce 非空且未被消费过（防重放）。
//!
//! 通过后返回 [`GovernanceAction`]，调用方才允许执行 slash / deposit。
//! 仲裁者身份一律取自信封 `sender_did`，**禁止**请求体自报。
//!
//! # 与 PMB（plugin/bus.rs）的关系
//!
//! 进程内插件总线 PMB 用共享会话 HMAC 做 sender+capability+nonce 认证；
//! 跨节点没有共享密钥，故治理命令用 Ed25519 非对称签名（与
//! `marketplace/qa_committee.rs` 的 `SignedQaVote` 同一模式）。
//!
//! # 生产口径（未验证 / 需外部审计）
//!
//! - `governance:credit`（off-chain 授信）在生产必须由链上支付凭证支持；
//!   本模块只做 off-chain 治理签名，链上凭证验证为后续接口，**未验证**。
//! - 本模块全部密码学改动需外部审计后才可上线。

use crate::identity::signer::Ed25519Signer;
use crate::identity::{is_weak_pubkey, Did, Keypair};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// 签名域标签（domain separation）。
pub const GOV_DOMAIN_TAG: &str = "AU-GOV-CMD/v1";

/// 特权 capability：仲裁（罚没质押）。
pub const GOV_CAP_ARBITRATE: &str = "governance:arbitrate";
/// 特权 capability：off-chain 授信（dev/faucet；生产须链上凭证，未验证）。
pub const GOV_CAP_CREDIT: &str = "governance:credit";

/// 默认签名有效期（秒）。
pub const GOV_DEFAULT_TTL_SECS: u64 = 300;

/// 治理集成员文件环境变量名。
pub const GOV_FILE_ENV: &str = "GSN_GOVERNANCE_FILE";

/// 受权治理集成员文件条目：`[{"did": "did:nau:...", "pubkey_hex": "ab12..."}]`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceMemberEntry {
    pub did: String,
    pub pubkey_hex: String,
}

/// 签名治理命令信封。
///
/// `target` 为绑定目标：仲裁时是 dispute_id，授信时是 account。
/// `claim` 为命令参数（仲裁：`{"guilty": bool}`；授信：`{"amount": i64}`），
/// 规范化后参与签名，杜绝 JSON 键序漂移。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedGovernanceCommand {
    /// 命令唯一 id（留痕/幂等键，参与签名）
    pub id: String,
    /// 签发者 DID
    pub sender_did: String,
    /// 签发者公钥 hex（32 字节）
    pub sender_pubkey: String,
    /// capability（`governance:arbitrate` / `governance:credit`）
    pub capability: String,
    /// 绑定目标（dispute_id 或 account）
    pub target: String,
    /// 命令声明（verdict / amount 等）
    pub claim: Value,
    /// 一次性随机数
    pub nonce: String,
    /// 签发时间（unix 秒）
    pub issued_at: u64,
    /// 过期时间（unix 秒）
    pub expires_at: u64,
    /// Ed25519 签名（hex，64 字节）
    pub signature: String,
}

/// claim 的规范化 JSON（递归按键排序，不依赖线上键序）。
fn canonical_claim(claim: &Value) -> String {
    fn sort(v: Value) -> Value {
        match v {
            Value::Object(m) => {
                let sorted: std::collections::BTreeMap<String, Value> =
                    m.into_iter().map(|(k, val)| (k, sort(val))).collect();
                Value::Object(serde_json::Map::from_iter(sorted))
            }
            Value::Array(a) => Value::Array(a.into_iter().map(sort).collect()),
            other => other,
        }
    }
    serde_json::to_string(&sort(claim.clone())).unwrap_or_else(|_| "{}".to_string())
}

impl SignedGovernanceCommand {
    /// 规范化待签名字节：域标签首行 + 固定字段顺序。
    pub fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "{}\nid={}\nsender_did={}\nsender_pubkey={}\ncapability={}\ntarget={}\nclaim={}\nnonce={}\nissued_at={}\nexpires_at={}",
            GOV_DOMAIN_TAG,
            self.id,
            self.sender_did,
            self.sender_pubkey,
            self.capability,
            self.target,
            canonical_claim(&self.claim),
            self.nonce,
            self.issued_at,
            self.expires_at,
        )
        .into_bytes()
    }

    /// 治理集成员用自己的密钥对签发命令。
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        id: &str,
        sender_did: &str,
        keypair: &Keypair,
        capability: &str,
        target: &str,
        claim: Value,
        nonce: &str,
        issued_at: u64,
        ttl_secs: u64,
    ) -> Self {
        let mut cmd = Self {
            id: id.to_string(),
            sender_did: sender_did.to_string(),
            sender_pubkey: hex::encode(keypair.public_key()),
            capability: capability.to_string(),
            target: target.to_string(),
            claim,
            nonce: nonce.to_string(),
            issued_at,
            expires_at: issued_at.saturating_add(ttl_secs),
            signature: String::new(),
        };
        let sig = Ed25519Signer::new(keypair).sign(&cmd.signing_bytes());
        cmd.signature = hex::encode(sig);
        cmd
    }

    /// 解析签发者公钥为 32 字节。
    pub fn sender_pubkey_bytes(&self) -> Result<[u8; 32], String> {
        let raw = hex::decode(&self.sender_pubkey)
            .map_err(|e| format!("sender_pubkey hex 解码失败: {e}"))?;
        raw.as_slice()
            .try_into()
            .map_err(|_| format!("sender_pubkey 必须为 32 字节，实际 {} 字节", raw.len()))
    }
}

/// 校验通过后的治理动作（调用方据此执行特权写）。
#[derive(Debug, Clone)]
pub struct GovernanceAction {
    pub sender_did: String,
    pub capability: String,
    pub target: String,
    pub claim: Value,
}

/// 受权治理集与已消费 nonce。
#[derive(Debug, Default)]
pub struct Governance {
    members: HashMap<String, [u8; 32]>,
    seen_nonces: HashSet<String>,
    capabilities: HashSet<String>,
}

impl Governance {
    /// 空治理集（fail-closed：特权路径整体不可用）。
    pub fn empty() -> Self {
        Self {
            members: HashMap::new(),
            seen_nonces: HashSet::new(),
            capabilities: HashSet::from_iter([
                GOV_CAP_ARBITRATE.to_string(),
                GOV_CAP_CREDIT.to_string(),
            ]),
        }
    }

    /// 由成员列表构造治理集。
    pub fn from_members(members: Vec<(String, [u8; 32])>) -> Self {
        let mut g = Self::empty();
        for (did, pk) in members {
            g.members.insert(did, pk);
        }
        g
    }

    /// 从成员文件构造（JSON 数组：`[{"did","pubkey_hex"}]`）。
    pub fn from_member_file(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("治理集文件 {path} 读取失败: {e}"))?;
        let entries: Vec<GovernanceMemberEntry> =
            serde_json::from_str(&text).map_err(|e| format!("治理集文件 {path} 解析失败: {e}"))?;
        let mut g = Self::empty();
        for e in entries {
            let pk = hex::decode(&e.pubkey_hex)
                .map_err(|err| format!("成员 {} pubkey_hex 解码失败: {err}", e.did))?;
            let pk: [u8; 32] = pk
                .as_slice()
                .try_into()
                .map_err(|_| format!("成员 {} 公钥必须为 32 字节", e.did))?;
            g.members.insert(e.did, pk);
        }
        Ok(g)
    }

    /// 从环境变量 `GSN_GOVERNANCE_FILE` 构造。
    ///
    /// 未设置 / 空路径 / 文件不存在 → 空治理集（fail-closed）。
    /// 显式设置但读取/解析失败 → eprintln 告警后降级为空治理集
    /// （特权路径不可用，宁可不授也不误授；与 whitelist 的严格错误不同，
    /// 这里在 market actor 启动路径上无法返回 Err，故用可观测告警降级）。
    pub fn from_env() -> Self {
        match std::env::var(GOV_FILE_ENV) {
            Ok(path) if !path.trim().is_empty() => match Self::from_member_file(&path) {
                Ok(g) => {
                    println!("🏛️ 治理集已从 {path} 加载：{} 名成员", g.members.len());
                    g
                }
                Err(e) => {
                    eprintln!(
                        "🚨 CRITICAL: {e}——降级为空治理集，特权经济写路径 fail-closed 不可用"
                    );
                    Self::empty()
                }
            },
            _ => Self::empty(),
        }
    }

    /// 治理集是否为空（空 = 特权路径不可用）。
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// 成员数。
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// 验证签名治理命令（见模块文档的校验顺序）。
    ///
    /// # 需外部审计
    pub fn verify(
        &mut self,
        cmd: &SignedGovernanceCommand,
        now: u64,
    ) -> Result<GovernanceAction, String> {
        // 1. fail-closed
        if self.members.is_empty() {
            return Err(
                "GOV_NO_MEMBERS: 治理集为空，特权经济写路径不可用（fail-closed）".to_string(),
            );
        }
        if cmd.target.trim().is_empty() {
            return Err("GOV_BAD_TARGET: 命令缺少绑定目标".to_string());
        }

        // 2. sender DID ↔ 公钥
        let did = Did::parse(&cmd.sender_did)?;
        let pk = cmd.sender_pubkey_bytes()?;
        if is_weak_pubkey(&pk) {
            return Err("GOV_WEAK_PUBKEY: 签发公钥退化，拒绝".to_string());
        }
        if did.identifier() != Did::fingerprint(&pk) {
            return Err(format!(
                "GOV_DID_MISMATCH: sender_did 标识 {} 与公钥指纹 {} 不一致",
                did.identifier(),
                Did::fingerprint(&pk)
            ));
        }

        // 3. 成员在治理集，且登记公钥与信封公钥一致（防换钥）
        let registered = self
            .members
            .get(&cmd.sender_did)
            .ok_or_else(|| format!("GOV_NOT_MEMBER: {} 不在受权治理集", cmd.sender_did))?;
        if *registered != pk {
            return Err("GOV_KEY_ROTATION: 信封公钥与治理集登记公钥不一致".to_string());
        }

        // 4. capability 受权
        if !self.capabilities.contains(&cmd.capability) {
            return Err(format!(
                "GOV_BAD_CAPABILITY: capability '{}' 不在受权集合",
                cmd.capability
            ));
        }

        // 5. 时间窗
        if cmd.expires_at <= cmd.issued_at {
            return Err("GOV_BAD_WINDOW: expires_at 必须晚于 issued_at".to_string());
        }
        if now < cmd.issued_at {
            return Err("GOV_NOT_YET_VALID: 命令尚未生效".to_string());
        }
        if now > cmd.expires_at {
            return Err("GOV_EXPIRED: 命令已过期".to_string());
        }

        // 6. 验签（先验签再消费 nonce）
        let sig = hex::decode(&cmd.signature).map_err(|e| format!("签名 hex 解码失败: {e}"))?;
        if !Ed25519Signer::verify_with_pubkey(&pk, &cmd.signing_bytes(), &sig) {
            return Err("GOV_BAD_SIGNATURE: 治理命令签名验证失败".to_string());
        }

        // 7. nonce 防重放
        if cmd.nonce.is_empty() {
            return Err("GOV_EMPTY_NONCE: 命令缺少 nonce".to_string());
        }
        if !self.seen_nonces.insert(cmd.nonce.clone()) {
            return Err(format!(
                "GOV_REPLAY: nonce {} 已被消费过（重放）",
                cmd.nonce
            ));
        }

        Ok(GovernanceAction {
            sender_did: cmd.sender_did.clone(),
            capability: cmd.capability.clone(),
            target: cmd.target.clone(),
            claim: cmd.claim.clone(),
        })
    }
}

/// 从仲裁命令 claim 解析 `guilty` 布尔（checked，与请求 target 由调用方再比对）。
pub fn parse_arbitrate_claim(claim: &Value) -> Result<bool, String> {
    claim
        .get("guilty")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| "仲裁命令 claim 缺少布尔字段 guilty".to_string())
}

/// 从授信命令 claim 解析金额（非负整数；checked，拒绝负数/浮点/缺失）。
pub fn parse_credit_claim(claim: &Value) -> Result<crate::marketplace::Money, String> {
    let v = claim
        .get("amount")
        .and_then(|x| x.as_i64())
        .ok_or_else(|| "授信命令 claim 缺少整数字段 amount".to_string())?;
    if v < 0 {
        return Err("授信金额不能为负".to_string());
    }
    Ok(crate::marketplace::Money::new(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 2_000_000;

    fn member(seed: u8) -> (String, Keypair) {
        let mut s = [0u8; 32];
        s[0] = seed;
        let kp = Keypair::from_seed(&s);
        let did = Did::from_public_key(kp.public_key()).to_string();
        (did, kp)
    }

    fn gov2() -> (Governance, Vec<(String, Keypair)>) {
        let mut mset = Vec::new();
        let mut kps = Vec::new();
        for i in 1u8..=2 {
            let (did, kp) = member(i);
            let pk: [u8; 32] = kp.public_key().try_into().unwrap();
            mset.push((did.clone(), pk));
            kps.push((did, kp));
        }
        (Governance::from_members(mset), kps)
    }

    #[test]
    fn empty_governance_rejects_everything() {
        let (_, kps) = gov2();
        let cmd = SignedGovernanceCommand::sign(
            "c1",
            &kps[0].0,
            &kps[0].1,
            GOV_CAP_ARBITRATE,
            "d1",
            json!({"guilty": true}),
            "n1",
            NOW - 10,
            GOV_DEFAULT_TTL_SECS,
        );
        let mut g = Governance::empty();
        assert!(g.verify(&cmd, NOW).unwrap_err().contains("NO_MEMBERS"));
    }

    #[test]
    fn signed_arbitrate_command_accepts() {
        let (mut g, kps) = gov2();
        let cmd = SignedGovernanceCommand::sign(
            "c1",
            &kps[0].0,
            &kps[0].1,
            GOV_CAP_ARBITRATE,
            "d1",
            json!({"guilty": true}),
            "n1",
            NOW - 10,
            GOV_DEFAULT_TTL_SECS,
        );
        let a = g.verify(&cmd, NOW).unwrap();
        assert_eq!(a.capability, GOV_CAP_ARBITRATE);
        assert_eq!(a.target, "d1");
        assert!(parse_arbitrate_claim(&a.claim).unwrap());
    }

    #[test]
    fn non_member_rejected() {
        let (mut g, _kps) = gov2();
        let mut s = [0u8; 32];
        s[0] = 77;
        let kp = Keypair::from_seed(&s);
        let did = Did::from_public_key(kp.public_key()).to_string();
        let cmd = SignedGovernanceCommand::sign(
            "c",
            &did,
            &kp,
            GOV_CAP_ARBITRATE,
            "d1",
            json!({"guilty": true}),
            "n",
            NOW - 10,
            300,
        );
        assert!(g.verify(&cmd, NOW).unwrap_err().contains("NOT_MEMBER"));
    }

    #[test]
    fn forged_signature_rejected() {
        let (mut g, kps) = gov2();
        let mut cmd = SignedGovernanceCommand::sign(
            "c",
            &kps[0].0,
            &kps[0].1,
            GOV_CAP_ARBITRATE,
            "d1",
            json!({"guilty": true}),
            "n",
            NOW - 10,
            300,
        );
        cmd.signature = "00".repeat(64);
        assert!(g.verify(&cmd, NOW).unwrap_err().contains("BAD_SIGNATURE"));
    }

    #[test]
    fn replay_nonce_rejected() {
        let (mut g, kps) = gov2();
        let cmd = SignedGovernanceCommand::sign(
            "c",
            &kps[0].0,
            &kps[0].1,
            GOV_CAP_CREDIT,
            "acct",
            json!({"amount": 5}),
            "once",
            NOW - 10,
            300,
        );
        g.verify(&cmd, NOW).unwrap();
        assert!(g.verify(&cmd, NOW).unwrap_err().contains("REPLAY"));
    }

    #[test]
    fn expired_command_rejected() {
        let (mut g, kps) = gov2();
        let cmd = SignedGovernanceCommand::sign(
            "c",
            &kps[0].0,
            &kps[0].1,
            GOV_CAP_CREDIT,
            "acct",
            json!({"amount": 5}),
            "n",
            NOW - 1000,
            30,
        );
        assert!(g.verify(&cmd, NOW).unwrap_err().contains("EXPIRED"));
    }

    #[test]
    fn wrong_capability_rejected() {
        let (mut g, kps) = gov2();
        let cmd = SignedGovernanceCommand::sign(
            "c",
            &kps[0].0,
            &kps[0].1,
            "governance:hack",
            "d1",
            json!({}),
            "n",
            NOW - 10,
            300,
        );
        assert!(g.verify(&cmd, NOW).unwrap_err().contains("BAD_CAPABILITY"));
    }

    #[test]
    fn tampered_claim_after_signature_breaks_verify() {
        let (mut g, kps) = gov2();
        let mut cmd = SignedGovernanceCommand::sign(
            "c",
            &kps[0].0,
            &kps[0].1,
            GOV_CAP_CREDIT,
            "acct",
            json!({"amount": 5}),
            "n",
            NOW - 10,
            300,
        );
        cmd.claim = json!({"amount": 999999});
        assert!(g.verify(&cmd, NOW).unwrap_err().contains("BAD_SIGNATURE"));
    }

    #[test]
    fn credit_claim_parses_nonnegative_amount() {
        assert_eq!(
            parse_credit_claim(&json!({"amount": 100})).unwrap(),
            crate::marketplace::Money::new(100)
        );
        assert!(parse_credit_claim(&json!({"amount": -1})).is_err());
        assert!(parse_credit_claim(&json!({"amount": 1.5})).is_err());
        assert!(parse_credit_claim(&json!({})).is_err());
    }

    #[test]
    fn from_member_file_roundtrip() {
        let (_, kps) = gov2();
        let tmp = std::env::temp_dir().join(format!("gov-{}.json", uuid::Uuid::new_v4()));
        let path = tmp.to_string_lossy().to_string();
        let entries: Vec<GovernanceMemberEntry> = kps
            .iter()
            .map(|(did, kp)| GovernanceMemberEntry {
                did: did.clone(),
                pubkey_hex: hex::encode(kp.public_key()),
            })
            .collect();
        std::fs::write(&path, serde_json::to_string(&entries).unwrap()).unwrap();
        let g = Governance::from_member_file(&path).unwrap();
        assert_eq!(g.member_count(), 2);
        let _ = std::fs::remove_file(&path);
    }
}
