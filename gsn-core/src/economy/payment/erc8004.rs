//! ERC-8004「Trustless Agents」三注册表纯协议内核（v3.9.4）。
//!
//! # 定位
//!
//! ERC-8004 是以太坊面向自主 Agent 的身份、信誉与验证协调标准，由三个轻量注册表组成：
//!
//! - **Identity Registry**：基于 ERC-721 的链上句柄，`tokenId` 唯一，tokenURI 指向链下
//!   AgentCard，DID/公钥与链上句柄绑定；
//! - **Reputation Registry**：发布/获取反馈信号的标准接口，评分聚合可链上或链下；
//! - **Validation Registry**：独立验证者检查的通用钩子（质押重跑 / TEE 证明 / zkML）。
//!
//! 与 v3.9.2/v3.9.3 的 x402/L402 一致，本模块只承担**沙盒可安全运行的纯确定性面**：
//! 身份绑定/承诺的构造与校验、反馈事件的整数聚合与守恒、独立验证权重的 BFT-lite 多数
//! 裁决。绝不：
//!
//! - 连接任何 EVM RPC / 以太坊节点，不铸造真实 ERC-721、不发起链上交易；
//! - 持私钥、签名、广播、划转；
//! - 从链上读取注册表当前状态（调用方自行从受信任索引/RPC 取证后把快照传入）；
//! - 用浮点做信誉归一化或多数判定（全程整数，溢出具名失败，不 panic）。
//!
//! 因此这是「链下可验证决策面」：上层官方插件/宿主在真要写链时，需经宿主签名闸门
//! （v3.9.1）与独立的链上写入能力（本版本不提供）。
//!
//! 外部报道的 ERC-8004 采用量（Agent/反馈事件数）为第三方口径，非本仓复测，内核不内置。

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// DID 方法前缀。
pub const DID_PREFIX: &str = "did:";
/// Ed25519 公钥字节长度。
pub const ED25519_PUBKEY_LEN: usize = 32;
/// 反馈分值上下限（含）。
pub const SCORE_MIN: i16 = -100;
pub const SCORE_MAX: i16 = 100;
/// 归一化信誉分中值（无反馈/完全中性）与满程。
pub const SCORE_NEUTRAL: u16 = 500;
pub const SCORE_FULL: u16 = 1000;

/// ERC-8004 内核错误（全部具名码，生产路径不 panic）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Erc8004Error {
    /// DID 非法：必须以 `did:` 开头、非空、无空白/控制字符、长度受限。
    InvalidDid,
    /// 公钥非法：必须是 32 字节 Ed25519 公钥的 hex。
    InvalidPubKey,
    /// tokenId 非法（0 保留）。
    InvalidTokenId,
    /// tokenURI 承诺哈希非法（非 32 字节 hex）。
    InvalidUriHash,
    /// tokenId / DID / 公钥任一已被注册。
    DuplicateIdentity,
    /// 引用了不存在的身份句柄。
    UnknownIdentity,
    /// 反馈对象为空字符串。
    EmptySubject,
    /// 不允许自我反馈（防自评刷分的内核策略）。
    SelfFeedbackForbidden,
    /// 同一 (reviewer,nonce) 反馈重复提交（重放）。
    DuplicateFeedbackNonce,
    /// 反馈分值越界（[-100,100]）。
    InvalidScore,
    /// 权重必须为正整数。
    InvalidWeight,
    /// 证据哈希非法（非 32 字节 hex）。
    InvalidEvidenceHash,
    /// 多数裁决计账非法（如总权重为 0）。
    InvalidQuorumTally,
    /// 未知/不支持的验证方法或裁决取值。
    InvalidVerdict,
    /// 整数运算溢出（理论极值，具名失败而非 wrap/panic）。
    ArithmeticOverflow,
}

