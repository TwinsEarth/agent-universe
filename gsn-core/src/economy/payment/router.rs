//! v3.9.0 结算路由（settlement router）：在三条支付轨道之间做**纯确定性选路**。
//!
//! # 只决策，不动钱
//!
//! 本模块是 v3.9.x「比特币/以太坊 Agent 经济体」的第一块：给定一笔待结算的金额、
//! 时延要求与条件（是否需要智能合约、是否需要 DeFi 组合）以及本节点实际可用的轨道，
//! 确定性地选出**唯一**一条结算轨道，并给出可审计的选路理由。它：
//!
//! - 不持有/不接触任何私钥（签名是宿主受限服务，见 v3.9.1），不签名、不广播交易；
//! - 不移动、不托管、不兑换任何资金，不调用闪电节点/EVM/RGB，纯函数；
//! - 金额一律内部整数 micro（与资源市场一致，1 credit = 10^6 micro），**零浮点**、
//!   零 syscall、零 unsafe、无 panic 路径，全部比较为整数比较。
//!
//! # 三条轨道（对应开发计划三层货币体系）
//!
//! - [`PaymentTrack::LightningL402`]：比特币闪电网络 L402/x402-LN——近乎即时、极低费，
//!   适合金额很小且要求即时的高频微支付；
//! - [`PaymentTrack::EvmX402`]：以太坊 L2 上的 x402 + 稳定币/ERC-4337——可编程条件支付、
//!   DeFi 组合与日常中等额结算的默认轨道；
//! - [`PaymentTrack::BtcRgbHtlc`]：比特币主链 RGB/Taproot 或 HTLC 原子交换——大额或
//!   跨周期最终结算的价值锚定轨道。
//!
//! 选路所需轨道若在本节点不可用，**具名拒绝而不静默降级**：把需要最终结算的大额悄悄
//! 改成 L2、或把需要即时的微支付悄悄改上主链，都会改变费用/最终性/信任假设，必须让
//! 调用方显式处理（[`PaymentError::RouteTrackUnavailable`]）。

use super::PaymentError;
use serde::{Deserialize, Serialize};

/// 结算轨道。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentTrack {
    /// 比特币闪电网络 L402（即时高频微支付）。
    LightningL402,
    /// 以太坊 L2 x402 + 稳定币/ERC-4337（可编程条件 / DeFi / 日常默认）。
    EvmX402,
    /// 比特币主链 RGB/Taproot/HTLC（大额或跨周期最终结算）。
    BtcRgbHtlc,
}

impl PaymentTrack {
    /// 规范蛇形字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            PaymentTrack::LightningL402 => "lightning_l402",
            PaymentTrack::EvmX402 => "evm_x402",
            PaymentTrack::BtcRgbHtlc => "btc_rgb_htlc",
        }
    }

    /// 全部已知轨道。
    pub const ALL: &[PaymentTrack] = &[
        PaymentTrack::LightningL402,
        PaymentTrack::EvmX402,
        PaymentTrack::BtcRgbHtlc,
    ];

    /// 是否为最终结算（比特币主链）轨道。
    pub fn is_finality(self) -> bool {
        matches!(self, PaymentTrack::BtcRgbHtlc)
    }
}

/// 结算时延/周期要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementUrgency {
    /// 要求即时（交互/在线评测等短任务）。
    Instant,
    /// 常规（无强即时或跨周期要求）。
    Normal,
    /// 跨周期/长期质押/最终结算（时间不敏感，追求最终性）。
    CrossEpoch,
}

/// 本节点（或本笔交易双方）实际可用的轨道集合；为空表示无任何结算通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackAvailability {
    /// 比特币闪电 L402 通道（有可达闪电节点/发票能力）。
    pub lightning_l402: bool,
    /// 以太坊 L2 x402 通道（稳定币余额/Paymaster/Gas）。
    pub evm_x402: bool,
    /// 比特币主链 RGB/HTLC 通道。
    pub btc_rgb_htlc: bool,
}

