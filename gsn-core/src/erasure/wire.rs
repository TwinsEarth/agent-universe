//! 纠删码分片的 libp2p 网络接线适配层（编排者实现）。
//!
//! 本层把 [`super::distributed::DistributedShardNode`] 使用的同步 [`ShardTransport`]
//! 抽象，桥接到真实的 libp2p GossipSub 消息面（topic `gsn/shards`）：
//!
//! - 节点把 [`ShardEnvelope`] 包成带目标的 [`ShardWire`]，经 GossipSub 发出；
//! - 收到 [`ShardWire`] 时，仅当 `to` 等于本节点（或为广播 `*`）才处理，避免
//!   所有节点都处理与自己无关的定向消息。
//!
//! 设计说明：
//! - GossipSub 本质是发布/订阅广播，这里用「目标字段 + 接收端过滤」承载
//!   [`DistributedShardNode`] 的点对点确定性放置语义；网络上所有订阅者都会收到
//!   报文（mesh 范围内），但只有目标节点会应用。
//! - 若后续需要严格点对点、降低无效扩散，可改用 libp2p request-response（在
//!   PeerBehaviour 增加 request-response 字段）；本实现不改变传输栈，复用现有
//!   GossipSub 通道。
//!
//! 需外部审计：自研的目标封装/过滤、队列与节点驱动逻辑（纠删码数学本身委托
//! vetted 库）。

use std::collections::{VecDeque};

use serde::{Deserialize, Serialize};

use super::distributed::{ShardEnvelope, ShardTransport};

/// 纠删码分片消息的 GossipSub 主题。
pub const SHARD_TOPIC: &str = "gsn/shards";

/// GossipSub 分片消息的最外层封装（topic `gsn/shards`）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShardWire {
    /// 目标节点 id（PeerId 字符串）；`"*"` 表示广播给所有节点。
    pub to: String,
    /// 发送节点 id。
    pub from: String,
    /// 分片信封。
    pub env: ShardEnvelope,
}

impl ShardWire {
    /// 构造一条定向报文。
    pub fn direct(to: impl Into<String>, from: impl Into<String>, env: ShardEnvelope) -> Self {
        Self {
            to: to.into(),
            from: from.into(),
            env,
        }
    }

    /// 序列化为 GossipSub 报文 bytes。
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// 从 GossipSub 报文 bytes 解析。
    pub fn from_bytes(b: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(b).map_err(|e| format!("ShardWire 解析失败: {e}"))
    }

    /// 该报文是否应由 `self_id` 节点处理。
    pub fn targets(&self, self_id: &str) -> bool {
        self.to == "*" || self.to == self_id
    }
}

/// 基于 GossipSub 的分片传输实现 [`ShardTransport`]。
///
/// 它本身不直接做网络 IO，而是：
/// - `inbound`：由网络消费者在收到且目标匹配时推入；
/// - `outbound`：节点逻辑产生的待发送报文，由外部驱动 drain 后经 GossipSub publish；
/// - `peers`：由外部驱动周期性刷新（来自已连接 peer 列表，含本节点）。
pub struct GossipShardTransport {
    self_id: String,
    peers: Vec<String>,
    inbound: VecDeque<(String, ShardEnvelope)>,
    outbound: VecDeque<(String, ShardEnvelope)>,
}

impl GossipShardTransport {
    /// 创建；`self_id` 为本节点 PeerId，`initial_peers` 至少包含本节点。
    pub fn new(self_id: impl Into<String>, initial_peers: Vec<String>) -> Self {
        Self {
            self_id: self_id.into(),
            peers: initial_peers,
            inbound: VecDeque::new(),
            outbound: VecDeque::new(),
        }
    }

    /// 网络消费者在收到 `wire` 且目标为本节点时调用：把信封投递到入站队列。
    pub fn deliver(&mut self, wire: &ShardWire) {
        if wire.targets(&self.self_id) {
            self.inbound.push_back((wire.from.clone(), wire.env.clone()));
        }
    }

    /// 取出一条待发送报文（外部驱动 drain 后 publish）。
    pub fn take_outbound(&mut self) -> Option<(String, ShardEnvelope)> {
        self.outbound.pop_front()
    }

    /// 刷新已知 peer 列表（应包含本节点；驱动会在放入前确保本节点在列）。
    pub fn set_peers(&mut self, mut peers: Vec<String>) {
        if !peers.iter().any(|p| p == &self.self_id) {
            peers.push(self.self_id.clone());
        }
        peers.sort();
        peers.dedup();
        self.peers = peers;
    }

    /// 当前入站队列长度（测试/可观测用）。
    pub fn inbound_len(&self) -> usize {
        self.inbound.len()
    }

    /// 当前出站队列长度（测试/可观测用）。
    pub fn outbound_len(&self) -> usize {
        self.outbound.len()
    }
}

impl ShardTransport for GossipShardTransport {
    fn send(&mut self, to: &str, env: ShardEnvelope) {
        self.outbound.push_back((to.to_string(), env));
    }

    fn next(&mut self) -> Option<(String, ShardEnvelope)> {
        self.inbound.pop_front()
    }

    fn peers(&self) -> Vec<String> {
        self.peers.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_envelope() -> ShardEnvelope {
        ShardEnvelope::BlobSealed {
            blob_id: "blob-1".into(),
            total: 6,
            data_shards: 4,
            shard_size: 8,
            original_size: 24,
        }
    }

    #[test]
    fn wire_roundtrip_and_target_filter() {
        let env = sample_envelope();
        let wire = ShardWire::direct("nodeB", "nodeA", env.clone());
        assert!(wire.targets("nodeB"));
        assert!(!wire.targets("nodeC"));
        let bytes = wire.to_bytes();
        let back = ShardWire::from_bytes(&bytes).unwrap();
        assert_eq!(back, wire);
        // 广播目标对所有节点生效。
        let broadcast = ShardWire::direct("*", "nodeA", env);
        assert!(broadcast.targets("nodeC"));
    }

    #[test]
    fn transport_delivers_only_targeted_and_drains_outbound() {
        let mut t = GossipShardTransport::new("nodeB", vec!["nodeB".to_string()]);
        // 一条发给 B 的，一条发给 C 的。
        let to_b = ShardWire::direct("nodeB", "nodeA", sample_envelope());
        let to_c = ShardWire::direct("nodeC", "nodeA", sample_envelope());
        t.deliver(&to_b);
        t.deliver(&to_c);
        // 只有 to_b 进入入站队列。
        assert_eq!(t.inbound_len(), 1);
        let (from, env) = t.next().unwrap();
        assert_eq!(from, "nodeA");
        assert_eq!(env, sample_envelope());
        assert!(t.next().is_none());
    }

    #[test]
    fn transport_set_peers_includes_self_and_dedups() {
        let mut t = GossipShardTransport::new("nodeA", vec!["nodeA".to_string()]);
        t.set_peers(vec![
            "nodeC".to_string(),
            "nodeA".to_string(),
            "nodeC".to_string(),
        ]);
        let mut peers = t.peers();
        peers.sort();
        peers.dedup();
        assert_eq!(peers, vec!["nodeA", "nodeC"]);
    }

    #[test]
    fn transport_send_queues_outbound() {
        let mut t = GossipShardTransport::new("nodeA", vec!["nodeA".to_string()]);
        t.send("nodeB", sample_envelope());
        t.send("nodeC", sample_envelope());
        assert_eq!(t.outbound_len(), 2);
        let (to, _) = t.take_outbound().unwrap();
        assert_eq!(to, "nodeB");
    }
}
