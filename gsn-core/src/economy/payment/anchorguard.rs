//! v3.9.7（8/9）——链上锚定最终性 + 跨链桥风控的**纯链下确定性决策内核**。
//!
//! ## 本版本只做什么
//! 在不连任何链/RPC/桥、不持私钥、不广播、不真转资产的前提下，对调用方取证后传入的
//! 快照做两类 fail-closed 决策：
//! 1. [`AnchorPolicy`] + [`AnchorEvidence`] → [`verify_anchor_finality`]：按确认数、
//!    显式最终性标志、可容忍重组深度与 32 字节承诺一致性，判定锚定是否可安全据以行动。
//! 2. [`BridgePolicy`] + [`BridgeTransfer`] → [`decide_bridge_transfer`]：按开关/暂停/
//!    方向与资产白名单/目的白名单/最小额/单笔上限/速率窗口/累计额度做确定性放行。
//!
//! ## 诚实边界（fail-closed）
//! - 不连 RPC/节点、不读区块/UTXO/事件日志、不校验真实链上确认数或最终性；确认数与
//!   observed_reorg_depth 均由调用方取证后显式传入，内核不读时钟、不联网。
//! - 不连接任何跨链桥/锁仓合约/ mint 合约，不构造、签名、广播跨链消息，不锁定/铸造/
//!   释放任何资产；[`relay_bridge_transfer`] 无桥后端时一律
//!   [`BridgeError::BridgeNotConfigured`]。
//! - 承诺只做链下 32 字节 hex 形状与「锚点承诺==声明承诺」的字节一致性；不做默克尔证明、
//!   不验签名/聚合签名、不做轻客户端证明验证。`anchor_safe=true` 仅表取证快照按策略自洽，
//!   不代表链上真实不可回滚。
//! - 零新依赖（仅 std + serde），零浮点/零 syscall/零 unsafe/零 IO/无生产 panic；
//!   金额/计数全部 u128/u64 checked 整数运算。

use serde::{Deserialize, Serialize};

/// 剥除可选 `0x`/`0X` 前缀后，校验为恰好 32 字节（64 hex 字符）的小写承诺。
///
/// 纯 std 实现，不引入新依赖；返回规范化的小写 64 字符 hex 字符串。
fn norm_commitment_hex(label: &str, raw: &str) -> Result<String, AnchorError> {
    let h = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);
    if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AnchorError::AnchorBadCommitmentHex(label.to_string()));
    }
    Ok(h.to_ascii_lowercase())
}

/// 锚定最终性策略（调用方显式提供；内核不内置任何链的确认数常数）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorPolicy {
    /// 进入「安全」所需的最小确认数；当 `require_finalized=true` 时可为 0（仅看最终性）。
    pub required_confirmations: u64,
    /// 是否要求对端/本端已给出显式最终性（finalized）标志。
    pub require_finalized: bool,
    /// 可容忍的最大已观测重组深度（含）；超过即拒绝。
    pub max_tolerated_reorg_depth: u64,
}

impl AnchorPolicy {
    /// 策略自身合法性：确认数要求与最终性要求不能同时为空，容忍深度仅需为任意 u64。
    fn validate(&self) -> Result<(), AnchorError> {
        if !self.require_finalized && self.required_confirmations == 0 {
            return Err(AnchorError::AnchorPolicyNoSafetyGate);
        }
        Ok(())
    }
}

/// 锚定取证快照（由受信任宿主/取证层取得后传入，内核不联网读取）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorEvidence {
    /// 源链标识（非空，如 "bitcoin" / "base"；内核不解释其含义）。
    pub source_chain: String,
    /// 取证到的确认数（区块高度差）。
    pub confirmations: u64,
    /// 取证层给出的显式最终性标志（无最终性证明能力时应传 false）。
    pub finalized: bool,
    /// 已观测到的重组深度（0 表示未见重组）。
    pub observed_reorg_depth: u64,
    /// 链下锚点中记录的 32 字节承诺（可选；不提供则不做承诺比对）。
    pub anchor_commitment_hex: Option<String>,
    /// 本笔被锚定数据声明的 32 字节承诺。
    pub claimed_commitment_hex: String,
}

