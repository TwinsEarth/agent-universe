//! v2.8.0: Agent Sandbox 集成测试
//!
//! 通过纯函数 handle_sandbox_api 跑完整生命周期：
//! create → get → exec(Python/JS) → pause → resume → list → destroy，
//! 并验证错误码与 is_sandbox_route 分流。

use gsn_core::sandbox::config::SandboxConfig;
use gsn_core::sandbox::manager::SandboxManager;
use gsn_core::sandbox::{handle_sandbox_api, is_sandbox_route};

fn mk_manager(dir: &std::path::Path) -> SandboxManager {
    // v2.9.1：默认配置无 waiver 会被能力闸门拒绝（G1 设计意图）；
    // 集成生命周期测试需真正创建沙箱，使用显式 trusted_local（带理由）。
    let cfg = SandboxConfig::trusted_local("v280 full lifecycle integration test");
    SandboxManager::new(dir.to_path_buf(), cfg, 0)
}

fn call(
    mgr: &mut SandboxManager,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, serde_json::Value) {
    // v2.8.7：变更类需认证，固定使用受信测试主体 "tester"
    handle_sandbox_api(method, path, body, Some("tester"), mgr)
}

#[test]
fn v280_full_lifecycle() {
    let base = std::env::temp_dir().join("au-v280-full");
    let _ = std::fs::remove_dir_all(&base);
    let mut mgr = mk_manager(&base);

    // create
    let (st, v) = call(&mut mgr, "POST", "/api/v1/sandboxes", "{}");
    assert_eq!(st, 201);
    let id = v.get("id").unwrap().as_str().unwrap().to_string();
    assert!(!id.is_empty());

    // get
    let (st, v) = call(&mut mgr, "GET", &format!("/api/v1/sandboxes/{id}"), "");
    assert_eq!(st, 200);
    assert_eq!(v.get("state").unwrap(), "running");

    // exec Python
    let body = r#"{"language":"python","code":"print('hello-au')"}"#;
    let (st, v) = call(
        &mut mgr,
        "POST",
        &format!("/api/v1/sandboxes/{id}/exec"),
        body,
    );
    assert_eq!(st, 200);
    let stdout = v.get("stdout").unwrap().as_str().unwrap();
    assert!(stdout.contains("hello-au"), "stdout={stdout}");

    // exec JavaScript
    let body = r#"{"language":"javascript","code":"console.log(1+1)"}"#;
    let (st, v) = call(
        &mut mgr,
        "POST",
        &format!("/api/v1/sandboxes/{id}/exec"),
        body,
    );
    assert_eq!(st, 200);
    assert!(v.get("stdout").unwrap().as_str().unwrap().contains("2"));

    // pause
    let (st, _) = call(
        &mut mgr,
        "POST",
        &format!("/api/v1/sandboxes/{id}/pause"),
        "",
    );
    assert_eq!(st, 200);
    let (_, v) = call(&mut mgr, "GET", &format!("/api/v1/sandboxes/{id}"), "");
    assert_eq!(v.get("state").unwrap(), "paused");

    // resume
    let (st, _) = call(
        &mut mgr,
        "POST",
        &format!("/api/v1/sandboxes/{id}/resume"),
        "",
    );
    assert_eq!(st, 200);
    let (_, v) = call(&mut mgr, "GET", &format!("/api/v1/sandboxes/{id}"), "");
    assert_eq!(v.get("state").unwrap(), "running");

    // list
    let (st, v) = call(&mut mgr, "GET", "/api/v1/sandboxes", "");
    assert_eq!(st, 200);
    assert_eq!(v.get("count").unwrap().as_u64().unwrap(), 1);

    // destroy
    let (st, _) = call(&mut mgr, "DELETE", &format!("/api/v1/sandboxes/{id}"), "");
    assert_eq!(st, 200);

    // get destroyed → 404
    let (st, _) = call(&mut mgr, "GET", &format!("/api/v1/sandboxes/{id}"), "");
    assert_eq!(st, 404);
}

#[test]
fn v280_e2b_alias_routes() {
    let base = std::env::temp_dir().join("au-v280-e2b");
    let _ = std::fs::remove_dir_all(&base);
    let mut mgr = mk_manager(&base);
    // E2B 别名 /v1/sandboxes
    let (st, v) = call(&mut mgr, "POST", "/v1/sandboxes", "{}");
    assert_eq!(st, 201);
    let id = v.get("id").unwrap().as_str().unwrap().to_string();
    // E2B /commands 别名
    let body = r#"{"language":"python","code":"print('e2b')"}"#;
    let (st, v) = call(
        &mut mgr,
        "POST",
        &format!("/v1/sandboxes/{id}/commands"),
        body,
    );
    assert_eq!(st, 200);
    assert!(v.get("stdout").unwrap().as_str().unwrap().contains("e2b"));
}

#[test]
fn v280_error_codes() {
    let base = std::env::temp_dir().join("au-v280-err");
    let _ = std::fs::remove_dir_all(&base);
    let mut mgr = mk_manager(&base);

    // 先创建一个真实沙箱（owner=tester）
    let (st, v) = call(&mut mgr, "POST", "/api/v1/sandboxes", "{}");
    assert_eq!(st, 201);
    let id = v.get("id").unwrap().as_str().unwrap().to_string();

    // exec 缺 code → 400
    let (st, _) = call(
        &mut mgr,
        "POST",
        &format!("/api/v1/sandboxes/{id}/exec"),
        r#"{"language":"python"}"#,
    );
    assert_eq!(st, 400);

    // 不支持的语言 → 400
    let (st, _) = call(
        &mut mgr,
        "POST",
        &format!("/api/v1/sandboxes/{id}/exec"),
        r#"{"language":"ruby","code":"x"}"#,
    );
    assert_eq!(st, 400);

    // 不存在的沙箱 destroy → 404
    let (st, _) = call(&mut mgr, "DELETE", "/api/v1/sandboxes/nope", "");
    assert_eq!(st, 404);
}

#[test]
fn v280_route_detection() {
    assert!(is_sandbox_route("/api/v1/sandboxes"));
    assert!(is_sandbox_route("/api/v1/sandboxes/sbx1/exec"));
    assert!(is_sandbox_route("/v1/sandboxes"));
    assert!(is_sandbox_route("/sandboxes"));
    assert!(!is_sandbox_route("/api/v1/agents"));
    assert!(!is_sandbox_route("/health"));
}

#[test]
fn v280_kubernetes_crd_manifests() {
    use gsn_core::sandbox::{crd_manifest, example_instance, reconcile_outline};
    let crd = crd_manifest();
    assert!(crd.contains("CustomResourceDefinition"));
    assert!(crd.contains("AgentSandbox"));
    assert!(crd.contains("agentsandboxes"));
    assert!(crd.contains("openAPIV3Schema"));
    assert!(crd.contains("additionalPrinterColumns"));
    let inst = example_instance("demo", "default");
    assert!(inst.contains("process:generic"));
    assert!(inst.contains("name: demo"));
    assert_eq!(reconcile_outline().len(), 5);
}
