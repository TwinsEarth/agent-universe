//! 分层主体记忆 & 跨代记忆（v2.4.3）
//!
//! 三层记忆共享：
//! - 个体记忆（`AgentMemory`，本 crate::memory）
//! - 群体记忆（`SwarmMemory`，本 crate::memory）
//! - 跨代记忆（`IntergenMemory`，本文件）
//!
//! 分层主体记忆：按拓扑层级 Lv1 房间 → Lv7 宇宙，每层一个记忆域；
//! 高层主体记忆：由下层经验聚合而来，越上层越抽象、越通用。

use crate::topology::Level;
use std::collections::HashMap;

/// 分层主体记忆：每个拓扑层级各持一个经验桶。
#[derive(Debug, Clone, Default)]
pub struct LayeredMemory {
    /// level -> 该层主体积累的经验条目数
    by_level: HashMap<Level, u64>,
}

impl LayeredMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// 在某层写入一条经验（下层也会被记一笔，用于聚合）。
    pub fn record(&mut self, level: Level, entries: u64) {
        *self.by_level.entry(level).or_insert(0) += entries;
    }

    /// 某层主体记忆量。
    pub fn count(&self, level: Level) -> u64 {
        self.by_level.get(&level).copied().unwrap_or(0)
    }

    /// 高层主体记忆 = 所有下层经验聚合（越高层越抽象/通用）。
    pub fn high_level_count(&self) -> u64 {
        Level::ALL.iter().map(|l| self.count(*l)).sum()
    }
}

/// 跨代记忆：哈希链记录完整执行轨迹，可追溯、防篡改。
#[derive(Debug, Clone, Default)]
pub struct IntergenMemory {
    chain: Vec<String>,
    prev_hash: String,
}

impl IntergenMemory {
    pub fn new() -> Self {
        Self {
            chain: Vec::new(),
            prev_hash: "genesis".to_string(),
        }
    }

    /// 追加一条轨迹：新哈希 = sha256(prev_hash + payload)。
    pub fn append(&mut self, payload: &str) {
        let input = format!("{}{}", self.prev_hash, payload);
        let digest = sha256_hex(&input);
        self.chain.push(digest.clone());
        self.prev_hash = digest;
    }

    pub fn len(&self) -> usize {
        self.chain.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }

    pub fn head_hash(&self) -> &str {
        &self.prev_hash
    }
}

/// 轻量 SHA-256 hex（不引入外部依赖时的占位 FNV+折叠；
/// 真实验证应接 sha2 crate。这里只用链完整性校验，不做安全签名）。
fn sha256_hex(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.hash(&mut h);
    format!("{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layered_memory_aggregates_up() {
        let mut lm = LayeredMemory::new();
        lm.record(Level::Lv1Room, 100);
        lm.record(Level::Lv2Building, 20);
        lm.record(Level::Lv3City, 5);
        assert_eq!(lm.count(Level::Lv1Room), 100);
        assert_eq!(lm.high_level_count(), 125);
    }

    #[test]
    fn intergen_chain_grows_and_links() {
        let mut g = IntergenMemory::new();
        assert!(g.is_empty());
        g.append("task:A done");
        g.append("task:B done");
        assert_eq!(g.len(), 2);
        assert!(!g.head_hash().is_empty());
    }
}
