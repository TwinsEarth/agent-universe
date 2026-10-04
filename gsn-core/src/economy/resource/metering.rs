//! 计量账本（v3.8.2）。
//!
//! # 定位
//!
//! 订单进入 `Executing → Metering` 后，Agent 在沙盒里**真实消耗**资源：多少 CPU 毫秒、
//! 多少存储 GB·秒、多少次调用。计量账本把这些用量按 `(订单, 资源形态, 计量维度)`
//! **累计（append-only 正计量）**，并与撮合时已预留的配额 `allocated`（= 成交量
//! `filled_quantity`）对账，为 v3.8.5 托管结算守恒提供「实际用量 / 待退余量」两笔账。
//!
//! # 记账口径
//!
//! - 每条 [`UsageLine`] 记录某订单在某维度的 `allocated`（撮合预留上限，正）、
//!   `consumed`（截至目前累计已用量）、`unit_price_micro`（成交单价，micro/单位）。
//! - `record` 只接受**正增量**，且累计后 `consumed <= allocated`；不允许负计量、
//!   不允许冲减（防止把已用量"记回去"作弊/重复退款），超配额 fail-closed。
//! - 金额整数化：`consumed_cost = consumed × unit_price`、`refund =
//!   (allocated - consumed) × unit_price`、`reserved_cost = allocated × unit_price`，
//!   恒有 `reserved_cost = consumed_cost + refund`（托管守恒的计量侧依据，v3.8.5 用）。
//!
//! # 确定性与诚实边界
//!
//! - 纯内存、纯确定性：无浮点、无 syscall、无 unsafe、无 panic 路径；数量/金额 `u128`
//!   checked 运算，越界/超配额具名拒绝。
//! - 本账本**不持久化、不是全局单例、不接真实用量上报通道**：它是宿主/沙盒计量探针
//!   在每笔订单上持有的内存记账面，内核只提供可单测的确定性累计与对账规则。真实探针
//!   采集、跨节点用量聚合、按墙钟的计费窗口不在本版本。
//! - 本版只**算出**应付/待退两个整数视图，**不动任何资金、不托管、不连链**；资金
//!   守恒分账在 v3.8.5，BTC/ETH/稳定币结算与私钥隔离在 v3.9.x。

use serde::{Deserialize, Serialize};

use super::catalog::default_units;
use super::{MeterUnit, ResourceError, ResourceKind};

/// 一条计量线：某订单在某资源形态 + 计量维度下的预留上限与累计实耗。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageLine {
    /// 订单 id（非空，可追责）。
    pub order_id: String,
    /// 资源形态。
    pub kind: ResourceKind,
    /// 计量单位（必须属于该形态目录）。
    pub unit: MeterUnit,
    /// 撮合预留上限（= 成交量 filled_quantity，正）。累计实耗不得超过它。
    pub allocated: u128,
    /// 截至目前累计实耗（恒满足 `consumed <= allocated`，只增不减）。
    pub consumed: u128,
    /// 成交单价（micro/单位，正）。
    pub unit_price_micro: u128,
}

impl UsageLine {
    /// 构造一条 `consumed = 0` 的新计量线，并做入场校验。
    pub fn new(
        order_id: impl Into<String>,
        kind: ResourceKind,
        unit: MeterUnit,
        allocated: u128,
        unit_price_micro: u128,
    ) -> Result<Self, ResourceError> {
        let order_id = order_id.into();
        if order_id.trim().is_empty() {
            return Err(ResourceError::EmptyOrderId);
        }
        if allocated == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        if unit_price_micro == 0 {
            return Err(ResourceError::NonPositiveUnitPrice);
        }
        if !default_units(kind).contains(&unit) {
            return Err(ResourceError::UnitNotInCatalog { kind, unit });
        }
        Ok(Self {
            order_id,
            kind,
            unit,
            allocated,
            consumed: 0,
            unit_price_micro,
        })
    }

    /// 剩余可耗 = allocated - consumed（不变量成立时恒可减）。
    pub fn remaining(&self) -> Result<u128, ResourceError> {
        self.allocated
            .checked_sub(self.consumed)
            .ok_or(ResourceError::ArithmeticOverflow)
    }

    /// 实耗金额（micro）= consumed × 单价，checked。
    pub fn consumed_cost_micro(&self) -> Result<u128, ResourceError> {
        self.consumed
            .checked_mul(self.unit_price_micro)
            .ok_or(ResourceError::ArithmeticOverflow)
    }

    /// 待退金额（micro）= (allocated - consumed) × 单价，checked。
    ///
    /// 这是「实耗少于预留」时应退回消费方的量；实耗等于预留时为 0。
    pub fn refund_micro(&self) -> Result<u128, ResourceError> {
        self.remaining()?
            .checked_mul(self.unit_price_micro)
            .ok_or(ResourceError::ArithmeticOverflow)
    }

