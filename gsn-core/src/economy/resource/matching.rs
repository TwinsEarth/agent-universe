//! 撮合编排器（v3.8.3）。
//!
//! # 定位
//!
//! v3.8.0 给出了撮合入场纯函数 [`ResourceOrder::match_offer_ask`]（形态/单位/限价/数量
//! 校验，建单即 `Matched`），v3.8.1 给出了容量账本（register/hold/release），
//! v3.8.2 给出了计量账本（open/record/结算视图）。但三者此前**互不接线**：成交后不会真的
//! 预留供给方容量，取消/结算也不会归还，执行期更不会自动开计量线。
//!
//! 本模块用 [`MatchingEngine`] 把三张账按**订单状态机的边**确定性地编排起来：
//!
//! ```text
//! submit（挂单容量闸门 check_offer）
//!   -> Matched
//! hold_capacity：Matched -> CapacityHeld   （registry.hold(filled)，防超卖）
//! begin_execution：CapacityHeld -> Executing
//! begin_metering：Executing -> Metering     （ledger.open(allocated=filled, 成交价)）
//! report_usage：仅 Metering                 （ledger.record 正计量，consumed<=allocated）
//! to_qa：Metering -> QaPending
//! settle：QaPending -> Settled              （registry.release(filled)，容量归还）
//! dispute：QaPending -> Disputed
//! adjudicate_settle：Disputed -> Settled    （release）
//! adjudicate_slash：Disputed -> Slashed     （release；资金罚没在 v3.8.4/3.8.5）
//! cancel：Matched（未 hold，不释放）| CapacityHeld（release）-> Cancelled
//! ```
//!
//! # 守恒：一次 hold 恰好一次 release
//!
//! - 容量只在 `Matched -> CapacityHeld` 这一条边上 hold（量 = 成交量 `filled_quantity`）。
//! - 容量只在「持有容量期间进入终态」时 release：正常 `Settled`、仲裁 `Settled/Slashed`、
//!   以及从 `CapacityHeld` 的 `Cancelled`；从 `Matched` 取消时尚未 hold，不释放。
//! - 因此对任一供给方键 `(provider, kind, unit)` 恒有：
//!   `capacity.held == Σ 处于「已 hold 未终态」订单的 filled_quantity`，
//!   见 [`MatchingEngine::invariant_holds`]。
//!
//! # 失败不改写（fail-closed 补偿）
//!
//! 每个副作用都先做**可能失败的记账动作**（hold/open/release/record），成功后才推进订单
//! 状态。底层账本在出错路径自身不改写（hold 超量/record 超配额/open 重复都在赋值前拒绝），
//! 故一旦某条边被具名拒绝，订单停在原态、容量与计量账也不被污染，可安全重试或改道取消。
//!
//! # 确定性与诚实边界
//!
//! - 纯内存、纯确定性编排：无浮点、无 syscall、无 unsafe、无 panic 路径；数量/金额 `u128`。
//! - 本引擎**不动资金、不托管、不连链、不持久化、不做跨节点供给发现/网络撮合**，也**不新增
//!   对外写能力**：它是宿主/T1 市场门面持有的内存领域编排面，撮合结果仍由门面在后续小版本
//!   经 PMB 受理，结算资金守恒在 v3.8.5，BTC/ETH/稳定币在 v3.9.x。

use super::capacity::CapacityRegistry;
use super::metering::MeteringLedger;
use super::{
    MeterUnit, OrderState, ResourceAsk, ResourceError, ResourceKind, ResourceOffer, ResourceOrder,
};

/// 撮合编排器：持有订单集、容量账本与计量账本，按状态机边驱动三者联动。
#[derive(Debug, Clone, Default)]
pub struct MatchingEngine {
    /// 已成交订单（提交即 `Matched`），有序 Vec 保证回放/序列化顺序确定。
    orders: Vec<ResourceOrder>,
    /// 供给方注册容量账本（v3.8.1）。
    capacity: CapacityRegistry,
    /// 订单计量账本（v3.8.2）。
    metering: MeteringLedger,
}

