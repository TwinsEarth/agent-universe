//! Kademlia DHT 客户端（跨平台轻量版）

use std::collections::HashMap;

pub struct KademliaClient {
    store: HashMap<String, Vec<u8>>,
    pub shard_count: u32,
}

impl KademliaClient {
    pub fn new(shard_count: u32) -> Self {
        Self {
            store: HashMap::new(),
            shard_count,
        }
    }

    pub fn shard_of(&self, key: &str) -> u32 {
        use sha2::{Sha256, Digest};
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
}
