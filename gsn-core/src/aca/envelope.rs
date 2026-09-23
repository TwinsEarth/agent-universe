//! Task Envelope（任务信封）
//!
//! 增强版 Task：输入 CID、输出要求、验证策略、隐私要求、预算

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::verification::VerificationLevel;

/// 隐私要求
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyRequirement {
    /// 无特殊要求
    Public,
    /// 数据不离开本地
    LocalOnly,
    /// 差分隐私
    DifferentialPrivacy,
    /// TEE 执行
    TEE,
    /// 零知识证明
    ZK,
}

/// 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// 任务信封
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEnvelope {
    pub task_id: String,
    pub requester_did: String,
    pub capability: String,
    /// 输入内容 CID（IPFS 内容寻址）
    pub input_cid: String,
    /// 输出要求描述
    pub output_spec: String,
    /// 要求的验证级别
    pub verification_level: VerificationLevel,
    /// 隐私要求
    pub privacy: PrivacyRequirement,
    /// 预算（GSP）
    pub budget: u64,
    /// 超时（秒）
    pub timeout_secs: u64,
    /// 优先级
    pub priority: TaskPriority,
    /// MCP 兼容：调用的工具名（可选）
    pub mcp_tool: Option<String>,
    /// 时间戳
    pub created_at: u64,
}

impl TaskEnvelope {
    pub fn new(
        requester_did: String,
        capability: String,
        input_cid: String,
        output_spec: String,
    ) -> Self {
        Self {
            task_id: Uuid::new_v4().to_string(),
            requester_did,
            capability,
            input_cid,
            output_spec,
            verification_level: VerificationLevel::L0Sample,
            privacy: PrivacyRequirement::Public,
            budget: 0,
            timeout_secs: 300,
            priority: TaskPriority::Normal,
            mcp_tool: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn with_verification(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    pub fn with_privacy(mut self, privacy: PrivacyRequirement) -> Self {
        self.privacy = privacy;
        self
    }

    pub fn with_budget(mut self, budget: u64) -> Self {
        self.budget = budget;
        self
    }

    pub fn with_mcp_tool(mut self, tool: String) -> Self {
        self.mcp_tool = Some(tool);
        self
    }

    /// 估算验证成本（GSP）
    pub fn estimated_verification_cost(&self) -> u64 {
        use VerificationLevel::*;
        match self.verification_level {
            L0Sample => 1,
            L1Redundant => 3,
            L2TEE => 50,
            L3ZkML => 500,
            L4Committee => 2000,
        }
    }

    /// 验证成本是否在预算内
    pub fn verification_affordable(&self) -> bool {
        self.estimated_verification_cost() <= self.budget
    }
}
