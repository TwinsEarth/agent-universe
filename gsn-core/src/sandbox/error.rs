//! Agent Sandbox 错误类型（v2.7.6）
//!
//! 沙箱错误必须能区分：环境不具备（应显式降级）、隔离被破坏（安全事故）、
//! 执行失败（业务错误）、资源超限（应回收）、配置非法（调用方问题）。

use serde::{Deserialize, Serialize};

/// Agent Sandbox 统一错误
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxError {
    /// 环境不具备（如无 /dev/kvm、无 Docker daemon），调用方应降级并如实标注。
    EnvBlocked(String),
    /// 隔离边界被破坏或尝试越界——安全级别错误，应立即终止并审计。
    IsolationViolation(String),
    /// 进程/代码执行失败（非零退出、信号、运行时错误）。
    ExecFailed {
        exit_code: Option<i32>,
        stderr: String,
    },
    /// 资源超限（CPU/内存/磁盘/进程数/超时），应终止并回收。
    ResourceLimitExceeded {
        kind: String,
        limit: u64,
        actual: u64,
    },
    /// 配置非法（调用方问题，不进入执行）。
    InvalidConfig(String),
    /// 生命周期状态非法（当前状态不允许该动作）。
    InvalidLifecycle {
        from: String,
        action: String,
    },
    /// 沙箱不存在 / 已被回收。
    NotFound(String),
    /// 网络策略拒绝（出站目标不在白名单）。
    NetworkDenied(String),
    /// 内部错误（不静默吞掉，必须上抛）。
    Internal(String),
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxError::EnvBlocked(m) => write!(f, "env blocked: {m}"),
            SandboxError::IsolationViolation(m) => write!(f, "ISOLATION VIOLATION: {m}"),
            SandboxError::ExecFailed { exit_code, stderr } => {
                write!(f, "exec failed (exit={exit_code:?}): {stderr}")
            }
            SandboxError::ResourceLimitExceeded { kind, limit, actual } => {
                write!(f, "resource limit exceeded: {kind} limit={limit} actual={actual}")
            }
            SandboxError::InvalidConfig(m) => write!(f, "invalid config: {m}"),
            SandboxError::InvalidLifecycle { from, action } => {
                write!(f, "invalid lifecycle: {action} from {from}")
            }
            SandboxError::NotFound(id) => write!(f, "sandbox not found: {id}"),
            SandboxError::NetworkDenied(target) => write!(f, "network denied: {target}"),
            SandboxError::Internal(m) => write!(f, "internal: {m}"),
        }
    }
}

impl std::error::Error for SandboxError {}

impl SandboxError {
    /// 是否安全级别错误（应立即终止并审计）
    pub fn is_security(&self) -> bool {
        matches!(self, SandboxError::IsolationViolation(_))
    }

    /// 是否环境不具备（应降级，不应视为业务失败）
    pub fn is_env_blocked(&self) -> bool {
        matches!(self, SandboxError::EnvBlocked(_))
    }

    /// 是否可重试（资源/执行类可能重试，配置/安全类不可重试）
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            SandboxError::ExecFailed { .. }
                | SandboxError::ResourceLimitExceeded { .. }
                | SandboxError::Internal(_)
        )
    }
}
