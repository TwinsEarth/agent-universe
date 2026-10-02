//! 任务定价系统
//!
//! 基于供需、难度、紧急程度的动态定价

/// 定价参数/计算错误（v3.5.3，AU-34）。
#[derive(Debug, Clone, PartialEq)]
pub enum PricingError {
    /// 供需比非有限（NaN / +Inf / -Inf）。
    NonFiniteSupplyDemandRatio(f64),
    /// 最终价格非有限或溢出 u64（例如乘数累乘出 Inf）。
    NonFiniteOrOverflowPrice(f64),
}

impl std::fmt::Display for PricingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PricingError::NonFiniteSupplyDemandRatio(r) => {
                write!(f, "PRICING_NON_FINITE_RATIO: 供需比非有限: {r}")
            }
            PricingError::NonFiniteOrOverflowPrice(p) => {
                write!(f, "PRICING_NON_FINITE_PRICE: 计算价格非有限/溢出 u64: {p}")
            }
        }
    }
}

impl std::error::Error for PricingError {}

#[derive(Debug, Clone)]
pub struct TaskPricing {
    base_price: u64,
    difficulty_multiplier: f64,
    urgency_multiplier: f64,
    supply_demand_multiplier: f64,
}

#[derive(Debug, Clone)]
pub enum DifficultyLevel {
    Trivial,
    Easy,
    Medium,
    Hard,
    Expert,
}

impl DifficultyLevel {
    pub fn multiplier(&self) -> f64 {
        match self {
            DifficultyLevel::Trivial => 0.5,
            DifficultyLevel::Easy => 1.0,
            DifficultyLevel::Medium => 2.0,
            DifficultyLevel::Hard => 4.0,
            DifficultyLevel::Expert => 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UrgencyLevel {
    Low,
    Normal,
    High,
    Critical,
}

impl UrgencyLevel {
    pub fn multiplier(&self) -> f64 {
        match self {
            UrgencyLevel::Low => 0.8,
            UrgencyLevel::Normal => 1.0,
            UrgencyLevel::High => 1.5,
            UrgencyLevel::Critical => 2.5,
        }
    }
}

impl TaskPricing {
    pub fn new(base_price: u64) -> Self {
        Self {
            base_price,
            difficulty_multiplier: 1.0,
            urgency_multiplier: 1.0,
            supply_demand_multiplier: 1.0,
        }
    }

    pub fn with_difficulty(mut self, difficulty: DifficultyLevel) -> Self {
        self.difficulty_multiplier = difficulty.multiplier();
        self
    }

    pub fn with_urgency(mut self, urgency: UrgencyLevel) -> Self {
        self.urgency_multiplier = urgency.multiplier();
        self
    }

    pub fn with_supply_demand(mut self, ratio: f64) -> Result<Self, PricingError> {
        // ratio = available_workers / demand
        // ratio < 1: 工人不足，价格上涨
        // ratio > 1: 工人过剩，价格下降
        // v3.5.3（AU-34）：非有限（NaN/+Inf/-Inf）必须显式拒绝——旧实现 NaN>0.0 为 false
        // 会静默落入 else=2.0，Inf 则把乘数放大到无穷且无报错。
        if !ratio.is_finite() {
            return Err(PricingError::NonFiniteSupplyDemandRatio(ratio));
        }
        self.supply_demand_multiplier = if ratio > 0.0 {
            1.0 / ratio.max(0.1)
        } else {
            2.0
        };
        Ok(self)
    }

    pub fn price(&self) -> Result<u64, PricingError> {
        let price = self.base_price as f64
            * self.difficulty_multiplier
            * self.urgency_multiplier
            * self.supply_demand_multiplier;
        // v3.5.3（AU-34）：旧实现 `price.round() as u64` 对 Inf 会饱和到 u64::MAX 且静默通过。
        // 非有限/溢出显式报错。
        if !price.is_finite() || !(0.0..=u64::MAX as f64).contains(&price) {
            return Err(PricingError::NonFiniteOrOverflowPrice(price));
        }
        Ok(price.round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn au34_non_finite_supply_demand_rejected() {
        // NaN/+Inf/-Inf 都必须显式拒绝——旧实现 NaN 静默落入 2.0、Inf 放大乘数无报错。
        assert!(matches!(
            TaskPricing::new(100).with_supply_demand(f64::NAN),
            Err(PricingError::NonFiniteSupplyDemandRatio(_))
        ));
        assert!(matches!(
            TaskPricing::new(100).with_supply_demand(f64::INFINITY),
            Err(PricingError::NonFiniteSupplyDemandRatio(_))
        ));
        assert!(matches!(
            TaskPricing::new(100).with_supply_demand(f64::NEG_INFINITY),
            Err(PricingError::NonFiniteSupplyDemandRatio(_))
        ));
    }

    #[test]
    fn au34_finite_ratio_ok_and_price_roundtrip() {
        let p = TaskPricing::new(1000).with_supply_demand(2.0).unwrap();
        // ratio=2 → 乘数 0.5 → 500。
        assert_eq!(p.price().unwrap(), 500);
        // ratio=0（无工人）→ 2.0 乘数 → 2000。
        let p0 = TaskPricing::new(1000).with_supply_demand(0.0).unwrap();
        assert_eq!(p0.price().unwrap(), 2000);
    }

    #[test]
    fn au34_non_finite_price_rejected_not_saturating() {
        // 乘数溢出到 Inf：旧实现 `as u64` 静默饱和成 u64::MAX；新实现显式报错。
        let p = TaskPricing::new(u64::MAX)
            .with_supply_demand(0.5) // 乘数 2.0
            .unwrap();
        // base(≈u64::MAX) * 2.0 → > u64::MAX，非有限/溢出。
        assert!(matches!(
            p.price(),
            Err(PricingError::NonFiniteOrOverflowPrice(_))
        ));
    }
}
