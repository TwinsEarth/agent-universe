//! 托管结算守恒账本（v3.8.5）。
//!
//! # 定位
//!
//! v3.8.2 的计量账本只**算出**每笔订单的整数结算视图 [`MeterSettlement`]
//! （`reserved_cost = consumed_cost + refund`），但不动钱；v3.8.4 的质押账本表达了
//! 保证金的冻结/罚没，却没定义罚没资金在一笔订单结算里的**去向**。v3.8.5 新增**托管
//! 结算账本 [`EscrowLedger`]**：消费者下单时把预留总额锁进托管，结算时把这笔钱确定性
//! 地分到五个互斥终局桶，并强守恒。零浮点、零 syscall、零 unsafe、无 panic 路径，
//! 金额全 `u128` checked 运算，费率用整数千分点（permyriad，1000 = 100%）。
//!
//! # 五个终局分桶与守恒
//!
//! 一笔订单锁定的托管资金 [`EscrowOrder::locked_micro`] 在结算后必被穷尽地分到：
//!
//! - `payout_provider`：实耗金额中**归供给方**的部分（扣除版税、治理费、罚没后）；
//! - `royalty`：实耗金额中按千分点抽给环境快照/技能创建者的**版税**；
//! - `governance_fee`：实耗金额中按千分点抽给治理池的**平台治理费**；
//! - `refund_buyer`：实耗少于预留时**退回消费者**的部分（=
//!   [`MeterSettlement::refund_micro`]）；
//! - `slashed`：争议判罚中从供给方应得出账里**罚没截留**的部分（终局，既不付供给方
//!   也不退买方，表达 v3.8.4 所言「罚没资金在分账桶中的归属」，真实划转在后续/链上版）。
//!
//! 托管守恒恒等式（每笔已结算订单恒成立）：
//! ```text
//! locked == payout_provider + royalty + governance_fee + refund_buyer + slashed
//! ```
//!
//! # 分账口径（整数、可审计）
//!
//! 设实耗总额 `gross = consumed_cost`（消费者为实耗应付的钱，内含版税与治理费）：
//! - `royalty     = gross × royalty_permyriad / 1000`（整数向下取整）；
//! - `governance = gross × governance_permyriad / 1000`（整数向下取整）；
//! - 费率之和必须 `≤ 1000`，否则 [`ResourceError::FeeRatesExceedTotal`] fail-closed；
//! - 供给方候选应得 `payout_gross = gross − royalty − governance`（取整余数留供给方，
//!   绝不凭空造出第 6 桶钱）；
//! - 罚没 `slash` 只能来自 `payout_gross`（`slash ≤ payout_gross`），终局
//!   `payout_provider = payout_gross − slash`、`slashed = slash`；罚没超候选应得
//!   fail-closed。退款 `refund` 不参与罚没（不能罚消费者待退款）。
//!
//! # 决策/记账分离与诚实边界
//!
//! 本账本**不判定违规、不决定费率高低、不接链、不做真实资金划转、不经 PMB 受理外部
//! 写、不新增能力令牌**：费率由治理配置给入，是否罚没/罚多少由后续 QA（v3.8.8）/审判
//! 裁决给入；账本只保证「给定计量视图、费率、罚没额，分钱结果唯一且五桶守恒」。
//! [`EscrowLedger`] 是纯内存、顺序确定的记账面，不持久化、不跨进程/跨节点。

use serde::{Deserialize, Serialize};

use super::metering::MeterSettlement;
use super::ResourceError;

/// 千分点分母：1000 permyriad = 100%。
pub const PERMYRIAD: u128 = 1000;

/// 分账费率（整数千分点）。两者之和必须 ≤ 1000。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeSchedule {
    /// 版税千分点（归环境快照/技能创建者）。
    pub royalty_permyriad: u32,
    /// 治理费千分点（归治理池）。
    pub governance_permyriad: u32,
}

impl FeeSchedule {
    /// 构造并校验两费率之和 ≤ 1000。
    pub fn new(royalty_permyriad: u32, governance_permyriad: u32) -> Result<Self, ResourceError> {
        let sum = (royalty_permyriad as u64) + (governance_permyriad as u64);
        if sum > PERMYRIAD as u64 {
            return Err(ResourceError::FeeRatesExceedTotal {
                sum_permyriad: sum as u32,
            });
        }
        Ok(Self {
            royalty_permyriad,
            governance_permyriad,
        })
    }

