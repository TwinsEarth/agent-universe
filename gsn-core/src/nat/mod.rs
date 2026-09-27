//! NAT 穿透（数据模型 / 教学占位）
//!
//! **本模块的 `NatTraversalManager` 不执行真实 STUN/TURN/打洞**：
//! `gather_candidates` 返回空、`detect_nat_type` 返回固定值、`connect` 直接假设成功。
//! 它只描述 ICE 候选地址与 NAT 类型的数据结构，不产生网络流量。
//!
//! 真实 NAT 穿透由 libp2p 运行时承担：
//! - [`crate::net::peer::P2pPeer`]：AutoNAT 探测 + DCUtR 直连 + Circuit Relay 中继
//! - [`crate::node`]：三端实测的打洞 / 中继 / 多通道切换（v2.5.3–v2.5.5）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// NAT 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NatType {
    /// 完全开放
    OpenInternet,
    /// 全锥型 NAT
    FullCone,
    /// 限制锥型 NAT
    RestrictedCone,
    /// 端口限制锥型 NAT
    PortRestrictedCone,
    /// 对称型 NAT
    Symmetric,
}

/// ICE 候选
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidate {
    pub candidate_id: String,
    pub addr: String,
    pub port: u16,
    pub priority: u32,
    pub candidate_type: CandidateType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateType {
    Host,
    ServerReflexive, // STUN
    PeerReflexive,
    Relayed, // TURN
}

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    New,
    Checking,
    Connected,
    Disconnected,
    Failed,
}

/// NAT 穿透管理器
#[allow(dead_code)]
pub struct NatTraversalManager {
    stun_servers: Vec<String>,
    turn_servers: Vec<String>,
    local_candidates: Vec<IceCandidate>,
    connections: HashMap<String, ConnectionState>,
    nat_type: Option<NatType>,
}

impl Default for NatTraversalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NatTraversalManager {
    pub fn new() -> Self {
        Self {
            stun_servers: vec![
                "stun:stun.l.google.com:19302".to_string(),
                "stun:stun1.l.google.com:19302".to_string(),
            ],
            turn_servers: Vec::new(),
            local_candidates: Vec::new(),
            connections: HashMap::new(),
            nat_type: None,
        }
    }

    pub fn add_stun_server(&mut self, server: String) {
        self.stun_servers.push(server);
    }

    pub fn add_turn_server(&mut self, server: String) {
        self.turn_servers.push(server);
    }

    /// 收集本地候选地址
    pub fn gather_candidates(&mut self) -> Vec<IceCandidate> {
        // 占位：不查询网卡/STUN；真实候选收集见 net::peer（libp2p identify/AutoNAT）
        self.local_candidates.clone()
    }

    /// 检测 NAT 类型
    pub fn detect_nat_type(&mut self) -> NatType {
        // 占位：不探测，固定返回；真实 NAT 类型由 libp2p AutoNAT 判定
        NatType::PortRestrictedCone
    }

    /// 建立连接
    pub fn connect(&mut self, peer_id: String, _remote_candidates: Vec<IceCandidate>) -> ConnectionState {
        // 占位：不进行 ICE 协商，直接置为 Connected；真实打洞/中继见 net::peer
        let state = ConnectionState::Connected;
        self.connections.insert(peer_id, state);
        state
    }

    pub fn get_connection_state(&self, peer_id: &str) -> Option<&ConnectionState> {
        self.connections.get(peer_id)
    }

    pub fn connected_count(&self) -> usize {
        self.connections.values()
            .filter(|s| **s == ConnectionState::Connected)
            .count()
    }

    pub fn has_turn_relay(&self) -> bool {
        !self.turn_servers.is_empty()
    }

    pub fn stun_server_count(&self) -> usize {
        self.stun_servers.len()
    }
}
