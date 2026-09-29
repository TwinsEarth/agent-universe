//! v2.7.8: SandboxManager 生命周期管理与弹性供给测试
//!
//! 验证：预热池、acquire 即时/预热、release 休眠、wake 唤醒、
//! checkpoint 快照、fork 状态复用、destroy 清理、shutdown、evict_idle。

use gsn_core::sandbox::config::SandboxConfig;
use gsn_core::sandbox::runtime::CodeLanguage;
use gsn_core::sandbox::state::SandboxState;
use gsn_core::sandbox::SandboxManager;

fn base_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("au-mgr-{tag}"))
}

fn mk_manager(tag: &str, warm: usize) -> SandboxManager {
    let dir = base_dir(tag);
    let _ = std::fs::remove_dir_all(&dir);
    SandboxManager::new(dir, SandboxConfig::default(), warm)
}

#[test]
fn v278_prewarm_fills_pool() {
    let mut mgr = mk_manager("prewarm", 3);
    let n = mgr.prewarm().unwrap();
    assert_eq!(n, 3);
    assert_eq!(mgr.warm_pool_len(), 3);
    assert_eq!(mgr.count(), 3);
    mgr.shutdown().unwrap();
}

#[test]
fn v278_acquire_from_warm_pool() {
    let mut mgr = mk_manager("acquire-warm", 2);
    mgr.prewarm().unwrap();
    let id = mgr.acquire(None).unwrap();
    // 取走一个后预热池减少
    assert_eq!(mgr.warm_pool_len(), 1);
    assert_eq!(mgr.state_of(&id), Some(SandboxState::Running));
    mgr.shutdown().unwrap();
}

#[test]
fn v278_acquire_creates_when_pool_empty() {
    let mut mgr = mk_manager("acquire-empty", 0);
    let id = mgr.acquire(None).unwrap();
    assert_eq!(mgr.state_of(&id), Some(SandboxState::Running));
    assert_eq!(mgr.count(), 1);
    mgr.shutdown().unwrap();
}

#[test]
fn v278_release_pauses_and_wake_resumes() {
    let mut mgr = mk_manager("release", 0);
    let id = mgr.acquire(None).unwrap();
    mgr.release(&id).unwrap();
    assert_eq!(mgr.state_of(&id), Some(SandboxState::Paused));
    mgr.wake(&id).unwrap();
    assert_eq!(mgr.state_of(&id), Some(SandboxState::Running));
    mgr.shutdown().unwrap();
}

#[test]
fn v278_run_code_on_acquired() {
    let mut mgr = mk_manager("run", 0);
    let id = mgr.acquire(None).unwrap();
    let r = mgr
        .sandbox_mut(&id)
        .unwrap()
        .run_code(CodeLanguage::Python, "print('managed')\n")
        .unwrap();
    assert!(r.stdout.contains("managed"));
    mgr.shutdown().unwrap();
}

#[test]
fn v278_checkpoint_creates_snapshot() {
    let mut mgr = mk_manager("cp", 0);
    let id = mgr.acquire(None).unwrap();
    mgr.sandbox_mut(&id)
        .unwrap()
        .write_file("state.txt", "checkpoint-data")
        .unwrap();
    let cp_id = mgr.checkpoint(&id).unwrap();
    assert!(cp_id.contains(&id));
    // 快照目录存在且含文件
    let cp_path = base_dir("cp").join("checkpoints").join(&cp_id).join("state.txt");
    assert!(cp_path.exists());
    mgr.shutdown().unwrap();
}

#[test]
fn v278_fork_reuses_state() {
    let mut mgr = mk_manager("fork", 0);
    let id = mgr.acquire(None).unwrap();
    mgr.sandbox_mut(&id)
        .unwrap()
        .write_file("origin.txt", "parent-state")
        .unwrap();
    let child = mgr.fork_from(&id, None).unwrap();
    assert_ne!(child, id);
    // 子沙箱继承父的文件
    let content = mgr
        .sandbox_mut(&child)
        .unwrap()
        .read_file("origin.txt")
        .unwrap();
    assert_eq!(content, "parent-state");
    // 父沙箱不受影响
    assert_eq!(mgr.count(), 2);
    mgr.shutdown().unwrap();
}

#[test]
fn v278_fork_from_checkpoint() {
    let mut mgr = mk_manager("forkcp", 0);
    let id = mgr.acquire(None).unwrap();
    mgr.sandbox_mut(&id)
        .unwrap()
        .write_file("cp_file.txt", "from-cp")
        .unwrap();
    let cp_id = mgr.checkpoint(&id).unwrap();
    let child = mgr.fork_from(&cp_id, None).unwrap();
    let content = mgr
        .sandbox_mut(&child)
        .unwrap()
        .read_file("cp_file.txt")
        .unwrap();
    assert_eq!(content, "from-cp");
    mgr.shutdown().unwrap();
}

#[test]
fn v278_destroy_removes_sandbox() {
    let mut mgr = mk_manager("destroy", 0);
    let id = mgr.acquire(None).unwrap();
    assert_eq!(mgr.count(), 1);
    mgr.destroy(&id).unwrap();
    assert_eq!(mgr.count(), 0);
    assert_eq!(mgr.state_of(&id), None);
}

#[test]
fn v278_shutdown_removes_all() {
    let mut mgr = mk_manager("shutdown", 2);
    mgr.prewarm().unwrap();
    // 从预热池取一个（不新增沙箱，总数仍为 2）
    let id = mgr.acquire(None).unwrap();
    let _ = id;
    let n = mgr.shutdown().unwrap();
    assert_eq!(n, 2);
    assert_eq!(mgr.count(), 0);
    assert_eq!(mgr.warm_pool_len(), 0);
}

#[test]
fn v278_evict_idle_reclaims_paused() {
    let mut mgr = mk_manager("evict", 0);
    let id = mgr.acquire(None).unwrap();
    mgr.release(&id).unwrap();
    // idle 阈值 0：所有休眠的都应被回收
    let n = mgr.evict_idle(0).unwrap();
    assert_eq!(n, 1);
    assert_eq!(mgr.state_of(&id), None);
}

#[test]
fn v278_evict_refills_warm_pool() {
    let mut mgr = mk_manager("evictrefill", 2);
    mgr.prewarm().unwrap();
    mgr.evict_idle(0).unwrap();
    // 预热池中的沙箱不应被回收，池应保持
    assert_eq!(mgr.warm_pool_len(), 2);
    mgr.shutdown().unwrap();
}

#[test]
fn v278_unknown_sandbox_errors() {
    let mut mgr = mk_manager("unknown", 0);
    assert!(mgr.release("nope").is_err());
    assert!(mgr.wake("nope").is_err());
    assert!(mgr.destroy("nope").is_err());
}