/// 锚定校验回执（只读、可复算、可审计）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnchorReceipt {
    pub anchor_safe: bool,
    pub source_chain: String,
    pub confirmations: u64,
    pub required_confirmations: u64,
    pub finalized: bool,
    pub require_finalized: bool,
    pub observed_reorg_depth: u64,
    pub max_tolerated_reorg_depth: u64,
    /// 提供锚点承诺时为两者逐字节一致；未提供锚点承诺时为 false（诚实不声称已核对）。
    pub commitment_verified: bool,
    /// 内核从不直接读链：始终 false。
    pub onchain_read_in_kernel: bool,
}

/// 校验锚定最终性。任何安全门槛不满足都具名拒绝（fail-closed），不返回「带警告的安全」。
pub fn verify_anchor_finality(
    policy: &AnchorPolicy,
    evidence: &AnchorEvidence,
) -> Result<AnchorReceipt, AnchorError> {
    policy.validate()?;

    if evidence.source_chain.trim().is_empty() {
        return Err(AnchorError::AnchorBadChain);
    }

    // 重组优先于确认数判定：一旦观测重组超过容忍深度，立即拒绝。
    if evidence.observed_reorg_depth > policy.max_tolerated_reorg_depth {
        return Err(AnchorError::AnchorReorgExceeded {
            observed: evidence.observed_reorg_depth,
            tolerated: policy.max_tolerated_reorg_depth,
        });
    }

    // 显式最终性要求：取证层未给出 finalized 即拒绝，不用确认数代替最终性。
    if policy.require_finalized && !evidence.finalized {
        return Err(AnchorError::AnchorNotFinalized);
    }

    // 确认数门槛（严格不足即拒；恰好达到门槛视为通过）。
    if evidence.confirmations < policy.required_confirmations {
        return Err(AnchorError::AnchorInsufficientConfirmations {
            observed: evidence.confirmations,
            required: policy.required_confirmations,
        });
    }

    // 承诺一致性：锚点承诺可选；一旦提供就必须与声明承诺逐字节相同。
    let claimed = norm_commitment_hex("claimed_commitment_hex", &evidence.claimed_commitment_hex)?;
    let commitment_verified = match &evidence.anchor_commitment_hex {
        None => false,
        Some(raw) => {
            let anchor = norm_commitment_hex("anchor_commitment_hex", raw)?;
            if anchor != claimed {
                return Err(AnchorError::AnchorCommitmentMismatch);
            }
            true
        }
    };

    Ok(AnchorReceipt {
        anchor_safe: true,
        source_chain: evidence.source_chain.trim().to_string(),
        confirmations: evidence.confirmations,
        required_confirmations: policy.required_confirmations,
        finalized: evidence.finalized,
        require_finalized: policy.require_finalized,
        observed_reorg_depth: evidence.observed_reorg_depth,
        max_tolerated_reorg_depth: policy.max_tolerated_reorg_depth,
        commitment_verified,
        onchain_read_in_kernel: false,
    })
}

/// 锚定相关的类型化错误（全 `ANCHOR_*` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorError {
    /// 策略既不要求最终性、又把确认数门槛设为 0：无任何安全闸门，拒绝运行。
    AnchorPolicyNoSafetyGate,
    AnchorBadChain,
    AnchorBadCommitmentHex(String),
    AnchorReorgExceeded {
        observed: u64,
        tolerated: u64,
    },
    AnchorNotFinalized,
    AnchorInsufficientConfirmations {
        observed: u64,
        required: u64,
    },
    AnchorCommitmentMismatch,
}