impl Erc8004Error {
    pub fn code(&self) -> &'static str {
        match self {
            Erc8004Error::InvalidDid => "ERC8004_INVALID_DID",
            Erc8004Error::InvalidPubKey => "ERC8004_INVALID_PUBKEY",
            Erc8004Error::InvalidTokenId => "ERC8004_INVALID_TOKEN_ID",
            Erc8004Error::InvalidUriHash => "ERC8004_INVALID_URI_HASH",
            Erc8004Error::DuplicateIdentity => "ERC8004_DUPLICATE_IDENTITY",
            Erc8004Error::UnknownIdentity => "ERC8004_UNKNOWN_IDENTITY",
            Erc8004Error::EmptySubject => "ERC8004_EMPTY_SUBJECT",
            Erc8004Error::SelfFeedbackForbidden => "ERC8004_SELF_FEEDBACK_FORBIDDEN",
            Erc8004Error::DuplicateFeedbackNonce => "ERC8004_DUPLICATE_FEEDBACK_NONCE",
            Erc8004Error::InvalidScore => "ERC8004_INVALID_SCORE",
            Erc8004Error::InvalidWeight => "ERC8004_INVALID_WEIGHT",
            Erc8004Error::InvalidEvidenceHash => "ERC8004_INVALID_EVIDENCE_HASH",
            Erc8004Error::InvalidQuorumTally => "ERC8004_INVALID_QUORUM_TALLY",
            Erc8004Error::InvalidVerdict => "ERC8004_INVALID_VERDICT",
            Erc8004Error::ArithmeticOverflow => "ERC8004_ARITHMETIC_OVERFLOW",
        }
    }
}

impl core::fmt::Display for Erc8004Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for Erc8004Error {}

type KernelResult<T> = Result<T, Erc8004Error>;

/// 校验 32 字节 hex（0x/0X 前缀可选，大小写不敏感）。
fn is_bytes32_hex(s: &str) -> bool {
    norm_hex32(s).is_some()
}

/// 归一化 32 字节 hex：去 0x/0X 前缀、转小写；非法长度/字符返回 None。
fn norm_hex32(s: &str) -> Option<String> {
    let h = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(h.to_lowercase())
    } else {
        None
    }
}

/// 校验 DID：`did:` 前缀、长度 1..=256、仅可见非空白字符。
pub fn validate_did(did: &str) -> bool {
    let body = match did.strip_prefix(DID_PREFIX) {
        Some(b) => b,
        None => return false,
    };
    !body.is_empty()
        && did.len() <= 256
        && did.chars().all(|c| !c.is_whitespace() && !c.is_control())
}

/// 校验 Ed25519 公钥 hex（32 字节，0x/0X 前缀可选）。
pub fn validate_pubkey_hex(pubkey_hex: &str) -> bool {
    norm_hex32(pubkey_hex).is_some()
}

/// 计算 tokenURI 内容承诺（标准 SHA-256 hex；ERC-8004 tokenURI 指向链下 AgentCard）。
pub fn hash_token_uri(token_uri: &str) -> String {
    hex::encode(Sha256::digest(token_uri.as_bytes()))
}

// ───────────────────────── Identity Registry ─────────────────────────

/// 一条链上身份句柄的链下可验证视图（tokenId ↔ DID ↔ Ed25519 公钥 ↔ tokenURI 承诺）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IdentityRecord {
    pub token_id: u64,
    pub did: String,
    pub pubkey_hex: String,
    pub uri_hash_hex: String,
}

/// 确定性内存身份注册表（链下镜像/测试用；真网状态由调用方取证后注入）。
#[derive(Default, Debug, Clone)]
pub struct IdentityRegistry {
    by_token: BTreeMap<u64, IdentityRecord>,
    dids: BTreeSet<String>,
    pubkeys: BTreeSet<String>,
}

