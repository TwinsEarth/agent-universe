//! 分层主体记忆 & 跨代记忆（v2.4.3）
//!
//! 三层记忆共享：
//! - 个体记忆（`AgentMemory`，本 crate::memory）
//! - 群体记忆（`SwarmMemory`，本 crate::memory）
//! - 跨代记忆（`IntergenMemory`，本文件）
//!
//! 分层主体记忆：按拓扑层级 Lv1 房间 → Lv7 宇宙，每层一个记忆域；
//! 高层主体记忆：由下层经验聚合而来，越上层越抽象、越通用。
//!
//! v2.6.7：跨代记忆哈希链改用真实 SHA-256（`sha2` crate），同时**保存载荷**
//! （此前只存 64 位 DefaultHasher 摘要、丢弃载荷，且 DefaultHasher 跨工具链不稳定），
//! 新增 `verify_chain()` 重算每一环并报出**首个**断裂索引。

use crate::topology::Level;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 真实 SHA-256，返回 64 个十六进制字符。跨工具链稳定、密码学安全。
pub(crate) fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    let out = h.finalize();
    out.iter().map(|b| format!("{:02x}", b)).collect()
}

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

const GENESIS: &str = "genesis";

/// 跨代记忆：哈希链记录完整执行轨迹（载荷+哈希），可追溯、防篡改。
#[derive(Debug, Clone)]
pub struct IntergenMemory {
    /// 每一环：(载荷, 该环哈希)。载荷被保留以便复现与篡改检测。
    chain: Vec<(String, String)>,
    prev_hash: String,
}

impl Default for IntergenMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl IntergenMemory {
    pub fn new() -> Self {
        Self { chain: Vec::new(), prev_hash: GENESIS.to_string() }
    }

    /// 追加一条轨迹：环哈希 = sha256(prev_hash + payload)。
    pub fn append(&mut self, payload: &str) {
        let digest = sha256_hex(&format!("{}{}", self.prev_hash, payload));
        self.chain.push((payload.to_string(), digest.clone()));
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

    /// 第 i 环的载荷（0 起），用于复现轨迹。
    pub fn payload(&self, i: usize) -> Option<&str> {
        self.chain.get(i).map(|(p, _)| p.as_str())
    }

    /// 重算整条链：返回 `Ok(())` 表示从 genesis 到 head 逐环一致；
    /// 否则返回 `Err(index)` 指出**首个**断裂的环索引。
    pub fn verify_chain(&self) -> Result<(), usize> {
        let mut prev = GENESIS.to_string();
        for (i, (payload, stored_hash)) in self.chain.iter().enumerate() {
            let expect = sha256_hex(&format!("{}{}", prev, payload));
            if &expect != stored_hash {
                return Err(i);
            }
            prev = stored_hash.clone();
        }
        Ok(())
    }
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
        assert!(g.verify_chain().is_ok());
        // 真 SHA-256 = 64 hex
        assert_eq!(g.head_hash().len(), 64);
    }

    #[test]
    fn detect_tampered_payload_at_first_break() {
        let mut g = IntergenMemory::new();
        g.append("one");
        g.append("two");
        g.append("three");
        // 篡改第 1 环载荷（hash 随之不再匹配）
        g.chain[1].0 = "TWO-TAMPERED".to_string();
        assert_eq!(g.verify_chain(), Err(1));
    }
}
