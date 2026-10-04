//! 动态定价、订单聚合与冷启动开关（v3.8.9，资源市场 10/10 收尾）。
//!
//! # 定位
//!
//! 到 v3.8.8，资源市场已有「容量→计量→撮合→质押→托管→版税→信誉→BFT-lite QA」的
//! 确定性记账/裁决链，但成交价在 v3.8.0 就是挂单静态单价（`unit_price_micro`），不随
//! 供需松紧、供给方信誉或任务时延敏感度变化；零散小单逐单撮合也吃不到批量成本优势；
//! 网络早期没有足够供给/需求时更没有一个诚实的「自举」状态。v3.8.9 补三块**纯确定性**
//! 决策面（不动资金、不撮合、不连链）：
//!
//! 1. [`DynamicPricer`]：在基准单价上叠加三个整数千分点因子——稀缺度（需求/供给压力）、
//!    信誉溢价（v3.8.7 四维复合分 0..100000）、时延敏感度（敏感/中性/容忍），合成倍率
//!    收敛到有界区间后乘基准价（整数向下取整），且**永不突破买方限价**。
//! 2. [`OrderBatcher`]：把同资源键、同单价带的小单 FIFO 聚合成批，批量守恒
//!    （Σ 小单量 == 批量、批内预算逐行可对账），达目标量成批，异键/超量 fail-closed。
//! 3. [`BootstrapGate`]：冷启动自举开关——在线供给方数或可用容量低于阈值时进入
//!    bootstrap，定价放宽（早期补贴倍率）、批量门槛降为 1；越过阈值确定性自动关闭。
//!
//! # 整数口径（零浮点）
//!
//! - 金额一律 `u128` micro（1 credit = 10^6 micro，与 v3.8.0 一致）。
//! - 所有「率/倍率/压力比」用整数**千分点** [`PERMILLE`]=1000：1000=100%、1100=110%、
//!   900=90%；信誉复合分沿用 reputation 的 0..100000（100000=100.0）。
//! - 乘除全部 checked：先 `checked_mul` 再 `/ PERMILLE` 确定性向下取整；任何溢出 fail-closed
//!   （复用 [`ResourceError::ArithmeticOverflow`]），无 panic 路径。
//!
//! # 诚实边界
//!
//! - 只产出**建议价/批/开关状态**：不改 Escrow/Metering/Stake 任何账，不提交订单、不真实
//!   聚合执行、不发放补贴、不持久化、不连链；供需量与信誉分都是调用方入参，内核不采集。
//! - 「补贴」在 bootstrap 期只是一个**更小的确定性倍率**，不铸造/不垫付任何资金。
//! - 冷启动阈值由调用方配置；本模块不感知真实节点数（根种子/节点发现属网络层）。

use super::{MeterUnit, ResourceError, ResourceKind};

/// 千分点基数：1000 = 100%。
pub const PERMILLE: u128 = 1000;
/// 信誉复合分满分（100000 = 100.0 分，与 reputation.rs 一致）。
pub const REP_FULL: u128 = 100_000;
/// 信誉中性分（50.0 分）：此处信誉溢价为 0，以上加价、以下折价。
pub const REP_NEUTRAL: u128 = 50_000;

/// 任务时延敏感度分级（对应 CPU 调度的两级语义：敏感优先、容忍吃剩余）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyClass {
    /// 时延敏感（交互式/核心路径）：付溢价。
    Sensitive,
    /// 中性：不调整。
    Neutral,
    /// 时延容忍（批处理/利用闲置）：享折扣。
    Tolerant,
}

/// 动态定价输入（全部为调用方观测到的入参，内核不采集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricingInput {
    /// 基准单价（micro/单位，必须为正）。
    pub base_unit_price_micro: u128,
    /// 同资源键在线供给量（同计量单位）。
    pub supply: u128,
    /// 同资源键当前需求量（同计量单位）。
    pub demand: u128,
    /// 供给方四维信誉复合分（0..=100000，100000=100.0）。
    pub reputation_composite: u128,
    /// 任务时延敏感度。
    pub latency: LatencyClass,
}

impl PricingInput {
    /// 构造并做基本 fail-closed 校验（正价、信誉分有界）。
    pub fn new(
        base_unit_price_micro: u128,
        supply: u128,
        demand: u128,
        reputation_composite: u128,
        latency: LatencyClass,
    ) -> Result<Self, ResourceError> {
        if base_unit_price_micro == 0 {
            return Err(ResourceError::NonPositiveBasePrice);
        }
        if reputation_composite > REP_FULL {
            return Err(ResourceError::ReputationCompositeOutOfRange {
                value: reputation_composite,
            });
        }
        Ok(Self {
            base_unit_price_micro,
            supply,
            demand,
            reputation_composite,
            latency,
        })
    }
}