    /// 预留总额（micro）= allocated × 单价（即托管的名义额），checked。
    pub fn reserved_cost_micro(&self) -> Result<u128, ResourceError> {
        self.allocated
            .checked_mul(self.unit_price_micro)
            .ok_or(ResourceError::ArithmeticOverflow)
    }
}

/// 计量账本：以 `(order_id, kind, unit)` 为唯一键的有序内存账本。
///
/// 用有序 `Vec` 而非 `HashMap`，保证遍历/序列化顺序确定，便于单测与回放。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeteringLedger {
    lines: Vec<UsageLine>,
}

impl MeteringLedger {
    /// 空账本。
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// 计量线条目数。
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    fn index_of(&self, order_id: &str, kind: ResourceKind, unit: MeterUnit) -> Option<usize> {
        self.lines
            .iter()
            .position(|l| l.order_id == order_id && l.kind == kind && l.unit == unit)
    }

    /// 只读获取一条计量线。
    pub fn get(&self, order_id: &str, kind: ResourceKind, unit: MeterUnit) -> Option<&UsageLine> {
        self.index_of(order_id, kind, unit).map(|i| &self.lines[i])
    }

    /// 为某订单在某维度开一条计量线：校验 + 唯一性，初始 `consumed = 0`。
    pub fn open(
        &mut self,
        order_id: impl Into<String>,
        kind: ResourceKind,
        unit: MeterUnit,
        allocated: u128,
        unit_price_micro: u128,
    ) -> Result<(), ResourceError> {
        let line = UsageLine::new(order_id, kind, unit, allocated, unit_price_micro)?;
        if self
            .index_of(&line.order_id, line.kind, line.unit)
            .is_some()
        {
            return Err(ResourceError::DuplicateUsageLine {
                order_id: line.order_id,
                kind: line.kind,
                unit: line.unit,
            });
        }
        self.lines.push(line);
        Ok(())
    }

    /// 上报一次正用量增量并累计；返回累计后的实耗量。
    ///
    /// fail-closed：计量线不存在 / 增量为 0 / 累计后超过预留上限，一律具名拒绝；
    /// 不接受负增量（append-only，防冲减作弊）。
    pub fn record(
        &mut self,
        order_id: &str,
        kind: ResourceKind,
        unit: MeterUnit,
        amount: u128,
    ) -> Result<u128, ResourceError> {
        if amount == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        let i = self
            .index_of(order_id, kind, unit)
            .ok_or(ResourceError::UsageLineNotFound)?;
        let line = &mut self.lines[i];
        let new_consumed = line
            .consumed
            .checked_add(amount)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if new_consumed > line.allocated {
            return Err(ResourceError::UsageExceedsAllocation {
                consumed_after: new_consumed,
                allocated: line.allocated,
            });
        }
        line.consumed = new_consumed;
        Ok(line.consumed)
    }

    /// 某订单某维度当前累计实耗；计量线不存在具名拒绝（区别于实耗为 0）。
    pub fn consumed(
        &self,
        order_id: &str,
        kind: ResourceKind,
        unit: MeterUnit,
    ) -> Result<u128, ResourceError> {
        self.get(order_id, kind, unit)
            .map(|l| l.consumed)
            .ok_or(ResourceError::UsageLineNotFound)
    }

    /// 计算某计量线的结算视图：实耗量、预留量、实耗金额、待退金额、预留总额。
    ///
    /// 纯整数视图，不动资金；供 v3.8.5 托管守恒分账使用。
    pub fn settlement_view(
        &self,
        order_id: &str,
        kind: ResourceKind,
        unit: MeterUnit,
    ) -> Result<MeterSettlement, ResourceError> {
        let line = self
            .get(order_id, kind, unit)
            .ok_or(ResourceError::UsageLineNotFound)?;
        Ok(MeterSettlement {
            consumed: line.consumed,
            allocated: line.allocated,
            consumed_cost_micro: line.consumed_cost_micro()?,
            refund_micro: line.refund_micro()?,
            reserved_cost_micro: line.reserved_cost_micro()?,
        })
    }

    /// 某资源形态 + 维度下所有订单的累计实耗之和（跨订单聚合，checked 累加）。
    pub fn total_consumed(
        &self,
        kind: ResourceKind,
        unit: MeterUnit,
    ) -> Result<u128, ResourceError> {
        self.lines
            .iter()
            .filter(|l| l.kind == kind && l.unit == unit)
            .try_fold(0u128, |acc, l| {
                acc.checked_add(l.consumed)
                    .ok_or(ResourceError::ArithmeticOverflow)
            })
    }