    /// 零费率（不抽版税/治理费），便于只测托管退款/罚没的场景。
    pub fn zero() -> Self {
        Self {
            royalty_permyriad: 0,
            governance_permyriad: 0,
        }
    }
}

/// 一笔订单的托管生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscrowState {
    /// 资金已锁定、尚未结算。
    Locked,
    /// 已完成五桶分账（终态）。
    Settled,
}

/// 一次结算的五桶分账结果（整数，单位 micro）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowSplit {
    /// 归供给方（实耗扣费、扣罚没后）。
    pub payout_provider_micro: u128,
    /// 版税（环境快照/技能创建者）。
    pub royalty_micro: u128,
    /// 治理费（治理池）。
    pub governance_fee_micro: u128,
    /// 退回消费者（实耗少于预留）。
    pub refund_buyer_micro: u128,
    /// 罚没截留（从供给方应得出账，终局）。
    pub slashed_micro: u128,
}

impl EscrowSplit {
    /// 五桶之和（checked）。
    pub fn total(&self) -> Result<u128, ResourceError> {
        let a = self
            .payout_provider_micro
            .checked_add(self.royalty_micro)
            .and_then(|v| v.checked_add(self.governance_fee_micro))
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let b = self
            .refund_buyer_micro
            .checked_add(self.slashed_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        a.checked_add(b).ok_or(ResourceError::ArithmeticOverflow)
    }

    /// 分账守恒：五桶之和必须等于托管锁定额。
    pub fn conserves(&self, locked_micro: u128) -> bool {
        matches!(self.total(), Ok(t) if t == locked_micro)
    }
}

/// 依据计量结算视图、费率与罚没额，纯函数计算五桶分账（可独立单测，不落账）。
///
/// 复用 [`MeterSettlement`] 的 `consumed_cost/refund/reserved`；先做计量守恒与费率
/// 校验，再算费、罚没，最后强校验五桶之和 == reserved，任一不满足具名拒绝且不产出。
pub fn split_for_settlement(
    ms: &MeterSettlement,
    fees: FeeSchedule,
    slash_micro: u128,
) -> Result<EscrowSplit, ResourceError> {
    let gross = ms.consumed_cost_micro;
    let refund = ms.refund_micro;
    let reserved = ms.reserved_cost_micro;

    // 计量侧守恒（正常由 MeteringLedger 保证，此处结算边界再 fail-closed 复核一次）。
    match gross.checked_add(refund) {
        Some(s) if s == reserved => {}
        _ => return Err(ResourceError::EscrowSplitNotConserved),
    }

    // 费率取整（向下），余数留供给方，避免凭空造钱。
    let royalty = gross
        .checked_mul(fees.royalty_permyriad as u128)
        .ok_or(ResourceError::ArithmeticOverflow)?
        / PERMYRIAD;
    let governance_fee = gross
        .checked_mul(fees.governance_permyriad as u128)
        .ok_or(ResourceError::ArithmeticOverflow)?
        / PERMYRIAD;
    let payout_gross = gross
        .checked_sub(royalty)
        .and_then(|v| v.checked_sub(governance_fee))
        .ok_or(ResourceError::ArithmeticOverflow)?;

    // 罚没只能来自供给方候选应得出账；不能罚消费者待退款。
    if slash_micro > payout_gross {
        return Err(ResourceError::SlashExceedsProviderPayout {
            attempted: slash_micro,
            payout: payout_gross,
        });
    }
    let payout_provider = payout_gross - slash_micro; // slash≤payout_gross 已校验

    let split = EscrowSplit {
        payout_provider_micro: payout_provider,
        royalty_micro: royalty,
        governance_fee_micro: governance_fee,
        refund_buyer_micro: refund,
        slashed_micro: slash_micro,
    };
    if !split.conserves(reserved) {
        return Err(ResourceError::EscrowSplitNotConserved);
    }
    Ok(split)
}

/// 单笔订单的托管记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowOrder {
    /// 订单 id（非空）。
    pub order_id: String,
    /// 托管锁定额（= 计量预留总额 reserved_cost）。
    pub locked_micro: u128,
    /// 生命周期状态。
    pub state: EscrowState,
    /// 结算后的五桶分账（Locked 时为 None）。
    pub split: Option<EscrowSplit>,
}

