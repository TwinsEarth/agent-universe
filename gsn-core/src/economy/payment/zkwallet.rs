//! v3.9.8（9/9）——ZK 私密支付意图 + OWS 统一钱包轨道路由 + 合规筛查的
//! **纯链下确定性决策内核**。
//!
//! ## 本版本只做什么
//! 在不连任何链/RPC/名单 API、不持私钥、不签名、不广播、不真转资产的前提下，提供三个
//! 互相独立的 fail-closed 决策面：
//! 1. [`ZkPolicy`] + [`ZkIntent`] → [`verify_zk_payment_intent`]：对“隐私支付意图”做
//!    **结构与额度**校验（32 字节承诺/空值符形状、整数信用额度 checked、选择性披露标签
//!    白名单、宿主取证的双花空值符标志、是否随附证明）。**不做任何 SNARK 验证**——
//!    [`verify_zk_proof`] 永远 [`ZkError::ZkProofVerificationNotImplemented`]。
//! 2. [`OwsPolicy`] + [`WalletCapability`] + [`OwsIntent`] → [`route_ows_wallet`]：把一个
//!    归一化支付意图，在“宿主声明已配置的钱包轨道能力”上做确定性选轨（OWS 统一钱包层的
//!    链下决策），缺轨/未配置/隐私或跨链要求得不到满足时 fail-closed。
//!    [`execute_ows_payment`] 无钱包后端，永远 [`OwsError::OwsWalletNotConfigured`]。
//! 3. [`CompliancePolicy`] + [`ComplianceSubject`] → [`screen_compliance`]：仅依据
//!    **调用方显式传入的**地址/司法辖区名单与整数阈值，做确定性筛查（放行 / 需上报 /
//!    阻断）。内核绝不联网拉取制裁或合规名单；[`enforce_compliance_decision`] 不冻结、
//!    不上报、不执行任何外部动作，永远 [`ComplianceError::ComplianceEnforcementNotConfigured`]。
//!
//! ## 诚实边界（fail-closed）
//! - 不做递归 SNARK / zkML 证明验证：`proof_verified` 恒为 false，
//!   `zk_proof_verification_implemented` 恒为 false；`privacy_ready` 只表示结构/额度/空值符/
//!   披露标签自洽且（按需）随附了证明字节，**绝不表示零知识证明在密码学上成立**。
//! - 空值符是否双花、轨道是否真的已配置/具备隐私与跨链能力，均由受信任宿主取证后以布尔/
//!   能力快照传入；内核不持久化、不联网、不读时钟。
//! - 合规名单完全由调用方提供；内核不内置任何国家/地址黑名单，不解释法律含义，阈值只是
//!   宿主策略整数常量。筛查通过不代表合法，阻断/上报也不构成法律意见。
//! - 零新依赖（仅 std + serde），零浮点/零 syscall/零 unsafe/零 IO/无生产 panic；
//!   金额/额度全部 u128 checked 整数运算。

use serde::{Deserialize, Serialize};

/// 剥除可选 `0x`/`0X` 前缀后，校验为恰好 32 字节（64 hex 字符）的小写十六进制串。
fn norm_hex32(label: &str, raw: &str) -> Result<String, ZkError> {
    let h = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);
    if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ZkError::ZkBadHex(label.to_string()));
    }
    Ok(h.to_ascii_lowercase())
}

/// ZK 意图允许使用的选择性披露标签（确定性白名单；标签语义内核不解释）。
pub const ALLOWED_DISCLOSURE_TAGS: &[&str] = &["none", "payee", "amount_range", "jurisdiction"];

/// ZK 隐私支付意图策略（宿主策略常量；内核不内置任何隐私方案参数）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZkPolicy {
    /// 是否要求意图必须随附一份证明（只检查“是否随附”，不验证证明密码学有效性）。
    pub require_proof_present: bool,
    /// 整数信用额度（含）；本笔使累计用量超过即拒。
    pub credit_line_micro: u128,
    /// 是否对宿主报告的“空值符已出现”进行双花拦截。
    pub block_seen_nullifier: bool,
}

/// 一笔待校验的 ZK 隐私支付意图（取证数据由调用方传入，内核不联网、不持久化）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZkIntent {
    /// 目标链标识（非空，如 "base"；内核不解释其含义）。
    pub chain: String,
    /// 资产标识（非空）。
    pub asset: String,
    /// 本笔支出（内部整数 micro，必须 > 0）。
    pub spend_micro: u128,
    /// 此前已用信用额度（宿主取证传入）。
    pub credit_used_prior_micro: u128,
    /// 32 字节承诺（可选 0x，64 hex）。
    pub commitment_hex: String,
    /// 32 字节空值符（可选 0x，64 hex）。
    pub nullifier_hex: String,
    /// 宿主是否已观测到同一空值符（双花取证；内核不自行查重）。
    pub nullifier_seen: bool,
    /// 选择性披露标签；每项必须在 [`ALLOWED_DISCLOSURE_TAGS`] 内。
    pub selective_disclosure: Vec<String>,
    /// 是否随附了证明字节（内核不验证，只据策略要求其存在）。
    pub proof_present: bool,
}