impl MatchingEngine {
    /// 空引擎（空容量 + 空计量账本）。
    pub fn new() -> Self {
        Self::with(CapacityRegistry::new(), MeteringLedger::new())
    }

    /// 以既有容量/计量账本装配（宿主门面可复用同一对账本）。
    pub fn with(capacity: CapacityRegistry, metering: MeteringLedger) -> Self {
        Self {
            orders: Vec::new(),
            capacity,
            metering,
        }
    }

    /// 容量账本只读引用。
    pub fn capacity(&self) -> &CapacityRegistry {
        &self.capacity
    }

    /// 计量账本只读引用。
    pub fn metering(&self) -> &MeteringLedger {
        &self.metering
    }

    /// 已成交订单数。
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    /// 遍历订单（提交顺序）。
    pub fn orders(&self) -> &[ResourceOrder] {
        &self.orders
    }

    fn index_of(&self, order_id: &str) -> Result<usize, ResourceError> {
        self.orders
            .iter()
            .position(|o| o.order_id == order_id)
            .ok_or(ResourceError::OrderNotFound)
    }

    /// 只读获取订单。
    pub fn order(&self, order_id: &str) -> Result<&ResourceOrder, ResourceError> {
        self.index_of(order_id).map(|i| &self.orders[i])
    }

    /// 某订单当前状态；未知订单具名拒绝。
    pub fn state(&self, order_id: &str) -> Result<OrderState, ResourceError> {
        Ok(self.order(order_id)?.state)
    }

    /// 「已 hold、尚未进入终态」的状态集合：这些状态的订单正占着供给方容量。
    fn holds_capacity(state: OrderState) -> bool {
        matches!(
            state,
            OrderState::CapacityHeld
                | OrderState::Executing
                | OrderState::Metering
                | OrderState::QaPending
                | OrderState::Disputed
        )
    }

    /// 提交一笔成交（自带显式订单 id）：先过挂单容量闸门，再用撮合入场函数建单
    /// （`Matched`）。订单 id 重复具名拒绝；本步**不**预留容量（预留发生在
    /// [`Self::hold_capacity`]）。
    pub fn submit_named(
        &mut self,
        order_id: impl Into<String>,
        offer: &ResourceOffer,
        ask: &ResourceAsk,
    ) -> Result<String, ResourceError> {
        // 挂单容量闸门（fail-closed）：供给方必须已注册容量，且挂单量不超当前可售。
        self.capacity.check_offer(offer)?;
        let order = ResourceOrder::match_offer_ask(order_id, offer, ask)?;
        if self.orders.iter().any(|o| o.order_id == order.order_id) {
            return Err(ResourceError::DuplicateOrder {
                order_id: order.order_id,
            });
        }
        let id = order.order_id.clone();
        self.orders.push(order);
        Ok(id)
    }

    /// 提交一笔成交并由引擎分配确定性 id `ord-{n}`（n 从 1 起，单调不重用）。
    pub fn submit(
        &mut self,
        offer: &ResourceOffer,
        ask: &ResourceAsk,
    ) -> Result<String, ResourceError> {
        let id = format!("ord-{}", self.orders.len() + 1);
        self.submit_named(id, offer, ask)
    }

    /// 撮合预留：`Matched -> CapacityHeld`，对供给方按成交量 hold 容量。
    ///
    /// hold 失败（容量被先到订单占满等）具名拒绝且**不改写**：订单停在 `Matched`。
    pub fn hold_capacity(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        let state = self.orders[i].state;
        if state != OrderState::Matched {
            return Err(ResourceError::InvalidTransition {
                from: state,
                to: OrderState::CapacityHeld,
            });
        }
        let (provider, kind, unit, filled) = Self::order_key(&self.orders[i]);
        // 先做可能失败的 hold，成功后再推进状态。
        self.capacity.hold(&provider, kind, unit, filled)?;
        self.orders[i].advance(OrderState::CapacityHeld)
    }

