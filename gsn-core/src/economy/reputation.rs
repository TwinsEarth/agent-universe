//! 信誉分系统
//!
//! 信誉 = 贡献 × 成功率 × 时间衰减
//! 信誉是智能体在网络中的"信用分数"

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ReputationRecord {
    pub agent_did: String,
    pub score: u16,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub total_value: u64,
    pub last_updated: u64,
}

pub struct ReputationSystem {
    records: HashMap<String, ReputationRecord>,
    /// 信誉衰减半衰期（秒）
    decay_halflife_secs: u64,
    /// 初始信誉
    initial_score: u16,
    /// 最大信誉
    max_score: u16,
}

impl ReputationSystem {
    pub fn new(decay_halflife_secs: u64) -> Self {
        Self {
            records: HashMap::new(),
            decay_halflife_secs,
            initial_score: 5000,
            max_score: 10000,
        }
    }

    pub fn register(&mut self, agent_did: String) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.records.insert(
            agent_did.clone(),
            ReputationRecord {
                agent_did,
                score: self.initial_score,
                tasks_completed: 0,
                tasks_failed: 0,
                total_value: 0,
                last_updated: now,
            },
        );
    }

    pub fn record_success(&mut self, agent_did: &str, value: u64) {
        if let Some(record) = self.records.get_mut(agent_did) {
            record.tasks_completed += 1;
            record.total_value += value;

            // 成功增加信誉
            let gain = (value as f64 * 0.01).round() as i32;
            let new_score = (record.score as i32 + gain).min(self.max_score as i32);
            record.score = new_score.max(0) as u16;

            record.last_updated = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
        }
    }

    pub fn record_failure(&mut self, agent_did: &str, penalty: u16) {
        if let Some(record) = self.records.get_mut(agent_did) {
            record.tasks_failed += 1;

            let new_score = (record.score as i32 - penalty as i32).max(0);
            record.score = new_score as u16;

            record.last_updated = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
        }
    }

    pub fn get_score(&self, agent_did: &str) -> u16 {
        self.records.get(agent_did).map(|r| r.score).unwrap_or(0)
    }

    pub fn apply_decay(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        for record in self.records.values_mut() {
            let elapsed = now - record.last_updated;
            // v3.5.3（AU-33）：halflife==0 视为「未配置半衰期、不做衰减」，直接跳过——
            // 否则下行 `elapsed / decay_halflife_secs` 在整数除零时 panic。
            if self.decay_halflife_secs == 0 {
                continue;
            }
            if elapsed > self.decay_halflife_secs {
                let halves = elapsed / self.decay_halflife_secs;
                let decay_factor = 2f64.powi(-(halves as i32));
                record.score = (record.score as f64 * decay_factor) as u16;
                record.last_updated = now;
            }
        }
    }

    pub fn top_reputation(&self, limit: usize) -> Vec<&ReputationRecord> {
        let mut records: Vec<_> = self.records.values().collect();
        records.sort_by_key(|r| std::cmp::Reverse(r.score));
        records.truncate(limit);
        records
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AU-33：halflife==0 时 apply_decay 不得整数除零 panic，且视为不衰减（分数不变）。
    /// 旧实现：elapsed>0 时下行 `elapsed / 0` 直接 panic。睡 1.1s 确定性令 elapsed>=1。
    #[test]
    fn au33_decay_halflife_zero_no_panic_and_no_decay() {
        let mut sys = ReputationSystem::new(0);
        sys.register("did:au33".to_string());
        std::thread::sleep(std::time::Duration::from_millis(1100));
        sys.apply_decay();
        assert_eq!(sys.get_score("did:au33"), 5000);
    }
}