/// ZK 意图校验回执（只读、可复算、可审计）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZkReceipt {
    pub chain: String,
    pub asset: String,
    pub spend_micro: String,
    pub projected_credit_micro: String,
    pub credit_line_micro: String,
    pub within_credit_line: bool,
    pub nullifier_double_spend: bool,
    pub commitment_hex: String,
    pub nullifier_hex: String,
    pub selective_disclosure: Vec<String>,
    pub proof_present: bool,
    /// 内核不做 SNARK 验证：始终 false。
    pub proof_verified: bool,
    /// 本版本不实现递归 SNARK 验证：始终 false。
    pub zk_proof_verification_implemented: bool,
    /// 仅表结构/额度/空值符/披露标签自洽且按需随附证明；不代表证明密码学成立。
    pub privacy_ready: bool,
    pub key_or_proof_materialized_in_kernel: bool,
    pub external_verifier_connection: bool,
}

/// 校验 ZK 隐私支付意图。任何门槛不满足都具名拒绝（fail-closed）。
pub fn verify_zk_payment_intent(
    policy: &ZkPolicy,
    intent: &ZkIntent,
) -> Result<ZkReceipt, ZkError> {
    if intent.chain.trim().is_empty() {
        return Err(ZkError::ZkBadChain);
    }
    if intent.asset.trim().is_empty() {
        return Err(ZkError::ZkBadAsset);
    }
    if intent.spend_micro == 0 {
        return Err(ZkError::ZkNonPositiveSpend);
    }

    // 32B 形状（承诺/空值符）。
    let commitment = norm_hex32("commitment_hex", &intent.commitment_hex)?;
    let nullifier = norm_hex32("nullifier_hex", &intent.nullifier_hex)?;

    // 选择性披露标签必须全部在确定性白名单内。
    for tag in &intent.selective_disclosure {
        if !ALLOWED_DISCLOSURE_TAGS.iter().any(|allowed| allowed == tag) {
            return Err(ZkError::ZkDisclosureTagNotAllowed(tag.clone()));
        }
    }

    // 双花空值符（宿主取证）拦截优先于额度判定。
    if policy.block_seen_nullifier && intent.nullifier_seen {
        return Err(ZkError::ZkNullifierDoubleSpend);
    }

    // 整数信用额度 checked 累加；恰好花完允许，超过即拒。
    let projected = intent
        .credit_used_prior_micro
        .checked_add(intent.spend_micro)
        .ok_or(ZkError::ZkArithmeticOverflow)?;
    if projected > policy.credit_line_micro {
        return Err(ZkError::ZkExceedsCreditLine {
            projected,
            line: policy.credit_line_micro,
        });
    }

    // 只检查“是否随附证明”，绝不声称已验证；要求随附但缺失即具名拒绝。
    if policy.require_proof_present && !intent.proof_present {
        return Err(ZkError::ZkProofRequiredButAbsent);
    }

    Ok(ZkReceipt {
        chain: intent.chain.trim().to_string(),
        asset: intent.asset.trim().to_string(),
        spend_micro: intent.spend_micro.to_string(),
        projected_credit_micro: projected.to_string(),
        credit_line_micro: policy.credit_line_micro.to_string(),
        within_credit_line: true,
        nullifier_double_spend: false,
        commitment_hex: commitment,
        nullifier_hex: nullifier,
        selective_disclosure: intent.selective_disclosure.clone(),
        proof_present: intent.proof_present,
        proof_verified: false,
        zk_proof_verification_implemented: false,
        privacy_ready: true,
        key_or_proof_materialized_in_kernel: false,
        external_verifier_connection: false,
    })
}

/// 真实递归 SNARK / zkML 证明验证。
///
/// 本版本不实现任何证明系统：对任何调用一律具名
/// [`ZkError::ZkProofVerificationNotImplemented`] fail-closed，绝不返回“证明成立”。
pub fn verify_zk_proof() -> Result<(), ZkError> {
    Err(ZkError::ZkProofVerificationNotImplemented)
}

/// ZK 意图相关的类型化错误（全 `ZK_*` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZkError {
    ZkBadChain,
    ZkBadAsset,
    ZkNonPositiveSpend,
    ZkBadHex(String),
    ZkDisclosureTagNotAllowed(String),
    ZkNullifierDoubleSpend,
    ZkExceedsCreditLine { projected: u128, line: u128 },
    ZkArithmeticOverflow,
    ZkProofRequiredButAbsent,
    ZkProofVerificationNotImplemented,
}

impl std::fmt::Display for ZkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZkError::ZkBadChain => write!(f, "ZK_BAD_CHAIN: chain 为空"),
            ZkError::ZkBadAsset => write!(f, "ZK_BAD_ASSET: asset 为空"),
            ZkError::ZkNonPositiveSpend => {
                write!(f, "ZK_NON_POSITIVE_SPEND: spend_micro 必须 > 0")
            }
            ZkError::ZkBadHex(label) => {
                write!(f, "ZK_BAD_HEX: {label} 必须为 0x 可选的 32 字节(64 hex)")
            }
            ZkError::ZkDisclosureTagNotAllowed(tag) => write!(
                f,
                "ZK_DISCLOSURE_TAG_NOT_ALLOWED: 选择性披露标签 {tag} 不在白名单"
            ),
            ZkError::ZkNullifierDoubleSpend => {
                write!(
                    f,
                    "ZK_NULLIFIER_DOUBLE_SPEND: 宿主报告空值符已出现，拦截双花"
                )
            }
            ZkError::ZkExceedsCreditLine { projected, line } => write!(
                f,
                "ZK_EXCEEDS_CREDIT_LINE: projected_credit={projected} > credit_line={line}"
            ),
            ZkError::ZkArithmeticOverflow => {
                write!(f, "ZK_ARITHMETIC_OVERFLOW: 整数 checked 运算溢出")
            }
            ZkError::ZkProofRequiredButAbsent => write!(
                f,
                "ZK_PROOF_REQUIRED_BUT_ABSENT: 策略要求随附证明，但 proof_present=false"
            ),
            ZkError::ZkProofVerificationNotImplemented => write!(
                f,
                "ZK_PROOF_VERIFICATION_NOT_IMPLEMENTED: 本版本不验证递归 SNARK/zkML，fail-closed"
            ),
        }
    }
}

