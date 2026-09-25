//! 群体外部共享记忆库（SwarmMemory）
//!
//! 所有智能体可发布经验（成功策略 / 失败教训），按发布者信誉加权检索。
//! 这是「后训练提升命中率」的载体：新任务冷启动时直接读群体经验。

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SharedEntry {
    pub agent_id: String,
    pub task_key: String,
    pub outcome: bool,
    pub strategy: String,
    /// 发布者信誉权重（0.0~1.0），检索时加权。
    pub weight: f64,
}

#[derive(Debug, Clone, Default)]
pub struct SwarmMemory {
    entries: HashMap<String, Vec<SharedEntry>>,
    publish_count: u64,
}

impl SwarmMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// 发布一条经验（成功或失败）。
    pub fn publish(&mut self, e: SharedEntry) {
        self.entries.entry(e.task_key.clone()).or_default().push(e);
        self.publish_count += 1;
    }

    /// 检索某任务的群体经验：返回按信誉加权后的最佳成功策略；
    /// 若无成功经验但有失败教训，返回 None 并标注需谨慎。
    pub fn best_strategy(&self, task_key: &str) -> Option<String> {
        let entries = self.entries.get(task_key)?;
        // 仅在成功经验里选；权重最高者胜出
        entries
            .iter()
            .filter(|e| e.outcome)
            .max_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap())
            .map(|e| e.strategy.clone())
    }

    /// 失败教训数量（用于提示新 Agent 避开哪些坑）。
    pub fn known_failures(&self, task_key: &str) -> usize {
        self.entries
            .get(task_key)
            .map(|v| v.iter().filter(|e| !e.outcome).count())
            .unwrap_or(0)
    }

    pub fn publish_count(&self) -> u64 {
        self.publish_count
    }

    pub fn task_keys(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_weighted_success_strategy() {
        let mut s = SwarmMemory::new();
        s.publish(SharedEntry {
            agent_id: "a1".into(), task_key: "ocr".into(), outcome: true,
            strategy: "tesseract".into(), weight: 0.5,
        });
        s.publish(SharedEntry {
            agent_id: "a2".into(), task_key: "ocr".into(), outcome: true,
            strategy: "paddleocr".into(), weight: 0.9,
        });
        s.publish(SharedEntry {
            agent_id: "a3".into(), task_key: "ocr".into(), outcome: false,
            strategy: "manual".into(), weight: 0.9,
        });
        assert_eq!(s.best_strategy("ocr"), Some("paddleocr".into()));
        assert_eq!(s.known_failures("ocr"), 1);
    }

    #[test]
    fn empty_when_no_success() {
        let s = SwarmMemory::new();
        assert_eq!(s.best_strategy("unknown"), None);
    }
}
