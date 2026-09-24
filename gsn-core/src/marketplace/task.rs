//! TaskSpec & 任务状态机
//!
//! 任务完整定义，六字段校验
//! DRAFT → OPEN → MATCHED → RUNNING → VERIFYING → SETTLED

use crate::marketplace::evidence::EvidenceGrade;
use serde::{Deserialize, Serialize};

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskState {
    /// 草稿
    Draft,
    /// 已发布，等待匹配
    Open,
    /// 已匹配
    Matched,
    /// 执行中
    Running,
    /// 验证中
    Verifying,
    /// 已验收
    Accepted,
    /// 已结算
    Settled,
    /// 需要返工
    Rework,
    /// 争议中
    Disputed,
    /// 仲裁中
    Arbitration,
    /// 已罚没
    Slashed,
    /// 无共识，等待视图变更
    NoQuorum,
}

impl TaskState {
    pub fn label(&self) -> &'static str {
        match self {
            TaskState::Draft => "DRAFT",
            TaskState::Open => "OPEN",
            TaskState::Matched => "MATCHED",
            TaskState::Running => "RUNNING",
            TaskState::Verifying => "VERIFYING",
            TaskState::Accepted => "ACCEPTED",
            TaskState::Settled => "SETTLED",
            TaskState::Rework => "REWORK",
            TaskState::Disputed => "DISPUTED",
            TaskState::Arbitration => "ARBITRATION",
            TaskState::Slashed => "SLASHED",
            TaskState::NoQuorum => "NO_QUORUM",
        }
    }

    /// 是否终态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Settled | TaskState::Slashed
        )
    }
}

/// 验证策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationPolicy {
    /// BFT-lite: n 个委员，最多 f 个恶意
    BftLite { n: u32, f: u32 },
    /// 简单抽样
    Sampling { ratio: f64 },
    /// 无需验证
    None,
}

/// 任务定义（TaskSpec）
///
/// 六字段：goal / context / done / todo / trace / owner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// 任务 ID
    pub task_id: String,
    /// 目标
    pub goal: String,
    /// 上下文
    pub context: String,
    /// 已完成
    pub done: Vec<String>,
    /// 待完成
    pub todo: Vec<String>,
    /// 追踪记录
    pub trace: Vec<String>,
    /// 责任所有者
    pub owner: Option<String>,
    /// 预算
    pub budget: f64,
    /// 截止时间（Unix 毫秒）
    pub deadline: u64,
    /// 所需技能
    pub required_skills: Vec<String>,
    /// 验证策略
    pub verification_policy: VerificationPolicy,
    /// 发布者
    pub requester: String,
    /// 当前状态
    pub state: TaskState,
    /// 创建时间
    pub created_at: u64,
}

impl TaskSpec {
    /// 校验六字段完整性
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut gaps = Vec::new();

        if self.goal.trim().is_empty() {
            gaps.push("goal 不能为空".to_string());
        }
        if self.context.trim().is_empty() {
            gaps.push("context 不能为空".to_string());
        }
        if self.todo.is_empty() {
            gaps.push("todo 不能为空".to_string());
        }
        if self.budget <= 0.0 {
            gaps.push("budget 必须大于 0".to_string());
        }
        if self.required_skills.is_empty() {
            gaps.push("required_skills 不能为空".to_string());
        }
        if self.requester.trim().is_empty() {
            gaps.push("requester 不能为空".to_string());
        }

        if gaps.is_empty() {
            Ok(())
        } else {
            Err(gaps)
        }
    }
}

/// 错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorType {
    None,
    BadInput,
    CapabilityGap,
    InternalError,
    Timeout,
    LowConfidence,
}

/// 结果信封（Result Envelope）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultEnvelope {
    /// 关联任务 ID
    pub task_id: String,
    /// 执行 Agent DID
    pub agent_id: String,
    /// 结果报告（JSON 字符串）
    pub report: String,
    /// 置信度（0-1）
    pub confidence: f64,
    /// 错误类型
    pub error_type: ErrorType,
    /// Trace 引用
    pub trace_ref: String,
    /// 证据等级
    pub evidence_grade: EvidenceGrade,
    /// 执行耗时（毫秒）
    pub latency_ms: u64,
}

impl ResultEnvelope {
    pub fn is_success(&self) -> bool {
        matches!(self.error_type, ErrorType::None) && self.confidence > 0.5
    }
}
