//! 容器共享内核沙箱（v2.7.6）
//!
//! Docker/Podman 等 runc 容器共享宿主内核，隔离强度弱于 microVM。
//! 容器后端在创建前探测容器 daemon 是否可用；不可用时显式 EnvBlocked，
//! 由调用方降级到进程级或 microVM，不静默冒充。
//!
//! v2.7.6 仅提供配置骨架与环境探测；真正的容器创建/exec/checkpoint
//! 在 v2.7.7+ 随运行时统一实现。

use super::config::IsolationLevel;
use super::error::SandboxError;
use super::state::SandboxState;

/// 容器共享内核后端
#[derive(Debug, Clone)]
pub struct DockerSandbox {
    pub sandbox_id: String,
    pub state: SandboxState,
    pub daemon_available: bool,
}

impl DockerSandbox {
    pub fn new(sandbox_id: &str) -> Self {
        let daemon_available = detect_container_daemon();
        Self {
            sandbox_id: sandbox_id.to_string(),
            state: SandboxState::Pending,
            daemon_available,
        }
    }

    /// 容器 daemon 是否可用
    pub fn daemon_available(&self) -> bool {
        self.daemon_available
    }

    /// 显式探测：不可用即 EnvBlocked
    pub fn ensure_available(&self) -> Result<(), SandboxError> {
        if self.daemon_available {
            Ok(())
        } else {
            Err(SandboxError::EnvBlocked(
                "no container daemon (docker/podman) reachable on this host".into(),
            ))
        }
    }

    pub fn isolation(&self) -> IsolationLevel {
        IsolationLevel::ContainerSharedKernel
    }
}

/// 探测宿主上是否存在容器 daemon（只读探测，不依赖网络）。
/// 通过 PATH 上是否有 docker/podman 可执行文件做初判；
/// 真正的 socket 连通性在执行时再确认。
fn detect_container_daemon() -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|dir| {
        ["docker", "podman"]
            .iter()
            .any(|bin| dir.join(bin).is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_isolation_level() {
        let d = DockerSandbox::new("sb");
        assert_eq!(d.isolation(), IsolationLevel::ContainerSharedKernel);
    }

    #[test]
    fn env_blocked_when_no_daemon() {
        let d = DockerSandbox::new("sb");
        if !d.daemon_available() {
            assert!(matches!(
                d.ensure_available(),
                Err(SandboxError::EnvBlocked(_))
            ));
        }
    }
}