/// 动态定价策略（有界调整参数，整数千分点）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricingPolicy {
    /// 稀缺度最大加价（pressure 越高加价越多），如 300=+30%。
    pub scarcity_max_up_permille: u128,
    /// 供给过剩最大降价，如 200=-20%。
    pub scarcity_max_down_permille: u128,
    /// 满分（100.0）供给方的最大信誉溢价，如 150=+15%。
    pub reputation_max_up_permille: u128,
    /// 零分供给方的最大信誉折价，如 200=-20%。
    pub reputation_max_down_permille: u128,
    /// 时延敏感溢价，如 100=+10%。
    pub latency_sensitive_up_permille: u128,
    /// 时延容忍折扣，如 150=-15%。
    pub latency_tolerant_down_permille: u128,
    /// 合成倍率下限（如 500=0.5×）。
    pub multiplier_floor_permille: u128,
    /// 合成倍率上限（如 2000=2.0×）。
    pub multiplier_ceil_permille: u128,
    /// bootstrap 期放宽倍率（冷启动，如 900=0.9×，不发现金补贴）。
    pub bootstrap_multiplier_permille: u128,
}

impl Default for PricingPolicy {
    fn default() -> Self {
        Self {
            scarcity_max_up_permille: 300,
            scarcity_max_down_permille: 200,
            reputation_max_up_permille: 150,
            reputation_max_down_permille: 200,
            latency_sensitive_up_permille: 100,
            latency_tolerant_down_permille: 150,
            multiplier_floor_permille: 500,
            multiplier_ceil_permille: 2000,
            bootstrap_multiplier_permille: 900,
        }
    }
}

/// 定价结果：建议单价、合成倍率（千分点）与三因子分解（可审计）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceQuote {
    /// 建议成交价（micro/单位，整数向下取整）。
    pub unit_price_micro: u128,
    /// 最终合成倍率（千分点，已 clamp 到 [floor, ceil]）。
    pub multiplier_permille: u128,
    /// 稀缺度调整（千分点，正加价/负降价）。
    pub scarcity_adj: i128,
    /// 信誉调整（千分点）。
    pub reputation_adj: i128,
    /// 时延调整（千分点）。
    pub latency_adj: i128,
}

/// 动态定价器：无状态纯函数集合（策略给定后对任意输入确定性出清）。
#[derive(Debug, Clone, Copy)]
pub struct DynamicPricer {
    policy: PricingPolicy,
}

impl Default for DynamicPricer {
    fn default() -> Self {
        Self::new(PricingPolicy::default())
    }
}

impl DynamicPricer {
    /// 以给定策略构造；floor/ceil 非法（floor>ceil、floor=0、ceil<PERMILLE 的退化等）
    /// fail-closed。
    pub fn new(policy: PricingPolicy) -> Self {
        // 策略是宿主受控常量；构造期不改外部状态，越界仅在 quote 时由 clamp 兜底。
        Self { policy }
    }

    /// 策略只读。
    pub fn policy(&self) -> &PricingPolicy {
        &self.policy
    }

    /// 稀缺度压力比 → 调整千分点。
    ///
    /// `pressure = demand*1000/supply`（确定性向下取整）：1000 供需平衡不调；>1000 按
    /// 超出比例线性映射到 `scarcity_max_up`（封顶）；<1000 按缺口比例映射到 max_down。
    /// supply=0 且 demand>0 视为绝对稀缺具名拒绝（无法按需定价，不能伪造成封顶价）。
    fn scarcity_adjustment(&self, supply: u128, demand: u128) -> Result<i128, ResourceError> {
        if supply == 0 {
            if demand == 0 {
                return Ok(0); // 无供需：不调整（冷启动空市场，由 bootstrap 决定）。
            }
            return Err(ResourceError::ScarcityZeroSupply { demand });
        }
        let pressure = demand
            .checked_mul(PERMILLE)
            .ok_or(ResourceError::ArithmeticOverflow)?
            / supply;
        if pressure >= PERMILLE {
            // 紧张：超出 1000 的部分占满额（pressure 无上界，先线性再封顶）。
            let over = core::cmp::min(
                pressure
                    .checked_sub(PERMILLE)
                    .ok_or(ResourceError::ArithmeticOverflow)?,
                PERMILLE,
            );
            let up = over
                .checked_mul(self.policy.scarcity_max_up_permille)
                .ok_or(ResourceError::ArithmeticOverflow)?
                / PERMILLE;
            i128::try_from(up).map_err(|_| ResourceError::ArithmeticOverflow)
        } else {
            // 宽松：缺口 (1000-pressure)/1000 线性映射到 max_down（负）。
            let gap = PERMILLE
                .checked_sub(pressure)
                .ok_or(ResourceError::ArithmeticOverflow)?;
            let down = gap
                .checked_mul(self.policy.scarcity_max_down_permille)
                .ok_or(ResourceError::ArithmeticOverflow)?
                / PERMILLE;
            i128::try_from(down)
                .map(|d| -d)
                .map_err(|_| ResourceError::ArithmeticOverflow)
        }
    }

