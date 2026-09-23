//! 信誉系统 & 质押管理
//!
//! 多维信誉：quality / speed / honesty / availability
//! 信誉不可转让，半衰期 90 天
//! 质押锁定，作恶罚没

use serde::{Deserialize, Serialize};

/// 多维信誉
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketReputation {
    /// 质量分（0-1）
    pub quality: f64,
    /// 速度分（0-1）
    pub speed: f64,
    /// 诚信分（0-1）
    pub honesty: f64,
    /// 可用性分（0-1）
    pub availability: f64,
    /// 总调用次数
    pub total_calls: u64,
    /// 成功次数
    pub success_calls: u64,
}

impl MarketReputation {
    pub fn new() -> Self {
        Self {
            quality: 0.5,
            speed: 0.5,
            honesty: 0.5,
            availability: 0.5,
            total_calls: 0,
            success_calls: 0,
        }
    }

    /// 综合评分
    pub fn overall(&self) -> f64 {
        self.quality * 0.35
            + self.speed * 0.20
            + self.honesty * 0.30
            + self.availability * 0.15
    }

    /// 成功率
    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            return 0.0;
        }
        self.success_calls as f64 / self.total_calls as f64
    }

    /// 记录一次调用结果
    pub fn record_call(&mut self, success: bool, latency_ratio: f64) {
        self.total_calls += 1;
        if success {
            self.success_calls += 1;
        }

        let alpha = 0.1; // 学习率

        // 更新质量
        let quality_target = if success { 1.0 } else { 0.0 };
        self.quality += alpha * (quality_target - self.quality);

        // 更新速度（latency_ratio: 实际/预期，越小越好）
        let speed_target = (1.0 / (1.0 + latency_ratio)).clamp(0.0, 1.0);
        self.speed += alpha * (speed_target - self.speed);

        // 更新可用性
        let avail_target = if success { 1.0 } else { 0.3 };
        self.availability += alpha * (avail_target - self.availability);
    }

    /// 诚信惩罚
    pub fn penalize_dishonesty(&mut self, severity: f64) {
        self.honesty = (self.honesty - severity).max(0.0);
    }

    /// 诚信奖励
    pub fn reward_honesty(&mut self) {
        self.honesty = (self.honesty + 0.05).min(1.0);
    }
}

impl Default for MarketReputation {
    fn default() -> Self {
        Self::new()
    }
}

/// 质押状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StakeStatus {
    /// 已锁定
    Locked,
    /// 申请退出中（锁定期）
    Withdrawing,
    /// 已退还
    Withdrawn,
    /// 已罚没
    Slashed,
}

/// 质押记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeRecord {
    pub agent_id: String,
    pub amount: f64,
    pub status: StakeStatus,
    pub locked_at: u64,
}

/// 信誉与质押管理器
#[derive(Debug, Clone)]
pub struct ReputationManager {
    reputations: std::collections::HashMap<String, MarketReputation>,
    stakes: std::collections::HashMap<String, StakeRecord>,
    min_stake: f64,
}

impl ReputationManager {
    pub fn new(min_stake: f64) -> Self {
        Self {
            reputations: std::collections::HashMap::new(),
            stakes: std::collections::HashMap::new(),
            min_stake,
        }
    }

    /// 注册质押
    pub fn register_stake(&mut self, agent_id: &str, amount: f64) -> Result<(), String> {
        if amount < self.min_stake {
            return Err(format!(
                "质押不足：需要至少 {}，当前 {}",
                self.min_stake, amount
            ));
        }
        self.stakes.insert(
            agent_id.to_string(),
            StakeRecord {
                agent_id: agent_id.to_string(),
                amount,
                status: StakeStatus::Locked,
                locked_at: Self::now(),
            },
        );
        self.reputations
            .entry(agent_id.to_string())
            .or_insert_with(MarketReputation::new);
        Ok(())
    }

    /// 获取信誉
    pub fn reputation(&self, agent_id: &str) -> Option<&MarketReputation> {
        self.reputations.get(agent_id)
    }

    /// 获取可变信誉
    pub fn reputation_mut(&mut self, agent_id: &str) -> Option<&mut MarketReputation> {
        self.reputations.get_mut(agent_id)
    }

    /// 获取质押
    pub fn stake(&self, agent_id: &str) -> Option<&StakeRecord> {
        self.stakes.get(agent_id)
    }

    /// 罚没质押
    pub fn slash_stake(&mut self, agent_id: &str, amount: f64) -> Result<f64, String> {
        let stake = self
            .stakes
            .get_mut(agent_id)
            .ok_or_else(|| format!("Agent {} 无质押记录", agent_id))?;

        if stake.amount < amount {
            return Err(format!(
                "质押不足：{} 有 {}，需罚没 {}",
                agent_id, stake.amount, amount
            ));
        }

        stake.amount -= amount;
        if stake.amount <= 0.0 {
            stake.status = StakeStatus::Slashed;
        }

        // 同时降低诚信分
        if let Some(rep) = self.reputations.get_mut(agent_id) {
            rep.penalize_dishonesty(0.3);
        }

        Ok(amount)
    }

    /// 信誉排行榜
    pub fn leaderboard(&self, limit: usize) -> Vec<(String, f64)> {
        let mut entries: Vec<_> = self
            .reputations
            .iter()
            .map(|(id, rep)| (id.clone(), rep.overall()))
            .collect();
        entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        entries.into_iter().take(limit).collect()
    }

    /// 检查 Agent 是否有资格（质押 + 信誉门槛）
    pub fn is_eligible(&self, agent_id: &str, min_reputation: f64) -> bool {
        let has_stake = self
            .stakes
            .get(agent_id)
            .map(|s| s.status == StakeStatus::Locked && s.amount >= self.min_stake)
            .unwrap_or(false);

        let has_reputation = self
            .reputations
            .get(agent_id)
            .map(|r| r.overall() >= min_reputation)
            .unwrap_or(false);

        has_stake && has_reputation
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}