impl std::fmt::Display for AnchorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnchorError::AnchorPolicyNoSafetyGate => write!(
                f,
                "ANCHOR_POLICY_NO_SAFETY_GATE: require_finalized=false 且 required_confirmations=0，无任何最终性闸门"
            ),
            AnchorError::AnchorBadChain => {
                write!(f, "ANCHOR_BAD_CHAIN: source_chain 为空")
            }
            AnchorError::AnchorBadCommitmentHex(label) => {
                write!(f, "ANCHOR_BAD_COMMITMENT_HEX: {label} 必须为 0x 可选的 32 字节(64 hex)")
            }
            AnchorError::AnchorReorgExceeded { observed, tolerated } => write!(
                f,
                "ANCHOR_REORG_EXCEEDED: observed_reorg_depth={observed} > max_tolerated={tolerated}"
            ),
            AnchorError::AnchorNotFinalized => {
                write!(f, "ANCHOR_NOT_FINALIZED: 策略要求 finalized，但取证快照 finalized=false")
            }
            AnchorError::AnchorInsufficientConfirmations { observed, required } => write!(
                f,
                "ANCHOR_INSUFFICIENT_CONFIRMATIONS: confirmations={observed} < required={required}"
            ),
            AnchorError::AnchorCommitmentMismatch => {
                write!(f, "ANCHOR_COMMITMENT_MISMATCH: 锚点承诺与声明承诺不一致")
            }
        }
    }
}

impl std::error::Error for AnchorError {}

/// 跨链桥风控策略（白名单为空默认全拒；所有阈值为宿主策略常量，内核不解释法币含义）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgePolicy {
    /// 总开关；false 时一切跨链请求拒绝。
    pub enabled: bool,
    /// 紧急暂停（优先级高于普通开关语义之外的额度判断）。
    pub paused: bool,
    /// 允许的方向（如 "btc->evm"）；**为空默认全拒**。
    pub allowed_directions: Vec<String>,
    /// 允许的资产标识；**为空默认全拒**。
    pub allowed_assets: Vec<String>,
    /// 是否要求目的端已在白名单中。
    pub require_allowlisted_destination: bool,
    /// 最小放行金额（含，内部整数 micro）。
    pub min_transfer: u128,
    /// 单笔上限（含）；必须 > 0。
    pub per_transfer_cap: u128,
    /// 单个速率窗口内允许的最大笔数（含）；必须 > 0。
    pub rate_window_max_count: u64,
    /// 累计额度上限（含）；0 表示不允许任何累计支出（首笔即拒）。
    pub cumulative_cap: u128,
}

impl BridgePolicy {
    fn validate(&self) -> Result<(), BridgeError> {
        if self.per_transfer_cap == 0 {
            return Err(BridgeError::BridgeBadPolicy("per_transfer_cap 必须 > 0"));
        }
        if self.rate_window_max_count == 0 {
            return Err(BridgeError::BridgeBadPolicy(
                "rate_window_max_count 必须 > 0",
            ));
        }
        if self.min_transfer > self.per_transfer_cap {
            return Err(BridgeError::BridgeBadPolicy(
                "min_transfer 不得大于 per_transfer_cap",
            ));
        }
        Ok(())
    }

    fn allows_direction(&self, dir: &str) -> bool {
        self.allowed_directions.iter().any(|d| d == dir)
    }

    fn allows_asset(&self, asset: &str) -> bool {
        self.allowed_assets.iter().any(|a| a == asset)
    }
}

/// 一笔跨链转移请求及调用方取证的当前用量（内核不持久化、不读时钟）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeTransfer {
    pub direction: String,
    pub asset: String,
    /// 金额（内部整数 micro，必须 > 0）。
    pub amount: u128,
    /// 目的端是否已在白名单。
    pub destination_allowlisted: bool,
    /// 当前速率窗口内已发生的笔数。
    pub window_prior_count: u64,
    /// 累计已用额度。
    pub cumulative_prior: u128,
}

/// 跨链放行回执（只决策；不代表桥已执行）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BridgeDecision {
    pub approved: bool,
    pub direction: String,
    pub asset: String,
    pub amount: String,
    pub projected_window_count: String,
    pub projected_cumulative: String,
    pub per_transfer_cap: String,
    pub cumulative_cap: String,
    /// 内核从不连桥/中继：始终 false。
    pub bridge_relay_executed: bool,
}

