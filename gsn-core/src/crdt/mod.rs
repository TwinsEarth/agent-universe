//! CRDT 无冲突复制数据类型
//!
//! - [`VersionVector`]：版本向量（计数与 pointwise-max 合并）；
//! - [`store`]：LWW 键值状态存储 + 同步消息（已接线到 net 同步路径）。

pub mod store;

pub use store::{
    CrdtMessage, CrdtOp, CrdtSnapshot, CrdtStats, CrdtStore, LwwEntry, CRDT_TOPIC,
    DEFAULT_MAX_KEYS,
};

use std::collections::HashMap;

/// 基于版本向量的 CRDT 合并
#[derive(Debug, Clone)]
pub struct VersionVector {
    versions: HashMap<String, u64>,
}

impl Default for VersionVector {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionVector {
    pub fn new() -> Self {
        Self {
            versions: HashMap::new(),
        }
    }

    pub fn increment(&mut self, node_id: &str) {
        let entry = self.versions.entry(node_id.to_string()).or_insert(0);
        *entry += 1;
    }

    pub fn get(&self, node_id: &str) -> u64 {
        self.versions.get(node_id).copied().unwrap_or(0)
    }

    pub fn merge(&mut self, other: &VersionVector) {
        for (node, version) in &other.versions {
            let entry = self.versions.entry(node.clone()).or_insert(0);
            *entry = (*entry).max(*version);
        }
    }
}
