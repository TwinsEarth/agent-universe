//! v2.7.6: Agent Sandbox 核心架构测试
//!
//! 覆盖：config 校验、隔离级别强度、生命周期状态机、身份令牌、
//! 后端环境探测（docker/kvm 显式 EnvBlocked）。

use gsn_core::sandbox::config::{
    FilesystemPolicy, IsolationLevel, NetworkPolicy, ResourceLimits, SandboxConfig,
};
use gsn_core::sandbox::error::SandboxError;
use gsn_core::sandbox::identity::{AgentIdentity, ExecutionToken};
use gsn_core::sandbox::state::{LifecycleAction, SandboxState};

/// 从默认配置构造，经闭包修改个别字段（避免 field_reassign_with_default）
fn cfg_with(f: impl FnOnce(&mut SandboxConfig)) -> SandboxConfig {
    let mut cfg = SandboxConfig::default();
    f(&mut cfg);
    cfg
}

#[test]
fn v276_default_config_is_minimal_privilege() {
    let cfg = SandboxConfig::default();
    // 默认无网络、只读根
    assert!(!cfg.network.allow_egress);
    assert!(!cfg.network.allow_ingress);
    assert!(cfg.filesystem.read_only_root);
    assert!(!cfg.allow_shell);
}

#[test]
fn v276_default_resources_are_bounded() {
    let r = ResourceLimits::default();
    assert!(r.cpu_millis > 0 && r.mem_mb > 0 && r.timeout_ms > 0);
    assert!(r.max_processes > 0 && r.max_open_files > 0);
}

#[test]
fn v276_isolation_strength_ordered() {
    assert!(IsolationLevel::MicroVM.strength() > IsolationLevel::ContainerSharedKernel.strength());
    assert!(IsolationLevel::ContainerSharedKernel.strength() > IsolationLevel::Process.strength());
}

#[test]
fn v276_validate_rejects_zero_cpu() {
    let cfg = cfg_with(|c| c.resources.cpu_millis = 0);
    let r = cfg.validate();
    assert!(r.is_err());
    assert!(r.unwrap_err().iter().any(|m| m.contains("cpu_millis")));
}

#[test]
fn v276_validate_rejects_open_egress_no_allowlist() {
    let cfg = cfg_with(|c| c.network.allow_egress = true);
    assert!(cfg.validate().is_err());
}

#[test]
fn v276_validate_rejects_escaping_initial_file() {
    let cfg = cfg_with(|c| c.initial_files = vec![("../../etc/passwd".into(), "x".into())]);
    assert!(cfg.validate().is_err());
}

#[test]
fn v276_validate_accepts_safe_config() {
    let cfg = SandboxConfig::default();
    assert!(cfg.validate().is_ok());
}

#[test]
fn v276_lifecycle_full_cycle() {
    let mut s = SandboxState::Pending;
    for a in [
        LifecycleAction::Create,
        LifecycleAction::Start,
        LifecycleAction::MarkReady,
        LifecycleAction::Pause,
        LifecycleAction::MarkPaused,
        LifecycleAction::Resume,
        LifecycleAction::MarkReady,
        LifecycleAction::Stop,
        LifecycleAction::MarkStopped,
    ] {
        s = s.transition(a).unwrap();
    }
    assert_eq!(s, SandboxState::Stopped);
}

#[test]
fn v276_terminal_states_reject_transitions() {
    assert!(SandboxState::Stopped
        .transition(LifecycleAction::Pause)
        .is_err());
    assert!(SandboxState::Failed
        .transition(LifecycleAction::Start)
        .is_err());
}

#[test]
fn v276_holds_resources_only_when_active() {
    assert!(SandboxState::Running.holds_resources());
    assert!(!SandboxState::Paused.holds_resources());
    assert!(!SandboxState::Pending.holds_resources());
}

#[test]
fn v276_execution_token_min_scope() {
    let t = ExecutionToken {
        token_id: "t".into(),
        sandbox_id: "sb".into(),
        agent_did: "did:aip:a".into(),
        scopes: vec!["code.exec".into()],
        issued_ms: 0,
        expires_ms: 1000,
    };
    assert!(t.authorize("sb", "code.exec", 100).is_ok());
    assert!(t.authorize("sb", "fs.delete", 100).is_err());
    assert!(t.authorize("other", "code.exec", 100).is_err());
    assert!(t.authorize("sb", "code.exec", 1000).is_err());
}

#[test]
fn v276_agent_identity_holds_did() {
    let a = AgentIdentity {
        agent_did: "did:aip:agent1".into(),
        agent_name: "Agent One".into(),
        tenant: "twins".into(),
    };
    assert!(a.agent_did.starts_with("did:"));
}

#[test]
fn v276_docker_backend_env_blocked_when_absent() {
    let d = gsn_core::sandbox::docker::DockerSandbox::new("sb");
    if !d.daemon_available() {
        assert!(matches!(
            d.ensure_available(),
            Err(SandboxError::EnvBlocked(_))
        ));
    }
}

#[test]
fn v276_firecracker_env_blocked_when_no_kvm() {
    let f = gsn_core::sandbox::firecracker::FirecrackerSandbox::new(
        "sb",
        std::path::PathBuf::from("/vmlinux"),
        std::path::PathBuf::from("/rootfs"),
    );
    if !f.kvm_available() {
        assert!(matches!(
            f.ensure_available(),
            Err(SandboxError::EnvBlocked(_))
        ));
    }
}

#[test]
fn v276_network_policy_default_deny() {
    let n = NetworkPolicy::default();
    assert!(!n.allow_egress && n.egress_allowlist.is_empty());
    let f = FilesystemPolicy::default();
    assert!(f.read_only_root);
}
