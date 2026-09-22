//! 群体智能核心模块
//! 
//! 群体智能 = 网络结构的 Scaling Law
//! 从 token 网络结构向智能体网络结构演进

use std::collections::HashMap;
use crate::identity::Did;
use crate::agent::AgentCard;

/// 群体中的智能体节点
#[derive(Debug, Clone)]
pub struct AgentNode {
    pub did: Did,
    pub card: AgentCard,
    pub stake: u64,
    pub reputation: u16,
    pub online: bool,
    pub tasks_completed: u64,
    pub success_rate: f64,
}

impl AgentNode {
    pub fn new(did: Did, card: AgentCard) -> Self {
        Self {
            did,
            card,
            stake: 0,
            reputation: 5000,
            online: true,
            tasks_completed: 0,
            success_rate: 1.0,
        }
    }

    pub fn with_stake(mut self, stake: u64) -> Self {
        self.stake = stake;
        self
    }

    pub fn update_reputation(&mut self, delta: i16) {
        let new_rep = (self.reputation as i32 + delta as i32).clamp(0, 10000);
        self.reputation = new_rep as u16;
    }

    pub fn record_task(&mut self, success: bool) {
        self.tasks_completed += 1;
        if success {
            self.success_rate = (self.success_rate * (self.tasks_completed - 1) as f64 + 1.0) / self.tasks_completed as f64;
        } else {
            self.success_rate = (self.success_rate * (self.tasks_completed - 1) as f64) / self.tasks_completed as f64;
        }
    }

    /// 综合评分 = 信誉 × 成功率 × 质押权重
    pub fn score(&self) -> f64 {
        if !self.online {
            return 0.0;
        }
        let rep_factor = self.reputation as f64 / 10000.0;
        let stake_factor = (self.stake as f64).ln().max(1.0) / 10.0;
        rep_factor * self.success_rate * (1.0 + stake_factor * 0.1)
    }
}

/// 群体智能网络
#[derive(Debug, Clone)]
pub struct Swarm {
    nodes: HashMap<String, AgentNode>,
    total_stake: u64,
    task_count: u64,
}

impl Swarm {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            total_stake: 0,
            task_count: 0,
        }
    }

    pub fn join(&mut self, node: AgentNode) {
        self.total_stake += node.stake;
        self.nodes.insert(node.did.as_str().to_string(), node);
    }

    pub fn leave(&mut self, did: &Did) {
        if let Some(node) = self.nodes.remove(did.as_str()) {
            self.total_stake -= node.stake;
        }
    }

    pub fn get_node(&self, did: &Did) -> Option<&AgentNode> {
        self.nodes.get(did.as_str())
    }

    pub fn online_nodes(&self) -> Vec<&AgentNode> {
        self.nodes.values().filter(|n| n.online).collect()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn online_count(&self) -> usize {
        self.nodes.values().filter(|n| n.online).count()
    }

    pub fn total_stake(&self) -> u64 {
        self.total_stake
    }

    /// 群体平均信誉
    pub fn avg_reputation(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.nodes.values().map(|n| n.reputation as u64).sum();
        sum as f64 / self.nodes.len() as f64
    }

    /// 群体平均成功率
    pub fn avg_success_rate(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.nodes.values().map(|n| n.success_rate).sum();
        sum / self.nodes.len() as f64
    }

    /// 按能力筛选并排序节点
    pub fn select_by_capability(&self, capability: &str, limit: usize) -> Vec<&AgentNode> {
        let mut candidates: Vec<&AgentNode> = self.nodes.values()
            .filter(|n| n.online && n.card.capabilities.iter().any(|c| c == capability))
            .collect();
        candidates.sort_by(|a, b| b.score().partial_cmp(&a.score()).unwrap());
        candidates.truncate(limit);
        candidates
    }

    pub fn record_task_completion(&mut self, did: &Did, success: bool) {
        self.task_count += 1;
        if let Some(node) = self.nodes.get_mut(did.as_str()) {
            node.record_task(success);
        }
    }
}

/// 群体决策结果
#[derive(Debug, Clone)]
pub struct CollectiveDecision {
    pub proposal_id: String,
    pub approvals: u64,
    pub rejections: u64,
    pub quorum_reached: bool,
    pub accepted: bool,
}