impl TrackAvailability {
    /// 三轨全开（测试/全能力节点）。
    pub const fn all() -> Self {
        Self {
            lightning_l402: true,
            evm_x402: true,
            btc_rgb_htlc: true,
        }
    }

    /// 三轨全关。
    pub const fn none() -> Self {
        Self {
            lightning_l402: false,
            evm_x402: false,
            btc_rgb_htlc: false,
        }
    }

    /// 仅开指定轨道。
    pub const fn only(track: PaymentTrack) -> Self {
        match track {
            PaymentTrack::LightningL402 => Self {
                lightning_l402: true,
                evm_x402: false,
                btc_rgb_htlc: false,
            },
            PaymentTrack::EvmX402 => Self {
                lightning_l402: false,
                evm_x402: true,
                btc_rgb_htlc: false,
            },
            PaymentTrack::BtcRgbHtlc => Self {
                lightning_l402: false,
                evm_x402: false,
                btc_rgb_htlc: true,
            },
        }
    }

    /// 是否一条轨道都没有。
    pub const fn is_empty(self) -> bool {
        !self.lightning_l402 && !self.evm_x402 && !self.btc_rgb_htlc
    }

    /// 是否支持指定轨道。
    pub const fn supports(self, track: PaymentTrack) -> bool {
        match track {
            PaymentTrack::LightningL402 => self.lightning_l402,
            PaymentTrack::EvmX402 => self.evm_x402,
            PaymentTrack::BtcRgbHtlc => self.btc_rgb_htlc,
        }
    }
}

/// 选路阈值策略（宿主受控常量；金额单位为内部 micro，内核不宣称其法币/币价含义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingPolicy {
    /// 即时微支付上限：`urgency=Instant` 且 `amount <= instant_cap_micro` 走闪电 L402。
    pub instant_cap_micro: u128,
    /// 最终结算下限：`amount >= large_floor_micro`（或跨周期）走比特币主链 RGB/HTLC。
    pub large_floor_micro: u128,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        // 0.01 credit（=10_000 micro）与 100 credit（=100_000_000 micro）的默认分界；
        // 仅为整数策略阈值，不构成对法币/币价的承诺。
        Self {
            instant_cap_micro: 10_000,
            large_floor_micro: 100_000_000,
        }
    }
}

impl RoutingPolicy {
    /// 校验阈值：两个阈值必须为正且即时上限严格小于最终结算下限，否则策略退化/矛盾。
    pub const fn validated(
        instant_cap_micro: u128,
        large_floor_micro: u128,
    ) -> Result<Self, PaymentError> {
        if instant_cap_micro == 0
            || large_floor_micro == 0
            || instant_cap_micro >= large_floor_micro
        {
            return Err(PaymentError::RoutingPolicyInvalid {
                instant_cap_micro,
                large_floor_micro,
            });
        }
        Ok(Self {
            instant_cap_micro,
            large_floor_micro,
        })
    }
}

/// 选路理由（可审计：为什么选这条轨道）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteReason {
    /// 需要 DeFi 组合，只有 EVM 可表达 → EVM x402。
    DefiComboRequiresEvm,
    /// 需要智能合约条件支付，只有 EVM 可表达 → EVM x402（可编程性优先于最终结算偏好）。
    SmartContractRequiresEvm,
    /// 大额（达下限）或跨周期最终结算 → 比特币主链 RGB/HTLC。
    LargeOrCrossEpochFinalSettlement,
    /// 小额且要求即时 → 闪电 L402。
    InstantMicroPayment,
    /// 无合约/非即时小额/非大额的常规区间 → 稳定币 x402 默认轨道。
    StablecoinDefaultMidRange,
}