    /// 遍历所有计量线（快照顺序）。
    pub fn lines(&self) -> &[UsageLine] {
        &self.lines
    }

    /// 全表不变量：每条线恒有 `consumed <= allocated`，且
    /// `reserved_cost = consumed_cost + refund`（计量侧托管守恒）。
    /// 供宿主在关键边界 fail-closed 自检；正常路径由 open/record 保证。
    pub fn invariant_holds(&self) -> bool {
        self.lines.iter().all(|l| {
            l.consumed <= l.allocated
                && match (
                    l.reserved_cost_micro(),
                    l.consumed_cost_micro(),
                    l.refund_micro(),
                ) {
                    (Ok(reserved), Ok(cost), Ok(refund)) => {
                        cost.checked_add(refund) == Some(reserved)
                    }
                    _ => false,
                }
        })
    }
}

/// 计量线结算视图（整数，单位 micro）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterSettlement {
    /// 累计实耗量。
    pub consumed: u128,
    /// 撮合预留量（上限）。
    pub allocated: u128,
    /// 实耗金额 = consumed × 单价。
    pub consumed_cost_micro: u128,
    /// 待退金额 = (allocated - consumed) × 单价。
    pub refund_micro: u128,
    /// 预留总额 = allocated × 单价。
    pub reserved_cost_micro: u128,
}

#[cfg(test)]
mod tests {
    use super::*;

    const O: &str = "order-1";

    fn open_compute(ledger: &mut MeteringLedger, order: &str, allocated: u128, price: u128) {
        ledger
            .open(
                order,
                ResourceKind::Compute,
                MeterUnit::CpuMillis,
                allocated,
                price,
            )
            .unwrap();
    }

    #[test]
    fn open_valid_starts_zero_consumed_full_remaining() {
        let mut ledger = MeteringLedger::new();
        assert!(ledger.is_empty());
        open_compute(&mut ledger, O, 1000, 2);
        assert_eq!(ledger.len(), 1);
        let line = ledger
            .get(O, ResourceKind::Compute, MeterUnit::CpuMillis)
            .unwrap();
        assert_eq!(line.consumed, 0);
        assert_eq!(line.allocated, 1000);
        assert_eq!(line.remaining().unwrap(), 1000);
        assert!(ledger.invariant_holds());
    }

    #[test]
    fn open_rejects_empty_order_zero_allocation_zero_price_foreign_unit() {
        let mut ledger = MeteringLedger::new();
        assert!(matches!(
            ledger.open("  ", ResourceKind::Compute, MeterUnit::CpuMillis, 10, 2),
            Err(ResourceError::EmptyOrderId)
        ));
        assert!(matches!(
            ledger.open(O, ResourceKind::Compute, MeterUnit::CpuMillis, 0, 2),
            Err(ResourceError::NonPositiveQuantity)
        ));
        assert!(matches!(
            ledger.open(O, ResourceKind::Compute, MeterUnit::CpuMillis, 10, 0),
            Err(ResourceError::NonPositiveUnitPrice)
        ));
        // Compute 不接受 StorageGbSec。
        assert!(matches!(
            ledger.open(O, ResourceKind::Compute, MeterUnit::StorageGbSec, 10, 2),
            Err(ResourceError::UnitNotInCatalog { .. })
        ));
    }

    #[test]
    fn duplicate_open_rejected_distinct_units_and_orders_coexist() {
        let mut ledger = MeteringLedger::new();
        open_compute(&mut ledger, O, 1000, 2);
        // 同键重复开线拒绝。
        assert!(matches!(
            ledger.open(O, ResourceKind::Compute, MeterUnit::CpuMillis, 2000, 2),
            Err(ResourceError::DuplicateUsageLine { .. })
        ));
        // 同订单不同维度可共存。
        ledger
            .open(O, ResourceKind::Compute, MeterUnit::GpuMillis, 500, 3)
            .unwrap();
        // 不同订单同维度可共存。
        open_compute(&mut ledger, "order-2", 800, 2);
        assert_eq!(ledger.len(), 3);
    }

    #[test]
    fn record_accumulates_and_never_exceeds_allocation() {
        let mut ledger = MeteringLedger::new();
        open_compute(&mut ledger, O, 1000, 2);
        assert_eq!(
            ledger
                .record(O, ResourceKind::Compute, MeterUnit::CpuMillis, 600)
                .unwrap(),
            600
        );
        assert_eq!(
            ledger
                .record(O, ResourceKind::Compute, MeterUnit::CpuMillis, 300)
                .unwrap(),
            900
        );
        assert_eq!(
            ledger
                .get(O, ResourceKind::Compute, MeterUnit::CpuMillis)
                .unwrap()
                .remaining()
                .unwrap(),
            100
        );
        // 再耗 200 会越过上限 1000，fail-closed 拒绝，且账本不被改写。
        assert!(matches!(
            ledger.record(O, ResourceKind::Compute, MeterUnit::CpuMillis, 200),
            Err(ResourceError::UsageExceedsAllocation {
                consumed_after: 1100,
                allocated: 1000
            })
        ));
        assert_eq!(
            ledger
                .consumed(O, ResourceKind::Compute, MeterUnit::CpuMillis)
                .unwrap(),
            900
        );
        // 恰好到上限允许。
        assert_eq!(
            ledger
                .record(O, ResourceKind::Compute, MeterUnit::CpuMillis, 100)
                .unwrap(),
            1000
        );
        assert!(ledger.invariant_holds());
    }