    /// `CapacityHeld -> Executing`：任务进入沙盒执行（本版本不产生额外记账副作用）。
    pub fn begin_execution(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        if self.orders[i].state != OrderState::CapacityHeld {
            return Err(ResourceError::InvalidTransition {
                from: self.orders[i].state,
                to: OrderState::Executing,
            });
        }
        self.orders[i].advance(OrderState::Executing)
    }

    /// `Executing -> Metering`，同时为订单开计量线（allocated=成交量、单价=成交价）。
    ///
    /// 开线失败具名拒绝且订单停在 `Executing`（计量账不被污染）。
    pub fn begin_metering(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        let state = self.orders[i].state;
        if state != OrderState::Executing {
            return Err(ResourceError::InvalidTransition {
                from: state,
                to: OrderState::Metering,
            });
        }
        let (_provider, kind, unit, filled) = Self::order_key(&self.orders[i]);
        let price = self.orders[i].fill_unit_price_micro;
        let id = self.orders[i].order_id.clone();
        self.metering.open(id, kind, unit, filled, price)?;
        self.orders[i].advance(OrderState::Metering)
    }

    /// 上报一次正用量；仅 `Metering` 态允许，累计不得超预留（底层 fail-closed）。
    pub fn report_usage(&mut self, order_id: &str, amount: u128) -> Result<u128, ResourceError> {
        let i = self.index_of(order_id)?;
        if self.orders[i].state != OrderState::Metering {
            return Err(ResourceError::InvalidTransition {
                from: self.orders[i].state,
                to: OrderState::Metering,
            });
        }
        let (_provider, kind, unit, _filled) = Self::order_key(&self.orders[i]);
        let id = self.orders[i].order_id.clone();
        self.metering.record(&id, kind, unit, amount)
    }

    /// `Metering -> QaPending`：结果交验证（BFT-lite QA 在 v3.8.8）。
    pub fn to_qa(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        if self.orders[i].state != OrderState::Metering {
            return Err(ResourceError::InvalidTransition {
                from: self.orders[i].state,
                to: OrderState::QaPending,
            });
        }
        self.orders[i].advance(OrderState::QaPending)
    }

    /// `QaPending -> Settled`：验证通过，归还撮合预留容量。
    pub fn settle(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        if self.orders[i].state != OrderState::QaPending {
            return Err(ResourceError::InvalidTransition {
                from: self.orders[i].state,
                to: OrderState::Settled,
            });
        }
        let (provider, kind, unit, filled) = Self::order_key(&self.orders[i]);
        self.capacity.release(&provider, kind, unit, filled)?;
        self.orders[i].advance(OrderState::Settled)
    }

    /// `QaPending -> Disputed`：验证失败进入仲裁（容量继续持有，不释放）。
    pub fn dispute(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        if self.orders[i].state != OrderState::QaPending {
            return Err(ResourceError::InvalidTransition {
                from: self.orders[i].state,
                to: OrderState::Disputed,
            });
        }
        self.orders[i].advance(OrderState::Disputed)
    }

    /// `Disputed -> Settled`：仲裁认定履约，归还容量。
    pub fn adjudicate_settle(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        if self.orders[i].state != OrderState::Disputed {
            return Err(ResourceError::InvalidTransition {
                from: self.orders[i].state,
                to: OrderState::Settled,
            });
        }
        let (provider, kind, unit, filled) = Self::order_key(&self.orders[i]);
        self.capacity.release(&provider, kind, unit, filled)?;
        self.orders[i].advance(OrderState::Settled)
    }

    /// `Disputed -> Slashed`：仲裁判违规，归还撮合预留容量（资金罚没在 v3.8.4/3.8.5）。
    pub fn adjudicate_slash(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        if self.orders[i].state != OrderState::Disputed {
            return Err(ResourceError::InvalidTransition {
                from: self.orders[i].state,
                to: OrderState::Slashed,
            });
        }
        let (provider, kind, unit, filled) = Self::order_key(&self.orders[i]);
        self.capacity.release(&provider, kind, unit, filled)?;
        self.orders[i].advance(OrderState::Slashed)
    }

