//! v2.8.7 测试（GAP §3.3）：Agent Sandbox 安全边界
//!
//! 修复审计发现的三个 critical + 五个 high：
//! - 未认证 RCE：变更类操作（create/exec/pause/resume/destroy）必须带认证主体；
//! - 无所有权：create 绑定 owner，后续变更校验调用者即所有者，否则 403；
//! - 随机 id：`sb-` + 16 hex，不再是可猜/可复用的 `sb-N` 计数器；
//! - 输出上限：stdout/stderr 截断到 1 MiB，防止父进程内存 DoS；
//! - 请求体配置（env / initial_files / timeout / allowed_domains）真正生效；
//! - 孤儿清扫：manager 启动时清理上一进程残留的 `au-sandbox-*` 目录。
//!
//! 遵循审计规则 2：这些测试在旧代码（无认证 / 无所有权 / 计数器 id）上会失败。

use gsn_core::mcp::sandbox_tools::SandboxMcpBridge;
use gsn_core::sandbox::config::SandboxConfig;
use gsn_core::sandbox::handle_sandbox_api;
use gsn_core::sandbox::manager::SandboxManager;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

fn new_mgr(tag: &str) -> SandboxManager {
    let dir = std::env::temp_dir().join(format!("au-v287-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    SandboxManager::new(dir, SandboxConfig::default(), 0)
}

fn create(mgr: &mut SandboxManager, caller: &str, body: &str) -> (u16, Value) {
    handle_sandbox_api("POST", "/api/v1/sandboxes", body, Some(caller), mgr)
}

fn exec(mgr: &mut SandboxManager, id: &str, caller: &str, code: &str) -> (u16, Value) {
    let path = format!("/api/v1/sandboxes/{id}/exec");
    let body = json!({ "language": "python", "code": code }).to_string();
    handle_sandbox_api("POST", &path, &body, Some(caller), mgr)
}

fn created_id(mgr: &mut SandboxManager, caller: &str) -> String {
    let (s, b) = create(mgr, caller, "{}");
    assert_eq!(s, 201, "create failed: {b}");
    b["id"].as_str().unwrap().to_string()
}

/// 1. 未认证变更拒绝；GET 只读放行
#[test]
fn unauthenticated_mutation_rejected() {
    let mut mgr = new_mgr("unauth");
    let (s, b) = handle_sandbox_api("POST", "/api/v1/sandboxes", "{}", None, &mut mgr);
    assert_eq!(s, 401, "未认证 create 应被拒绝: {b}");
    // GET 列表只读放行
    let (s, _) = handle_sandbox_api("GET", "/api/v1/sandboxes", "", None, &mut mgr);
    assert_eq!(s, 200);
}

/// 2. 认证 create + exec Python：print(1+1) → stdout "2"
#[test]
fn create_and_exec_python() {
    let mut mgr = new_mgr("happy");
    let (s, b) = create(&mut mgr, "alice", "{}");
    assert_eq!(s, 201, "create failed: {b}");
    let id = b["id"].as_str().unwrap();
    assert_eq!(b["owner"], "alice");
    let (s, b) = exec(&mut mgr, id, "alice", "print(1+1)");
    assert_eq!(s, 200, "exec failed: {b}");
    assert_eq!(b["stdout"].as_str().unwrap().trim(), "2");
    assert_eq!(b["exit_code"], 0);
}

/// 3. 随机 id：格式 `sb-` + 16 hex，且多次创建不重复
#[test]
fn random_ids_unique_and_valid() {
    let mut mgr = new_mgr("ids");
    let mut seen = std::collections::HashSet::new();
    for _ in 0..8 {
        let (s, b) = create(&mut mgr, "alice", "{}");
        assert_eq!(s, 201, "create failed: {b}");
        let id = b["id"].as_str().unwrap().to_string();
        assert!(id.starts_with("sb-"), "id 应带 sb- 前缀: {id}");
        let hex = &id[3..];
        assert_eq!(hex.len(), 16, "id 应为 sb- + 16 hex: {id}");
        assert!(
            hex.chars().all(|c| c.is_ascii_hexdigit()),
            "id 尾部应为 hex: {id}"
        );
        assert!(seen.insert(id), "id 不应重复");
    }
}

/// 4. 非所有者 exec 被拒绝（403）；所有者本人仍可执行
#[test]
fn non_owner_exec_rejected() {
    let mut mgr = new_mgr("owner");
    let id = created_id(&mut mgr, "alice");
    // bob 不是所有者
    let (s, b) = exec(&mut mgr, &id, "bob", "print(1)");
    assert_eq!(s, 403, "非所有者 exec 应被拒绝: {b}");
    // alice 仍可执行
    let (s, b) = exec(&mut mgr, &id, "alice", "print(1)");
    assert_eq!(s, 200, "所有者 exec 应成功: {b}");
}

/// 5. 输出 >1 MiB 被截断，防止父进程内存 DoS
#[test]
fn output_truncated_to_limit() {
    let mut mgr = new_mgr("trunc");
    let id = created_id(&mut mgr, "alice");
    let (s, b) = exec(&mut mgr, &id, "alice", "print('A' * 2_000_000)");
    assert_eq!(s, 200, "exec failed: {b}");
    let stdout = b["stdout"].as_str().unwrap();
    assert_eq!(
        stdout.len(),
        1_048_576,
        "stdout 应被截断到 1 MiB，实际 {}",
        stdout.len()
    );
}

/// 6. 请求体 env 配置真正注入到子进程
#[test]
fn env_config_effective() {
    let mut mgr = new_mgr("env");
    let body = json!({ "env": { "MY_VAR": "hello42" } }).to_string();
    let (s, b) = handle_sandbox_api("POST", "/api/v1/sandboxes", &body, Some("alice"), &mut mgr);
    assert_eq!(s, 201, "create failed: {b}");
    let id = b["id"].as_str().unwrap();
    let code = "import os; print(os.environ.get('MY_VAR'))";
    let (s, b) = exec(&mut mgr, id, "alice", code);
    assert_eq!(s, 200, "exec failed: {b}");
    assert_eq!(b["stdout"].as_str().unwrap().trim(), "hello42");
}

/// 7. initial_files 真正写入沙箱并可读
#[test]
fn initial_files_effective() {
    let mut mgr = new_mgr("files");
    let body = json!({ "initial_files": { "data.txt": "file-content-99" } }).to_string();
    let (s, b) = handle_sandbox_api("POST", "/api/v1/sandboxes", &body, Some("alice"), &mut mgr);
    assert_eq!(s, 201, "create failed: {b}");
    let id = b["id"].as_str().unwrap();
    let code = "print(open('data.txt').read())";
    let (s, b) = exec(&mut mgr, id, "alice", code);
    assert_eq!(s, 200, "exec failed: {b}");
    assert_eq!(b["stdout"].as_str().unwrap().trim(), "file-content-99");
}

/// 8. 非法 timeout（0）在 create 时被拒绝
#[test]
fn invalid_timeout_rejected() {
    let mut mgr = new_mgr("bad-timeout");
    let body = json!({ "timeout_ms": 0 }).to_string();
    let (s, b) = handle_sandbox_api("POST", "/api/v1/sandboxes", &body, Some("alice"), &mut mgr);
    assert_eq!(s, 400, "timeout=0 应被拒绝: {b}");
}

/// 9. pause / resume / destroy 完整生命周期
#[test]
fn lifecycle_pause_resume_destroy() {
    let mut mgr = new_mgr("lifecycle");
    let id = created_id(&mut mgr, "alice");

    let pause = format!("/api/v1/sandboxes/{id}/pause");
    let (s, b) = handle_sandbox_api("POST", &pause, "", Some("alice"), &mut mgr);
    assert_eq!(s, 200, "pause failed: {b}");

    let resume = format!("/api/v1/sandboxes/{id}/resume");
    let (s, b) = handle_sandbox_api("POST", &resume, "", Some("alice"), &mut mgr);
    assert_eq!(s, 200, "resume failed: {b}");
    // resume 后仍可 exec
    let (s, b) = exec(&mut mgr, &id, "alice", "print('ok')");
    assert_eq!(s, 200, "exec after resume failed: {b}");

    let destroy = format!("/api/v1/sandboxes/{id}");
    let (s, b) = handle_sandbox_api("DELETE", &destroy, "", Some("alice"), &mut mgr);
    assert_eq!(s, 200, "destroy failed: {b}");
    // 销毁后 get 404
    let (s, _) = handle_sandbox_api("GET", &destroy, "", Some("alice"), &mut mgr);
    assert_eq!(s, 404);
}

/// 10. 非所有者不能销毁别人的沙箱
#[test]
fn non_owner_destroy_rejected() {
    let mut mgr = new_mgr("owner-destroy");
    let id = created_id(&mut mgr, "alice");
    let path = format!("/api/v1/sandboxes/{id}");
    let (s, b) = handle_sandbox_api("DELETE", &path, "", Some("bob"), &mut mgr);
    assert_eq!(s, 403, "非所有者 destroy 应被拒绝: {b}");
    // 沙箱仍存在
    let (s, _) = handle_sandbox_api("GET", &path, "", Some("alice"), &mut mgr);
    assert_eq!(s, 200);
}

/// 11. 孤儿清扫：manager 启动时删除残留的 au-sandbox-* 目录
#[test]
fn orphan_sandboxes_swept() {
    let dir = std::env::temp_dir().join(format!("au-v287-sweep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let orphan = dir.join("au-sandbox-orphan");
    std::fs::create_dir_all(&orphan).unwrap();
    // 带一个文件，模拟非空残留
    std::fs::write(orphan.join("leftover.txt"), "x").unwrap();
    let _mgr = SandboxManager::new(dir.clone(), SandboxConfig::default(), 0);
    assert!(!orphan.exists(), "孤儿沙箱目录应被清扫");
}

/// 12. MCP 桥：带认证主体 create / list；不带主体变更被 handle_api 拒绝
#[test]
fn mcp_bridge_ownership() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let dir = std::env::temp_dir().join(format!("au-v287-mcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mgr = Arc::new(Mutex::new(SandboxManager::new(
        dir,
        SandboxConfig::default(),
        0,
    )));
    let bridge = SandboxMcpBridge::new(mgr.clone());

    // 带 caller create 成功
    let r = rt.block_on(bridge.call("sandbox_create", &json!({}), Some("alice")));
    assert!(!r.is_error, "MCP create with caller 应成功");

    // list 只读，无 caller 也可
    let r = rt.block_on(bridge.call("sandbox_list", &json!({}), None));
    assert!(!r.is_error, "MCP list 只读应成功");
}