impl std::error::Error for ZkError {}

/// OWS 统一钱包支持的支付轨道（归一化标识）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwsTrack {
    /// 比特币闪电网络（即时微支付，L402/x402-LN）。
    BtcLightning,
    /// 以太坊 L2 稳定币 x402 轨道。
    EvmX402Stable,
    /// 比特币 RGB/HTLC 大额/跨周期轨道。
    BtcRgbHtlc,
}

impl OwsTrack {
    pub fn as_str(&self) -> &'static str {
        match self {
            OwsTrack::BtcLightning => "btc_lightning",
            OwsTrack::EvmX402Stable => "evm_x402_stable",
            OwsTrack::BtcRgbHtlc => "btc_rgb_htlc",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "btc_lightning" => Some(OwsTrack::BtcLightning),
            "evm_x402_stable" => Some(OwsTrack::EvmX402Stable),
            "btc_rgb_htlc" => Some(OwsTrack::BtcRgbHtlc),
            _ => None,
        }
    }
}

/// 宿主声明的某个钱包轨道能力快照（内核不探测真实钱包，只信这份取证）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletCapability {
    /// [`OwsTrack::as_str`] 之一。
    pub track: String,
    /// 宿主是否已配置该轨道（有可用钱包/节点后端）。
    pub configured: bool,
    /// 该轨道是否支持隐私支付（ZK）。
    pub privacy: bool,
    /// 该轨道是否支持跨链。
    pub cross_chain: bool,
}

/// OWS 选轨策略。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwsPolicy {
    pub enabled: bool,
    /// 允许选用的轨道；**为空默认无轨道可用**。
    pub allowed_tracks: Vec<String>,
    /// 是否强制要求选中轨道已被宿主配置。
    pub require_configured: bool,
}

/// 归一化的 OWS 支付意图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwsIntent {
    /// 资产族："btc" 或 "stable"。
    pub asset_family: String,
    /// 金额（内部整数 micro，必须 > 0）。
    pub amount_micro: u128,
    /// 是否要求即时（影响 btc 族在闪电/RGB 间的优先序）。
    pub instant: bool,
    pub privacy_required: bool,
    pub cross_chain_required: bool,
}

/// OWS 选轨回执（只决策；不代表钱包已执行）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwsRoute {
    pub selected_track: String,
    pub track_configured: bool,
    pub track_privacy: bool,
    pub track_cross_chain: bool,
    pub asset_family: String,
    pub amount_micro: String,
    /// 私钥不进沙盒/内核：始终 false。
    pub key_held_in_sandbox: bool,
    /// 内核从不执行钱包支付：始终 false。
    pub wallet_execute_executed: bool,
}

/// 按确定性优先序为意图选择钱包轨道；无满足条件的轨道时 fail-closed。
pub fn route_ows_wallet(
    policy: &OwsPolicy,
    caps: &[WalletCapability],
    intent: &OwsIntent,
) -> Result<OwsRoute, OwsError> {
    if !policy.enabled {
        return Err(OwsError::OwsDisabled);
    }
    if intent.amount_micro == 0 {
        return Err(OwsError::OwsNonPositiveAmount);
    }

    // 允许轨道白名单为空默认全拒；逐项必须是已知轨道。
    if policy.allowed_tracks.is_empty() {
        return Err(OwsError::OwsNoTrackAllowed);
    }
    for t in &policy.allowed_tracks {
        if OwsTrack::parse(t).is_none() {
            return Err(OwsError::OwsBadPolicy(format!("未知轨道 {t}")));
        }
    }

    // 资产族 → 确定性优先序。
    let preference: Vec<OwsTrack> = match intent.asset_family.as_str() {
        "stable" => vec![OwsTrack::EvmX402Stable],
        "btc" => {
            if intent.instant {
                vec![OwsTrack::BtcLightning, OwsTrack::BtcRgbHtlc]
            } else {
                vec![OwsTrack::BtcRgbHtlc, OwsTrack::BtcLightning]
            }
        }
        other => return Err(OwsError::OwsUnknownAssetFamily(other.to_string())),
    };

    // 能力快照中的轨道名也必须合法。
    for c in caps {
        if OwsTrack::parse(&c.track).is_none() {
            return Err(OwsError::OwsBadPolicy(format!(
                "能力快照含未知轨道 {}",
                c.track
            )));
        }
    }

    for cand in preference {
        let name = cand.as_str();
        // 不在宿主策略白名单内，跳过。
        if !policy.allowed_tracks.iter().any(|t| t == name) {
            continue;
        }
        // 必须存在该轨道的能力快照（缺失视为不满足，fail-closed）。
        let Some(cap) = caps.iter().find(|c| c.track == name) else {
            continue;
        };
        if policy.require_configured && !cap.configured {
            continue;
        }
        if intent.privacy_required && !cap.privacy {
            continue;
        }
        if intent.cross_chain_required && !cap.cross_chain {
            continue;
        }
        return Ok(OwsRoute {
            selected_track: name.to_string(),
            track_configured: cap.configured,
            track_privacy: cap.privacy,
            track_cross_chain: cap.cross_chain,
            asset_family: intent.asset_family.clone(),
            amount_micro: intent.amount_micro.to_string(),
            key_held_in_sandbox: false,
            wallet_execute_executed: false,
        });
    }

    Err(OwsError::OwsNoAvailableTrack)
}

