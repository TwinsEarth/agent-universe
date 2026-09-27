//! TaskSpec & 任务状态机
//!
//! 任务完整定义，六字段校验
//! DRAFT → OPEN → MATCHED → RUNNING → VERIFYING → SETTLED

use crate::marketplace::evidence::EvidenceGrade;
use crate::marketplace::money::Money;
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
    /// 验收终局不通过（拒绝付款，可罚没）；仍需结算以释放托管、退预算
    Rejected,
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
            TaskState::Rejected => "REJECTED",
        }
    }

    /// 是否终态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Settled | TaskState::Slashed
        )
    }

    /// 状态机合法转移表。
    ///
    /// 主生命周期：
    /// `Draft/Open → Matched → Running → Verifying → Accepted → Settled`
    /// 两条**恢复边**（v2.6.0，GAP §3.4，消除吸收态）：
    /// - `Rework → Running`：返工后执行者继续执行；
    /// - `NoQuorum → Open`：本轮无共识，任务重新开放（可重新匹配 / 重新组织验收）。
    pub fn can_transition_to(&self, to: TaskState) -> bool {
        use TaskState::*;
        matches!(
            (*self, to),
            // 发布（publish 对传入状态做归一化，亦允许自环）
            (Draft, Open)
                | (Open, Open)
                // 匹配
                | (Open, Matched)
                | (Matched, Running)
                // 提交结果（匹配后可直接提交，或执行中提交）
                | (Matched, Verifying)
                | (Running, Verifying)
                // policy=None：无需 QA，提交结果即验收
                | (Matched, Accepted)
                | (Running, Accepted)
                // QA 验收
                | (Verifying, Accepted)
                | (Verifying, Rework)
                | (Verifying, NoQuorum)
                // 结算
                | (Accepted, Settled)
                // 恢复边：返工 → 继续执行
                | (Rework, Running)
                // 恢复边：无共识 → 重新开放
                | (NoQuorum, Open)
                // 验收终局拒绝（QA 明确拒绝 / 返工后重复劳动）
                | (Verifying, Rejected)
                | (Running, Rejected)
                | (Rework, Rejected)
                // 拒绝后仍走结算（付 0、退预算、罚没）到终态
                | (Rejected, Settled)
                // 争议 / 仲裁
                | (Accepted, Disputed)
                | (Running, Disputed)
                | (Verifying, Disputed)
                | (Rework, Disputed)
                | (Disputed, Arbitration)
                | (Disputed, Slashed)
                | (Disputed, Accepted)
                | (Arbitration, Slashed)
                | (Arbitration, Accepted)
        )
    }

    /// 校验并执行一次状态转移；非法转移返回错误（不改变状态）。
    pub fn transition(self, to: TaskState) -> Result<TaskState, String> {
        if self.can_transition_to(to) {
            Ok(to)
        } else {
            Err(format!(
                "非法状态转移：{} → {}",
                self.label(),
                to.label()
            ))
        }
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
    /// 预算（整数，发布时锁定到托管账户）
    pub budget: Money,
    /// 中标价（匹配时确定；结算按此，余款退回需求方）
    pub winner_price: Option<Money>,
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
        if !self.budget.is_positive() {
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