/// 选路输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingInput {
    /// 待结算金额（内部 micro，必须为正）。
    pub amount_micro: u128,
    /// 时延/周期要求。
    pub urgency: SettlementUrgency,
    /// 是否需要智能合约条件支付（托管/分批/可编程授权）。
    pub needs_smart_contract: bool,
    /// 是否需要 DeFi 组合（兑换/流动性/借贷等）。
    pub needs_defi_combo: bool,
    /// 本笔可用轨道集合（不可为空）。
    pub available: TrackAvailability,
}

impl RoutingInput {
    /// 构造并校验：金额为正、至少一条轨道可用；条件标记合法（defi 蕴含 smart_contract）。
    pub const fn new(
        amount_micro: u128,
        urgency: SettlementUrgency,
        needs_smart_contract: bool,
        needs_defi_combo: bool,
        available: TrackAvailability,
    ) -> Result<Self, PaymentError> {
        if amount_micro == 0 {
            return Err(PaymentError::NonPositiveAmount);
        }
        if available.is_empty() {
            return Err(PaymentError::NoAvailableTrack);
        }
        // 需要 DeFi 组合必然需要智能合约能力；标记自相矛盾 fail-closed，不静默纠正。
        if needs_defi_combo && !needs_smart_contract {
            return Err(PaymentError::DefiWithoutSmartContract);
        }
        Ok(Self {
            amount_micro,
            urgency,
            needs_smart_contract,
            needs_defi_combo,
            available,
        })
    }
}

/// 选路结果（轨道 + 理由）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteDecision {
    /// 选中的结算轨道。
    pub track: PaymentTrack,
    /// 选路理由。
    pub reason: RouteReason,
}

/// 确定性结算路由器（持有阈值策略，无任何资金/密钥状态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PaymentRouter {
    policy: RoutingPolicy,
}

impl PaymentRouter {
    /// 用默认策略构造。
    pub fn new() -> Self {
        Self::default()
    }

    /// 用显式（已校验）策略构造。
    pub const fn with_policy(policy: RoutingPolicy) -> Self {
        Self { policy }
    }

    /// 当前策略只读。
    pub const fn policy(&self) -> RoutingPolicy {
        self.policy
    }

