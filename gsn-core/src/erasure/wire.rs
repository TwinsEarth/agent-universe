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

use std::collections::{HashSet, VecDeque};

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
/// - **放置集合（协议子网成员发现）**：真机 libp2p 的已连接 peer 里混有大量公共
///   IPFS/kubo 节点，它们不运行 `gsn/shards` 协议。若直接拿「全部连接」做 FNV 放置，
///   分片会被永久发给这些外部节点、NeedShards 也无人应答，导致重建硬失败。
///
/// 因此放置集合不等于全部连接，而等于：
/// ```text
///   (曾在 gsn/shards 上实际观测到的协议节点) ∩ (当前仍连接的节点) ∪ { self }
/// ```
/// - `observed`：只要收到一条来自 `from` 的合法分片报文（含 Hello 心跳），就证明
///   `from` 在运行本协议，记入观测集（只增）；
/// - `alive`：外部驱动周期性用「当前真实连接集合」调用 [`GossipShardTransport::set_peers`]
///   作为存活上界，把已断连的旧协议成员裁剪掉，避免成员表只增不减。
pub struct GossipShardTransport {
    self_id: String,
    /// 累计观测到的协议节点（曾在 gsn/shards 上发出过合法报文的节点）。
    observed: HashSet<String>,
    /// 当前存活的协议成员 = (observed ∩ connected) ∪ { self }；即纠删码放置集合。
    alive: HashSet<String>,
    inbound: VecDeque<(String, ShardEnvelope)>,
    outbound: VecDeque<(String, ShardEnvelope)>,
}

impl GossipShardTransport {
    /// 创建；`self_id` 为本节点 PeerId。初始放置集合仅含本节点（尚无任何协议对端观测）。
    pub fn new(self_id: impl Into<String>, _initial_peers: Vec<String>) -> Self {
        let self_id = self_id.into();
        let mut alive = HashSet::new();
        alive.insert(self_id.clone());
        Self {
            self_id,
            observed: HashSet::new(),
            alive,
            inbound: VecDeque::new(),
            outbound: VecDeque::new(),
        }
    }

    /// 网络消费者在收到 `wire` 时调用：
    /// 1) 无论报文是否定向给本节点，只要它是来自 `from` 的合法 gsn/shards 报文，
    ///    就把 `from` 记为协议成员（能在本主题发出可解析报文即证明其运行本协议）；
    /// 2) 仅当目标匹配本节点（或广播 `*`）才把信封投递到入站队列。
    pub fn deliver(&mut self, wire: &ShardWire) {
        if !wire.from.is_empty() && wire.from != self.self_id {
            self.observed.insert(wire.from.clone());
            // 刚收到该成员的报文，它必然在线，立即纳入存活放置集。
            self.alive.insert(wire.from.clone());
        }
        if wire.targets(&self.self_id) {
            self.inbound
                .push_back((wire.from.clone(), wire.env.clone()));
        }
    }

    /// 取出一条待发送报文（外部驱动 drain 后 publish）。
    pub fn take_outbound(&mut self) -> Option<(String, ShardEnvelope)> {
        self.outbound.pop_front()
    }

    /// 用「当前真实的 libp2p 已连接 peer 集合」裁剪存活协议成员。
    ///
    /// 注意：这里传入的是**全部连接**（含公共 IPFS），但只有此前在 gsn/shards 上
    /// 被观测过的节点才会保留——公共 IPFS 从未进入 `observed`，故永不进入放置集合。
    /// 本节点始终在存活集中（即使它暂时不在连接列表里，例如根种子启动瞬间）。
    pub fn set_peers(&mut self, connected: Vec<String>) {
        let connected_set: HashSet<String> = connected.into_iter().collect();
        let mut next: HashSet<String> = self
            .observed
            .iter()
            .filter(|p| connected_set.contains(*p))
            .cloned()
            .collect();
        next.insert(self.self_id.clone());
        self.alive = next;
    }

    /// 当前入站队列长度（测试/可观测用）。
    pub fn inbound_len(&self) -> usize {
        self.inbound.len()
    }

    /// 当前出站队列长度（测试/可观测用）。
    pub fn outbound_len(&self) -> usize {
        self.outbound.len()
    }

    /// 当前存活协议成员数（含本节点；用于 /metrics 与放置集合可观测）。
    pub fn protocol_peer_count(&self) -> usize {
        self.alive.len()
    }
}

impl ShardTransport for GossipShardTransport {
    fn send(&mut self, to: &str, env: ShardEnvelope) {
        self.outbound.push_back((to.to_string(), env));
    }

    fn next(&mut self) -> Option<(String, ShardEnvelope)> {
        self.inbound.pop_front()
    }

    /// 纠删码确定性放置/拉取集合：仅返回存活的 gsn 协议子网成员（含本节点），
    /// 已排序去重，保证所有节点对同一集合的 FNV 放置决策一致。
    fn peers(&self) -> Vec<String> {
        let mut v: Vec<String> = self.alive.iter().cloned().collect();
        v.sort();
        v.dedup();
        v
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
    fn transport_peers_only_include_observed_protocol_members() {
        let mut t = GossipShardTransport::new("nodeA", vec!["nodeA".to_string()]);
        // 初始：放置集合只含本节点，即便随后喂入大量「全部连接」也不扩大。
        t.set_peers(vec![
            "ipfs-x".to_string(),
            "ipfs-y".to_string(),
            "nodeA".to_string(),
        ]);
        assert_eq!(t.peers(), vec!["nodeA"]);
        assert_eq!(t.protocol_peer_count(), 1);

        // 收到一条来自 nodeC 的合法 gsn/shards 报文 → nodeC 被观测为协议成员。
        let from_c = ShardWire::direct("nodeA", "nodeC", sample_envelope());
        t.deliver(&from_c);
        assert_eq!(t.peers(), vec!["nodeA", "nodeC"]);

        // 再用「全部连接」刷新：nodeC 仍连接则保留；公共 IPFS 永不进入。
        t.set_peers(vec![
            "ipfs-x".to_string(),
            "ipfs-y".to_string(),
            "nodeA".to_string(),
            "nodeC".to_string(),
        ]);
        assert_eq!(t.peers(), vec!["nodeA", "nodeC"]);
    }

    #[test]
    fn transport_set_peers_prunes_disconnected_members() {
        let mut t = GossipShardTransport::new("nodeA", vec!["nodeA".to_string()]);
        // nodeC 曾被观测为协议成员。
        t.deliver(&ShardWire::direct("nodeA", "nodeC", sample_envelope()));
        assert_eq!(t.protocol_peer_count(), 2);
        // 下一轮刷新时 nodeC 已不在连接集合 → 从存活放置集裁剪（self 恒在）。
        t.set_peers(vec!["ipfs-x".to_string(), "nodeA".to_string()]);
        assert_eq!(t.peers(), vec!["nodeA"]);
    }

    #[test]
    fn transport_does_not_register_self_or_empty_from() {
        // deliver 只记录「报文真实来源」；调用方（node.rs 驱动）用 GossipSub 认证
        // source 覆盖 wire.from。self 与空来源都不应产生额外放置成员。
        let mut t = GossipShardTransport::new("nodeA", vec!["nodeA".to_string()]);
        let from_self = ShardWire::direct("nodeA", "nodeA", sample_envelope());
        t.deliver(&from_self);
        assert_eq!(t.peers(), vec!["nodeA"]);
        let mut empty = ShardWire::direct("nodeA", "nodeA", sample_envelope());
        empty.from = String::new();
        t.deliver(&empty);
        assert_eq!(t.peers(), vec!["nodeA"]);
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