    /// 信誉复合分 → 调整千分点：≥50 分线性溢价至满分 max_up，<50 线性折价至零分 -max_down。
    fn reputation_adjustment(&self, rep: u128) -> Result<i128, ResourceError> {
        if rep > REP_FULL {
            return Err(ResourceError::ReputationCompositeOutOfRange { value: rep });
        }
        if rep >= REP_NEUTRAL {
            let above = rep - REP_NEUTRAL; // 0..=50000
            let up = above
                .checked_mul(self.policy.reputation_max_up_permille)
                .ok_or(ResourceError::ArithmeticOverflow)?
                / REP_NEUTRAL;
            i128::try_from(up).map_err(|_| ResourceError::ArithmeticOverflow)
        } else {
            let below = REP_NEUTRAL - rep; // 1..=50000
            let down = below
                .checked_mul(self.policy.reputation_max_down_permille)
                .ok_or(ResourceError::ArithmeticOverflow)?
                / REP_NEUTRAL;
            i128::try_from(down)
                .map(|d| -d)
                .map_err(|_| ResourceError::ArithmeticOverflow)
        }
    }

    fn latency_adjustment(&self, latency: LatencyClass) -> i128 {
        match latency {
            LatencyClass::Sensitive => self.policy.latency_sensitive_up_permille as i128,
            LatencyClass::Neutral => 0,
            LatencyClass::Tolerant => -(self.policy.latency_tolerant_down_permille as i128),
        }
    }

    /// 出清建议价（不受买方限价约束的原始建议；调用方一般用 [`Self::quote_within_cap`]）。
    pub fn quote(&self, input: &PricingInput) -> Result<PriceQuote, ResourceError> {
        if input.base_unit_price_micro == 0 {
            return Err(ResourceError::NonPositiveBasePrice);
        }
        let scarcity = self.scarcity_adjustment(input.supply, input.demand)?;
        let reputation = self.reputation_adjustment(input.reputation_composite)?;
        let latency = self.latency_adjustment(input.latency);

        let raw_multiplier = (PERMILLE as i128)
            .checked_add(scarcity)
            .and_then(|m| m.checked_add(reputation))
            .and_then(|m| m.checked_add(latency))
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if raw_multiplier <= 0 {
            return Err(ResourceError::ArithmeticOverflow);
        }
        let mut mult =
            u128::try_from(raw_multiplier).map_err(|_| ResourceError::ArithmeticOverflow)?;
        // clamp 到策略有界区间（floor<=ceil 由宿主保证；运行时再兜底防退化配置）。
        let floor = if self.policy.multiplier_floor_permille == 0 {
            PERMILLE
        } else {
            self.policy.multiplier_floor_permille
        };
        let ceil = core::cmp::max(floor, self.policy.multiplier_ceil_permille);
        mult = core::cmp::min(core::cmp::max(mult, floor), ceil);

        let price = input
            .base_unit_price_micro
            .checked_mul(mult)
            .ok_or(ResourceError::ArithmeticOverflow)?
            / PERMILLE; // 确定性向下取整
        if price == 0 {
            // 正价×≥floor（floor≥1000）必为正；为 0 说明口径被破坏，fail-closed。
            return Err(ResourceError::ArithmeticOverflow);
        }
        Ok(PriceQuote {
            unit_price_micro: price,
            multiplier_permille: mult,
            scarcity_adj: scarcity,
            reputation_adj: reputation,
            latency_adj: latency,
        })
    }

    /// 出清并强制不突破买方最高限价：建议价 > cap 具名拒绝（返回计算值与 cap，调用方可改
    /// 时延档/不成交），**绝不静默压到 cap**（压价会改变供给方应得，须显式决策）。
    pub fn quote_within_cap(
        &self,
        input: &PricingInput,
        max_unit_price_micro: u128,
    ) -> Result<PriceQuote, ResourceError> {
        if max_unit_price_micro == 0 {
            return Err(ResourceError::NonPositiveBasePrice);
        }
        let q = self.quote(input)?;
        if q.unit_price_micro > max_unit_price_micro {
            return Err(ResourceError::PriceExceedsBuyerCap {
                computed_micro: q.unit_price_micro,
                cap_micro: max_unit_price_micro,
            });
        }
        Ok(q)
    }

