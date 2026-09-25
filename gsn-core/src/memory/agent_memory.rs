//! 个体智能体内部记忆库
//!
//! 记录本 Agent 的成功/失败轨迹与学到的模式；不依赖网络，纯本地。

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MemoryRecord {
    pub task_key: String,
    pub outcome: bool, // true=success, false=failure
    pub strategy: String,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct AgentMemory {
    agent_id: String,
    success: Vec<MemoryRecord>,
    failure: Vec<MemoryRecord>,
    /// task_key -> 本 Agent 验证过的最佳策略
    patterns: HashMap<String, String>,
}

impl AgentMemory {
    pub fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            success: Vec::new(),
            failure: Vec::new(),
            patterns: HashMap::new(),
        }
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// 记录一次成功执行；若该策略优于旧记录则更新模式。
    pub fn record_success(&mut self, task_key: &str, strategy: &str, note: &str) {
        self.success.push(MemoryRecord {
            task_key: task_key.to_string(),
            outcome: true,
            strategy: strategy.to_string(),
            note: note.to_string(),
        });
        self.patterns
            .entry(task_key.to_string())
            .and_modify(|s| *s = strategy.to_string())
            .or_insert_with(|| strategy.to_string());
    }

    pub fn record_failure(&mut self, task_key: &str, strategy: &str, note: &str) {
        self.failure.push(MemoryRecord {
            task_key: task_key.to_string(),
            outcome: false,
            strategy: strategy.to_string(),
            note: note.to_string(),
        });
    }

    /// 召回某任务的本地经验。
    pub fn recall(&self, task_key: &str) -> Option<&String> {
        self.patterns.get(task_key)
    }

    /// 历史命中率 = 成功 / (成功+失败)。
    pub fn hit_rate(&self) -> f64 {
        let total = self.success.len() + self.failure.len();
        if total == 0 {
            return 0.0;
        }
        self.success.len() as f64 / total as f64
    }

    pub fn total_records(&self) -> usize {
        self.success.len() + self.failure.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_recall_pattern() {
        let mut m = AgentMemory::new("did:aip:0001");
        m.record_success("summarize", "map-reduce", "fast");
        m.record_failure("summarize", "single-pass", "timeout");
        assert_eq!(m.recall("summarize"), Some(&"map-reduce".to_string()));
        assert!((m.hit_rate() - 0.5).abs() < 1e-9);
        assert_eq!(m.total_records(), 2);
    }
}