    /// 纯确定性选路。相同输入恒得相同结果；所需轨道不可用具名拒绝，不静默降级。
    pub fn route(&self, input: &RoutingInput) -> Result<RouteDecision, PaymentError> {
        // 选轨优先级（确定性、自上而下短路）：
        // 1) DeFi 组合  → 仅 EVM 可表达；
        // 2) 智能合约   → 仅 EVM 可表达（即便大额/跨周期，可编程性也优先于主链最终性）；
        // 3) 大额达下限或跨周期（且无合约要求）→ 比特币主链 RGB/HTLC 最终结算；
        // 4) 即时且金额 ≤ 微支付上限 → 闪电 L402；
        // 5) 其余常规区间 → 以太坊 L2 稳定币 x402 默认。
        let required = if input.needs_defi_combo {
            (PaymentTrack::EvmX402, RouteReason::DefiComboRequiresEvm)
        } else if input.needs_smart_contract {
            (PaymentTrack::EvmX402, RouteReason::SmartContractRequiresEvm)
        } else if input.amount_micro >= self.policy.large_floor_micro
            || input.urgency == SettlementUrgency::CrossEpoch
        {
            (
                PaymentTrack::BtcRgbHtlc,
                RouteReason::LargeOrCrossEpochFinalSettlement,
            )
        } else if input.urgency == SettlementUrgency::Instant
            && input.amount_micro <= self.policy.instant_cap_micro
        {
            (
                PaymentTrack::LightningL402,
                RouteReason::InstantMicroPayment,
            )
        } else {
            (
                PaymentTrack::EvmX402,
                RouteReason::StablecoinDefaultMidRange,
            )
        };

        let (track, reason) = required;
        if !input.available.supports(track) {
            return Err(PaymentError::RouteTrackUnavailable { required: track });
        }
        Ok(RouteDecision { track, reason })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_of(
        amount: u128,
        urgency: SettlementUrgency,
        sc: bool,
        defi: bool,
        avail: TrackAvailability,
    ) -> RoutingInput {
        RoutingInput::new(amount, urgency, sc, defi, avail).expect("valid routing input")
    }

    #[test]
    fn instant_micro_routes_lightning() {
        let r = PaymentRouter::new();
        let d = r
            .route(&input_of(
                5_000,
                SettlementUrgency::Instant,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(d.track, PaymentTrack::LightningL402);
        assert_eq!(d.reason, RouteReason::InstantMicroPayment);
        // 恰等于即时上限（含边界）仍走闪电。
        let edge = r
            .route(&input_of(
                10_000,
                SettlementUrgency::Instant,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(edge.track, PaymentTrack::LightningL402);
    }

    #[test]
    fn instant_above_cap_and_mid_normal_default_to_evm_stablecoin() {
        let r = PaymentRouter::new();
        // 即时但金额超过微支付上限 → 不降费轨、也不上主链，走稳定币默认。
        let above = r
            .route(&input_of(
                50_000,
                SettlementUrgency::Instant,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(above.track, PaymentTrack::EvmX402);
        assert_eq!(above.reason, RouteReason::StablecoinDefaultMidRange);
        // 常规区间 Normal。
        let mid = r
            .route(&input_of(
                1_000_000,
                SettlementUrgency::Normal,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(mid.track, PaymentTrack::EvmX402);
        assert_eq!(mid.reason, RouteReason::StablecoinDefaultMidRange);
        // 恰低于最终结算下限仍留 EVM。
        let just_below = r
            .route(&input_of(
                99_999_999,
                SettlementUrgency::Normal,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(just_below.track, PaymentTrack::EvmX402);
    }

    #[test]
    fn large_amount_boundary_routes_btc_finality() {
        let r = PaymentRouter::new();
        // 恰等于大额下限（含边界）→ 主链最终结算。
        let at = r
            .route(&input_of(
                100_000_000,
                SettlementUrgency::Normal,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(at.track, PaymentTrack::BtcRgbHtlc);
        assert_eq!(at.reason, RouteReason::LargeOrCrossEpochFinalSettlement);
        assert!(at.track.is_finality());
        // 明显大额。
        let big = r
            .route(&input_of(
                500_000_000,
                SettlementUrgency::Normal,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(big.track, PaymentTrack::BtcRgbHtlc);
    }

    #[test]
    fn cross_epoch_mid_amount_routes_btc_even_when_small() {
        let r = PaymentRouter::new();
        let d = r
            .route(&input_of(
                1_000_000,
                SettlementUrgency::CrossEpoch,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(d.track, PaymentTrack::BtcRgbHtlc);
        assert_eq!(d.reason, RouteReason::LargeOrCrossEpochFinalSettlement);
    }

    #[test]
    fn smart_contract_wins_over_large_and_cross_epoch() {
        let r = PaymentRouter::new();
        let small = r
            .route(&input_of(
                1_000,
                SettlementUrgency::Normal,
                true,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(small.track, PaymentTrack::EvmX402);
        assert_eq!(small.reason, RouteReason::SmartContractRequiresEvm);
        // 大额 + 跨周期 + 需要合约：可编程性优先，仍 EVM（BTC 脚本表达不了通用条件）。
        let heavy = r
            .route(&input_of(
                500_000_000,
                SettlementUrgency::CrossEpoch,
                true,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(heavy.track, PaymentTrack::EvmX402);
        assert_eq!(heavy.reason, RouteReason::SmartContractRequiresEvm);
    }

    #[test]
    fn defi_combo_forces_evm_regardless_and_implies_contract() {
        let r = PaymentRouter::new();
        let d = r
            .route(&input_of(
                500_000_000,
                SettlementUrgency::CrossEpoch,
                true,
                true,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(d.track, PaymentTrack::EvmX402);
        assert_eq!(d.reason, RouteReason::DefiComboRequiresEvm);
        // defi=true 但 sc=false 自相矛盾 → fail-closed。
        let bad = RoutingInput::new(
            1_000,
            SettlementUrgency::Normal,
            false,
            true,
            TrackAvailability::all(),
        );
        assert_eq!(bad, Err(PaymentError::DefiWithoutSmartContract));
    }

    #[test]
    fn missing_required_track_fails_closed_without_downgrade() {
        let r = PaymentRouter::new();
        // 需要闪电但只有 EVM → 具名拒绝，不静默改稳定币。
        let no_ln = r.route(&input_of(
            1_000,
            SettlementUrgency::Instant,
            false,
            false,
            TrackAvailability::only(PaymentTrack::EvmX402),
        ));
        assert_eq!(
            no_ln,
            Err(PaymentError::RouteTrackUnavailable {
                required: PaymentTrack::LightningL402
            })
        );
        // 大额需要主链但只有 EVM/闪电 → 拒绝，不把最终结算悄悄降到 L2。
        let no_btc = r.route(&input_of(
            500_000_000,
            SettlementUrgency::Normal,
            false,
            false,
            TrackAvailability {
                lightning_l402: true,
                evm_x402: true,
                btc_rgb_htlc: false,
            },
        ));
        assert_eq!(
            no_btc,
            Err(PaymentError::RouteTrackUnavailable {
                required: PaymentTrack::BtcRgbHtlc
            })
        );
        // 需要合约（EVM）但只有主链 → 拒绝，不悄悄改成无条件 BTC 结算。
        let no_evm = r.route(&input_of(
            1_000,
            SettlementUrgency::Normal,
            true,
            false,
            TrackAvailability::only(PaymentTrack::BtcRgbHtlc),
        ));
        assert_eq!(
            no_evm,
            Err(PaymentError::RouteTrackUnavailable {
                required: PaymentTrack::EvmX402
            })
        );
    }

    #[test]
    fn zero_amount_empty_availability_bad_policy_and_determinism() {
        // 零金额拒绝。
        assert_eq!(
            RoutingInput::new(
                0,
                SettlementUrgency::Normal,
                false,
                false,
                TrackAvailability::all()
            ),
            Err(PaymentError::NonPositiveAmount)
        );
        // 无任何可用轨道拒绝。
        assert_eq!(
            RoutingInput::new(
                1_000,
                SettlementUrgency::Normal,
                false,
                false,
                TrackAvailability::none()
            ),
            Err(PaymentError::NoAvailableTrack)
        );
        // 阈值策略非法：0 值 / 上限不小于下限。
        assert!(matches!(
            RoutingPolicy::validated(0, 1000),
            Err(PaymentError::RoutingPolicyInvalid { .. })
        ));
        assert!(matches!(
            RoutingPolicy::validated(1000, 0),
            Err(PaymentError::RoutingPolicyInvalid { .. })
        ));
        assert!(matches!(
            RoutingPolicy::validated(1000, 1000),
            Err(PaymentError::RoutingPolicyInvalid { .. })
        ));
        // 自定义合法策略生效：把最终下限降到 2000，则 2000 即走主链。
        let rp = PaymentRouter::with_policy(RoutingPolicy::validated(500, 2000).unwrap());
        let d = rp
            .route(&input_of(
                2000,
                SettlementUrgency::Normal,
                false,
                false,
                TrackAvailability::all(),
            ))
            .unwrap();
        assert_eq!(d.track, PaymentTrack::BtcRgbHtlc);
        // 确定性：同一输入重复路由结果逐字相等。
        let r = PaymentRouter::new();
        let inp = input_of(
            7_777,
            SettlementUrgency::Instant,
            false,
            false,
            TrackAvailability::all(),
        );
        assert_eq!(r.route(&inp), r.route(&inp));
        // 轨道字符串往返稳定。
        assert_eq!(PaymentTrack::EvmX402.as_str(), "evm_x402");
        assert_eq!(PaymentTrack::ALL.len(), 3);
    }
}