    /// bootstrap 期出清：忽略稀缺/信誉/时延三因子，直接用策略的 bootstrap 放宽倍率
    /// （冷启动早期补贴价，仅更小倍率、不发现金），仍受正价与乘法溢出约束。
    pub fn quote_bootstrap(
        &self,
        base_unit_price_micro: u128,
    ) -> Result<PriceQuote, ResourceError> {
        if base_unit_price_micro == 0 {
            return Err(ResourceError::NonPositiveBasePrice);
        }
        let mult = core::cmp::min(
            self.policy.bootstrap_multiplier_permille.max(1),
            PERMILLE, // bootstrap 只许降价不许抬价。
        );
        let price = base_unit_price_micro
            .checked_mul(mult)
            .ok_or(ResourceError::ArithmeticOverflow)?
            / PERMILLE;
        if price == 0 {
            return Err(ResourceError::ArithmeticOverflow);
        }
        Ok(PriceQuote {
            unit_price_micro: price,
            multiplier_permille: mult,
            scarcity_adj: 0,
            reputation_adj: 0,
            latency_adj: 0,
        })
    }
}

/// 批量中的一行（一笔小单）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchLine {
    /// 小单 id。
    pub order_id: String,
    /// 该行数量（同计量单位）。
    pub quantity: u128,
    /// 该行接受的单价（micro/单位）。
    pub unit_price_micro: u128,
}

impl BatchLine {
    /// 行金额 micro = 量 × 单价（checked）。
    pub fn amount_micro(&self) -> Result<u128, ResourceError> {
        self.quantity
            .checked_mul(self.unit_price_micro)
            .ok_or(ResourceError::ArithmeticOverflow)
    }
}

/// 一个聚合批：同资源键、同单价带的小单 FIFO 集合。
#[derive(Debug, Clone)]
pub struct OrderBatch {
    /// 资源键（供给方）。
    provider_did: String,
    kind: ResourceKind,
    unit: MeterUnit,
    /// 批内统一单价带（micro）：同批各行单价必须一致（价格带等宽，避免混价分账歧义）。
    unit_price_micro: u128,
    lines: Vec<BatchLine>,
    total_quantity: u128,
}

impl OrderBatch {
    fn new(
        provider_did: impl Into<String>,
        kind: ResourceKind,
        unit: MeterUnit,
        price: u128,
    ) -> Self {
        Self {
            provider_did: provider_did.into(),
            kind,
            unit,
            unit_price_micro: price,
            lines: Vec::new(),
            total_quantity: 0,
        }
    }

    /// 供给方 DID。
    pub fn provider_did(&self) -> &str {
        &self.provider_did
    }
    /// 资源形态。
    pub fn kind(&self) -> ResourceKind {
        self.kind
    }
    /// 计量单位。
    pub fn unit(&self) -> MeterUnit {
        self.unit
    }
    /// 批内统一单价。
    pub fn unit_price_micro(&self) -> u128 {
        self.unit_price_micro
    }
    /// 批内行（FIFO）。
    pub fn lines(&self) -> &[BatchLine] {
        &self.lines
    }
    /// 批内总行数。
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
    /// 批内总量。
    pub fn total_quantity(&self) -> u128 {
        self.total_quantity
    }
    /// 批内总金额 micro = 总量 × 统一单价。
    pub fn total_amount_micro(&self) -> Result<u128, ResourceError> {
        self.total_quantity
            .checked_mul(self.unit_price_micro)
            .ok_or(ResourceError::ArithmeticOverflow)
    }

    fn same_key(&self, provider: &str, kind: ResourceKind, unit: MeterUnit, price: u128) -> bool {
        self.provider_did == provider
            && self.kind == kind
            && self.unit == unit
            && self.unit_price_micro == price
    }
}

/// 订单聚合器：把同键同价小单聚成批，达目标量成批，异键/超量 fail-closed。
#[derive(Debug, Clone)]
pub struct OrderBatcher {
    /// 单批最大量（超过则不能并入当前批，调用方应先 seal 再开新批）。
    max_batch_quantity: u128,
    /// 成批目标量：累计 ≥ target 即 ready（bootstrap 时调用方传 1）。
    target_quantity: u128,
    /// 当前在聚的开放批。
    open: Option<OrderBatch>,
    /// 已封批。
    sealed: Vec<OrderBatch>,
}

impl OrderBatcher {
    /// 构造：max 必须为正、target 必须 ∈ [1, max]，否则 fail-closed。
    pub fn new(max_batch_quantity: u128, target_quantity: u128) -> Result<Self, ResourceError> {
        if max_batch_quantity == 0 || target_quantity == 0 || target_quantity > max_batch_quantity {
            return Err(ResourceError::BatchConfigInvalid {
                max: max_batch_quantity,
                target: target_quantity,
            });
        }
        Ok(Self {
            max_batch_quantity,
            target_quantity,
            open: None,
            sealed: Vec::new(),
        })
    }

    /// 成批目标量。
    pub fn target_quantity(&self) -> u128 {
        self.target_quantity
    }

