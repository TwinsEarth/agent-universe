//! 群体外部共享记忆库（SwarmMemory）
//!
//! 所有智能体可发布经验（成功策略 / 失败教训），按**派生质量**检索。
//! 这是「后训练提升命中率」的载体：新任务冷启动时直接读群体经验。
//!
//! v2.6.7 防污染重写：
//! - 不再接受发布者自报的 `weight`（恶意发布者设 1.0 即可胜出）；
//! - 质量由记录的成功/失败次数**派生**（成功率，整数 bps），发布者无法谎报；
//! - 派生质量低于阈值的经验**拒绝上群体库**；
//! - 排序用整数比较，不再 `partial_cmp().unwrap()`（NaN 即 panic）。

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SharedEntry {
    pub agent_id: String,
    pub task_key: String,
    pub strategy: String,
    /// 该策略被记录为成功的次数。
    pub successes: u32,
    /// 该策略被记录为失败的次数。
    pub failures: u32,
}

impl SharedEntry {
    pub fn new(
        agent_id: &str,
        task_key: &str,
        strategy: &str,
        successes: u32,
        failures: u32,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            task_key: task_key.into(),
            strategy: strategy.into(),
            successes,
            failures,
        }
    }

    /// 派生质量（整数 bps = 成功率 × 10000）。无样本则 0，拒绝上库。
    pub fn quality_bps(&self) -> i32 {
        let total = self.successes + self.failures;
        if total == 0 {
            0
        } else {
            (self.successes as i32) * 10_000 / (total as i32)
        }
    }
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

    /// 发布一条经验；派生质量 < `min_quality_bps` 时拒绝（防污染）。
    /// 失败教训（failures>successes）也允许记录，但不计入最佳策略。
    pub fn publish(&mut self, e: SharedEntry, min_quality_bps: i32) -> Result<(), String> {
        if e.quality_bps() < min_quality_bps {
            return Err(format!(
                "BAD_REQUEST: 经验质量 {} bps 低于阈值 {} bps，拒绝上群体库",
                e.quality_bps(),
                min_quality_bps
            ));
        }
        self.entries.entry(e.task_key.clone()).or_default().push(e);
        self.publish_count += 1;
        Ok(())
    }

    /// 检索某任务的群体经验：返回派生质量（成功率）最高的策略。
    pub fn best_strategy(&self, task_key: &str) -> Option<String> {
        let entries = self.entries.get(task_key)?;
        entries
            .iter()
            .max_by_key(|e| e.quality_bps())
            .map(|e| e.strategy.clone())
    }

    /// 失败教训数量（用于提示新 Agent 避开哪些坑）。
    pub fn known_failures(&self, task_key: &str) -> usize {
        self.entries
            .get(task_key)
            .map(|v| v.iter().filter(|e| e.failures > e.successes).count())
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
    fn picks_highest_derived_quality() {
        let mut s = SwarmMemory::new();
        // a1: 1成功0失败 = 10000 bps；a2: 8成功2失败 = 8000 bps
        s.publish(SharedEntry::new("a1", "ocr", "tesseract", 1, 0), 5000)
            .unwrap();
        s.publish(SharedEntry::new("a2", "ocr", "paddleocr", 8, 2), 5000)
            .unwrap();
        assert_eq!(s.best_strategy("ocr"), Some("tesseract".into()));
    }

    #[test]
    fn rejects_low_quality_entry() {
        let mut s = SwarmMemory::new();
        // 1成功4失败 = 2000 bps < 5000 阈值，拒绝
        let r = s.publish(SharedEntry::new("bad", "ocr", "manual", 1, 4), 5000);
        assert!(r.is_err());
        assert_eq!(s.publish_count(), 0);
        assert_eq!(s.best_strategy("ocr"), None);
    }

    #[test]
    fn empty_when_no_success() {
        let s = SwarmMemory::new();
        assert_eq!(s.best_strategy("unknown"), None);
    }
}