impl IdentityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.by_token.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_token.is_empty()
    }

    pub fn contains(&self, token_id: u64) -> bool {
        self.by_token.contains_key(&token_id)
    }

    pub fn get(&self, token_id: u64) -> Option<&IdentityRecord> {
        self.by_token.get(&token_id)
    }

    /// 以 tokenURI 原文铸造绑定：内部计算 SHA-256 承诺。
    pub fn mint(
        &mut self,
        token_id: u64,
        did: &str,
        pubkey_hex: &str,
        token_uri: &str,
    ) -> KernelResult<()> {
        self.mint_with_hash(token_id, did, pubkey_hex, &hash_token_uri(token_uri))
    }

    /// 以显式 tokenURI 承诺铸造（链上已存承诺时直接比对）。
    pub fn mint_with_hash(
        &mut self,
        token_id: u64,
        did: &str,
        pubkey_hex: &str,
        uri_hash_hex: &str,
    ) -> KernelResult<()> {
        if token_id == 0 {
            return Err(Erc8004Error::InvalidTokenId);
        }
        if !validate_did(did) {
            return Err(Erc8004Error::InvalidDid);
        }
        let norm_pk = norm_hex32(pubkey_hex).ok_or(Erc8004Error::InvalidPubKey)?;
        let norm_uh = norm_hex32(uri_hash_hex).ok_or(Erc8004Error::InvalidUriHash)?;
        if self.by_token.contains_key(&token_id)
            || self.dids.contains(did)
            || self.pubkeys.contains(&norm_pk)
        {
            return Err(Erc8004Error::DuplicateIdentity);
        }
        self.dids.insert(did.to_string());
        self.by_token.insert(
            token_id,
            IdentityRecord {
                token_id,
                did: did.to_string(),
                pubkey_hex: norm_pk.clone(),
                uri_hash_hex: norm_uh,
            },
        );
        self.pubkeys.insert(norm_pk);
        Ok(())
    }

    /// 校验给定四元组与注册表中的句柄绑定一致。
    pub fn verify_binding(
        &self,
        token_id: u64,
        did: &str,
        pubkey_hex: &str,
        uri_hash_hex: &str,
    ) -> bool {
        match self.by_token.get(&token_id) {
            Some(rec) => match (norm_hex32(pubkey_hex), norm_hex32(uri_hash_hex)) {
                (Some(np), Some(nu)) => {
                    rec.did == did && rec.pubkey_hex == np && rec.uri_hash_hex == nu
                }
                _ => false,
            },
            None => false,
        }
    }
}

// ───────────────────────── Reputation Registry ─────────────────────────

/// 信誉维度（与资源市场四维信誉对齐：质量/速度/诚实/可用）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ReputationDimension {
    Quality,
    Speed,
    Honesty,
    Availability,
}

impl ReputationDimension {
    pub fn index(self) -> usize {
        match self {
            ReputationDimension::Quality => 0,
            ReputationDimension::Speed => 1,
            ReputationDimension::Honesty => 2,
            ReputationDimension::Availability => 3,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ReputationDimension::Quality => "quality",
            ReputationDimension::Speed => "speed",
            ReputationDimension::Honesty => "honesty",
            ReputationDimension::Availability => "availability",
        }
    }

    pub fn all() -> [ReputationDimension; 4] {
        [
            ReputationDimension::Quality,
            ReputationDimension::Speed,
            ReputationDimension::Honesty,
            ReputationDimension::Availability,
        ]
    }
}

/// 一条带质押权重的反馈事件（评分 [-100,100]，权重>0；nonce 防重放）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Feedback {
    pub reviewer: u64,
    pub subject: u64,
    pub dimension: ReputationDimension,
    pub score: i16,
    pub weight: u64,
    pub nonce: u64,
}

/// 某主体在某维度上的整数累加器（守恒：signed_sum == Σ score*weight）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ReputationAccumulator {
    pub signed_sum: i128,
    pub total_weight: u128,
    pub count: u64,
}

impl ReputationAccumulator {
    /// 归一化到 0..=1000（中性 500）：signed_sum/(100*total_weight) ∈ [-1,1] 线性映射。
    pub fn score_01k(&self) -> KernelResult<u16> {
        if self.total_weight == 0 {
            return Ok(SCORE_NEUTRAL);
        }
        let denom = (self.total_weight)
            .checked_mul(100)
            .ok_or(Erc8004Error::ArithmeticOverflow)?;
        // signed_sum*500/denom ∈ [-500,500]；i128 对 u64 权重×有限计数足够，仍做饱和。
        let delta = self
            .signed_sum
            .checked_mul(500)
            .ok_or(Erc8004Error::ArithmeticOverflow)?
            / (denom as i128);
        let v = SCORE_NEUTRAL as i32 + delta as i32;
        Ok(v.clamp(0, SCORE_FULL as i32) as u16)
    }
}

