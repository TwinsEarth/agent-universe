//! 多维声誉档案
//!
//! quality / speed / honesty / availability 四维
//! 衰减半衰期 90 天，不可转让

use std::collections::HashMap;

/// 声誉维度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReputationDimension {
    /// 结果质量
    Quality,
    /// 响应速度
    Speed,
    /// 诚实性
    Honesty,
    /// 可用性
    Availability,
}

impl ReputationDimension {
    pub fn all() -> Vec<ReputationDimension> {
        vec![
            ReputationDimension::Quality,
            ReputationDimension::Speed,
            ReputationDimension::Honesty,
            ReputationDimension::Availability,
        ]
    }
}

/// 多维声誉
#[derive(Debug, Clone)]
pub struct MultiReputation {
    did: String,
    /// 四维分数 0-10000
    scores: HashMap<ReputationDimension, f64>,
    /// 总交互次数
    interactions: u64,
    /// 上次更新时间
    last_updated: u64,
    /// 衰减半衰期（秒），默认 90 天
    half_life_secs: u64,
    /// 是否可转让（永远 false）
    transferable: bool,
}

impl MultiReputation {
    pub fn new(did: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut scores = HashMap::new();
        for dim in ReputationDimension::all() {
            scores.insert(dim, 5000.0); // 初始中性分
        }

        Self {
            did,
            scores,
            interactions: 0,
            last_updated: now,
            half_life_secs: 90 * 24 * 3600, // 90 天
            transferable: false,
        }
    }

    pub fn did(&self) -> &str {
        &self.did
    }

    pub fn is_transferable(&self) -> bool {
        self.transferable
    }

    /// 获取某维度分数（含时间衰减）
    pub fn get(&self, dim: ReputationDimension) -> f64 {
        let raw = self.scores.get(&dim).copied().unwrap_or(5000.0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let elapsed = now.saturating_sub(self.last_updated);
        let decay_factor = 0.5f64.powf(elapsed as f64 / self.half_life_secs as f64);
        raw * decay_factor
    }

    /// 记录一次成功交互
    pub fn record_success(&mut self, dim: ReputationDimension, amount: f64) {
        let entry = self.scores.entry(dim).or_insert(5000.0);
        *entry = (*entry + amount).min(10000.0);
        self.interactions += 1;
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// 记录一次失败交互
    pub fn record_failure(&mut self, dim: ReputationDimension, amount: f64) {
        let entry = self.scores.entry(dim).or_insert(5000.0);
        *entry = (*entry - amount).max(0.0);
        self.interactions += 1;
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// 综合声誉分（加权平均）
    pub fn overall(&self) -> f64 {
        let quality = self.get(ReputationDimension::Quality);
        let speed = self.get(ReputationDimension::Speed);
        let honesty = self.get(ReputationDimension::Honesty);
        let availability = self.get(ReputationDimension::Availability);
        // 诚实性权重最高
        quality * 0.25 + speed * 0.15 + honesty * 0.40 + availability * 0.20
    }

    pub fn interactions(&self) -> u64 {
        self.interactions
    }
}