/// 按 fail-closed 短路链决定是否放行一笔跨链转移。
pub fn decide_bridge_transfer(
    policy: &BridgePolicy,
    req: &BridgeTransfer,
) -> Result<BridgeDecision, BridgeError> {
    if !policy.enabled {
        return Err(BridgeError::BridgeDisabled);
    }
    if policy.paused {
        return Err(BridgeError::BridgePaused);
    }
    policy.validate()?;

    // 白名单为空默认全拒（方向、资产各自独立）。
    if policy.allowed_directions.is_empty() || !policy.allows_direction(&req.direction) {
        return Err(BridgeError::BridgeDirectionNotAllowed(
            req.direction.clone(),
        ));
    }
    if policy.allowed_assets.is_empty() || !policy.allows_asset(&req.asset) {
        return Err(BridgeError::BridgeAssetNotAllowed(req.asset.clone()));
    }
    if policy.require_allowlisted_destination && !req.destination_allowlisted {
        return Err(BridgeError::BridgeDestinationNotAllowlisted);
    }

    if req.amount == 0 {
        return Err(BridgeError::BridgeNonPositiveAmount);
    }
    if req.amount < policy.min_transfer {
        return Err(BridgeError::BridgeBelowMinimum {
            amount: req.amount,
            min: policy.min_transfer,
        });
    }
    if req.amount > policy.per_transfer_cap {
        return Err(BridgeError::BridgeExceedsPerTransferCap {
            amount: req.amount,
            cap: policy.per_transfer_cap,
        });
    }

    // 速率：prior_count 已达上限（本笔将变成 max+1）即拒。checked 防 +1 溢出。
    let projected_count = req
        .window_prior_count
        .checked_add(1)
        .ok_or(BridgeError::BridgeArithmeticOverflow)?;
    if projected_count > policy.rate_window_max_count {
        return Err(BridgeError::BridgeRateLimited {
            prior: req.window_prior_count,
            max: policy.rate_window_max_count,
        });
    }

    // 累计：checked_add 防溢出；恰好花完允许，超过即拒。cumulative_cap=0 时首笔即拒。
    let projected_cumulative = req
        .cumulative_prior
        .checked_add(req.amount)
        .ok_or(BridgeError::BridgeArithmeticOverflow)?;
    if projected_cumulative > policy.cumulative_cap {
        return Err(BridgeError::BridgeExceedsCumulativeCap {
            projected: projected_cumulative,
            cap: policy.cumulative_cap,
        });
    }

    Ok(BridgeDecision {
        approved: true,
        direction: req.direction.clone(),
        asset: req.asset.clone(),
        amount: req.amount.to_string(),
        projected_window_count: projected_count.to_string(),
        projected_cumulative: projected_cumulative.to_string(),
        per_transfer_cap: policy.per_transfer_cap.to_string(),
        cumulative_cap: policy.cumulative_cap.to_string(),
        bridge_relay_executed: false,
    })
}

/// 真实跨链中继（锁仓/广播/铸造）。
///
/// 本版本无桥后端：对任何请求一律具名 [`BridgeError::BridgeNotConfigured`] fail-closed，
/// 绝不构造、签名或广播跨链消息，也不伪造锁定/释放结果。真实桥接须由宿主经独立闸门注入。
pub fn relay_bridge_transfer() -> Result<(), BridgeError> {
    Err(BridgeError::BridgeNotConfigured)
}

