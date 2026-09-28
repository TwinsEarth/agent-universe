//! 个体记忆增强（v2.4.4）
//!
//! 回答三个问题：
//! 1. 检索：标签 + 关键词匹配（进程内原型；真实向量检索需 embedding，留接口）。
//! 2. 评价：每条记忆带 good/bad 评分，只有高分经验才允许上群体库，防污染。
//! 3. 压缩：容量上限 + LRU 遗忘，不能无限增长。
//!
//! v2.6.7：
//! - 淘汰改真 LRU（按 `last_used` 逻辑时钟），此前按单调 `access_count` 实为 LFU；
//! - 评分改整数 bps（基点），排序用整数比较，不再 `partial_cmp().unwrap()`（NaN 即 panic）；
//! - 防污染阈值以整数 bps 表达。

use std::collections::HashMap;

/// 逻辑时钟：每写/查一次自增，供真 LRU 淘汰。
#[derive(Debug, Clone, Default)]
pub struct RatedMemory {
    pub key: String,
    pub tags: Vec<String>,
    pub payload: String,
    /// 评价分（整数基点 bps，1 bps = 0.01%）：>0 好经验，<0 坏经验。
    pub score_bps: i32,
    /// 最近使用的逻辑时钟值（越大越近）。
    pub last_used: u64,
}

impl RatedMemory {
    /// `score_bps`：整数基点。例如 9000 bps = 90% 好。
    pub fn new(key: &str, tags: &[&str], payload: &str, score_bps: i32) -> Self {
        Self {
            key: key.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            payload: payload.into(),
            score_bps,
            last_used: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnhancedMemory {
    store: HashMap<String, RatedMemory>,
    capacity: usize,
    clock: u64,
}

impl EnhancedMemory {
    pub fn new(capacity: usize) -> Self {
        Self { store: HashMap::new(), capacity: capacity.max(1), clock: 0 }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// 写入一条记忆；超容量时淘汰**最近最少使用**（last_used 最小）的一条。
    pub fn write(&mut self, mut m: RatedMemory) {
        if self.store.len() >= self.capacity && !self.store.contains_key(&m.key) {
            // 真 LRU：淘汰 last_used 最小（最久未访问）
            let victim = self.store
                .iter()
                .min_by_key(|(_, v)| v.last_used)
                .map(|(k, _)| k.clone());
            if let Some(v) = victim {
                self.store.remove(&v);
            }
        }
        m.last_used = self.tick();
        self.store.insert(m.key.clone(), m);
    }

    /// 检索：标签命中或关键词出现在 payload 里；按 score_bps 整数降序。
    pub fn query(&mut self, tag: &str, kw: &str) -> Vec<&RatedMemory> {
        let now = self.tick();
        let mut hits: Vec<(String, i32)> = self
            .store
            .iter()
            .filter(|(_, m)| m.tags.iter().any(|t| t == tag) || m.payload.contains(kw))
            .map(|(k, m)| (k.clone(), m.score_bps))
            .collect();
        // 整数降序比较，不使用浮点 partial_cmp
        hits.sort_by_key(|a| std::cmp::Reverse(a.1));
        // 命中即刷新 LRU 时间戳
        for (k, _) in &hits {
            if let Some(e) = self.store.get_mut(k) {
                e.last_used = now;
            }
        }
        hits.iter().map(|(k, _)| &self.store[k]).collect()
    }

    /// 防污染：只有 score_bps >= 阈值(bps) 的经验才允许发布到群体库。
    pub fn shareable(&self, key: &str, min_score_bps: i32) -> Option<&RatedMemory> {
        self.store.get(key).filter(|m| m.score_bps >= min_score_bps)
    }

    pub fn len(&self) -> usize { self.store.len() }
    pub fn is_empty(&self) -> bool { self.store.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_ranks_by_score_and_filters_bad() {
        let mut m = EnhancedMemory::new(100);
        m.write(RatedMemory::new("k1", &["ocr"], "paddleocr fast", 9000));
        m.write(RatedMemory::new("k2", &["ocr"], "tesseract slow", 3000));
        m.write(RatedMemory::new("k3", &["ocr"], "manual bad", -8000));
        let hits = m.query("ocr", "");
        assert_eq!(hits[0].key, "k1");
        // 防污染：5000 bps 以上才可共享
        assert!(m.shareable("k1", 5000).is_some());
        assert!(m.shareable("k3", 5000).is_none());
    }

    /// 真 LRU：在 LFU（按访问次数）规则下本测试会失败。
    ///
    /// 构造：a 被访问 3 次（LFU 下 a 计数最高、绝不会被淘汰）；
    /// 之后写 b、访问 b 一次，使 b 成为最近使用、a 成为最久未用。
    /// 按真 LRU 应淘汰 a（最久未用）；按 LFU 应淘汰 b（计数最小）。
    #[test]
    fn eviction_is_lru_not_lfu() {
        let mut m = EnhancedMemory::new(2);
        m.write(RatedMemory::new("a", &["x"], "alpha", 5000));
        // 访问 a 三次 -> LFU 视角 a 最热
        m.query("", "alpha");
        m.query("", "alpha");
        m.query("", "alpha");
        // 写 b 并访问一次 -> b 最近使用，a 最久未用
        m.write(RatedMemory::new("b", &["x"], "beta", 5000));
        m.query("", "beta");
        // 触发淘汰：LRU 淘汰 a；若按 LFU 会淘汰 b
        m.write(RatedMemory::new("c", &["x"], "gamma", 5000));
        assert_eq!(m.len(), 2);
        assert!(m.store.contains_key("b"), "b 是最近使用，应保留");
        assert!(!m.store.contains_key("a"), "a 最久未用，应被 LRU 淘汰（LFU 会错误保留 a）");
    }
}