/// 真实钱包支付执行。
///
/// 本版本无统一钱包后端：对任何请求一律具名 [`OwsError::OwsWalletNotConfigured`]
/// fail-closed，绝不持私钥、签名或广播。
pub fn execute_ows_payment() -> Result<(), OwsError> {
    Err(OwsError::OwsWalletNotConfigured)
}

/// OWS 选轨相关的类型化错误（全 `OWS_*` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwsError {
    OwsDisabled,
    OwsNonPositiveAmount,
    OwsNoTrackAllowed,
    OwsBadPolicy(String),
    OwsUnknownAssetFamily(String),
    OwsNoAvailableTrack,
    OwsWalletNotConfigured,
}

impl std::fmt::Display for OwsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OwsError::OwsDisabled => write!(f, "OWS_DISABLED: 统一钱包选轨总开关关闭"),
            OwsError::OwsNonPositiveAmount => {
                write!(f, "OWS_NON_POSITIVE_AMOUNT: amount_micro 必须 > 0")
            }
            OwsError::OwsNoTrackAllowed => {
                write!(
                    f,
                    "OWS_NO_TRACK_ALLOWED: allowed_tracks 为空，默认无轨道可用"
                )
            }
            OwsError::OwsBadPolicy(msg) => write!(f, "OWS_BAD_POLICY: {msg}"),
            OwsError::OwsUnknownAssetFamily(fam) => {
                write!(f, "OWS_UNKNOWN_ASSET_FAMILY: 资产族 {fam} 不在 btc/stable")
            }
            OwsError::OwsNoAvailableTrack => write!(
                f,
                "OWS_NO_AVAILABLE_TRACK: 白名单内无同时满足已配置/隐私/跨链要求的轨道"
            ),
            OwsError::OwsWalletNotConfigured => write!(
                f,
                "OWS_WALLET_NOT_CONFIGURED: 本版本无统一钱包后端，不持钥/签名/广播，fail-closed"
            ),
        }
    }
}

impl std::error::Error for OwsError {}

/// 合规筛查策略（名单与阈值全部由调用方提供；内核不内置任何司法辖区或地址）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompliancePolicy {
    pub enabled: bool,
    /// 已阻断地址（按 subject 的 address_kind 同规则归一化后比对）。
    pub blocked_addresses: Vec<String>,
    /// 已阻断司法辖区（精确匹配）。
    pub blocked_jurisdictions: Vec<String>,
    /// 是否要求 subject 的司法辖区在允许名单内。
    pub require_allowed_jurisdiction: bool,
    /// 允许的司法辖区；当 require=true 时为空意味着全拒。
    pub allowed_jurisdictions: Vec<String>,
    /// 需上报阈值（含）；0 表示关闭该档检查。
    pub report_threshold_micro: u128,
    /// 硬阻断阈值（含）；0 表示关闭该档检查。
    pub hard_block_threshold_micro: u128,
}

/// 待筛查主体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceSubject {
    /// 地址族："evm"（0x + 20 字节，按小写 hex 归一化）或 "other"（去空白精确串）。
    pub address_kind: String,
    pub address: String,
    /// 司法辖区码（非空，内核不解释其法律含义）。
    pub jurisdiction: String,
    /// 金额（内部整数 micro，必须 > 0）。
    pub amount_micro: u128,
}

/// 筛查裁决。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreeningVerdict {
    Allow,
    ReportRequired,
    Blocked,
}

impl ScreeningVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScreeningVerdict::Allow => "allow",
            ScreeningVerdict::ReportRequired => "report_required",
            ScreeningVerdict::Blocked => "blocked",
        }
    }
}

/// 合规筛查回执（只决策；不代表已执行任何外部合规动作）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScreeningDecision {
    pub verdict: String,
    pub normalized_address: String,
    pub jurisdiction: String,
    pub amount_micro: String,
    pub reasons: Vec<String>,
    /// 内核不执行冻结/上报：始终 false。
    pub enforcement_executed: bool,
    /// 内核不联网拉取名单：始终 false。
    pub external_list_connection: bool,
    /// 名单确实来自调用方而非内核内置：始终 true。
    pub list_provided_by_caller: bool,
}

/// 按地址族归一化地址（evm：去 0x、40 hex、小写；other：去首尾空白）。
fn normalize_address(kind: &str, raw: &str) -> Result<String, ComplianceError> {
    match kind {
        "evm" => {
            let h = raw
                .trim()
                .strip_prefix("0x")
                .or_else(|| raw.trim().strip_prefix("0X"))
                .unwrap_or(raw.trim());
            if h.len() != 40 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(ComplianceError::ComplianceBadEvmAddress);
            }
            Ok(h.to_ascii_lowercase())
        }
        "other" => {
            let t = raw.trim();
            if t.is_empty() {
                return Err(ComplianceError::ComplianceBadAddress);
            }
            Ok(t.to_string())
        }
        other => Err(ComplianceError::ComplianceBadAddressKind(other.to_string())),
    }
}