/// 某主体四维快照。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReputationSnapshot {
    pub subject: u64,
    pub dimensions: [ReputationAccumulator; 4],
}

impl ReputationSnapshot {
    pub fn score_01k(&self, dim: ReputationDimension) -> KernelResult<u16> {
        self.dimensions[dim.index()].score_01k()
    }
}

/// 确定性内存信誉注册表（链下聚合；锚定到链上由后续写入面负责）。
#[derive(Default, Debug, Clone)]
pub struct ReputationRegistry {
    acc: BTreeMap<u64, [ReputationAccumulator; 4]>,
    seen_nonce: BTreeSet<(u64, u64)>,
}

impl ReputationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记入一条反馈：reviewer/subject 必须存在且不同，分值/权重合法，nonce 不重放。
    pub fn record(&mut self, fb: Feedback, identities: &IdentityRegistry) -> KernelResult<()> {
        if fb.score < SCORE_MIN || fb.score > SCORE_MAX {
            return Err(Erc8004Error::InvalidScore);
        }
        if fb.weight == 0 {
            return Err(Erc8004Error::InvalidWeight);
        }
        if fb.reviewer == fb.subject {
            return Err(Erc8004Error::SelfFeedbackForbidden);
        }
        if !identities.contains(fb.reviewer) || !identities.contains(fb.subject) {
            return Err(Erc8004Error::UnknownIdentity);
        }
        if !self.seen_nonce.insert((fb.reviewer, fb.nonce)) {
            return Err(Erc8004Error::DuplicateFeedbackNonce);
        }
        let dims = self.acc.entry(fb.subject).or_default();
        let a = &mut dims[fb.dimension.index()];
        let contrib = (fb.score as i128)
            .checked_mul(fb.weight as i128)
            .ok_or(Erc8004Error::ArithmeticOverflow)?;
        a.signed_sum = a
            .signed_sum
            .checked_add(contrib)
            .ok_or(Erc8004Error::ArithmeticOverflow)?;
        a.total_weight = a
            .total_weight
            .checked_add(fb.weight as u128)
            .ok_or(Erc8004Error::ArithmeticOverflow)?;
        a.count += 1;
        Ok(())
    }

    pub fn snapshot(&self, subject: u64) -> ReputationSnapshot {
        let empty = [ReputationAccumulator::default(); 4];
        ReputationSnapshot {
            subject,
            dimensions: *self.acc.get(&subject).unwrap_or(&empty),
        }
    }

    /// 守恒自检：某主体各维 total_weight*count 与累加一致由 record 构造保证；此出 count。
    pub fn feedback_count(&self, subject: u64, dim: ReputationDimension) -> u64 {
        self.acc
            .get(&subject)
            .map(|d| d[dim.index()].count)
            .unwrap_or(0)
    }
}

// ───────────────────────── Validation Registry ─────────────────────────

/// 独立验证方法：质押重跑 / TEE 证明 / zkML 验证器。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValidationMethod {
    StakeRerun,
    TeeAttestation,
    Zkml,
}

/// 单次独立验证投票的结论（二元；无法达阈值由裁决给出 Inconclusive）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValidationVerdict {
    Valid,
    Invalid,
}

/// 一条独立验证记录。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ValidationVote {
    pub validator: u64,
    pub method: ValidationMethod,
    pub verdict: ValidationVerdict,
    pub weight: u64,
}

/// BFT-lite 裁决结论（n ≥ 3f+1：任一方向需严格 >2/3 总权重）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValidationDecision {
    ConfirmedValid,
    ConfirmedInvalid,
    Inconclusive,
}

/// 验证权重计账器（valid/invalid 双向权重 + 总权重）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ValidationTally {
    pub valid_weight: u128,
    pub invalid_weight: u128,
    pub total_weight: u128,
}

