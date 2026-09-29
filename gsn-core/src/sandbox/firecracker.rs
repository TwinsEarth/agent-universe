//! Firecracker microVM 沙箱（v2.7.6）
//!
//! microVM 是最强隔离级别：每个环境独立硬件虚拟化边界，内核也独立。
//! 启动前探测 /dev/kvm；不存在显式 EnvBlocked，由调用方降级到容器/进程，
//! 不静默冒充。
//!
//! v2.7.6 仅提供配置骨架与 KVM 探测；真正的 microVM 启动/exec/checkpoint
//! 在 v2.7.7+ 随运行时统一实现。

use super::config::IsolationLevel;
use super::error::SandboxError;
use super::state::SandboxState;
use std::path::PathBuf;

/// Firecracker microVM 后端
#[derive(Debug, Clone)]
pub struct FirecrackerSandbox {
    pub sandbox_id: String,
    pub state: SandboxState,
    pub vmlinux: PathBuf,
    pub rootfs: PathBuf,
    pub vcpu: u32,
    pub mem_mb: u32,
    pub kvm_available: bool,
}

impl FirecrackerSandbox {
    pub fn new(sandbox_id: &str, vmlinux: PathBuf, rootfs: PathBuf) -> Self {
        Self {
            sandbox_id: sandbox_id.to_string(),
            state: SandboxState::Pending,
            vmlinux,
            rootfs,
            vcpu: 1,
            mem_mb: 128,
            kvm_available: PathBuf::from("/dev/kvm").exists(),
        }
    }

    /// /dev/kvm 是否可用
    pub fn kvm_available(&self) -> bool {
        self.kvm_available
    }

    /// 显式探测：无 KVM 即 EnvBlocked
    pub fn ensure_available(&self) -> Result<(), SandboxError> {
        if self.kvm_available {
            Ok(())
        } else {
            Err(SandboxError::EnvBlocked(
                "no /dev/kvm on this host; run on bare metal with nested virtualization".into(),
            ))
        }
    }

    pub fn isolation(&self) -> IsolationLevel {
        IsolationLevel::MicroVM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_isolation_level() {
        let f = FirecrackerSandbox::new("sb", PathBuf::from("/vmlinux"), PathBuf::from("/rootfs"));
        assert_eq!(f.isolation(), IsolationLevel::MicroVM);
    }

    #[test]
    fn env_blocked_when_no_kvm() {
        let f = FirecrackerSandbox::new("sb", PathBuf::from("/vmlinux"), PathBuf::from("/rootfs"));
        if !f.kvm_available() {
            assert!(matches!(f.ensure_available(), Err(SandboxError::EnvBlocked(_))));
        }
    }
}