/// 依据调用方提供的名单与整数阈值做确定性合规筛查（fail-closed）。
pub fn screen_compliance(
    policy: &CompliancePolicy,
    subject: &ComplianceSubject,
) -> Result<ScreeningDecision, ComplianceError> {
    if !policy.enabled {
        return Err(ComplianceError::ComplianceScreeningDisabled);
    }
    if subject.jurisdiction.trim().is_empty() {
        return Err(ComplianceError::ComplianceBadJurisdiction);
    }
    if subject.amount_micro == 0 {
        return Err(ComplianceError::ComplianceNonPositiveAmount);
    }

    let normalized = normalize_address(&subject.address_kind, &subject.address)?;

    // 策略中的阻断地址按相同规则归一化；策略自身脏数据具名拒绝（不放行）。
    let mut blocked_norm = Vec::with_capacity(policy.blocked_addresses.len());
    for a in &policy.blocked_addresses {
        blocked_norm.push(normalize_address(&subject.address_kind, a)?);
    }

    let jurisdiction = subject.jurisdiction.trim();

    let blocked = if blocked_norm.iter().any(|a| a == &normalized) {
        Some("address_blocklisted")
    } else if policy
        .blocked_jurisdictions
        .iter()
        .any(|j| j == jurisdiction)
    {
        Some("jurisdiction_blocked")
    } else if policy.require_allowed_jurisdiction
        && !policy
            .allowed_jurisdictions
            .iter()
            .any(|j| j == jurisdiction)
    {
        // 要求允许名单且（名单为空或）不在其中：fail-closed 阻断。
        Some("jurisdiction_not_allowed")
    } else if policy.hard_block_threshold_micro > 0
        && subject.amount_micro >= policy.hard_block_threshold_micro
    {
        Some("amount_hard_block_threshold")
    } else {
        None
    };

    if let Some(reason) = blocked {
        return Ok(ScreeningDecision {
            verdict: ScreeningVerdict::Blocked.as_str().to_string(),
            normalized_address: normalized,
            jurisdiction: jurisdiction.to_string(),
            amount_micro: subject.amount_micro.to_string(),
            reasons: vec![reason.to_string()],
            enforcement_executed: false,
            external_list_connection: false,
            list_provided_by_caller: true,
        });
    }

    let mut reasons = Vec::new();
    if policy.report_threshold_micro > 0 && subject.amount_micro >= policy.report_threshold_micro {
        reasons.push("amount_report_threshold".to_string());
    }

    let verdict = if reasons.is_empty() {
        ScreeningVerdict::Allow
    } else {
        ScreeningVerdict::ReportRequired
    };

    Ok(ScreeningDecision {
        verdict: verdict.as_str().to_string(),
        normalized_address: normalized,
        jurisdiction: jurisdiction.to_string(),
        amount_micro: subject.amount_micro.to_string(),
        reasons,
        enforcement_executed: false,
        external_list_connection: false,
        list_provided_by_caller: true,
    })
}

/// 真实合规执行（冻结资产 / 对外上报）。
///
/// 本版本无外部合规执行后端：对任何调用一律具名
/// [`ComplianceError::ComplianceEnforcementNotConfigured`] fail-closed。
pub fn enforce_compliance_decision() -> Result<(), ComplianceError> {
    Err(ComplianceError::ComplianceEnforcementNotConfigured)
}

/// 合规筛查相关的类型化错误（全 `COMPLIANCE_*` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComplianceError {
    ComplianceScreeningDisabled,
    ComplianceBadJurisdiction,
    ComplianceBadAddressKind(String),
    ComplianceBadEvmAddress,
    ComplianceBadAddress,
    ComplianceNonPositiveAmount,
    ComplianceEnforcementNotConfigured,
}

impl std::fmt::Display for ComplianceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComplianceError::ComplianceScreeningDisabled => write!(
                f,
                "COMPLIANCE_SCREENING_DISABLED: 合规筛查关闭时不产出放行结论，fail-closed"
            ),
            ComplianceError::ComplianceBadJurisdiction => {
                write!(f, "COMPLIANCE_BAD_JURISDICTION: jurisdiction 为空")
            }
            ComplianceError::ComplianceBadAddressKind(k) => write!(
                f,
                "COMPLIANCE_BAD_ADDRESS_KIND: address_kind {k} 不在 evm/other"
            ),
            ComplianceError::ComplianceBadEvmAddress => write!(
                f,
                "COMPLIANCE_BAD_EVM_ADDRESS: EVM 地址必须为 0x 可选的 20 字节(40 hex)"
            ),
            ComplianceError::ComplianceBadAddress => {
                write!(f, "COMPLIANCE_BAD_ADDRESS: 地址为空")
            }
            ComplianceError::ComplianceNonPositiveAmount => {
                write!(f, "COMPLIANCE_NON_POSITIVE_AMOUNT: amount_micro 必须 > 0")
            }
            ComplianceError::ComplianceEnforcementNotConfigured => write!(
                f,
                "COMPLIANCE_ENFORCEMENT_NOT_CONFIGURED: 本版本不冻结/不上报，外部执行 fail-closed"
            ),
        }
    }
}