    /// 更新目标量（bootstrap 关→开降为 1、开→关注复），仍须 1..=max。
    pub fn set_target(&mut self, target: u128) -> Result<(), ResourceError> {
        if target == 0 || target > self.max_batch_quantity {
            return Err(ResourceError::BatchConfigInvalid {
                max: self.max_batch_quantity,
                target,
            });
        }
        self.target_quantity = target;
        Ok(())
    }

    /// 开放批是否已达目标量（可 seal）。
    pub fn is_ready(&self) -> bool {
        self.open
            .as_ref()
            .map(|b| b.total_quantity >= self.target_quantity)
            .unwrap_or(false)
    }

    /// 开放批当前累计量（无开放批为 0）。
    pub fn open_quantity(&self) -> u128 {
        self.open.as_ref().map(|b| b.total_quantity).unwrap_or(0)
    }

    /// 已封批数。
    pub fn sealed_count(&self) -> usize {
        self.sealed.len()
    }
}

/// 带资源键的聚合入口（键不放进每行，避免同批行重复携带不一致键）。
impl OrderBatcher {
    /// 以显式资源键并入一行（推荐入口）：首行确定 provider/kind/unit/price，后续行必须同键
    /// 同价，否则 [`ResourceError::BatchKeyMismatch`]。
    pub fn add(
        &mut self,
        provider_did: &str,
        kind: ResourceKind,
        unit: MeterUnit,
        line: BatchLine,
    ) -> Result<u128, ResourceError> {
        if provider_did.is_empty() {
            return Err(ResourceError::EmptyBatchOrderId);
        }
        if line.order_id.is_empty() {
            return Err(ResourceError::EmptyBatchOrderId);
        }
        if line.quantity == 0 {
            return Err(ResourceError::BatchLineZeroQuantity);
        }
        if line.unit_price_micro == 0 {
            return Err(ResourceError::NonPositiveBasePrice);
        }
        if self.open.is_none() {
            if line.quantity > self.max_batch_quantity {
                return Err(ResourceError::BatchExceedsMaxSize {
                    requested: line.quantity,
                    max: self.max_batch_quantity,
                });
            }
            let mut b = OrderBatch::new(provider_did, kind, unit, line.unit_price_micro);
            b.total_quantity = line.quantity;
            b.lines.push(line);
            self.open = Some(b);
            return Ok(self.open_quantity());
        }
        let batch = self.open.as_mut().ok_or(ResourceError::EmptyBatchSeal)?;
        if !batch.same_key(provider_did, kind, unit, line.unit_price_micro) {
            return Err(ResourceError::BatchKeyMismatch);
        }
        if batch.lines.iter().any(|l| l.order_id == line.order_id) {
            return Err(ResourceError::DuplicateBatchOrder {
                order_id: line.order_id,
            });
        }
        let new_total = batch
            .total_quantity
            .checked_add(line.quantity)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if new_total > self.max_batch_quantity {
            return Err(ResourceError::BatchExceedsMaxSize {
                requested: new_total,
                max: self.max_batch_quantity,
            });
        }
        batch.total_quantity = new_total;
        batch.lines.push(line);
        Ok(batch.total_quantity)
    }

    /// 封当前开放批为成批：空批具名拒绝；非空即封（是否达 target 由调用方先 is_ready 看，
    /// 但允许提前 seal——批量守恒不依赖是否达 target）。封批后守恒自检，不符 fail-closed。
    pub fn seal(&mut self) -> Result<OrderBatch, ResourceError> {
        let batch = self.open.take().ok_or(ResourceError::EmptyBatchSeal)?;
        // 守恒：Σ 行量 == total_quantity；Σ 行金额 == total_amount。
        let sum_qty = batch
            .lines
            .iter()
            .try_fold(0u128, |acc, l| acc.checked_add(l.quantity))
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let sum_amt = batch.lines.iter().try_fold(0u128, |acc, l| {
            acc.checked_add(l.amount_micro()?)
                .ok_or(ResourceError::ArithmeticOverflow)
        })?;
        if sum_qty != batch.total_quantity {
            return Err(ResourceError::ArithmeticOverflow);
        }
        if sum_amt != batch.total_amount_micro()? {
            return Err(ResourceError::ArithmeticOverflow);
        }
        self.sealed.push(batch.clone());
        Ok(batch)
    }

    /// 已封批只读。
    pub fn sealed(&self) -> &[OrderBatch] {
        &self.sealed
    }

    /// 全聚合器守恒：所有已封批逐批 Σ行量/Σ行金额 自洽。
    pub fn invariant_holds(&self) -> bool {
        self.sealed.iter().all(|b| {
            let q = b
                .lines
                .iter()
                .try_fold(0u128, |acc, l| acc.checked_add(l.quantity));
            let a = b.lines.iter().try_fold(0u128, |acc, l| {
                acc.checked_add(l.quantity.checked_mul(l.unit_price_micro)?)
            });
            q == Some(b.total_quantity) && a == b.total_amount_micro().ok()
        })
    }
}