    #[test]
    fn record_zero_and_unknown_line_rejected() {
        let mut ledger = MeteringLedger::new();
        // 未开线直接计量：NotFound（区别于实耗 0）。
        assert!(matches!(
            ledger.record(O, ResourceKind::Compute, MeterUnit::CpuMillis, 1),
            Err(ResourceError::UsageLineNotFound)
        ));
        open_compute(&mut ledger, O, 1000, 2);
        // 零增量拒绝（不接受负/零计量）。
        assert!(matches!(
            ledger.record(O, ResourceKind::Compute, MeterUnit::CpuMillis, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        // 查询未知线具名拒绝。
        assert!(matches!(
            ledger.consumed("nope", ResourceKind::Compute, MeterUnit::CpuMillis),
            Err(ResourceError::UsageLineNotFound)
        ));
    }

    #[test]
    fn settlement_view_conserves_reserved_equals_cost_plus_refund() {
        let mut ledger = MeteringLedger::new();
        // allocated 1000，单价 2 micro；预留总额 2000。
        open_compute(&mut ledger, O, 1000, 2);
        ledger
            .record(O, ResourceKind::Compute, MeterUnit::CpuMillis, 700)
            .unwrap();
        let s = ledger
            .settlement_view(O, ResourceKind::Compute, MeterUnit::CpuMillis)
            .unwrap();
        assert_eq!(s.consumed, 700);
        assert_eq!(s.allocated, 1000);
        assert_eq!(s.consumed_cost_micro, 1400); // 700 × 2
        assert_eq!(s.refund_micro, 600); // 300 × 2
        assert_eq!(s.reserved_cost_micro, 2000); // 1000 × 2
                                                 // 守恒：预留 = 实耗 + 待退。
        assert_eq!(
            s.consumed_cost_micro + s.refund_micro,
            s.reserved_cost_micro
        );
        // 全耗尽时待退为 0。
        ledger
            .record(O, ResourceKind::Compute, MeterUnit::CpuMillis, 300)
            .unwrap();
        let s2 = ledger
            .settlement_view(O, ResourceKind::Compute, MeterUnit::CpuMillis)
            .unwrap();
        assert_eq!(s2.refund_micro, 0);
        assert_eq!(s2.consumed_cost_micro, s2.reserved_cost_micro);
        assert!(ledger.invariant_holds());
    }

    #[test]
    fn unknown_settlement_view_rejected() {
        let ledger = MeteringLedger::new();
        assert!(matches!(
            ledger.settlement_view(O, ResourceKind::Compute, MeterUnit::CpuMillis),
            Err(ResourceError::UsageLineNotFound)
        ));
    }

    #[test]
    fn total_consumed_aggregates_only_matching_kind_and_unit() {
        let mut ledger = MeteringLedger::new();
        open_compute(&mut ledger, "o1", 1000, 2);
        open_compute(&mut ledger, "o2", 1000, 2);
        ledger
            .open("o1", ResourceKind::Compute, MeterUnit::GpuMillis, 500, 3)
            .unwrap();
        ledger
            .record("o1", ResourceKind::Compute, MeterUnit::CpuMillis, 400)
            .unwrap();
        ledger
            .record("o2", ResourceKind::Compute, MeterUnit::CpuMillis, 250)
            .unwrap();
        ledger
            .record("o1", ResourceKind::Compute, MeterUnit::GpuMillis, 100)
            .unwrap();
        // CPU 跨两订单聚合 650；GPU 仅 100，不混入。
        assert_eq!(
            ledger
                .total_consumed(ResourceKind::Compute, MeterUnit::CpuMillis)
                .unwrap(),
            650
        );
        assert_eq!(
            ledger
                .total_consumed(ResourceKind::Compute, MeterUnit::GpuMillis)
                .unwrap(),
            100
        );
        // 无任何线条目的维度聚合为 0。
        assert_eq!(
            ledger
                .total_consumed(ResourceKind::Storage, MeterUnit::StorageGbSec)
                .unwrap(),
            0
        );
        assert!(ledger.invariant_holds());
    }
}
