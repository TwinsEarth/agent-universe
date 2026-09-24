//! Agent 协作分层架构
//!
//! 三层结构：MCP Agent（中央资源调度）→ Route Agent（边缘路由转发）→ End Agent（终端设备交互）
//! Group/Scene 逻辑隔离，支持家庭、企业、教育等多场景

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent 层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentTier {
    /// 中央资源调度层
    McpAgent,
    /// 边缘路由转发层
    RouteAgent,
    /// 终端设备交互层
    EndAgent,
}

/// 协作组（Group）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationGroup {
    pub group_id: String,
    pub name: String,
    pub scene: SceneType,
    pub members: HashMap<String, AgentTier>,
    pub created_at: u64,
}

/// 场景类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneType {
    Home,
    Enterprise,
    Education,
    Research,
    Public,
}

/// 协作消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationMessage {
    pub message_id: String,
    pub group_id: String,
    pub sender: String,
    pub recipient: Option<String>,
    pub msg_type: MessageType,
    pub payload: serde_json::Value,
    pub ttl: u32,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    Discovery,
    TaskProposal,
    TaskAccept,
    TaskReject,
    ResultDelivery,
    Heartbeat,
    Leave,
}

/// 协作管理器
#[allow(dead_code)]
pub struct CollaborationManager {
    groups: HashMap<String, CollaborationGroup>,
    messages: Vec<CollaborationMessage>,
    local_agent_id: String,
    local_tier: AgentTier,
}

impl CollaborationManager {
    pub fn new(agent_id: String, tier: AgentTier) -> Self {
        Self {
            groups: HashMap::new(),
            messages: Vec::new(),
            local_agent_id: agent_id,
            local_tier: tier,
        }
    }

    pub fn create_group(&mut self, name: String, scene: SceneType) -> String {
        let group_id = format!("grp_{}", uuid::Uuid::new_v4());
        let group = CollaborationGroup {
            group_id: group_id.clone(),
            name,
            scene,
            members: HashMap::new(),
            created_at: chrono::Utc::now().timestamp() as u64,
        };
        self.groups.insert(group_id.clone(), group);
        group_id
    }

    pub fn join_group(&mut self, group_id: String, agent_id: String, tier: AgentTier) -> bool {
        if let Some(group) = self.groups.get_mut(&group_id) {
            group.members.insert(agent_id, tier);
            true
        } else {
            false
        }
    }

    pub fn send_message(&mut self, msg: CollaborationMessage) {
        self.messages.push(msg);
    }

    pub fn get_groups(&self) -> Vec<&CollaborationGroup> {
        self.groups.values().collect()
    }

    pub fn local_tier(&self) -> AgentTier {
        self.local_tier
    }
}