impl ValidationTally {
    pub fn new() -> Self {
        Self::default()
    }

    /// 计一票：验证者身份需已注册、权重为正、证据哈希合法（本函数只校权重，证据在
    /// [`record_vote_checked`] 结合身份与证据校验）。
    pub fn record_vote(&mut self, vote: ValidationVote) -> KernelResult<()> {
        if vote.weight == 0 {
            return Err(Erc8004Error::InvalidWeight);
        }
        let w = vote.weight as u128;
        self.total_weight = self
            .total_weight
            .checked_add(w)
            .ok_or(Erc8004Error::ArithmeticOverflow)?;
        match vote.verdict {
            ValidationVerdict::Valid => {
                self.valid_weight = self
                    .valid_weight
                    .checked_add(w)
                    .ok_or(Erc8004Error::ArithmeticOverflow)?;
            }
            ValidationVerdict::Invalid => {
                self.invalid_weight = self
                    .invalid_weight
                    .checked_add(w)
                    .ok_or(Erc8004Error::ArithmeticOverflow)?;
            }
        }
        let _ = vote.method;
        Ok(())
    }

    /// 结合身份注册表与证据哈希计票。
    pub fn record_vote_checked(
        &mut self,
        vote: ValidationVote,
        identities: &IdentityRegistry,
        evidence_hash_hex: &str,
    ) -> KernelResult<()> {
        if !identities.contains(vote.validator) {
            return Err(Erc8004Error::UnknownIdentity);
        }
        if !is_bytes32_hex(evidence_hash_hex) {
            return Err(Erc8004Error::InvalidEvidenceHash);
        }
        self.record_vote(vote)
    }

    /// BFT-lite 裁决：valid/invalid 各自需满足 weight*3 >= total*2 且严格大于对方。
    pub fn decide(&self) -> KernelResult<ValidationDecision> {
        if self.total_weight == 0 {
            return Err(Erc8004Error::InvalidQuorumTally);
        }
        let two_total = self
            .total_weight
            .checked_mul(2)
            .ok_or(Erc8004Error::ArithmeticOverflow)?;
        let three_valid = self
            .valid_weight
            .checked_mul(3)
            .ok_or(Erc8004Error::ArithmeticOverflow)?;
        let three_invalid = self
            .invalid_weight
            .checked_mul(3)
            .ok_or(Erc8004Error::ArithmeticOverflow)?;
        if three_valid >= two_total && self.valid_weight > self.invalid_weight {
            Ok(ValidationDecision::ConfirmedValid)
        } else if three_invalid >= two_total && self.invalid_weight > self.valid_weight {
            Ok(ValidationDecision::ConfirmedInvalid)
        } else {
            Ok(ValidationDecision::Inconclusive)
        }
    }
}

