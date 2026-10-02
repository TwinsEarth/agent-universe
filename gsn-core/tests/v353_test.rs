//! v3.5.3 沙箱真实行为测试（AU-27，反假测试：每条都断言被强制的行为本身）。
//!
//! 仅在本 Linux 容器可稳定、确定性断言的行为上写测试；对无法可靠断言的
//! （内存 ulimit 触发 OOM、Windows Job Object 强制）不写空洞/恒真测试，
//! 见模块底部「未在 CI 断言的强制项」注释与 CHANGELOG。

use gsn_core::sandbox::config::{IsolationLevel, SandboxConfig};
use gsn_core::sandbox::runtime::{CodeLanguage, ProcessSandbox};
use gsn_core::sandbox::Sandbox;

fn mk_cfg(id: &str) -> SandboxConfig {
    SandboxConfig {
        sandbox_id: id.to_string(),
        isolation: IsolationLevel::Process,
        ..SandboxConfig::trusted_local("v353 真实行为测试：进程后端，无 egress/FS/quota primitive")
    }
}

fn spawned(id: &str) -> ProcessSandbox {
    let mut s = ProcessSandbox::new(id);
    s.create(&mk_cfg(id)).unwrap();
    s.start().unwrap();
    s
}

/// AU-27：子进程 env_clear 后只剩 PATH + cfg.env 白名单——宿主秘密环境变量不得泄漏进沙箱。
#[test]
fn au27_env_whitelist_scrubs_host_secrets() {
    // 在测试进程（=宿主）里塞一个秘密变量；build_command 用 env_clear() 后不继承它。
    std::env::set_var("GSN_AU27_HOST_SECRET", "leak-me-please");
    let mut s = spawned("au27-env");
    let r = s
        .run_code(
            CodeLanguage::Python,
            "import os\nprint('SECRET=[' + os.environ.get('GSN_AU27_HOST_SECRET', '') + ']')\n",
        )
        .unwrap();
    std::env::remove_var("GSN_AU27_HOST_SECRET");
    assert_eq!(r.exit_code, 0, "stderr={}", r.stderr);
    assert!(
        r.stdout.contains("SECRET=[]"),
        "宿主秘密环境变量不应泄漏进沙箱，stdout={}",
        r.stdout
    );
    s.destroy().unwrap();
}

/// AU-27：子进程工作目录独立于宿主 cwd（落在沙箱 work_dir 下）。
#[test]
fn au27_workdir_is_isolated_from_host_cwd() {
    let host_cwd = std::env::current_dir().unwrap();
    let mut s = spawned("au27-cwd");
    let r = s
        .run_code(
            CodeLanguage::Python,
            "import os; print('CWD=[' + os.getcwd() + ']')\n",
        )
        .unwrap();
    assert_eq!(r.exit_code, 0, "stderr={}", r.stderr);
    let got = r.stdout;
    let start = got.find("CWD=[").unwrap() + 5;
    let end = got.find(']').unwrap();
    let child_cwd = std::path::PathBuf::from(&got[start..end]);
    // 子进程 cwd 不应等于宿主 cwd（被隔离到沙箱 work_dir）。
    assert_ne!(
        child_cwd, host_cwd,
        "子进程 cwd 应与宿主 cwd 隔离：child={child_cwd:?} host={host_cwd:?}"
    );
    s.destroy().unwrap();
}

/// AU-27：cfg.env 里显式白名单的变量必须透传（与上一条对照，证明白名单通道本身工作）。
#[test]
fn au27_whitelisted_env_passes_through() {
    let mut cfg = mk_cfg("au27-env-pass");
    cfg.env
        .push(("GSN_AU27_ALLOWED".to_string(), "visible-value".to_string()));
    let mut s = ProcessSandbox::new("au27-env-pass");
    s.create(&cfg).unwrap();
    s.start().unwrap();
    let r = s
        .run_code(
            CodeLanguage::Python,
            "import os\nprint('GOT=[' + os.environ.get('GSN_AU27_ALLOWED', '') + ']')\n",
        )
        .unwrap();
    assert_eq!(r.exit_code, 0, "stderr={}", r.stderr);
    assert!(
        r.stdout.contains("GOT=[visible-value]"),
        "白名单变量应透传，stdout={}",
        r.stdout
    );
    s.destroy().unwrap();
}

// ── 未在 CI 断言的强制项（平台/容器限制，宁缺毋滥，不写恒真测试）──
//  - 内存 ulimit 真正触发 OOM 杀进程：依赖 cgroup/ulimit 在 CI 容器内不可确定性复现（不同
//    容器的 memory.max 配置差异大，易 flaky），仅在 build_command 中设置 bash -c ulimit，
//    不在单测里断言 OOM。
//  - Windows Job Object 强制杀进程树：仅 #[cfg(windows)] 路径，本 Linux CI 不覆盖。
//  - 网络出口强制 / 文件系统子树限制 / 磁盘配额：进程后端在 Linux 容器内无对应内核原语，
//    已由 AU-28 在 create 时具名拒绝（请求更高隔离级别且无豁免），不在此伪造“已强制”断言。
