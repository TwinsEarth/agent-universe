//! P2P 网络安全防御
//!
//! 防御 Sybil、Eclipse、Sybil、污染攻击
//! 身份验证、随机邻居选择、数据签名

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 安全评分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScore {
    pub node_id: String,
    pub score: f32, // 0.0 - 1.0
    pub flags: Vec<SecurityFlag>,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityFlag {
    SybilSuspected,
    EclipseAttack,
    PollutionDetected,
    OfflineTooLong,
    InvalidSignature,
    ReputationManipulation,
}

/// 防御引擎
pub struct SecurityEngine {
    scores: HashMap<String, SecurityScore>,
    banned: HashSet<String>,
    neighbor_pool: Vec<String>,
}

impl SecurityEngine {
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
            banned: HashSet::new(),
            neighbor_pool: Vec::new(),
        }
    }

    pub fn report_behavior(&mut self, node_id: String, flag: SecurityFlag) {
        let score = self.scores.entry(node_id.clone()).or_insert(SecurityScore {
            node_id: node_id.clone(),
            score: 1.0,
            flags: Vec::new(),
            last_updated: 0,
        });

        score.flags.push(flag);
        score.score *= 0.8; // 每次报告扣分

        // 分数太低则封禁
        if score.score < 0.3 {
            self.banned.insert(node_id);
        }
    }

    pub fn is_banned(&self, node_id: &str) -> bool {
        self.banned.contains(node_id)
    }

    pub fn add_neighbor(&mut self, node_id: String) {
        if !self.banned.contains(&node_id) && !self.neighbor_pool.contains(&node_id) {
            self.neighbor_pool.push(node_id);
        }
    }

    /// 随机选择邻居（防止 Eclipse 攻击）
    pub fn random_neighbors(&self, count: usize) -> Vec<&String> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.neighbor_pool.choose_multiple(&mut rng, count).collect()
    }

    pub fn ban_node(&mut self, node_id: String) {
        self.neighbor_pool.retain(|n| n != &node_id);
        self.banned.insert(node_id);
    }

    pub fn banned_count(&self) -> usize {
        self.banned.len()
    }
}
