//! 任务定价系统
//! 
//! 基于供需、难度、紧急程度的动态定价

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

    pub fn with_supply_demand(mut self, ratio: f64) -> Self {
        // ratio = available_workers / demand
        // ratio < 1: 工人不足，价格上涨
        // ratio > 1: 工人过剩，价格下降
        self.supply_demand_multiplier = if ratio > 0.0 {
            1.0 / ratio.max(0.1)
        } else {
            2.0
        };
        self
    }

    pub fn price(&self) -> u64 {
        let price = self.base_price as f64
            * self.difficulty_multiplier
            * self.urgency_multiplier
            * self.supply_demand_multiplier;
        price.round() as u64
    }
}
