//! 个体记忆增强（v2.4.4）
//!
//! 回答三个问题：
//! 1. 检索：标签 + 关键词匹配（进程内原型；真实向量检索需 embedding，留接口）。
//! 2. 评价：每条记忆带 good/bad 评分，只有高分经验才允许上群体库，防污染。
//! 3. 压缩：容量上限 + LRU 遗忘，不能无限增长。

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RatedMemory {
    pub key: String,
    pub tags: Vec<String>,
    pub payload: String,
    /// 评价分：>0 好经验，<0 坏经验（避免重蹈覆辙）。
    pub score: f64,
    pub access_count: u32,
}

impl RatedMemory {
    pub fn new(key: &str, tags: &[&str], payload: &str, score: f64) -> Self {
        Self {
            key: key.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            payload: payload.into(),
            score,
            access_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnhancedMemory {
    store: HashMap<String, RatedMemory>,
    capacity: usize,
}

impl EnhancedMemory {
    pub fn new(capacity: usize) -> Self {
        Self { store: HashMap::new(), capacity: capacity.max(1) }
    }

    /// 写入一条记忆；超容量时淘汰最久未访问（access_count 最低）的一条。
    pub fn write(&mut self, m: RatedMemory) {
        if self.store.len() >= self.capacity && !self.store.contains_key(&m.key) {
            // LRU：淘汰 access_count 最小的
            let victim = self.store
                .iter()
                .min_by_key(|(_, v)| v.access_count)
                .map(|(k, _)| k.clone());
            if let Some(v) = victim {
                self.store.remove(&v);
            }
        }
        self.store.insert(m.key.clone(), m);
    }

    /// 检索：标签命中或关键词出现在 payload 里；按 score 排序。
    pub fn query(&mut self, tag: &str, kw: &str) -> Vec<&RatedMemory> {
        let mut keys: Vec<(String, f64)> = self.store
            .values()
            .filter(|m| m.tags.iter().any(|t| t == tag) || m.payload.contains(kw))
            .map(|m| (m.key.clone(), m.score))
            .collect();
        keys.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        // 更新访问计数（LRU）
        for (k, _) in &keys {
            if let Some(e) = self.store.get_mut(k) {
                e.access_count += 1;
            }
        }
        keys.iter().map(|(k, _)| &self.store[k]).collect()
    }

    /// 防污染：只有 score >= 阈值的经验才允许发布到群体库。
    pub fn shareable(&self, key: &str, min_score: f64) -> Option<&RatedMemory> {
        self.store.get(key).filter(|m| m.score >= min_score)
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
        m.write(RatedMemory::new("k1", &["ocr"], "paddleocr fast", 0.9));
        m.write(RatedMemory::new("k2", &["ocr"], "tesseract slow", 0.3));
        m.write(RatedMemory::new("k3", &["ocr"], "manual bad", -0.8));
        let hits = m.query("ocr", "");
        assert_eq!(hits[0].key, "k1");
        // 防污染：0.5 以上才可共享
        assert!(m.shareable("k1", 0.5).is_some());
        assert!(m.shareable("k3", 0.5).is_none());
    }

    #[test]
    fn lru_eviction_under_capacity() {
        let mut m = EnhancedMemory::new(2);
        m.write(RatedMemory::new("a", &["x"], "1", 0.5));
        m.write(RatedMemory::new("b", &["x"], "2", 0.5));
        // 访问 a，使 b 成为 LRU
        m.query("x", "a");
        m.write(RatedMemory::new("c", &["x"], "3", 0.5));
        assert_eq!(m.len(), 2);
        assert!(m.store.contains_key("a"));
        assert!(!m.store.contains_key("b"));
    }
}