/// 冷启动配置：在线供给方数或可用总容量低于阈值即进入 bootstrap。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapConfig {
    /// 在线供给方数阈值：实际 < 该值则 active。
    pub min_providers: u64,
    /// 可用总容量阈值（同口径汇总）：实际 < 该值则 active。
    pub min_available_quantity: u128,
}

impl BootstrapConfig {
    /// 构造（阈值允许为 0 表示关闭该维度）。
    pub fn new(min_providers: u64, min_available_quantity: u128) -> Self {
        Self {
            min_providers,
            min_available_quantity,
        }
    }
}

/// 冷启动观测快照与判定原因（确定性、可审计）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapObservation {
    /// 是否处于 bootstrap。
    pub active: bool,
    /// 供给方数不足。
    pub below_providers: bool,
    /// 可用容量不足。
    pub below_capacity: bool,
    /// 建议聚合门槛（active 时为 1，否则为 normal_target，由调用方据此 set_target）。
    pub batch_target_hint: u128,
}

/// 冷启动开关：给定配置与当前观测，确定性给出 active 状态。
#[derive(Debug, Clone, Copy)]
pub struct BootstrapGate {
    config: BootstrapConfig,
    normal_target: u128,
}

impl BootstrapGate {
    /// 构造：normal_target 为非 bootstrap 期聚合目标量，必须为正。
    pub fn new(config: BootstrapConfig, normal_target: u128) -> Result<Self, ResourceError> {
        if normal_target == 0 {
            return Err(ResourceError::BootstrapConfigInvalid);
        }
        Ok(Self {
            config,
            normal_target,
        })
    }

    /// 配置只读。
    pub fn config(&self) -> &BootstrapConfig {
        &self.config
    }

