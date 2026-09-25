//! 任务沙箱（v2.4.0）
//!
//! 统一隔离抽象：Docker 共享内核为默认后端；Firecracker microVM 为强隔离后端，
//! 在带 /dev/kvm 的裸金属上真跑。无 KVM 环境显式 EnvBlocked，不静默冒充。

pub mod docker;
pub mod firecracker;

pub use docker::DockerSandbox;
pub use firecracker::FirecrackerSandbox;

use serde::{Deserialize, Serialize};

/// 隔离级别（如实标注，不得静默升级/降级）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Docker 共享内核
    DockerSharedKernel,
    /// Firecracker microVM（硬件虚拟化）
    MicroVM,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    /// 环境不具备（如无 /dev/kvm），调用方需自行降级并标注。
    EnvBlocked(String),
    IsolationViolation(String),
    ExecFailed(String),
}

/// 任务规格（沙箱只认这个）。
#[derive(Debug, Clone)]
pub struct TaskSpec {
    pub task_id: String,
    pub image: String,
    pub cpu_quota_mhz: u32,
    pub mem_mb: u32,
}

#[derive(Debug, Clone)]
pub struct SandboxResult {
    pub exit_code: i32,
    pub cpu_time_ms: u64,
    pub isolation: IsolationLevel,
}

pub trait Sandbox {
    fn isolation(&self) -> IsolationLevel;
    fn run(&mut self, task: &TaskSpec) -> Result<SandboxResult, SandboxError>;
}
