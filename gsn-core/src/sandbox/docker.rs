//! Docker 共享内核沙箱（默认后端）
//!
//! 当前为进程内原型：记录 CPU 时间，不真起容器。

use super::{IsolationLevel, Sandbox, SandboxError, SandboxResult, TaskSpec};

#[derive(Debug, Clone, Default)]
pub struct DockerSandbox {
    pub last_cpu_ms: u64,
}

impl DockerSandbox {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Sandbox for DockerSandbox {
    fn isolation(&self) -> IsolationLevel {
        IsolationLevel::DockerSharedKernel
    }

    fn run(&mut self, task: &TaskSpec) -> Result<SandboxResult, SandboxError> {
        // 原型：按 cpu_quota 估算一个有界 CPU 时间
        let cpu = (task.cpu_quota_mhz as u64 / 10).max(1);
        self.last_cpu_ms = cpu;
        Ok(SandboxResult {
            exit_code: 0,
            cpu_time_ms: cpu,
            isolation: IsolationLevel::DockerSharedKernel,
        })
    }
}
