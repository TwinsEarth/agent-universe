//! 邻居节点管理
//! 
//! Kademlia 风格的 k-bucket 邻居管理

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Neighbor {
    pub did: String,
    pub address: String,
    pub latency_ms: u64,
    pub last_seen: u64,
    pub failed_pings: u32,
}

pub struct NeighborManager {
    neighbors: HashMap<String, Neighbor>,
    k_bucket_size: usize,
    max_failed_pings: u32,
}

impl NeighborManager {
    pub fn new(k_bucket_size: usize) -> Self {
        Self {
            neighbors: HashMap::new(),
            k_bucket_size,
            max_failed_pings: 3,
        }
    }

    pub fn add_neighbor(&mut self, did: String, address: String, latency_ms: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.neighbors.insert(did.clone(), Neighbor {
            did,
            address,
            latency_ms,
            last_seen: now,
            failed_pings: 0,
        });
    }

    pub fn record_ping_success(&mut self, did: &str, latency_ms: u64) {
        if let Some(neighbor) = self.neighbors.get_mut(did) {
            neighbor.latency_ms = (neighbor.latency_ms + latency_ms) / 2;
            neighbor.failed_pings = 0;
            neighbor.last_seen = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }

    pub fn record_ping_failure(&mut self, did: &str) {
        if let Some(neighbor) = self.neighbors.get_mut(did) {
            neighbor.failed_pings += 1;
        }
    }

    pub fn remove_unresponsive(&mut self) -> Vec<String> {
        let failed: Vec<String> = self.neighbors.iter()
            .filter(|(_, n)| n.failed_pings >= self.max_failed_pings)
            .map(|(did, _)| did.clone())
            .collect();

        for did in &failed {
            self.neighbors.remove(did);
        }

        failed
    }

    pub fn nearest_neighbors(&self, count: usize) -> Vec<&Neighbor> {
        let mut neighbors: Vec<_> = self.neighbors.values().collect();
        neighbors.sort_by_key(|n| n.latency_ms);
        neighbors.truncate(count.min(self.k_bucket_size));
        neighbors
    }

    pub fn neighbor_count(&self) -> usize {
        self.neighbors.len()
    }

    pub fn get_neighbor(&self, did: &str) -> Option<&Neighbor> {
        self.neighbors.get(did)
    }
}