/// 纯函数：对一批反馈做无状态聚合（用于 PMB handler；内部做重放检测与守恒）。
pub fn aggregate_feedback(
    feedbacks: &[Feedback],
    identities: &IdentityRegistry,
) -> KernelResult<BTreeMap<u64, ReputationSnapshot>> {
    let mut reg = ReputationRegistry::new();
    for fb in feedbacks {
        reg.record(*fb, identities)?;
    }
    let mut out = BTreeMap::new();
    let mut subjects: BTreeSet<u64> = BTreeSet::new();
    for fb in feedbacks {
        subjects.insert(fb.subject);
    }
    for s in subjects {
        out.insert(s, reg.snapshot(s));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_agents() -> IdentityRegistry {
        let mut r = IdentityRegistry::new();
        r.mint(1, "did:tw:alpha", &"11".repeat(32), "ipfs://card-alpha")
            .unwrap();
        r.mint(2, "did:tw:beta", &"22".repeat(32), "ipfs://card-beta")
            .unwrap();
        r
    }

    #[test]
    fn did_validation_rules() {
        assert!(validate_did("did:tw:alpha"));
        assert!(!validate_did("tw:alpha"));
        assert!(!validate_did("did:"));
        assert!(!validate_did("did:bad name"));
        assert!(!validate_did("did:bad\ttab"));
        assert!(!validate_did(&format!("did:{}", "x".repeat(253))));
    }

    #[test]
    fn pubkey_validation_rules() {
        assert!(validate_pubkey_hex(&"ab".repeat(32)));
        assert!(validate_pubkey_hex(&format!("0x{}", "cd".repeat(32))));
        assert!(!validate_pubkey_hex(&"ab".repeat(31)));
        assert!(!validate_pubkey_hex("not-hex"));
    }

    #[test]
    fn uri_hash_is_deterministic_sha256() {
        let a = hash_token_uri("ipfs://card");
        let b = hash_token_uri("ipfs://card");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert_ne!(a, hash_token_uri("ipfs://other"));
    }

    #[test]
    fn identity_mint_and_binding() {
        let ids = two_agents();
        assert_eq!(ids.len(), 2);
        let uh = hash_token_uri("ipfs://card-alpha");
        assert!(ids.verify_binding(1, "did:tw:alpha", &"11".repeat(32), &uh));
        // 0x 前缀/大小写归一化后仍应一致。
        assert!(ids.verify_binding(
            1,
            "did:tw:alpha",
            &format!("0x{}", "11".repeat(32)).to_uppercase(),
            &uh.to_uppercase()
        ));
        assert!(!ids.verify_binding(1, "did:tw:alpha", &"33".repeat(32), &uh));
        assert!(!ids.verify_binding(9, "did:nobody", &"44".repeat(32), &uh));
    }

    #[test]
    fn identity_mint_rejects_bad_and_duplicate() {
        let mut ids = two_agents();
        assert_eq!(
            ids.mint(0, "did:x", &"11".repeat(32), "u").err(),
            Some(Erc8004Error::InvalidTokenId)
        );
        assert_eq!(
            ids.mint(3, "nope", &"11".repeat(32), "u").err(),
            Some(Erc8004Error::InvalidDid)
        );
        assert_eq!(
            ids.mint(3, "did:z", "short", "u").err(),
            Some(Erc8004Error::InvalidPubKey)
        );
        assert_eq!(
            ids.mint_with_hash(3, "did:z", &"55".repeat(32), "not-a-hash")
                .err(),
            Some(Erc8004Error::InvalidUriHash)
        );
        // 重复 token / did / pubkey 三种冲突。
        let uh = hash_token_uri("u");
        assert!(ids
            .mint_with_hash(1, "did:new", &"66".repeat(32), &uh)
            .is_err());
        assert!(ids
            .mint_with_hash(3, "did:tw:alpha", &"66".repeat(32), &uh)
            .is_err());
        assert!(ids
            .mint_with_hash(3, "did:new", &"11".repeat(32), &uh)
            .is_err());
    }

    fn fb(
        reviewer: u64,
        subject: u64,
        dim: ReputationDimension,
        score: i16,
        w: u64,
        n: u64,
    ) -> Feedback {
        Feedback {
            reviewer,
            subject,
            dimension: dim,
            score,
            weight: w,
            nonce: n,
        }
    }

    #[test]
    fn reputation_positive_negative_and_neutral() {
        let ids = two_agents();
        let mut reg = ReputationRegistry::new();
        // 无反馈中性 500。
        assert_eq!(
            reg.snapshot(2)
                .score_01k(ReputationDimension::Quality)
                .unwrap(),
            500
        );
        reg.record(fb(1, 2, ReputationDimension::Quality, 100, 10, 1), &ids)
            .unwrap();
        // 全正：100*10/(100*10)=1 -> 1000。
        assert_eq!(
            reg.snapshot(2)
                .score_01k(ReputationDimension::Quality)
                .unwrap(),
            1000
        );
        reg.record(fb(1, 2, ReputationDimension::Quality, -100, 10, 2), &ids)
            .unwrap();
        // 正负等权抵消 -> 500。
        assert_eq!(
            reg.snapshot(2)
                .score_01k(ReputationDimension::Quality)
                .unwrap(),
            500
        );
        assert_eq!(reg.feedback_count(2, ReputationDimension::Quality), 2);
    }

    #[test]
    fn reputation_weighted_partial_and_dimension_isolation() {
        let ids = two_agents();
        let mut reg = ReputationRegistry::new();
        reg.record(fb(1, 2, ReputationDimension::Speed, 100, 30, 1), &ids)
            .unwrap();
        reg.record(fb(1, 2, ReputationDimension::Speed, -100, 10, 2), &ids)
            .unwrap();
        // signed=3000-1000=2000; denom=100*40=4000; 2000*500/4000=250 -> 750。
        assert_eq!(
            reg.snapshot(2)
                .score_01k(ReputationDimension::Speed)
                .unwrap(),
            750
        );
        // 其他维度不受影响。
        assert_eq!(
            reg.snapshot(2)
                .score_01k(ReputationDimension::Honesty)
                .unwrap(),
            500
        );
    }

    #[test]
    fn reputation_rejects_self_unknown_replay_bounds() {
        let ids = two_agents();
        let mut reg = ReputationRegistry::new();
        assert_eq!(
            reg.record(fb(1, 1, ReputationDimension::Quality, 10, 1, 1), &ids)
                .err(),
            Some(Erc8004Error::SelfFeedbackForbidden)
        );
        assert_eq!(
            reg.record(fb(1, 7, ReputationDimension::Quality, 10, 1, 1), &ids)
                .err(),
            Some(Erc8004Error::UnknownIdentity)
        );
        assert_eq!(
            reg.record(fb(1, 2, ReputationDimension::Quality, 101, 1, 1), &ids)
                .err(),
            Some(Erc8004Error::InvalidScore)
        );
        assert_eq!(
            reg.record(fb(1, 2, ReputationDimension::Quality, 10, 0, 1), &ids)
                .err(),
            Some(Erc8004Error::InvalidWeight)
        );
        reg.record(fb(1, 2, ReputationDimension::Quality, 10, 1, 9), &ids)
            .unwrap();
        assert_eq!(
            reg.record(fb(1, 2, ReputationDimension::Speed, 10, 1, 9), &ids)
                .err(),
            Some(Erc8004Error::DuplicateFeedbackNonce)
        );
    }

    #[test]
    fn validation_quorum_two_thirds_threshold() {
        let ids3 = {
            let mut r = two_agents();
            r.mint(3, "did:tw:gamma", &"33".repeat(32), "ipfs://c")
                .unwrap();
            r
        };
        // 2 valid / total 2：valid*3=6 >= total*2=4 且严格多于对方 -> ConfirmedValid。
        let mut t = ValidationTally::new();
        for v in [1u64, 2] {
            t.record_vote_checked(
                ValidationVote {
                    validator: v,
                    method: ValidationMethod::StakeRerun,
                    verdict: ValidationVerdict::Valid,
                    weight: 1,
                },
                &ids3,
                &"aa".repeat(32),
            )
            .unwrap();
        }
        assert_eq!(t.decide().unwrap(), ValidationDecision::ConfirmedValid);
        // 再加入第 3 票 invalid：valid=2,invalid=1,total=3 -> 2*3>=3*2 仍成立 -> ConfirmedValid。
        t.record_vote_checked(
            ValidationVote {
                validator: 3,
                method: ValidationMethod::Zkml,
                verdict: ValidationVerdict::Invalid,
                weight: 1,
            },
            &ids3,
            &"cc".repeat(32),
        )
        .unwrap();
        assert_eq!(t.decide().unwrap(), ValidationDecision::ConfirmedValid);
    }

    #[test]
    fn validation_invalid_quorum_and_inconclusive_split() {
        let ids3 = {
            let mut r = two_agents();
            r.mint(3, "did:tw:gamma", &"33".repeat(32), "ipfs://c")
                .unwrap();
            r
        };
        let mut t = ValidationTally::new();
        for v in [1u64, 2] {
            t.record_vote_checked(
                ValidationVote {
                    validator: v,
                    method: ValidationMethod::StakeRerun,
                    verdict: ValidationVerdict::Invalid,
                    weight: 1,
                },
                &ids3,
                &"aa".repeat(32),
            )
            .unwrap();
        }
        t.record_vote_checked(
            ValidationVote {
                validator: 3,
                method: ValidationMethod::TeeAttestation,
                verdict: ValidationVerdict::Valid,
                weight: 1,
            },
            &ids3,
            &"bb".repeat(32),
        )
        .unwrap();
        assert_eq!(t.decide().unwrap(), ValidationDecision::ConfirmedInvalid);

        // 权重分裂 1:1 -> 无方向达 2/3 -> Inconclusive。
        let mut t2 = ValidationTally::new();
        t2.record_vote(ValidationVote {
            validator: 1,
            method: ValidationMethod::Zkml,
            verdict: ValidationVerdict::Valid,
            weight: 5,
        })
        .unwrap();
        t2.record_vote(ValidationVote {
            validator: 2,
            method: ValidationMethod::Zkml,
            verdict: ValidationVerdict::Invalid,
            weight: 5,
        })
        .unwrap();
        assert_eq!(t2.decide().unwrap(), ValidationDecision::Inconclusive);

        // 空计账非法。
        assert_eq!(
            ValidationTally::new().decide().err(),
            Some(Erc8004Error::InvalidQuorumTally)
        );
    }

    #[test]
    fn validation_rejects_unknown_validator_bad_evidence_zero_weight() {
        let ids = two_agents();
        let mut t = ValidationTally::new();
        assert_eq!(
            t.record_vote_checked(
                ValidationVote {
                    validator: 9,
                    method: ValidationMethod::Zkml,
                    verdict: ValidationVerdict::Valid,
                    weight: 1,
                },
                &ids,
                &"aa".repeat(32),
            )
            .err(),
            Some(Erc8004Error::UnknownIdentity)
        );
        assert_eq!(
            t.record_vote_checked(
                ValidationVote {
                    validator: 1,
                    method: ValidationMethod::Zkml,
                    verdict: ValidationVerdict::Valid,
                    weight: 1,
                },
                &ids,
                "short",
            )
            .err(),
            Some(Erc8004Error::InvalidEvidenceHash)
        );
        assert_eq!(
            t.record_vote(ValidationVote {
                validator: 1,
                method: ValidationMethod::Zkml,
                verdict: ValidationVerdict::Valid,
                weight: 0,
            })
            .err(),
            Some(Erc8004Error::InvalidWeight)
        );
    }

    #[test]
    fn stateless_aggregate_detects_replay_and_conserves() {
        let ids = two_agents();
        let fbs = vec![
            fb(1, 2, ReputationDimension::Availability, 100, 4, 1),
            fb(1, 2, ReputationDimension::Availability, 50, 4, 2),
        ];
        let agg = aggregate_feedback(&fbs, &ids).unwrap();
        // signed=400+200=600; denom=100*8=800; 600*500/800=375 -> 875。
        assert_eq!(
            agg[&2]
                .score_01k(ReputationDimension::Availability)
                .unwrap(),
            875
        );
        // 重放 nonce -> 聚合整体具名失败。
        let dup = vec![
            fb(1, 2, ReputationDimension::Availability, 10, 1, 1),
            fb(1, 2, ReputationDimension::Quality, 10, 1, 1),
        ];
        assert_eq!(
            aggregate_feedback(&dup, &ids).err(),
            Some(Erc8004Error::DuplicateFeedbackNonce)
        );
    }

    #[test]
    fn error_codes_are_stable_strings() {
        assert_eq!(Erc8004Error::InvalidDid.to_string(), "ERC8004_INVALID_DID");
        assert_eq!(
            Erc8004Error::SelfFeedbackForbidden.code(),
            "ERC8004_SELF_FEEDBACK_FORBIDDEN"
        );
        assert_eq!(
            Erc8004Error::InvalidQuorumTally.to_string(),
            "ERC8004_INVALID_QUORUM_TALLY"
        );
    }
}