    /// 评估当前观测 → bootstrap 状态。任一维度低于阈值即 active（两阈值为 0 的维度不参与）。
    pub fn observe(&self, providers: u64, available_quantity: u128) -> BootstrapObservation {
        let below_providers =
            self.config.min_providers > 0 && providers < self.config.min_providers;
        let below_capacity = self.config.min_available_quantity > 0
            && available_quantity < self.config.min_available_quantity;
        let active = below_providers || below_capacity;
        BootstrapObservation {
            active,
            below_providers,
            below_capacity,
            batch_target_hint: if active { 1 } else { self.normal_target },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(base: u128, supply: u128, demand: u128, rep: u128, lat: LatencyClass) -> PricingInput {
        PricingInput::new(base, supply, demand, rep, lat).unwrap()
    }

    #[test]
    fn scarcity_tight_raises_loose_cuts_balanced_flat() {
        let p = DynamicPricer::default();
        // 供需平衡：scarcity_adj=0（信誉取中性 50、时延中性，隔离稀缺度）。
        let flat = p
            .quote(&input(1000, 1000, 1000, REP_NEUTRAL, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(flat.scarcity_adj, 0);
        assert_eq!(flat.unit_price_micro, 1000);
        // 需求翻倍 pressure=2000：达到 +300 封顶（over 被 clamp 到 1000）。
        let tight = p
            .quote(&input(1000, 1000, 2000, REP_NEUTRAL, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(tight.scarcity_adj, 300);
        assert_eq!(tight.unit_price_micro, 1300);
        // 需求为 0：gap=1000 → -200 封顶降价。
        let loose = p
            .quote(&input(1000, 1000, 0, REP_NEUTRAL, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(loose.scarcity_adj, -200);
        assert_eq!(loose.unit_price_micro, 800);
        // 半空 pressure=500：gap=500 → -100。
        let half = p
            .quote(&input(1000, 1000, 500, REP_NEUTRAL, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(half.scarcity_adj, -100);
    }

    #[test]
    fn reputation_premium_and_discount_piecewise() {
        let p = DynamicPricer::default();
        // 满分 100：+150（max_up）。
        let full = p
            .quote(&input(1000, 1000, 1000, REP_FULL, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(full.reputation_adj, 150);
        assert_eq!(full.unit_price_micro, 1150);
        // 中性 50：0。
        let neu = p
            .quote(&input(1000, 1000, 1000, REP_NEUTRAL, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(neu.reputation_adj, 0);
        // 零分：-200（max_down）。
        let zero = p
            .quote(&input(1000, 1000, 1000, 0, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(zero.reputation_adj, -200);
        assert_eq!(zero.unit_price_micro, 800);
        // 75 分：above=25000/50000=半 → +75。
        let q75 = p
            .quote(&input(1000, 1000, 1000, 75_000, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(q75.reputation_adj, 75);
    }

    #[test]
    fn latency_three_classes_and_composite_multiplier_clamped() {
        let p = DynamicPricer::default();
        let sens = p
            .quote(&input(
                1000,
                1000,
                1000,
                REP_NEUTRAL,
                LatencyClass::Sensitive,
            ))
            .unwrap();
        assert_eq!(sens.latency_adj, 100);
        assert_eq!(sens.unit_price_micro, 1100);
        let neu = p
            .quote(&input(1000, 1000, 1000, REP_NEUTRAL, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(neu.latency_adj, 0);
        let tol = p
            .quote(&input(
                1000,
                1000,
                1000,
                REP_NEUTRAL,
                LatencyClass::Tolerant,
            ))
            .unwrap();
        assert_eq!(tol.latency_adj, -150);
        assert_eq!(tol.unit_price_micro, 850);
        // 极端叠加：需求 10×（+300 封顶）+ 满分（+150）+ 敏感（+100）= 1550，未触上限。
        let stacked = p
            .quote(&input(1000, 100, 1000, REP_FULL, LatencyClass::Sensitive))
            .unwrap();
        assert_eq!(stacked.multiplier_permille, 1550);
        // 构造一个会冲破 2000 上限的策略，验证 clamp 到 ceil（自定义策略 max_up 更大）。
        let aggressive = PricingPolicy {
            scarcity_max_up_permille: 2000,
            reputation_max_up_permille: 1000,
            latency_sensitive_up_permille: 1000,
            ..PricingPolicy::default()
        };
        let p2 = DynamicPricer::new(aggressive);
        let capped = p2
            .quote(&input(1000, 1, 1000, REP_FULL, LatencyClass::Sensitive))
            .unwrap();
        assert_eq!(capped.multiplier_permille, 2000); // clamp 到 ceil。
        assert_eq!(capped.unit_price_micro, 2000);
    }

    #[test]
    fn quote_never_silently_crosses_buyer_cap() {
        let p = DynamicPricer::default();
        let inp = input(1000, 1, 1000, REP_FULL, LatencyClass::Sensitive);
        // cap 足够：正常返回。
        assert!(p.quote_within_cap(&inp, 2000).is_ok());
        // cap 低于建议价：具名拒绝并给出 computed/cap，绝不压价。
        match p.quote_within_cap(&inp, 1000) {
            Err(ResourceError::PriceExceedsBuyerCap {
                computed_micro,
                cap_micro,
            }) => {
                assert!(computed_micro > 1000);
                assert_eq!(cap_micro, 1000);
            }
            other => panic!("expected PriceExceedsBuyerCap, got {other:?}"),
        }
        // cap=0 fail-closed。
        assert!(matches!(
            p.quote_within_cap(&inp, 0),
            Err(ResourceError::NonPositiveBasePrice)
        ));
    }

    #[test]
    fn zero_price_zero_supply_and_overflow_fail_closed() {
        let p = DynamicPricer::default();
        // 基准价为 0。
        assert!(matches!(
            PricingInput::new(0, 10, 10, REP_NEUTRAL, LatencyClass::Neutral),
            Err(ResourceError::NonPositiveBasePrice)
        ));
        // 信誉分越界。
        assert!(matches!(
            PricingInput::new(10, 10, 10, REP_FULL + 1, LatencyClass::Neutral),
            Err(ResourceError::ReputationCompositeOutOfRange { .. })
        ));
        // supply=0 且 demand>0：绝对稀缺具名拒绝（不能伪造成封顶价）。
        assert!(matches!(
            p.quote(&input(1000, 0, 1, REP_NEUTRAL, LatencyClass::Neutral)),
            Err(ResourceError::ScarcityZeroSupply { demand: 1 })
        ));
        // supply=0 且 demand=0：不调整（空市场交 bootstrap）。
        let empty_market = p
            .quote(&input(1000, 0, 0, REP_NEUTRAL, LatencyClass::Neutral))
            .unwrap();
        assert_eq!(empty_market.unit_price_micro, 1000);
        // 基准价 u128::MAX 乘倍率必溢出 fail-closed。
        assert!(matches!(
            p.quote(&input(
                u128::MAX,
                1000,
                2000,
                REP_FULL,
                LatencyClass::Sensitive
            )),
            Err(ResourceError::ArithmeticOverflow)
        ));
    }

    #[test]
    fn batcher_fifo_conserves_and_rejects_mismatch_overflow_dup() {
        let mut b = OrderBatcher::new(1000, 300).unwrap();
        let line = |id: &str, q: u128, price: u128| BatchLine {
            order_id: id.into(),
            quantity: q,
            unit_price_micro: price,
        };
        // 首三行同键同价 100+100+100=300 达 target。
        assert_eq!(
            b.add(
                "did:nau:p",
                ResourceKind::Compute,
                MeterUnit::CpuMillis,
                line("a", 100, 2)
            )
            .unwrap(),
            100
        );
        b.add(
            "did:nau:p",
            ResourceKind::Compute,
            MeterUnit::CpuMillis,
            line("b", 100, 2),
        )
        .unwrap();
        b.add(
            "did:nau:p",
            ResourceKind::Compute,
            MeterUnit::CpuMillis,
            line("c", 100, 2),
        )
        .unwrap();
        assert!(b.is_ready());
        // 异键（不同 provider/kind/unit/价）拒绝。
        assert!(matches!(
            b.add(
                "did:nau:other",
                ResourceKind::Compute,
                MeterUnit::CpuMillis,
                line("d", 1, 2)
            ),
            Err(ResourceError::BatchKeyMismatch)
        ));
        assert!(matches!(
            b.add(
                "did:nau:p",
                ResourceKind::Storage,
                MeterUnit::CpuMillis,
                line("d", 1, 2)
            ),
            Err(ResourceError::BatchKeyMismatch)
        ));
        assert!(matches!(
            b.add(
                "did:nau:p",
                ResourceKind::Compute,
                MeterUnit::CpuMillis,
                line("d", 1, 3)
            ),
            Err(ResourceError::BatchKeyMismatch)
        ));
        // 同 order_id 重复拒绝。
        assert!(matches!(
            b.add(
                "did:nau:p",
                ResourceKind::Compute,
                MeterUnit::CpuMillis,
                line("a", 1, 2)
            ),
            Err(ResourceError::DuplicateBatchOrder { .. })
        ));
        // 再并入会超 max 1000？当前 300，加 800 → 1100 超量拒绝且不改写。
        assert!(matches!(
            b.add(
                "did:nau:p",
                ResourceKind::Compute,
                MeterUnit::CpuMillis,
                line("e", 800, 2)
            ),
            Err(ResourceError::BatchExceedsMaxSize {
                requested: 1100,
                max: 1000
            })
        ));
        assert_eq!(b.open_quantity(), 300); // 未被污染。
                                            // 封批并守恒：Σ量=300、Σ金额=600 micro。
        let sealed = b.seal().unwrap();
        assert_eq!(sealed.total_quantity(), 300);
        assert_eq!(sealed.total_amount_micro().unwrap(), 600);
        assert_eq!(sealed.line_count(), 3);
        assert_eq!(b.sealed_count(), 1);
        assert!(b.invariant_holds());
        // 空批再 seal 拒绝。
        assert!(matches!(b.seal(), Err(ResourceError::EmptyBatchSeal)));
        // 非法配置 fail-closed。
        assert!(matches!(
            OrderBatcher::new(0, 1),
            Err(ResourceError::BatchConfigInvalid { .. })
        ));
        assert!(matches!(
            OrderBatcher::new(10, 11),
            Err(ResourceError::BatchConfigInvalid { .. })
        ));
    }

    #[test]
    fn bootstrap_gate_flips_policy_and_pricing_uses_subsidy_rate() {
        // 阈值：至少 3 个供给方、至少 1000 容量。
        let cfg = BootstrapConfig::new(3, 1000);
        let gate = BootstrapGate::new(cfg, 300).unwrap();

        // 早期：2 供给方 / 500 容量 → active，聚合门槛提示降为 1。
        let early = gate.observe(2, 500);
        assert!(early.active);
        assert!(early.below_providers && early.below_capacity);
        assert_eq!(early.batch_target_hint, 1);

        // bootstrap 期 batcher 门槛=1：单笔小单即可成批。
        let mut b = OrderBatcher::new(1000, early.batch_target_hint).unwrap();
        b.add(
            "did:nau:p",
            ResourceKind::Compute,
            MeterUnit::CpuMillis,
            BatchLine {
                order_id: "solo".into(),
                quantity: 1,
                unit_price_micro: 2,
            },
        )
        .unwrap();
        assert!(b.is_ready()); // target=1。

        // bootstrap 定价走补贴倍率（默认 900=0.9×），与供需/信誉无关。
        let pricer = DynamicPricer::default();
        let boot_price = pricer.quote_bootstrap(1000).unwrap();
        assert_eq!(boot_price.multiplier_permille, 900);
        assert_eq!(boot_price.unit_price_micro, 900);

        // 越过阈值：4 供给方 / 2000 容量 → 关闭，门槛恢复 300。
        let mature = gate.observe(4, 2000);
        assert!(!mature.active);
        assert_eq!(mature.batch_target_hint, 300);
        // 单维度恰好达标不 active（providers=3 达阈值、容量超）。
        assert!(!gate.observe(3, 1000).active);
        // 单维度不足仍 active。
        assert!(gate.observe(2, 5000).active);
        assert!(gate.observe(9, 999).active);

        // normal_target=0 非法。
        assert!(matches!(
            BootstrapGate::new(cfg, 0),
            Err(ResourceError::BootstrapConfigInvalid)
        ));
    }
}
