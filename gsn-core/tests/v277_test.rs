//! v2.7.7: 进程级隔离运行时测试
//!
//! 在云电脑上真实执行 Python / JavaScript 代码，验证：
//! 独立临时目录、代码执行、stdout 捕获、超时强杀、文件隔离、
//! 路径逃逸拒绝、非白名单程序拒绝、pause/resume、destroy 清理。

use gsn_core::sandbox::config::{IsolationLevel, NetworkPolicy, ResourceLimits, SandboxConfig};
use gsn_core::sandbox::error::SandboxError;
use gsn_core::sandbox::runtime::{CodeLanguage, ProcessSandbox};
use gsn_core::sandbox::Sandbox;

fn mk_cfg(id: &str) -> SandboxConfig {
    SandboxConfig {
        sandbox_id: id.to_string(),
        isolation: IsolationLevel::Process,
        ..SandboxConfig::default()
    }
}

fn spawned(id: &str) -> ProcessSandbox {
    let mut s = ProcessSandbox::new(id);
    s.create(&mk_cfg(id)).unwrap();
    s.start().unwrap();
    s
}

#[test]
fn v277_run_python_hello() {
    let mut s = spawned("v277-py");
    let r = s
        .run_code(
            CodeLanguage::Python,
            "print('hello from python')\n",
        )
        .unwrap();
    assert_eq!(r.exit_code, 0);
    assert!(r.stdout.contains("hello from python"), "stdout: {}", r.stdout);
    s.destroy().unwrap();
}

#[test]
fn v277_run_javascript_hello() {
    let mut s = spawned("v277-js");
    let r = s
        .run_code(
            CodeLanguage::JavaScript,
            "console.log('hello from node');\n",
        )
        .unwrap();
    assert_eq!(r.exit_code, 0);
    assert!(r.stdout.contains("hello from node"));
    s.destroy().unwrap();
}

#[test]
fn v277_python_compute_result() {
    let mut s = spawned("v277-compute");
    let r = s
        .run_code(CodeLanguage::Python, "print(21*2)\n")
        .unwrap();
    assert!(r.stdout.trim().contains("42"), "got: {}", r.stdout);
    s.destroy().unwrap();
}

#[test]
fn v277_nonzero_exit_reported() {
    let mut s = spawned("v277-exit");
    let r = s.run_code(CodeLanguage::Python, "import sys; sys.exit(3)\n");
    // 非零退出：run_code 返回 Ok 但 exit_code 非零（业务可据 exit_code 判断）
    let res = r.unwrap();
    assert_eq!(res.exit_code, 3);
    s.destroy().unwrap();
}

#[test]
fn v277_timeout_kills_long_running() {
    let mut cfg = mk_cfg("v277-timeout");
    cfg.resources.timeout_ms = 1000;
    let mut s = ProcessSandbox::new("v277-timeout");
    s.create(&cfg).unwrap();
    s.start().unwrap();
    let r = s.run_code(CodeLanguage::Python, "while True:\n    pass\n");
    assert!(matches!(
        r,
        Err(SandboxError::ResourceLimitExceeded { .. })
    ));
    s.destroy().unwrap();
}

#[test]
fn v277_files_are_isolated_per_sandbox() {
    let mut a = spawned("v277-iso-a");
    let mut b = spawned("v277-iso-b");
    a.write_file("data.txt", "from-a").unwrap();
    // b 里不应有 a 的文件
    assert!(b.read_file("data.txt").is_err());
    b.write_file("data.txt", "from-b").unwrap();
    assert_eq!(a.read_file("data.txt").unwrap(), "from-a");
    assert_eq!(b.read_file("data.txt").unwrap(), "from-b");
    a.destroy().unwrap();
    b.destroy().unwrap();
}

#[test]
fn v277_path_traversal_rejected() {
    let s = spawned("v277-trav");
    assert!(matches!(
        s.write_file("../../evil.txt", "x"),
        Err(SandboxError::IsolationViolation(_))
    ));
    assert!(matches!(
        s.read_file("../../etc/passwd"),
        Err(SandboxError::IsolationViolation(_))
    ));
}

#[test]
fn v277_absolute_path_rejected() {
    let s = spawned("v277-abs");
    assert!(matches!(
        s.write_file("/etc/evil", "x"),
        Err(SandboxError::IsolationViolation(_))
    ));
}

#[test]
fn v277_non_whitelist_program_rejected() {
    let mut s = spawned("v277-wl");
    let r = s.exec("curl", &["example.com".to_string()]);
    assert!(matches!(r, Err(SandboxError::IsolationViolation(_))));
}

#[test]
fn v277_shell_blocked_by_default() {
    let mut s = spawned("v277-shell");
    let r = s.exec("bash", &["-c".to_string(), "echo hi".to_string()]);
    assert!(matches!(r, Err(SandboxError::IsolationViolation(_))));
}

#[test]
fn v277_shell_allowed_when_configured() {
    let mut cfg = mk_cfg("v277-shellok");
    cfg.allow_shell = true;
    let mut s = ProcessSandbox::new("v277-shellok");
    s.create(&cfg).unwrap();
    s.start().unwrap();
    let r = s
        .exec("bash", &["-c".to_string(), "echo shell-ok".to_string()])
        .unwrap();
    assert!(r.stdout.contains("shell-ok"));
    s.destroy().unwrap();
}

#[test]
fn v277_pause_resume_keeps_files() {
    let mut s = spawned("v277-pause");
    s.write_file("keep.txt", "data").unwrap();
    s.pause().unwrap();
    assert_eq!(s.state(), gsn_core::sandbox::state::SandboxState::Paused);
    s.resume().unwrap();
    assert_eq!(s.state(), gsn_core::sandbox::state::SandboxState::Running);
    assert_eq!(s.read_file("keep.txt").unwrap(), "data");
    s.destroy().unwrap();
}

#[test]
fn v277_destroy_removes_workdir() {
    let mut s = spawned("v277-destroy");
    let dir = s.work_dir().unwrap().to_path_buf();
    assert!(dir.exists());
    s.destroy().unwrap();
    assert!(!dir.exists());
}

#[test]
fn v277_initial_files_land_in_sandbox() {
    let mut cfg = mk_cfg("v277-init");
    cfg.initial_files = vec![("input.txt".into(), "seed".into())];
    let mut s = ProcessSandbox::new("v277-init");
    s.create(&cfg).unwrap();
    s.start().unwrap();
    assert_eq!(s.read_file("input.txt").unwrap(), "seed");
    s.destroy().unwrap();
}

#[test]
fn v277_exec_before_start_rejected() {
    let mut s = ProcessSandbox::new("v277-notstarted");
    s.create(&mk_cfg("v277-notstarted")).unwrap();
    // 未 start 状态是 Creating，exec 应拒绝
    let r = s.run_code(CodeLanguage::Python, "print(1)\n");
    assert!(r.is_err());
}

#[test]
fn v277_language_parse() {
    assert_eq!(CodeLanguage::from_label("py"), Some(CodeLanguage::Python));
    assert_eq!(CodeLanguage::from_label("node"), Some(CodeLanguage::JavaScript));
    assert_eq!(CodeLanguage::from_label("ruby"), None);
}

#[test]
fn v277_resource_limits_struct_sane() {
    let r = ResourceLimits::default();
    assert!(r.timeout_ms > 0);
    let n = NetworkPolicy::default();
    assert!(!n.allow_egress);
}
