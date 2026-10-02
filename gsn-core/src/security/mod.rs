//! P2P 网络安全防御
//!
//! 防御 Sybil、Eclipse、Sybil、污染攻击
//! 身份验证、随机邻居选择、数据签名

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// v3.5.1（AU-32）：常量时间比较两个字节串是否相等（防时序侧信道）。
///
/// 用于 Bearer Token / 共享密钥这类「逐字节比较会泄露匹配前缀长度」的场景。
/// 与 `==` 的关键区别：
/// - **不短路**：即便长度不等，也先把重叠前缀逐字节 XOR 累积完，再合并长度
///   判定，不在发现第一个不匹配字节 / 长度不同时提前返回，避免通过响应时延
///   反推密钥前缀；
/// - 无分支：累积量 `diff` 只做 `|=`，结果在末尾一次性产出。
///
/// 不引入新第三方依赖（与 `plugin::bus::constant_time_eq_hex` 同一范式，但这里
/// 直接比 UTF-8 字节，供 REST 与 MCP 鉴权统一复用）。
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = 0u8;
    let overlap = a.len().min(b.len());
    for i in 0..overlap {
        diff |= a[i] ^ b[i];
    }
    // 长度差异并入最终结果（不提前返回）；长度是否相等单独作为与项，
    // 不依赖 u8 截断（避免长度差恰好 256 的倍数时误判相等）。
    diff == 0 && a.len() == b.len()
}

/// v3.5.1（AU-32）：常量时间比较两个 `&str`（按 UTF-8 字节）。
pub(crate) fn constant_time_eq_str(a: &str, b: &str) -> bool {
    constant_time_eq(a.as_bytes(), b.as_bytes())
}

#[cfg(test)]
mod ct_tests {
    use super::{constant_time_eq, constant_time_eq_str};

    #[test]
    fn ct_equal_inputs() {
        assert!(constant_time_eq(b"secret-token-xyz", b"secret-token-xyz"));
        assert!(constant_time_eq_str("Bearer abc", "Bearer abc"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn ct_unequal_inputs() {
        // 内容不同
        assert!(!constant_time_eq(b"secret-token-xyz", b"secret-token-xyy"));
        // 前缀相同、仅末位不同（朴素 == 会在此提前退出）
        assert!(!constant_time_eq(b"aaaaaaaaaaaaab", b"aaaaaaaaaaaaac"));
        // 长度不同（不得提前返回，结果仍为 false）
        assert!(!constant_time_eq(b"short", b"short-longer"));
        assert!(!constant_time_eq_str("Bearer abc", "Bearer abcd"));
        assert!(!constant_time_eq(b"", b"x"));
    }
}

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

impl Default for SecurityEngine {
    fn default() -> Self {
        Self::new()
    }
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
        self.neighbor_pool
            .choose_multiple(&mut rng, count)
            .collect()
    }

    pub fn ban_node(&mut self, node_id: String) {
        self.neighbor_pool.retain(|n| n != &node_id);
        self.banned.insert(node_id);
    }

    pub fn banned_count(&self) -> usize {
        self.banned.len()
    }
}