    /// 取消订单：仅 `Matched`（尚未 hold，不释放）或 `CapacityHeld`（释放成交量）合法；
    /// 状态机本身拒绝从执行期及以后取消。
    pub fn cancel(&mut self, order_id: &str) -> Result<OrderState, ResourceError> {
        let i = self.index_of(order_id)?;
        let state = self.orders[i].state;
        if state != OrderState::Matched && state != OrderState::CapacityHeld {
            return Err(ResourceError::InvalidTransition {
                from: state,
                to: OrderState::Cancelled,
            });
        }
        if state == OrderState::CapacityHeld {
            let (provider, kind, unit, filled) = Self::order_key(&self.orders[i]);
            self.capacity.release(&provider, kind, unit, filled)?;
        }
        self.orders[i].advance(OrderState::Cancelled)
    }

    /// 取出订单的容量键与成交量（provider 克隆，其余 Copy）。
    fn order_key(order: &ResourceOrder) -> (String, ResourceKind, MeterUnit, u128) {
        (
            order.offer.provider_did.clone(),
            order.offer.kind,
            order.offer.unit,
            order.filled_quantity,
        )
    }

    /// 全引擎守恒不变量：
    /// 1. 容量账本自身 `held <= capacity`；2. 计量账本自身守恒；
    /// 3. **跨账守恒**：每条注册的 held 恰好等于「已 hold 未终态」同键订单成交量之和。
    pub fn invariant_holds(&self) -> bool {
        if !self.capacity.invariant_holds() || !self.metering.invariant_holds() {
            return false;
        }
        self.capacity.registrations().iter().all(|reg| {
            let expected: Option<u128> = self
                .orders
                .iter()
                .filter(|o| {
                    o.offer.provider_did == reg.provider_did
                        && o.offer.kind == reg.kind
                        && o.offer.unit == reg.unit
                        && Self::holds_capacity(o.state)
                })
                .map(|o| o.filled_quantity)
                .try_fold(0u128, |acc, q| acc.checked_add(q));
            expected == Some(reg.held)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "did:nau:provider";

    fn offer(qty: u128) -> ResourceOffer {
        ResourceOffer::new(P, ResourceKind::Compute, MeterUnit::CpuMillis, qty, 2, None).unwrap()
    }

    fn ask(qty: u128) -> ResourceAsk {
        ResourceAsk::new(
            "did:nau:buyer",
            ResourceKind::Compute,
            MeterUnit::CpuMillis,
            qty,
            5,
            true,
        )
        .unwrap()
    }

    fn engine_with_capacity(cap: u128) -> MatchingEngine {
        let mut capacity = CapacityRegistry::new();
        capacity
            .register(P, ResourceKind::Compute, MeterUnit::CpuMillis, cap, 0)
            .unwrap();
        MatchingEngine::with(capacity, MeteringLedger::new())
    }

    fn available(eng: &MatchingEngine) -> u128 {
        eng.capacity()
            .available(P, ResourceKind::Compute, MeterUnit::CpuMillis)
            .unwrap()
    }

    #[test]
    fn happy_path_holds_meters_releases_and_conserves() {
        let mut eng = engine_with_capacity(1000);
        let id = eng.submit(&offer(1000), &ask(300)).unwrap();
        assert_eq!(eng.state(&id).unwrap(), OrderState::Matched);
        assert_eq!(available(&eng), 1000); // Matched 尚未 hold。

        eng.hold_capacity(&id).unwrap();
        assert_eq!(eng.state(&id).unwrap(), OrderState::CapacityHeld);
        assert_eq!(available(&eng), 700); // hold 300。

        eng.begin_execution(&id).unwrap();
        eng.begin_metering(&id).unwrap();
        assert_eq!(eng.report_usage(&id, 250).unwrap(), 250);
        assert_eq!(eng.report_usage(&id, 40).unwrap(), 290); // 剩 10。
        eng.to_qa(&id).unwrap();
        eng.settle(&id).unwrap();
        assert_eq!(eng.state(&id).unwrap(), OrderState::Settled);
        assert_eq!(available(&eng), 1000); // 结算后容量归还。

        let view = eng
            .metering()
            .settlement_view(&id, ResourceKind::Compute, MeterUnit::CpuMillis)
            .unwrap();
        assert_eq!(view.consumed, 290);
        assert_eq!(view.allocated, 300);
        assert_eq!(view.refund_micro, 20); // 10 × 2
        assert!(eng.invariant_holds());
    }

    #[test]
    fn hold_is_failclosed_when_peer_consumes_capacity_first() {
        let mut eng = engine_with_capacity(1000);
        // 两笔都在容量尚满时提交（提交只校验挂单量 ≤ 当时可售），均停在 Matched。
        let a = eng.submit(&offer(1000), &ask(950)).unwrap();
        let b = eng.submit(&offer(100), &ask(100)).unwrap();
        // A 先 hold 950，仅剩 50。
        eng.hold_capacity(&a).unwrap();
        assert_eq!(available(&eng), 50);
        // B 再 hold 100 超量：具名拒绝，订单仍 Matched、容量账不改写。
        assert!(matches!(
            eng.hold_capacity(&b),
            Err(ResourceError::CapacityExceeded {
                hold_requested: 100,
                available: 50
            })
        ));
        assert_eq!(eng.state(&b).unwrap(), OrderState::Matched);
        assert_eq!(available(&eng), 50);
        // B 改道取消（未 hold，不释放）。
        assert_eq!(eng.cancel(&b).unwrap(), OrderState::Cancelled);
        assert_eq!(available(&eng), 50);
        assert!(eng.invariant_holds());
    }

    #[test]
    fn cancel_at_matched_no_release_and_at_capacity_held_releases() {
        let mut eng = engine_with_capacity(1000);
        let m = eng.submit(&offer(1000), &ask(200)).unwrap();
        // Matched 取消：未 hold，容量不变。
        assert_eq!(eng.cancel(&m).unwrap(), OrderState::Cancelled);
        assert_eq!(available(&eng), 1000);

        let h = eng.submit(&offer(800), &ask(300)).unwrap();
        eng.hold_capacity(&h).unwrap();
        assert_eq!(available(&eng), 700);
        assert_eq!(eng.cancel(&h).unwrap(), OrderState::Cancelled);
        assert_eq!(available(&eng), 1000); // 释放归还。

        // 进入执行后状态机拒绝取消。
        let e = eng.submit(&offer(500), &ask(100)).unwrap();
        eng.hold_capacity(&e).unwrap();
        eng.begin_execution(&e).unwrap();
        assert!(matches!(
            eng.cancel(&e),
            Err(ResourceError::InvalidTransition { .. })
        ));
        assert!(eng.invariant_holds());
    }

    #[test]
    fn metering_opens_only_in_metering_and_usage_is_bounded() {
        let mut eng = engine_with_capacity(1000);
        let id = eng.submit(&offer(1000), &ask(100)).unwrap();
        eng.hold_capacity(&id).unwrap();
        eng.begin_execution(&id).unwrap();
        // 执行期还没开计量线：上报用量被状态机拒绝。
        assert!(matches!(
            eng.report_usage(&id, 10),
            Err(ResourceError::InvalidTransition { .. })
        ));
        eng.begin_metering(&id).unwrap();
        // 超预留（allocated=100）fail-closed，状态/账本不改写。
        assert!(matches!(
            eng.report_usage(&id, 120),
            Err(ResourceError::UsageExceedsAllocation {
                consumed_after: 120,
                allocated: 100
            })
        ));
        assert_eq!(eng.report_usage(&id, 100).unwrap(), 100); // 恰好到顶。
        assert!(eng.invariant_holds());
    }

    #[test]
    fn dispute_slash_and_settle_both_release_capacity() {
        let mut eng = engine_with_capacity(1000);
        let slashed = eng.submit(&offer(1000), &ask(300)).unwrap();
        eng.hold_capacity(&slashed).unwrap();
        eng.begin_execution(&slashed).unwrap();
        eng.begin_metering(&slashed).unwrap();
        eng.report_usage(&slashed, 80).unwrap();
        eng.to_qa(&slashed).unwrap();
        eng.dispute(&slashed).unwrap();
        assert_eq!(available(&eng), 700); // 争议期间容量继续持有。
        eng.adjudicate_slash(&slashed).unwrap();
        assert_eq!(eng.state(&slashed).unwrap(), OrderState::Slashed);
        assert_eq!(available(&eng), 1000); // 判罚后容量同样归还。

        let settled = eng.submit(&offer(700), &ask(200)).unwrap();
        eng.hold_capacity(&settled).unwrap();
        eng.begin_execution(&settled).unwrap();
        eng.begin_metering(&settled).unwrap();
        eng.to_qa(&settled).unwrap();
        eng.dispute(&settled).unwrap();
        eng.adjudicate_settle(&settled).unwrap();
        assert_eq!(eng.state(&settled).unwrap(), OrderState::Settled);
        assert_eq!(available(&eng), 1000);
        assert!(eng.invariant_holds());
    }

    #[test]
    fn submit_enforces_listing_gate_and_unique_ids() {
        let mut eng = MatchingEngine::new(); // 无任何注册容量。
                                             // 未注册容量挂单：fail-closed。
        assert!(matches!(
            eng.submit(&offer(10), &ask(10)),
            Err(ResourceError::RegistrationNotFound)
        ));

        let mut capacity = CapacityRegistry::new();
        capacity
            .register(P, ResourceKind::Compute, MeterUnit::CpuMillis, 100, 0)
            .unwrap();
        let eng = std::cell::RefCell::new(MatchingEngine::with(capacity, MeteringLedger::new()));
        // 挂单量超注册可售。
        assert!(matches!(
            eng.borrow_mut().submit(&offer(500), &ask(500)),
            Err(ResourceError::OfferExceedsRegisteredCapacity {
                offer_quantity: 500,
                available: 100
            })
        ));
        eng.borrow_mut()
            .submit_named("x", &offer(100), &ask(50))
            .unwrap();
        // 同 id 重复提交拒绝。
        assert!(matches!(
            eng.borrow_mut().submit_named("x", &offer(50), &ask(20)),
            Err(ResourceError::DuplicateOrder { order_id }) if order_id == "x"
        ));
        assert!(eng.borrow().invariant_holds());
    }

    #[test]
    fn unknown_order_operations_named_not_found() {
        let mut eng = engine_with_capacity(1000);
        assert!(matches!(
            eng.order("nope"),
            Err(ResourceError::OrderNotFound)
        ));
        assert!(matches!(
            eng.hold_capacity("nope"),
            Err(ResourceError::OrderNotFound)
        ));
        assert!(matches!(
            eng.settle("nope"),
            Err(ResourceError::OrderNotFound)
        ));
    }

    #[test]
    fn released_capacity_is_reusable_by_next_order() {
        let mut eng = engine_with_capacity(1000);
        let first = eng.submit(&offer(1000), &ask(1000)).unwrap();
        eng.hold_capacity(&first).unwrap();
        assert_eq!(available(&eng), 0);
        // 容量占满期间，新挂单过不了闸门。
        assert!(matches!(
            eng.submit(&offer(10), &ask(10)),
            Err(ResourceError::OfferExceedsRegisteredCapacity { available: 0, .. })
        ));
        eng.begin_execution(&first).unwrap();
        eng.begin_metering(&first).unwrap();
        eng.report_usage(&first, 1000).unwrap();
        eng.to_qa(&first).unwrap();
        eng.settle(&first).unwrap();
        assert_eq!(available(&eng), 1000); // 全部归还。
                                           // 归还后新订单可重新吃满容量。
        let second = eng.submit(&offer(1000), &ask(1000)).unwrap();
        eng.hold_capacity(&second).unwrap();
        assert_eq!(available(&eng), 0);
        assert_eq!(eng.state(&second).unwrap(), OrderState::CapacityHeld);
        assert!(eng.invariant_holds());
    }
}