/// 托管结算账本：以订单 id 为键的有序内存记录集。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EscrowLedger {
    orders: Vec<EscrowOrder>,
}

impl EscrowLedger {
    /// 空账本。
    pub fn new() -> Self {
        Self { orders: Vec::new() }
    }

    /// 托管订单数。
    pub fn len(&self) -> usize {
        self.orders.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    fn index_of(&self, order_id: &str) -> Option<usize> {
        self.orders.iter().position(|o| o.order_id == order_id)
    }

    /// 只读获取某订单托管记录。
    pub fn get(&self, order_id: &str) -> Option<&EscrowOrder> {
        self.index_of(order_id).map(|i| &self.orders[i])
    }

    /// 遍历所有托管记录（插入顺序）。
    pub fn orders(&self) -> &[EscrowOrder] {
        &self.orders
    }

    /// 仍处于 Locked（未结算）的托管锁定额之和（checked）。
    pub fn total_locked_open(&self) -> Result<u128, ResourceError> {
        self.orders
            .iter()
            .filter(|o| o.state == EscrowState::Locked)
            .try_fold(0u128, |acc, o| {
                acc.checked_add(o.locked_micro)
                    .ok_or(ResourceError::ArithmeticOverflow)
            })
    }

    /// 消费者为订单锁定托管预留额：开一条 `Locked` 记录。金额正、id 非空、不可重复。
    pub fn lock(
        &mut self,
        order_id: impl Into<String>,
        locked_micro: u128,
    ) -> Result<(), ResourceError> {
        let order_id = order_id.into();
        if order_id.trim().is_empty() {
            return Err(ResourceError::EmptyOrderId);
        }
        if locked_micro == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        if self.index_of(&order_id).is_some() {
            return Err(ResourceError::DuplicateEscrowOrder { order_id });
        }
        self.orders.push(EscrowOrder {
            order_id,
            locked_micro,
            state: EscrowState::Locked,
            split: None,
        });
        Ok(())
    }

    /// 结算：仅 `Locked → Settled`，用计量视图 + 费率 + 罚没额算五桶并落账。
    ///
    /// 托管锁定额必须等于计量预留总额（`locked == reserved`），否则
    /// [`ResourceError::EscrowLockedReservedMismatch`]；重复结算
    /// [`ResourceError::EscrowAlreadySettled`]；任何失败都不改写（仍停在 Locked）。
    pub fn settle(
        &mut self,
        order_id: &str,
        ms: &MeterSettlement,
        fees: FeeSchedule,
        slash_micro: u128,
    ) -> Result<EscrowSplit, ResourceError> {
        let i = self
            .index_of(order_id)
            .ok_or(ResourceError::EscrowOrderNotFound)?;
        if self.orders[i].state == EscrowState::Settled {
            return Err(ResourceError::EscrowAlreadySettled {
                order_id: order_id.to_string(),
            });
        }
        if self.orders[i].locked_micro != ms.reserved_cost_micro {
            return Err(ResourceError::EscrowLockedReservedMismatch {
                locked: self.orders[i].locked_micro,
                reserved: ms.reserved_cost_micro,
            });
        }
        // 先在纯函数里完成全部可能失败的分账，成功后才推进状态落账。
        let split = split_for_settlement(ms, fees, slash_micro)?;
        let order = &mut self.orders[i];
        order.state = EscrowState::Settled;
        order.split = Some(split);
        Ok(split)
    }

    /// 全表守恒：已结算订单五桶之和恰等于其锁定额；未结算记录不得携带 split。
    pub fn invariant_holds(&self) -> bool {
        self.orders.iter().all(|o| match o.state {
            EscrowState::Locked => o.split.is_none(),
            EscrowState::Settled => match &o.split {
                Some(s) => s.conserves(o.locked_micro),
                None => false,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(allocated: u128, consumed: u128, price: u128) -> MeterSettlement {
        MeterSettlement {
            consumed,
            allocated,
            consumed_cost_micro: consumed * price,
            refund_micro: (allocated - consumed) * price,
            reserved_cost_micro: allocated * price,
        }
    }

    #[test]
    fn normal_split_conserves_with_provider_royalty_gov_refund() {
        // reserved1000（allocated1000×price1），实耗 800 → gross800，退 200。
        let m = ms(1000, 800, 1);
        let fees = FeeSchedule::new(50, 50).unwrap(); // 版税5% 治理5%
        let s = split_for_settlement(&m, fees, 0).unwrap();
        assert_eq!(s.royalty_micro, 40);
        assert_eq!(s.governance_fee_micro, 40);
        assert_eq!(s.payout_provider_micro, 720); // 800-40-40
        assert_eq!(s.refund_buyer_micro, 200);
        assert_eq!(s.slashed_micro, 0);
        assert!(s.conserves(1000));
        assert_eq!(s.total().unwrap(), 1000);
    }

    #[test]
    fn fee_floor_division_keeps_remainder_with_provider() {
        // gross100，版税/治理各 33‰（3.3%）→ 各取整 3，余数留供给方。
        let m = ms(100, 100, 1);
        let fees = FeeSchedule::new(33, 33).unwrap();
        let s = split_for_settlement(&m, fees, 0).unwrap();
        assert_eq!(s.royalty_micro, 3);
        assert_eq!(s.governance_fee_micro, 3);
        assert_eq!(s.payout_provider_micro, 94); // 100-3-3
        assert_eq!(s.refund_buyer_micro, 0);
        assert!(s.conserves(100));
    }

    #[test]
    fn fee_rates_over_total_failclosed() {
        assert!(matches!(
            FeeSchedule::new(600, 500),
            Err(ResourceError::FeeRatesExceedTotal {
                sum_permyriad: 1100
            })
        ));
        // 恰好 1000 合法（供给方候选应得可为 0）。
        let m = ms(100, 100, 1);
        let s = split_for_settlement(&m, FeeSchedule::new(1000, 0).unwrap(), 0).unwrap();
        assert_eq!(s.payout_provider_micro, 0);
        assert_eq!(s.royalty_micro, 100);
        assert!(s.conserves(100));
    }

    #[test]
    fn full_consumption_no_refund_and_zero_consumption_full_refund() {
        // 全消耗：refund=0。
        let full = ms(500, 500, 2); // reserved1000, gross1000
        let s = split_for_settlement(&full, FeeSchedule::zero(), 0).unwrap();
        assert_eq!(s.refund_buyer_micro, 0);
        assert_eq!(s.payout_provider_micro, 1000);
        assert!(s.conserves(1000));
        // 零消耗：gross0、费0、payout0，全额退买方。
        let zero = ms(500, 0, 2); // reserved1000
        let s2 = split_for_settlement(&zero, FeeSchedule::new(100, 100).unwrap(), 0).unwrap();
        assert_eq!(s2.payout_provider_micro, 0);
        assert_eq!(s2.royalty_micro, 0);
        assert_eq!(s2.governance_fee_micro, 0);
        assert_eq!(s2.refund_buyer_micro, 1000);
        assert!(s2.conserves(1000));
    }

    #[test]
    fn slash_comes_out_of_provider_payout_and_remains_conserved() {
        // gross800、费0 → 候选应得 800；罚 300：payout500/slashed300，退款200 不动。
        let m = ms(1000, 800, 1);
        let s = split_for_settlement(&m, FeeSchedule::zero(), 300).unwrap();
        assert_eq!(s.payout_provider_micro, 500);
        assert_eq!(s.slashed_micro, 300);
        assert_eq!(s.refund_buyer_micro, 200);
        assert!(s.conserves(1000));
        // 罚没不能超过供给方候选应得（即便锁定额很大）。
        assert!(matches!(
            split_for_settlement(&m, FeeSchedule::zero(), 801),
            Err(ResourceError::SlashExceedsProviderPayout {
                attempted: 801,
                payout: 800
            })
        ));
        // 有费率时候选应得已扣费，罚没上限随之下降。
        let fees = FeeSchedule::new(50, 50).unwrap(); // 候选应得 720
        assert!(matches!(
            split_for_settlement(&m, fees, 721),
            Err(ResourceError::SlashExceedsProviderPayout {
                attempted: 721,
                payout: 720
            })
        ));
        let s2 = split_for_settlement(&m, fees, 720).unwrap();
        assert_eq!(s2.payout_provider_micro, 0);
        assert_eq!(s2.slashed_micro, 720);
        // 40+40+0+200+720=1000
        assert!(s2.conserves(1000));
    }

    #[test]
    fn ledger_lock_settle_lifecycle_and_named_errors() {
        let mut l = EscrowLedger::new();
        // 未知订单结算 → not found。
        assert!(matches!(
            l.settle("ord-x", &ms(10, 10, 1), FeeSchedule::zero(), 0),
            Err(ResourceError::EscrowOrderNotFound)
        ));
        // 空 id / 零额锁定拒绝。
        assert!(matches!(l.lock("  ", 10), Err(ResourceError::EmptyOrderId)));
        assert!(matches!(
            l.lock("ord-1", 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        l.lock("ord-1", 1000).unwrap();
        assert!(matches!(
            l.lock("ord-1", 500),
            Err(ResourceError::DuplicateEscrowOrder { .. })
        ));
        assert_eq!(l.get("ord-1").unwrap().state, EscrowState::Locked);
        assert_eq!(l.total_locked_open().unwrap(), 1000);
        // 锁定额与计量预留不一致 → mismatch，且仍停 Locked 不改写。
        let bad = ms(900, 900, 1); // reserved900 ≠ locked1000
        assert!(matches!(
            l.settle("ord-1", &bad, FeeSchedule::zero(), 0),
            Err(ResourceError::EscrowLockedReservedMismatch {
                locked: 1000,
                reserved: 900
            })
        ));
        assert_eq!(l.get("ord-1").unwrap().state, EscrowState::Locked);
        // 正确结算。
        let good = ms(1000, 800, 1);
        let sp = l
            .settle("ord-1", &good, FeeSchedule::new(50, 50).unwrap(), 100)
            .unwrap();
        assert_eq!(sp.payout_provider_micro, 620); // 候选720-罚100
        assert_eq!(sp.slashed_micro, 100);
        // 重复结算拒绝。
        assert!(matches!(
            l.settle("ord-1", &good, FeeSchedule::zero(), 0),
            Err(ResourceError::EscrowAlreadySettled { .. })
        ));
        assert_eq!(l.total_locked_open().unwrap(), 0);
        assert!(l.invariant_holds());
    }

    #[test]
    fn nonconserved_meter_view_rejected_at_boundary() {
        // 人为构造一个不满足 reserved=consumed_cost+refund 的视图（探针/上游被污染），
        // 结算边界必须 fail-closed 拒绝，绝不带着错账分钱。
        let bad = MeterSettlement {
            consumed: 800,
            allocated: 1000,
            consumed_cost_micro: 800,
            refund_micro: 200,
            reserved_cost_micro: 999, // 被篡改：应为 1000
        };
        assert!(matches!(
            split_for_settlement(&bad, FeeSchedule::zero(), 0),
            Err(ResourceError::EscrowSplitNotConserved)
        ));
    }

    #[test]
    fn multiple_orders_are_isolated_and_total_conservation_holds() {
        let mut l = EscrowLedger::new();
        l.lock("ord-a", 1000).unwrap();
        l.lock("ord-b", 2000).unwrap();
        let sa = l
            .settle(
                "ord-a",
                &ms(1000, 800, 1),
                FeeSchedule::new(50, 50).unwrap(),
                0,
            )
            .unwrap();
        // b 仍 Locked，a 已 Settled：互不串账。
        assert_eq!(l.get("ord-b").unwrap().state, EscrowState::Locked);
        assert_eq!(l.total_locked_open().unwrap(), 2000);
        let sb = l
            .settle("ord-b", &ms(1000, 1000, 2), FeeSchedule::zero(), 200)
            .unwrap();
        // b: reserved2000/gross2000/费0/候选2000/罚200 → payout1800 slashed200 refund0。
        assert_eq!(sb.payout_provider_micro, 1800);
        assert_eq!(sb.slashed_micro, 200);
        assert!(l.invariant_holds());
        // 两单分账各自等于各自锁定额。
        assert!(sa.conserves(1000));
        assert!(sb.conserves(2000));
        // 全表已结算五桶总和 = 1000+2000。
        let settled_sum: u128 = l
            .orders()
            .iter()
            .filter_map(|o| o.split)
            .map(|s| s.total().unwrap())
            .sum();
        assert_eq!(settled_sum, 3000);
        assert_eq!(l.total_locked_open().unwrap(), 0);
    }
}
