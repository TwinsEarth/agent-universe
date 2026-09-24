//! NAT 穿透
//!
//! STUN、TURN、UDP 打洞
//! ICE 框架实现 P2P 直连

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
        // 实际实现：查询本机网卡、STUN 服务器获取 reflexive 地址
        self.local_candidates.clone()
    }

    /// 检测 NAT 类型
    pub fn detect_nat_type(&mut self) -> NatType {
        // 实际实现：向多个 STUN 服务器发送请求，比较返回的映射地址
        // 简化：返回默认值
        NatType::PortRestrictedCone
    }

    /// 建立连接
    pub fn connect(&mut self, peer_id: String, _remote_candidates: Vec<IceCandidate>) -> ConnectionState {
        // 实际实现：ICE 协商，尝试打洞
        // 简化：假设连接成功
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
