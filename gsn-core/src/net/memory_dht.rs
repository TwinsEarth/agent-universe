//! 进程内 Kademlia 替身（in-memory，测试/演示用）
//!
//! **这不是真实 Kademlia DHT**：`put` / `get` / `remove` 只操作本地 `HashMap`，
//! 不与任何 peer 通信；`shard_of` 仅用 SHA-256 做本地一致性哈希定位。
//!
//! 真实分布式哈希表见 [`crate::net::peer::P2pPeer`]（libp2p Kademlia）与 [`crate::node`]。

use std::collections::HashMap;

/// 进程内 DHT 替身：仅本地键值存储，不联网。
pub struct InMemoryKademlia {
    store: HashMap<String, Vec<u8>>,
    pub shard_count: u32,
}

impl InMemoryKademlia {
    pub fn new(shard_count: u32) -> Self {
        Self {
            store: HashMap::new(),
            shard_count,
        }
    }

    pub fn shard_of(&self, key: &str) -> u32 {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        let num = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        num % self.shard_count
    }

    pub fn put(&mut self, key: String, value: Vec<u8>) {
        self.store.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.store.get(key)
    }

    pub fn remove(&mut self, key: &str) {
        self.store.remove(key);
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}