/// 跨链桥相关的类型化错误（全 `BRIDGE_*` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    BridgeDisabled,
    BridgePaused,
    BridgeBadPolicy(&'static str),
    BridgeDirectionNotAllowed(String),
    BridgeAssetNotAllowed(String),
    BridgeDestinationNotAllowlisted,
    BridgeNonPositiveAmount,
    BridgeBelowMinimum { amount: u128, min: u128 },
    BridgeExceedsPerTransferCap { amount: u128, cap: u128 },
    BridgeRateLimited { prior: u64, max: u64 },
    BridgeExceedsCumulativeCap { projected: u128, cap: u128 },
    BridgeArithmeticOverflow,
    BridgeNotConfigured,
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::BridgeDisabled => write!(f, "BRIDGE_DISABLED: 跨链桥总开关关闭"),
            BridgeError::BridgePaused => write!(f, "BRIDGE_PAUSED: 跨链桥已紧急暂停"),
            BridgeError::BridgeBadPolicy(msg) => write!(f, "BRIDGE_BAD_POLICY: {msg}"),
            BridgeError::BridgeDirectionNotAllowed(d) => {
                write!(f, "BRIDGE_DIRECTION_NOT_ALLOWED: 方向 {d} 不在白名单")
            }
            BridgeError::BridgeAssetNotAllowed(a) => {
                write!(f, "BRIDGE_ASSET_NOT_ALLOWED: 资产 {a} 不在白名单")
            }
            BridgeError::BridgeDestinationNotAllowlisted => {
                write!(f, "BRIDGE_DESTINATION_NOT_ALLOWLISTED: 目的端不在白名单")
            }
            BridgeError::BridgeNonPositiveAmount => {
                write!(f, "BRIDGE_NON_POSITIVE_AMOUNT: 金额必须 > 0")
            }
            BridgeError::BridgeBelowMinimum { amount, min } => write!(
                f,
                "BRIDGE_BELOW_MINIMUM: amount={amount} < min_transfer={min}"
            ),
            BridgeError::BridgeExceedsPerTransferCap { amount, cap } => write!(
                f,
                "BRIDGE_EXCEEDS_PER_TRANSFER_CAP: amount={amount} > cap={cap}"
            ),
            BridgeError::BridgeRateLimited { prior, max } => write!(
                f,
                "BRIDGE_RATE_LIMITED: window_prior_count={prior} 已达窗口上限 {max}"
            ),
            BridgeError::BridgeExceedsCumulativeCap { projected, cap } => write!(
                f,
                "BRIDGE_EXCEEDS_CUMULATIVE_CAP: projected={projected} > cap={cap}"
            ),
            BridgeError::BridgeArithmeticOverflow => {
                write!(f, "BRIDGE_ARITHMETIC_OVERFLOW: 整数 checked 运算溢出")
            }
            BridgeError::BridgeNotConfigured => write!(
                f,
                "BRIDGE_NOT_CONFIGURED: 本版本无跨链桥后端，不锁定/铸造/广播，fail-closed"
            ),
        }
    }
}

