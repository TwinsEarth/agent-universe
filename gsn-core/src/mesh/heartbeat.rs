//! 心跳机制：定期探测 peer 存活状态

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 心跳配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    /// 心跳间隔（秒）
    pub interval_secs: u64,
    /// 超时阈值：连续多少次未响应判定为离线
    pub miss_threshold: u32,
    /// 探测超时（秒）
    pub probe_timeout_secs: u64,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval_secs: 15,
            miss_threshold: 3,
            probe_timeout_secs: 5,
        }
    }
}

/// Peer 存活状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerLiveness {
    /// 在线（最近一次心跳成功）
    Online,
    /// 可疑（有心跳丢失但未超阈值）
    Suspicious,
    /// 离线（连续丢失超过阈值）
    Offline,
}

/// 单个 peer 的心跳追踪
#[derive(Debug, Clone)]
struct HeartbeatState {
    liveness: PeerLiveness,
    consecutive_misses: u32,
    last_seen_tick: u64,
    rtt_ms: u64,
}

/// 心跳追踪器：管理所有 peer 的存活状态
pub struct HeartbeatTracker {
    config: HeartbeatConfig,
    peers: HashMap<String, HeartbeatState>,
    current_tick: u64,
}

impl HeartbeatTracker {
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            peers: HashMap::new(),
            current_tick: 0,
        }
    }

    /// 注册一个新 peer
    pub fn register(&mut self, peer_id: &str) {
        self.peers.insert(
            peer_id.to_string(),
            HeartbeatState {
                liveness: PeerLiveness::Online,
                consecutive_misses: 0,
                last_seen_tick: self.current_tick,
                rtt_ms: 0,
            },
        );
    }

    /// 收到某 peer 的心跳响应
    pub fn record_pong(&mut self, peer_id: &str, rtt_ms: u64) {
        if let Some(state) = self.peers.get_mut(peer_id) {
            state.liveness = PeerLiveness::Online;
            state.consecutive_misses = 0;
            state.last_seen_tick = self.current_tick;
            state.rtt_ms = rtt_ms;
        }
    }

    /// 某 peer 心跳超时（未收到响应）
    pub fn record_miss(&mut self, peer_id: &str) {
        if let Some(state) = self.peers.get_mut(peer_id) {
            state.consecutive_misses += 1;
            if state.consecutive_misses >= self.config.miss_threshold {
                state.liveness = PeerLiveness::Offline;
            } else {
                state.liveness = PeerLiveness::Suspicious;
            }
        }
    }

    /// 推进一个心跳周期
    pub fn tick(&mut self) {
        self.current_tick += 1;
    }

    pub fn liveness(&self, peer_id: &str) -> Option<PeerLiveness> {
        self.peers.get(peer_id).map(|s| s.liveness)
    }

    pub fn rtt_ms(&self, peer_id: &str) -> Option<u64> {
        self.peers.get(peer_id).map(|s| s.rtt_ms)
    }

    pub fn online_peers(&self) -> Vec<String> {
        self.peers
            .iter()
            .filter(|(_, s)| s.liveness == PeerLiveness::Online)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 可用 peers：Online + Suspicious（未确认离线）
    pub fn available_peers(&self) -> Vec<String> {
        let mut v: Vec<String> = self.peers
            .iter()
            .filter(|(_, s)| s.liveness != PeerLiveness::Offline)
            .map(|(id, _)| id.clone())
            .collect();
        v.sort();
        v
    }

    pub fn offline_peers(&self) -> Vec<String> {
        self.peers
            .iter()
            .filter(|(_, s)| s.liveness == PeerLiveness::Offline)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn config(&self) -> &HeartbeatConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_peer_is_online() {
        let mut h = HeartbeatTracker::new(HeartbeatConfig::default());
        h.register("peer-a");
        assert_eq!(h.liveness("peer-a"), Some(PeerLiveness::Online));
        assert_eq!(h.online_peers(), vec!["peer-a"]);
    }

    #[test]
    fn miss_below_threshold_is_suspicious() {
        let mut h = HeartbeatTracker::new(HeartbeatConfig { miss_threshold: 3, ..Default::default() });
        h.register("peer-a");
        h.record_miss("peer-a");
        assert_eq!(h.liveness("peer-a"), Some(PeerLiveness::Suspicious));
        h.record_miss("peer-a");
        assert_eq!(h.liveness("peer-a"), Some(PeerLiveness::Suspicious));
    }

    #[test]
    fn miss_at_threshold_is_offline() {
        let mut h = HeartbeatTracker::new(HeartbeatConfig { miss_threshold: 3, ..Default::default() });
        h.register("peer-a");
        h.record_miss("peer-a");
        h.record_miss("peer-a");
        h.record_miss("peer-a");
        assert_eq!(h.liveness("peer-a"), Some(PeerLiveness::Offline));
        assert_eq!(h.offline_peers(), vec!["peer-a"]);
        assert_eq!(h.online_peers().len(), 0);
    }

    #[test]
    fn pong_resets_miss_count() {
        let mut h = HeartbeatTracker::new(HeartbeatConfig { miss_threshold: 3, ..Default::default() });
        h.register("peer-a");
        h.record_miss("peer-a");
        h.record_miss("peer-a");
        assert_eq!(h.liveness("peer-a"), Some(PeerLiveness::Suspicious));
        h.record_pong("peer-a", 42);
        assert_eq!(h.liveness("peer-a"), Some(PeerLiveness::Online));
        assert_eq!(h.rtt_ms("peer-a"), Some(42));
    }

    #[test]
    fn multiple_peers_independent_tracking() {
        let mut h = HeartbeatTracker::new(HeartbeatConfig::default());
        h.register("mac-mini");
        h.register("cloud-linux");
        h.register("win-pc");
        assert_eq!(h.peer_count(), 3);

        h.record_miss("win-pc");
        h.record_miss("win-pc");
        h.record_miss("win-pc");
        assert_eq!(h.liveness("win-pc"), Some(PeerLiveness::Offline));
        assert_eq!(h.liveness("mac-mini"), Some(PeerLiveness::Online));
        assert_eq!(h.liveness("cloud-linux"), Some(PeerLiveness::Online));
        assert_eq!(h.online_peers().len(), 2);
    }
}