impl std::error::Error for ComplianceError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex32(byte: u8) -> String {
        format!("0x{}", vec![format!("{byte:02x}"); 32].concat())
    }

    fn zk_policy() -> ZkPolicy {
        ZkPolicy {
            require_proof_present: true,
            credit_line_micro: 1_000_000,
            block_seen_nullifier: true,
        }
    }

    fn zk_intent() -> ZkIntent {
        ZkIntent {
            chain: "base".to_string(),
            asset: "USDC".to_string(),
            spend_micro: 200_000,
            credit_used_prior_micro: 300_000,
            commitment_hex: hex32(0x11),
            nullifier_hex: hex32(0x22),
            nullifier_seen: false,
            selective_disclosure: vec!["amount_range".to_string()],
            proof_present: true,
        }
    }

    #[test]
    fn zk_happy_path_ready_but_never_verified() {
        let r = verify_zk_payment_intent(&zk_policy(), &zk_intent()).unwrap();
        assert!(r.privacy_ready);
        assert!(r.within_credit_line);
        assert_eq!(r.projected_credit_micro, "500000");
        assert!(!r.proof_verified);
        assert!(!r.zk_proof_verification_implemented);
        assert!(!r.key_or_proof_materialized_in_kernel);
        assert!(!r.external_verifier_connection);
    }

    #[test]
    fn zk_hex_prefix_and_case_normalized() {
        let mut i = zk_intent();
        i.commitment_hex = hex32(0x11).to_uppercase();
        i.nullifier_hex = hex32(0x22).to_uppercase();
        let r = verify_zk_payment_intent(&zk_policy(), &i).unwrap();
        assert!(r.commitment_hex.starts_with("11"));
        assert_eq!(r.commitment_hex.len(), 64);
    }

    #[test]
    fn zk_bad_chain_asset_amount_hex_named() {
        let mut i = zk_intent();
        i.chain = "  ".to_string();
        assert_eq!(
            verify_zk_payment_intent(&zk_policy(), &i).unwrap_err(),
            ZkError::ZkBadChain
        );
        let mut i = zk_intent();
        i.asset = String::new();
        assert_eq!(
            verify_zk_payment_intent(&zk_policy(), &i).unwrap_err(),
            ZkError::ZkBadAsset
        );
        let mut i = zk_intent();
        i.spend_micro = 0;
        assert_eq!(
            verify_zk_payment_intent(&zk_policy(), &i).unwrap_err(),
            ZkError::ZkNonPositiveSpend
        );
        let mut i = zk_intent();
        i.nullifier_hex = "0x1234".to_string();
        assert!(matches!(
            verify_zk_payment_intent(&zk_policy(), &i).unwrap_err(),
            ZkError::ZkBadHex(_)
        ));
    }

    #[test]
    fn zk_disclosure_tag_whitelist_enforced() {
        let mut i = zk_intent();
        i.selective_disclosure = vec!["full_identity".to_string()];
        assert_eq!(
            verify_zk_payment_intent(&zk_policy(), &i).unwrap_err(),
            ZkError::ZkDisclosureTagNotAllowed("full_identity".to_string())
        );
    }

    #[test]
    fn zk_double_spend_credit_line_and_proof_absence() {
        // 双花空值符（宿主取证）优先拦截。
        let mut i = zk_intent();
        i.nullifier_seen = true;
        assert_eq!(
            verify_zk_payment_intent(&zk_policy(), &i).unwrap_err(),
            ZkError::ZkNullifierDoubleSpend
        );
        // 超信用额度（checked）；恰好花完允许。
        let mut i = zk_intent();
        i.credit_used_prior_micro = 900_000;
        assert!(matches!(
            verify_zk_payment_intent(&zk_policy(), &i).unwrap_err(),
            ZkError::ZkExceedsCreditLine { .. }
        ));
        let mut i = zk_intent();
        i.credit_used_prior_micro = 800_000;
        let r = verify_zk_payment_intent(&zk_policy(), &i).unwrap();
        assert_eq!(r.projected_credit_micro, "1000000");
        // 策略要求随附证明但缺失。
        let mut i = zk_intent();
        i.proof_present = false;
        assert_eq!(
            verify_zk_payment_intent(&zk_policy(), &i).unwrap_err(),
            ZkError::ZkProofRequiredButAbsent
        );
    }

    #[test]
    fn zk_real_proof_verification_is_never_implemented() {
        assert_eq!(
            verify_zk_proof().unwrap_err(),
            ZkError::ZkProofVerificationNotImplemented
        );
    }

    fn cap(name: &str, configured: bool, privacy: bool, cross: bool) -> WalletCapability {
        WalletCapability {
            track: name.to_string(),
            configured,
            privacy,
            cross_chain: cross,
        }
    }

    fn ows_policy() -> OwsPolicy {
        OwsPolicy {
            enabled: true,
            allowed_tracks: vec![
                OwsTrack::BtcLightning.as_str().to_string(),
                OwsTrack::EvmX402Stable.as_str().to_string(),
                OwsTrack::BtcRgbHtlc.as_str().to_string(),
            ],
            require_configured: true,
        }
    }

    fn all_caps() -> Vec<WalletCapability> {
        vec![
            cap("btc_lightning", true, false, false),
            cap("evm_x402_stable", true, true, false),
            cap("btc_rgb_htlc", true, false, true),
        ]
    }

    #[test]
    fn ows_stable_prefers_evm_and_btc_instant_prefers_lightning() {
        let r = route_ows_wallet(
            &ows_policy(),
            &all_caps(),
            &OwsIntent {
                asset_family: "stable".to_string(),
                amount_micro: 100,
                instant: false,
                privacy_required: false,
                cross_chain_required: false,
            },
        )
        .unwrap();
        assert_eq!(r.selected_track, "evm_x402_stable");
        assert!(!r.wallet_execute_executed);
        assert!(!r.key_held_in_sandbox);

        let r = route_ows_wallet(
            &ows_policy(),
            &all_caps(),
            &OwsIntent {
                asset_family: "btc".to_string(),
                amount_micro: 100,
                instant: true,
                privacy_required: false,
                cross_chain_required: false,
            },
        )
        .unwrap();
        assert_eq!(r.selected_track, "btc_lightning");

        // 非即时 btc 优先 RGB/HTLC。
        let r = route_ows_wallet(
            &ows_policy(),
            &all_caps(),
            &OwsIntent {
                asset_family: "btc".to_string(),
                amount_micro: 100,
                instant: false,
                privacy_required: false,
                cross_chain_required: false,
            },
        )
        .unwrap();
        assert_eq!(r.selected_track, "btc_rgb_htlc");
    }

    #[test]
    fn ows_privacy_and_crosschain_constraints_select_or_fail() {
        // 需要隐私：只有 evm_x402_stable 标记 privacy，即使 btc 也应落到它？
        // 资产族固定优先序内若无隐私轨道则 fail-closed；stable + 隐私命中 evm。
        let r = route_ows_wallet(
            &ows_policy(),
            &all_caps(),
            &OwsIntent {
                asset_family: "stable".to_string(),
                amount_micro: 100,
                instant: false,
                privacy_required: true,
                cross_chain_required: false,
            },
        )
        .unwrap();
        assert_eq!(r.selected_track, "evm_x402_stable");
        // btc 族两条轨道均无 privacy 能力 → 无可用轨道 fail-closed。
        assert_eq!(
            route_ows_wallet(
                &ows_policy(),
                &all_caps(),
                &OwsIntent {
                    asset_family: "btc".to_string(),
                    amount_micro: 100,
                    instant: true,
                    privacy_required: true,
                    cross_chain_required: false,
                }
            )
            .unwrap_err(),
            OwsError::OwsNoAvailableTrack
        );
        // 跨链：btc 非即时 → rgb_htlc 具备 cross_chain。
        let r = route_ows_wallet(
            &ows_policy(),
            &all_caps(),
            &OwsIntent {
                asset_family: "btc".to_string(),
                amount_micro: 100,
                instant: false,
                privacy_required: false,
                cross_chain_required: true,
            },
        )
        .unwrap();
        assert_eq!(r.selected_track, "btc_rgb_htlc");
    }

    #[test]
    fn ows_disabled_unconfigured_empty_unknown_and_execute_fail_closed() {
        let mut p = ows_policy();
        p.enabled = false;
        assert_eq!(
            route_ows_wallet(&p, &all_caps(), &zk_like_ows_intent()).unwrap_err(),
            OwsError::OwsDisabled
        );
        // 零金额。
        let mut i = zk_like_ows_intent();
        i.amount_micro = 0;
        assert_eq!(
            route_ows_wallet(&ows_policy(), &all_caps(), &i).unwrap_err(),
            OwsError::OwsNonPositiveAmount
        );
        // 白名单为空默认无轨道。
        let p = OwsPolicy {
            allowed_tracks: vec![],
            ..ows_policy()
        };
        assert_eq!(
            route_ows_wallet(&p, &all_caps(), &zk_like_ows_intent()).unwrap_err(),
            OwsError::OwsNoTrackAllowed
        );
        // 未配置（require_configured）导致无可用轨道。
        let unconfigured = vec![cap("btc_lightning", false, false, false)];
        assert_eq!(
            route_ows_wallet(
                &ows_policy(),
                &unconfigured,
                &OwsIntent {
                    asset_family: "btc".to_string(),
                    amount_micro: 100,
                    instant: true,
                    privacy_required: false,
                    cross_chain_required: false,
                }
            )
            .unwrap_err(),
            OwsError::OwsNoAvailableTrack
        );
        // 未知资产族 / 坏策略轨道名 / 坏能力轨道名。
        let mut i = zk_like_ows_intent();
        i.asset_family = "sol".to_string();
        assert!(matches!(
            route_ows_wallet(&ows_policy(), &all_caps(), &i).unwrap_err(),
            OwsError::OwsUnknownAssetFamily(_)
        ));
        let p = OwsPolicy {
            allowed_tracks: vec!["solana_wallet".to_string()],
            ..ows_policy()
        };
        assert!(matches!(
            route_ows_wallet(&p, &all_caps(), &zk_like_ows_intent()).unwrap_err(),
            OwsError::OwsBadPolicy(_)
        ));
        let bad_caps = vec![cap("solana_wallet", true, true, true)];
        assert!(matches!(
            route_ows_wallet(&ows_policy(), &bad_caps, &zk_like_ows_intent()).unwrap_err(),
            OwsError::OwsBadPolicy(_)
        ));
        // 真实支付执行无后端 fail-closed。
        assert_eq!(
            execute_ows_payment().unwrap_err(),
            OwsError::OwsWalletNotConfigured
        );
    }

    fn zk_like_ows_intent() -> OwsIntent {
        OwsIntent {
            asset_family: "stable".to_string(),
            amount_micro: 100,
            instant: false,
            privacy_required: false,
            cross_chain_required: false,
        }
    }

    fn compliance_policy() -> CompliancePolicy {
        CompliancePolicy {
            enabled: true,
            blocked_addresses: vec!["0xDEAD000000000000000000000000000000000000".to_string()],
            blocked_jurisdictions: vec!["KP".to_string()],
            require_allowed_jurisdiction: true,
            allowed_jurisdictions: vec!["SG".to_string(), "US".to_string()],
            report_threshold_micro: 10_000,
            hard_block_threshold_micro: 1_000_000,
        }
    }

    fn evm_subject(addr: &str, jurisdiction: &str, amount: u128) -> ComplianceSubject {
        ComplianceSubject {
            address_kind: "evm".to_string(),
            address: addr.to_string(),
            jurisdiction: jurisdiction.to_string(),
            amount_micro: amount,
        }
    }

    #[test]
    fn compliance_allow_report_block_ladder() {
        // 小额、干净地址、允许辖区 → allow。
        let d = screen_compliance(
            &compliance_policy(),
            &evm_subject("0x00000000000000000000000000000000000000ab", "SG", 100),
        )
        .unwrap();
        assert_eq!(d.verdict, "allow");
        assert!(d.reasons.is_empty());
        assert!(!d.enforcement_executed);
        assert!(!d.external_list_connection);
        assert!(d.list_provided_by_caller);
        assert_eq!(
            d.normalized_address,
            "00000000000000000000000000000000000000ab"
        );
        // 达上报阈值（含）但未到硬阻断 → report_required。
        let d = screen_compliance(
            &compliance_policy(),
            &evm_subject("0x00000000000000000000000000000000000000ab", "SG", 10_000),
        )
        .unwrap();
        assert_eq!(d.verdict, "report_required");
        assert_eq!(d.reasons, vec!["amount_report_threshold"]);
        // 达硬阻断阈值（含）→ blocked。
        let d = screen_compliance(
            &compliance_policy(),
            &evm_subject(
                "0x00000000000000000000000000000000000000ab",
                "SG",
                1_000_000,
            ),
        )
        .unwrap();
        assert_eq!(d.verdict, "blocked");
        assert_eq!(d.reasons, vec!["amount_hard_block_threshold"]);
    }

    #[test]
    fn compliance_blocklist_address_and_jurisdiction() {
        // 地址大小写/前缀不敏感，命中阻断名单。
        let d = screen_compliance(
            &compliance_policy(),
            &evm_subject("0xdead000000000000000000000000000000000000", "SG", 100),
        )
        .unwrap();
        assert_eq!(d.verdict, "blocked");
        assert_eq!(d.reasons, vec!["address_blocklisted"]);
        // 阻断辖区优先于金额。
        let d = screen_compliance(
            &compliance_policy(),
            &evm_subject("0x00000000000000000000000000000000000000ab", "KP", 100),
        )
        .unwrap();
        assert_eq!(d.reasons, vec!["jurisdiction_blocked"]);
        // 要求允许名单但辖区不在其中（空名单同样全拒）。
        let d = screen_compliance(
            &compliance_policy(),
            &evm_subject("0x00000000000000000000000000000000000000ab", "RU", 100),
        )
        .unwrap();
        assert_eq!(d.reasons, vec!["jurisdiction_not_allowed"]);
        let p = CompliancePolicy {
            allowed_jurisdictions: vec![],
            ..compliance_policy()
        };
        let d = screen_compliance(
            &p,
            &evm_subject("0x00000000000000000000000000000000000000ab", "SG", 100),
        )
        .unwrap();
        assert_eq!(d.verdict, "blocked");
    }

    #[test]
    fn compliance_other_kind_exact_match_and_fail_closed_inputs() {
        let p = CompliancePolicy {
            blocked_addresses: vec!["bc1qexample".to_string()],
            ..compliance_policy()
        };
        let s = ComplianceSubject {
            address_kind: "other".to_string(),
            address: "  bc1qexample  ".to_string(),
            jurisdiction: "SG".to_string(),
            amount_micro: 100,
        };
        let d = screen_compliance(&p, &s).unwrap();
        assert_eq!(d.verdict, "blocked");
        assert_eq!(d.normalized_address, "bc1qexample");

        // 关闭筛查不出具放行结论。
        let mut off = compliance_policy();
        off.enabled = false;
        assert_eq!(
            screen_compliance(&off, &evm_subject("0x1", "SG", 100)).unwrap_err(),
            ComplianceError::ComplianceScreeningDisabled
        );
        // 坏 EVM 地址 / 坏 kind / 空辖区 / 零金额。
        assert_eq!(
            screen_compliance(&compliance_policy(), &evm_subject("0x12", "SG", 100)).unwrap_err(),
            ComplianceError::ComplianceBadEvmAddress
        );
        let mut s = evm_subject("0x00000000000000000000000000000000000000ab", "SG", 100);
        s.address_kind = "solana".to_string();
        assert!(matches!(
            screen_compliance(&compliance_policy(), &s).unwrap_err(),
            ComplianceError::ComplianceBadAddressKind(_)
        ));
        let s = evm_subject("0x00000000000000000000000000000000000000ab", " ", 100);
        assert_eq!(
            screen_compliance(&compliance_policy(), &s).unwrap_err(),
            ComplianceError::ComplianceBadJurisdiction
        );
        let s = evm_subject("0x00000000000000000000000000000000000000ab", "SG", 0);
        assert_eq!(
            screen_compliance(&compliance_policy(), &s).unwrap_err(),
            ComplianceError::ComplianceNonPositiveAmount
        );
        // 真实外部执行无后端 fail-closed。
        assert_eq!(
            enforce_compliance_decision().unwrap_err(),
            ComplianceError::ComplianceEnforcementNotConfigured
        );
    }
}