impl std::error::Error for BridgeError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim32(byte: u8) -> String {
        format!("0x{}", vec![format!("{byte:02x}"); 32].concat())
    }

    fn base_policy() -> AnchorPolicy {
        AnchorPolicy {
            required_confirmations: 6,
            require_finalized: false,
            max_tolerated_reorg_depth: 1,
        }
    }

    fn base_evidence() -> AnchorEvidence {
        AnchorEvidence {
            source_chain: "bitcoin".to_string(),
            confirmations: 7,
            finalized: false,
            observed_reorg_depth: 0,
            anchor_commitment_hex: Some(claim32(0xab)),
            claimed_commitment_hex: claim32(0xab),
        }
    }

    #[test]
    fn anchor_happy_path_passes_and_matches_commitment() {
        let r = verify_anchor_finality(&base_policy(), &base_evidence()).unwrap();
        assert!(r.anchor_safe);
        assert!(r.commitment_verified);
        assert!(!r.onchain_read_in_kernel);
        // 大小写/前缀不敏感。
        let mut ev = base_evidence();
        ev.anchor_commitment_hex = Some(claim32(0xab).to_uppercase());
        ev.claimed_commitment_hex = claim32(0xab);
        assert!(verify_anchor_finality(&base_policy(), &ev).is_ok());
    }

    #[test]
    fn anchor_exactly_at_threshold_passes() {
        let mut ev = base_evidence();
        ev.confirmations = 6;
        assert!(verify_anchor_finality(&base_policy(), &ev).is_ok());
    }

    #[test]
    fn anchor_insufficient_confirmations_named() {
        let mut ev = base_evidence();
        ev.confirmations = 5;
        let err = verify_anchor_finality(&base_policy(), &ev).unwrap_err();
        assert_eq!(
            err.to_string(),
            "ANCHOR_INSUFFICIENT_CONFIRMATIONS: confirmations=5 < required=6"
        );
    }

    #[test]
    fn anchor_finalized_required_but_not_finalized_named() {
        let policy = AnchorPolicy {
            required_confirmations: 0,
            require_finalized: true,
            max_tolerated_reorg_depth: 0,
        };
        let mut ev = base_evidence();
        ev.finalized = false;
        assert_eq!(
            verify_anchor_finality(&policy, &ev).unwrap_err(),
            AnchorError::AnchorNotFinalized
        );
        ev.finalized = true;
        assert!(verify_anchor_finality(&policy, &ev).is_ok());
    }

    #[test]
    fn anchor_policy_without_any_gate_rejected() {
        let policy = AnchorPolicy {
            required_confirmations: 0,
            require_finalized: false,
            max_tolerated_reorg_depth: 0,
        };
        assert_eq!(
            verify_anchor_finality(&policy, &base_evidence()).unwrap_err(),
            AnchorError::AnchorPolicyNoSafetyGate
        );
    }

    #[test]
    fn anchor_reorg_depth_enforced_before_confirmations() {
        let mut ev = base_evidence();
        ev.observed_reorg_depth = 2;
        assert_eq!(
            verify_anchor_finality(&base_policy(), &ev).unwrap_err(),
            AnchorError::AnchorReorgExceeded {
                observed: 2,
                tolerated: 1
            }
        );
        // 恰好等于容忍深度（含）放行。
        ev.observed_reorg_depth = 1;
        assert!(verify_anchor_finality(&base_policy(), &ev).is_ok());
    }

    #[test]
    fn anchor_commitment_mismatch_and_bad_hex_named() {
        let mut ev = base_evidence();
        ev.anchor_commitment_hex = Some(claim32(0xcd));
        assert_eq!(
            verify_anchor_finality(&base_policy(), &ev).unwrap_err(),
            AnchorError::AnchorCommitmentMismatch
        );
        ev.anchor_commitment_hex = Some("0x1234".to_string());
        assert!(matches!(
            verify_anchor_finality(&base_policy(), &ev).unwrap_err(),
            AnchorError::AnchorBadCommitmentHex(_)
        ));
        // 不提供锚点承诺时不核对也不报错，但 commitment_verified=false。
        ev.anchor_commitment_hex = None;
        let r = verify_anchor_finality(&base_policy(), &ev).unwrap();
        assert!(!r.commitment_verified);
    }

    #[test]
    fn anchor_bad_chain_named() {
        let mut ev = base_evidence();
        ev.source_chain = "   ".to_string();
        assert_eq!(
            verify_anchor_finality(&base_policy(), &ev).unwrap_err(),
            AnchorError::AnchorBadChain
        );
    }

    fn bridge_policy() -> BridgePolicy {
        BridgePolicy {
            enabled: true,
            paused: false,
            allowed_directions: vec!["btc->evm".to_string()],
            allowed_assets: vec!["WBTC".to_string(), "BTC".to_string()],
            require_allowlisted_destination: true,
            min_transfer: 100,
            per_transfer_cap: 1_000_000,
            rate_window_max_count: 3,
            cumulative_cap: 5_000_000,
        }
    }

    fn bridge_req() -> BridgeTransfer {
        BridgeTransfer {
            direction: "btc->evm".to_string(),
            asset: "WBTC".to_string(),
            amount: 500_000,
            destination_allowlisted: true,
            window_prior_count: 1,
            cumulative_prior: 1_000_000,
        }
    }

    #[test]
    fn bridge_happy_path_projects_usage() {
        let d = decide_bridge_transfer(&bridge_policy(), &bridge_req()).unwrap();
        assert!(d.approved);
        assert!(!d.bridge_relay_executed);
        assert_eq!(d.projected_window_count, "2");
        assert_eq!(d.projected_cumulative, "1500000");
    }

    #[test]
    fn bridge_fail_closed_short_circuit_chain() {
        // disabled
        let mut p = bridge_policy();
        p.enabled = false;
        assert_eq!(
            decide_bridge_transfer(&p, &bridge_req()).unwrap_err(),
            BridgeError::BridgeDisabled
        );
        // paused
        let mut p = bridge_policy();
        p.paused = true;
        assert_eq!(
            decide_bridge_transfer(&p, &bridge_req()).unwrap_err(),
            BridgeError::BridgePaused
        );
        // 空方向白名单默认全拒
        let mut p = bridge_policy();
        p.allowed_directions = vec![];
        assert!(matches!(
            decide_bridge_transfer(&p, &bridge_req()).unwrap_err(),
            BridgeError::BridgeDirectionNotAllowed(_)
        ));
        // 未知方向
        let mut r = bridge_req();
        r.direction = "evm->sol".to_string();
        assert!(matches!(
            decide_bridge_transfer(&bridge_policy(), &r).unwrap_err(),
            BridgeError::BridgeDirectionNotAllowed(_)
        ));
        // 未知资产
        let mut r = bridge_req();
        r.asset = "DOGE".to_string();
        assert!(matches!(
            decide_bridge_transfer(&bridge_policy(), &r).unwrap_err(),
            BridgeError::BridgeAssetNotAllowed(_)
        ));
        // 空资产白名单默认全拒
        let mut p = bridge_policy();
        p.allowed_assets = vec![];
        assert!(matches!(
            decide_bridge_transfer(&p, &bridge_req()).unwrap_err(),
            BridgeError::BridgeAssetNotAllowed(_)
        ));
        // 目的端不在白名单
        let mut r = bridge_req();
        r.destination_allowlisted = false;
        assert_eq!(
            decide_bridge_transfer(&bridge_policy(), &r).unwrap_err(),
            BridgeError::BridgeDestinationNotAllowlisted
        );
    }

    #[test]
    fn bridge_amount_rate_and_cumulative_limits() {
        // 零金额
        let mut r = bridge_req();
        r.amount = 0;
        assert_eq!(
            decide_bridge_transfer(&bridge_policy(), &r).unwrap_err(),
            BridgeError::BridgeNonPositiveAmount
        );
        // 低于最小额
        let mut r = bridge_req();
        r.amount = 50;
        assert!(matches!(
            decide_bridge_transfer(&bridge_policy(), &r).unwrap_err(),
            BridgeError::BridgeBelowMinimum { .. }
        ));
        // 超单笔上限
        let mut r = bridge_req();
        r.amount = 2_000_000;
        assert!(matches!(
            decide_bridge_transfer(&bridge_policy(), &r).unwrap_err(),
            BridgeError::BridgeExceedsPerTransferCap { .. }
        ));
        // 速率窗口：prior 已达上限
        let mut r = bridge_req();
        r.window_prior_count = 3;
        assert!(matches!(
            decide_bridge_transfer(&bridge_policy(), &r).unwrap_err(),
            BridgeError::BridgeRateLimited { .. }
        ));
        // 累计超限
        let mut r = bridge_req();
        r.cumulative_prior = 4_800_000;
        assert!(matches!(
            decide_bridge_transfer(&bridge_policy(), &r).unwrap_err(),
            BridgeError::BridgeExceedsCumulativeCap { .. }
        ));
        // 恰好花完累计额度允许（守恒边界）
        let mut r = bridge_req();
        r.cumulative_prior = 4_500_000;
        r.amount = 500_000;
        let d = decide_bridge_transfer(&bridge_policy(), &r).unwrap();
        assert_eq!(d.projected_cumulative, "5000000");
    }

    #[test]
    fn bridge_cumulative_cap_zero_denies_first_transfer() {
        let p = BridgePolicy {
            cumulative_cap: 0,
            ..bridge_policy()
        };
        let r = BridgeTransfer {
            cumulative_prior: 0,
            ..bridge_req()
        };
        assert!(matches!(
            decide_bridge_transfer(&p, &r).unwrap_err(),
            BridgeError::BridgeExceedsCumulativeCap { .. }
        ));
    }

    #[test]
    fn bridge_bad_policy_named_and_relay_fail_closed() {
        let p = BridgePolicy {
            per_transfer_cap: 0,
            ..bridge_policy()
        };
        assert!(matches!(
            decide_bridge_transfer(&p, &bridge_req()).unwrap_err(),
            BridgeError::BridgeBadPolicy(_)
        ));
        let p = BridgePolicy {
            min_transfer: 10,
            per_transfer_cap: 5,
            ..bridge_policy()
        };
        assert!(matches!(
            decide_bridge_transfer(&p, &bridge_req()).unwrap_err(),
            BridgeError::BridgeBadPolicy(_)
        ));
        assert_eq!(
            relay_bridge_transfer().unwrap_err(),
            BridgeError::BridgeNotConfigured
        );
    }
}
