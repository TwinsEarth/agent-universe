//! 进程级隔离沙箱（v2.7.7）
//!
//! 在云电脑 / 普通 PC 上真实可跑，不依赖 Docker/KVM。
//!
//! 隔离手段：
//! 1. 每个沙箱独立临时目录（`base/au-sandbox-{id}-{pid}-{seq}`），作为唯一工作目录；
//! 2. 子进程只在该目录内运行，文件操作经路径校验拒绝逃逸；
//! 3. 子进程不继承宿主环境，仅注入 cfg.env 白名单；
//! 4. 超时由 Rust 侧轮询后强杀（不依赖外部 timeout/gtimeout）+ `bash -c ulimit`（进程/句柄上限）；
//! 5. 不挂载宿主路径，根只读由"不向临时目录外写"保证。
//!
//! 不提供内核级隔离（strength=1），不用于完全不可信代码。

use super::super::config::{IsolationLevel, SandboxConfig};
use super::super::error::SandboxError;
use super::super::state::{LifecycleAction, SandboxState};
use super::super::Sandbox;
use super::super::SandboxResult;
use super::CodeLanguage;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 单次执行 stdout/stderr 最大捕获字节数（v2.8.7，防父进程内存 DoS）
const MAX_OUTPUT_BYTES: usize = 1_048_576; // 1 MiB

/// 沙箱工作目录唯一序号（同进程内严格递增）。
///
/// 同一个插件 id 在以下情形会产生多个并存实例：并行测试、`hot_reload` 双缓冲
/// （新旧版本短暂并存）。若目录名只含 id，它们会共享目录，一个 `destroy`
/// （`remove_dir_all`）会删掉另一个实例正在写 entry / 调用的目录——这正是
/// macOS CI `load_legacy` 偶发失败（`entry 写入失败: No such file or directory`）
/// 与热更新后新版本目录被旧实例误删的根因。用 `{pid}-{seq}` 使每个实例独占目录。
static SANDBOX_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 进程级隔离沙箱
pub struct ProcessSandbox {
    id: String,
    state: SandboxState,
    work_dir: Option<PathBuf>,
    cfg: Option<SandboxConfig>,
    created_ms: u64,
    /// Windows Job Object（create 时建立，destroy/Drop 时杀整棵树）
    #[cfg(windows)]
    job: Option<super::winjob::WinJob>,
}

impl ProcessSandbox {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            state: SandboxState::Pending,
            work_dir: None,
            cfg: None,
            created_ms: now_ms(),
            #[cfg(windows)]
            job: None,
        }
    }

    /// 工作目录（创建后可用）
    pub fn work_dir(&self) -> Option<&Path> {
        self.work_dir.as_deref()
    }

    /// 创建时间（毫秒，审计用）
    pub fn created_ms(&self) -> u64 {
        self.created_ms
    }

    fn apply(&mut self, action: LifecycleAction) -> Result<(), SandboxError> {
        let from = self.state;
        self.state = from
            .transition(action)
            .map_err(|_| SandboxError::InvalidLifecycle {
                from: from.label().to_string(),
                action: action.label().to_string(),
            })?;
        Ok(())
    }

    /// 在沙箱工作目录内写文件（相对路径，拒绝逃逸）
    pub fn write_file(&self, relative: &str, content: &str) -> Result<(), SandboxError> {
        let dir = self
            .work_dir
            .as_ref()
            .ok_or(SandboxError::NotFound("sandbox not created".into()))?;
        let target = safe_join(dir, relative)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SandboxError::Internal(e.to_string()))?;
        }
        std::fs::write(&target, content).map_err(|e| SandboxError::Internal(e.to_string()))
    }

    /// 读沙箱内文件（相对路径）
    pub fn read_file(&self, relative: &str) -> Result<String, SandboxError> {
        let dir = self
            .work_dir
            .as_ref()
            .ok_or(SandboxError::NotFound("sandbox not created".into()))?;
        let target = safe_join(dir, relative)?;
        std::fs::read_to_string(&target).map_err(|e| SandboxError::Internal(e.to_string()))
    }

    /// 直接执行一段代码（Agent 主用法）：写入临时文件，调用对应解释器
    pub fn run_code(
        &mut self,
        lang: CodeLanguage,
        code: &str,
    ) -> Result<SandboxResult, SandboxError> {
        if self.state != SandboxState::Running {
            return Err(SandboxError::InvalidLifecycle {
                from: self.state.label().to_string(),
                action: "exec".into(),
            });
        }
        let (entry, program) = match lang {
            // macOS/Linux 自带 python3；Windows 官方安装名为 python
            CodeLanguage::Python => (
                "main.py",
                if cfg!(target_os = "windows") {
                    "python"
                } else {
                    "python3"
                },
            ),
            CodeLanguage::JavaScript => ("main.js", "node"),
        };
        self.write_file(entry, code)?;
        self.exec(program, &[entry.to_string()])
    }

    fn build_command(
        &self,
        program: &str,
        args: &[String],
    ) -> Result<std::process::Command, SandboxError> {
        let cfg = self
            .cfg
            .as_ref()
            .ok_or(SandboxError::NotFound("sandbox not configured".into()))?;
        let dir = self
            .work_dir
            .as_ref()
            .ok_or(SandboxError::NotFound("sandbox not created".into()))?;

        // 仅允许白名单解释器；shell 需显式 allow_shell
        let resolved = resolve_program(program, cfg)?;
        build_platform_command(&resolved, args, dir, cfg)
    }
}

/// Unix（Linux/macOS）：bash 包装 ulimit 资源上限；
/// 超时由 Rust 侧强杀（见 exec），**不依赖外部 timeout/gtimeout**——
/// macOS 自带无 GNU timeout（只有 coreutils 的 gtimeout），这正是 mac CI 红的根因。
#[cfg(not(target_os = "windows"))]
fn build_platform_command(
    resolved: &str,
    args: &[String],
    dir: &Path,
    cfg: &SandboxConfig,
) -> Result<std::process::Command, SandboxError> {
    let mut inner = String::new();
    let is_node = resolved == "node";
    // 地址空间（-v）单位 KB。
    //
    // Node/V8：启动时 mmap 预留大块虚拟地址（CodeRange，跨版本大小不同，
    // 新版可达 1GB+），是虚拟预留而非实际占用。ulimit -v 无法区分预留与
    // 实际占用，按 mem_mb 设会导致高版本 Node 启动即被信号杀死（CI 红）。
    // 因此 Node 的实际内存由 V8 官方 `--max-old-space-size`（下方）限制，
    // ulimit -v 只给 V8 预留留足余量（至少 2GB，大内存时 mem_mb+1GB）。
    // Python：不预留大虚拟地址，Linux 上 ulimit -v 精确等于 mem_mb。
    //
    // **macOS：内核不支持 RLIMIT_AS/VMEM（ulimit -v 直接失败或静默无效），
    // 故 Darwin 跳过 -v；Python 内存无强制原语（需 waiver），Node 仍由
    // --max-old-space-size 限制 JS 堆。**
    let vmem_mb = if is_node {
        u64::from(cfg.resources.mem_mb).max(1024) + 1024
    } else {
        u64::from(cfg.resources.mem_mb)
    };
    let vmem_kb = vmem_mb * 1024;
    inner.push_str(&format!(
        "case \"$(uname -s)\" in Linux) ulimit -v {vmem_kb} || exit 200;; Darwin);; *) ulimit -v {vmem_kb} || exit 200;; esac; "
    ));
    // CPU 时间（-t）单位秒，毫秒向上取整，至少 1 秒；Linux/macOS 均支持
    let cpu_secs = u64::from(cfg.resources.cpu_millis).div_ceil(1_000).max(1);
    inner.push_str(&format!("ulimit -t {cpu_secs} || exit 200; "));
    inner.push_str(&format!(
        "ulimit -u {} || exit 200; ",
        cfg.resources.max_processes
    ));
    inner.push_str(&format!(
        "ulimit -n {} || exit 200; ",
        cfg.resources.max_open_files
    ));
    // exec 替换 bash 为目标程序，最终只有一个进程（pid 即 bash），
    // Rust 侧 kill 该 pid 即可无孤儿地强杀。
    inner.push_str("exec ");
    inner.push_str(&shell_quote(resolved));
    if is_node {
        // V8 老生代堆上限（MB）：留 96MB 给运行时/栈/外部内存，至少 16MB。
        // 超出即 V8「heap out of memory」非零退出——实际 JS 内存被强制。
        let heap = u64::from(cfg.resources.mem_mb).saturating_sub(96).max(16);
        inner.push_str(&format!(" --max-old-space-size={heap}"));
    }
    for a in args {
        inner.push(' ');
        inner.push_str(&shell_quote(a));
    }

    let mut cmd = std::process::Command::new("bash");
    cmd.arg("-c")
        .arg(&inner)
        .current_dir(dir)
        // 不继承宿主环境
        .env_clear();
    // 仅注入受限 PATH，用于定位白名单解释器；其余变量不继承
    if let Some(p) = std::env::var_os("PATH") {
        cmd.env("PATH", p);
    }
    for (k, v) in &cfg.env {
        cmd.env(k, v);
    }
    Ok(cmd)
}

/// Windows：直接 spawn 解释器，不套 shell（无 bash/ulimit/timeout）。
#[cfg(target_os = "windows")]
fn build_platform_command(
    resolved: &str,
    args: &[String],
    dir: &Path,
    cfg: &SandboxConfig,
) -> Result<std::process::Command, SandboxError> {
    let mut cmd = std::process::Command::new(resolved);
    // Node：V8 flag 必须在脚本名之前。真实 JS 堆上限由 --max-old-space-size
    // 精确限制（与 Linux 对称）；Job Object 的提交内存只作兜底余量。
    if resolved == "node" {
        let heap = u64::from(cfg.resources.mem_mb).saturating_sub(96).max(16);
        cmd.arg(format!("--max-old-space-size={heap}"));
    }
    cmd.args(args).current_dir(dir).env_clear();
    // 受限 PATH：定位白名单解释器
    if let Some(p) = std::env::var_os("PATH") {
        cmd.env("PATH", p);
    }
    // SystemRoot 是 Windows 系统组件（BCrypt/加密 API、DLL 加载）的必需
    // 变量。env_clear 后缺失它，Node 启动时 CSPRNG 初始化断言失败
    // （`ncrypto::CSPRNG(nullptr, 0)` → abort 134）。这是系统级、非用户
    // 特定变量，不携带用户数据，可安全保留。
    if let Some(sr) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", sr);
    }
    // TEMP/TMP 指向沙箱内 tmp（隔离，不使用宿主 temp）
    let tmp_dir = dir.join("tmp");
    if std::fs::create_dir_all(&tmp_dir).is_ok() {
        cmd.env("TEMP", &tmp_dir);
        cmd.env("TMP", &tmp_dir);
    }
    for (k, v) in &cfg.env {
        cmd.env(k, v);
    }
    Ok(cmd)
}

impl super::super::Sandbox for ProcessSandbox {
    fn isolation(&self) -> IsolationLevel {
        IsolationLevel::Process
    }

    fn state(&self) -> SandboxState {
        self.state
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn create(&mut self, cfg: &SandboxConfig) -> Result<(), SandboxError> {
        if let Err(errs) = cfg.validate() {
            return Err(SandboxError::InvalidConfig(errs.join("; ")));
        }
        // 能力校验（v2.9.1）：逐条检查配置要求的边界，进程后端无法强制的
        // （网络拒绝/文件系统子树限制/磁盘配额）必须显式豁免（带理由），
        // 否则具名拒绝——绝不静默接受一个做不到的隔离策略。
        let declaration = super::super::capability::process_declaration();
        for cap in cfg.required_boundaries() {
            declaration.check(cap, &cfg.waivers)?;
        }
        // v3.5.2（AU-28）：请求的隔离级别不得高于本后端实际可达级别。
        // 进程后端如实只交付 Process（strength=1）；调用方请求 Container/MicroVM
        // 而不豁免 → 具名拒绝，绝不静默降级后按 Process 跑。
        let actual = self.isolation();
        if cfg.isolation.strength() > actual.strength() {
            let waived = cfg.waivers.iter().any(|w| {
                w.boundary == super::super::capability::Capability::IsolationLevel
                    && !w.justification.trim().is_empty()
            });
            if !waived {
                return Err(SandboxError::PolicyNotEnforceable {
                    boundary: super::super::capability::Capability::IsolationLevel,
                    backend: "process".to_string(),
                    detail: format!(
                        "SANDBOX_ISOLATION_UNAVAILABLE: 请求隔离级别 {} 高于进程后端实际可达级别 {}；\
                         请改用容器/microVM 后端，或用带非空理由的 Waiver(Capability::IsolationLevel) 显式接受降级",
                        cfg.isolation.label(),
                        actual.label()
                    ),
                });
            }
        }
        let id = if cfg.sandbox_id.is_empty() {
            self.id.clone()
        } else {
            cfg.sandbox_id.clone()
        };
        self.id = id;
        self.apply(LifecycleAction::Create)?;

        let base = cfg.work_dir_base.clone().unwrap_or_else(std::env::temp_dir);
        let seq = SANDBOX_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = base.join(format!(
            "au-sandbox-{}-{}-{}",
            self.id,
            std::process::id(),
            seq
        ));
        std::fs::create_dir_all(&dir).map_err(|e| SandboxError::Internal(e.to_string()))?;
        // 写入初始文件
        for (p, content) in &cfg.initial_files {
            let target = safe_join(&dir, p)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| SandboxError::Internal(e.to_string()))?;
            }
            std::fs::write(&target, content).map_err(|e| SandboxError::Internal(e.to_string()))?;
        }
        self.work_dir = Some(dir);
        self.cfg = Some(cfg.clone());
        // v2.8.7：Windows 建立 Job Object（内存/进程数上限 + 句柄关闭杀树）
        #[cfg(windows)]
        {
            // Job 在 create 时建立、其后可跑 python/node，故提交内存上限须能
            // 覆盖最吃内存的 Node：与 Linux ulimit -v 对称地留足余量（至少
            // 2GB），否则高版本 Node 启动即 abort（134）。Node 的真实 JS 堆
            // 在 build_platform_command 用 --max-old-space-size 精确限制。
            let effective_mem = cfg.resources.mem_mb.max(1024).saturating_add(1024);
            self.job = Some(
                super::winjob::WinJob::new(effective_mem, cfg.resources.max_processes)
                    .map_err(SandboxError::Internal)?,
            );
        }
        Ok(())
    }

    fn start(&mut self) -> Result<(), SandboxError> {
        self.apply(LifecycleAction::Start)?;
        self.apply(LifecycleAction::MarkReady)
    }

    fn exec(&mut self, program: &str, args: &[String]) -> Result<SandboxResult, SandboxError> {
        if self.state != SandboxState::Running {
            return Err(SandboxError::InvalidLifecycle {
                from: self.state.label().to_string(),
                action: "exec".into(),
            });
        }
        let cfg = self
            .cfg
            .clone()
            .ok_or(SandboxError::Internal("no cfg".into()))?;
        let mut cmd = self.build_command(program, args)?;
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let start = Instant::now();
        let mut child = cmd
            .spawn()
            .map_err(|e| SandboxError::Internal(e.to_string()))?;

        // v2.8.7：Windows 把子进程纳入 Job Object（其后 fork 的子进程也在 job）
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            if let Some(job) = &self.job {
                if let Err(e) = job.assign(child.as_raw_handle() as isize) {
                    kill_process_tree(&mut child);
                    return Err(SandboxError::Internal(e));
                }
            }
        }

        // 独立线程有界读取 stdout/stderr（v2.8.7：take 上限防父进程内存 DoS；
        // 写满后不再读，管道缓冲反压会让子进程阻塞，等超时被 kill）
        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();
        let stdout_handle = std::thread::spawn(move || read_bounded(&mut stdout_pipe));
        let stderr_handle = std::thread::spawn(move || read_bounded(&mut stderr_pipe));

        // 轮询等待；超时由 Rust 侧强杀整棵进程树（不依赖外部 timeout/gtimeout）
        let timeout = cfg.resources.timeout_ms;
        let status;
        loop {
            match child.try_wait() {
                Ok(Some(s)) => {
                    status = s;
                    break;
                }
                Ok(None) => {
                    if start.elapsed() >= std::time::Duration::from_millis(timeout) {
                        let actual = start.elapsed().as_millis() as u64;
                        kill_process_tree(&mut child);
                        let _ = stdout_handle.join();
                        let _ = stderr_handle.join();
                        return Err(SandboxError::ResourceLimitExceeded {
                            kind: "timeout".into(),
                            limit: timeout,
                            actual,
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                // v2.8.7：try_wait 出错也必须先 kill 回收，避免孤儿进程
                Err(e) => {
                    kill_process_tree(&mut child);
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(SandboxError::Internal(format!(
                        "等待子进程失败并已清理: {e}"
                    )));
                }
            }
        }

        let wall_ms = start.elapsed().as_millis() as u64;
        let stdout_bytes = stdout_handle
            .join()
            .map_err(|_| SandboxError::Internal("stdout join".into()))?;
        let stderr_bytes = stderr_handle
            .join()
            .map_err(|_| SandboxError::Internal("stderr join".into()))?;
        let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
        let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();
        let exit_code = status.code().unwrap_or(-1);
        Ok(SandboxResult {
            exit_code,
            stdout,
            stderr,
            // v3.5.3（AU-31）：进程后端不测量 CPU 时间，如实标 None（未测量），
            // 不再填 0 冒充"测得 0ms"。
            cpu_time_ms: None,
            wall_ms,
            isolation: IsolationLevel::Process,
            state: self.state,
        })
    }

    fn pause(&mut self) -> Result<(), SandboxError> {
        // 进程级沙箱无常驻进程；同步 exec 之外没有运行中的子进程，
        // pause 仅标记状态（不真正保存进程内存）。真正的 checkpoint 在 v2.7.8。
        self.apply(LifecycleAction::Pause)?;
        self.apply(LifecycleAction::MarkPaused)
    }

    fn resume(&mut self) -> Result<(), SandboxError> {
        self.apply(LifecycleAction::Resume)?;
        self.apply(LifecycleAction::MarkReady)
    }

    fn destroy(&mut self) -> Result<(), SandboxError> {
        if !self.state.is_terminal() {
            self.apply(LifecycleAction::Stop)?;
            self.apply(LifecycleAction::MarkStopped)?;
        }
        // v2.8.7：Windows 关闭 job 句柄 → 杀整棵进程树
        #[cfg(windows)]
        {
            let _ = self.job.take();
        }
        if let Some(dir) = self.work_dir.take() {
            // 尽力清理临时目录；失败不阻断（标记 Internal 供审计）
            let _ = std::fs::remove_dir_all(&dir);
        }
        Ok(())
    }
}

// ---------- helpers ----------

/// 解析允许执行的程序：仅白名单解释器；bash 需 allow_shell
fn resolve_program(program: &str, cfg: &SandboxConfig) -> Result<String, SandboxError> {
    let allowed = matches!(program, "python" | "python3" | "node");
    if allowed {
        // 用 PATH 上已有的（env_clear 后我们注入 PATH，见下），直接给名字
        return Ok(program.to_string());
    }
    if program == "bash" || program == "sh" {
        if cfg.allow_shell {
            return Ok(program.to_string());
        }
        return Err(SandboxError::IsolationViolation(format!(
            "shell 执行未被允许（cfg.allow_shell=false）: {program}"
        )));
    }
    Err(SandboxError::IsolationViolation(format!(
        "程序不在沙箱白名单: {program}"
    )))
}

/// 把相对路径安全地 join 到沙箱目录，拒绝绝对路径、Windows 盘符/UNC 与 `..` 逃逸。
///
/// v3.5.2（AU-26）：旧实现只查 `relative.starts_with('/')` 与 ParentDir，在 Windows
/// 上漏放 `C:\...` 与 `\\server\share`。统一委托
/// [`super::super::config::path_escapes_sandbox`]（跨平台字符串判定）。
fn safe_join(base: &Path, relative: &str) -> Result<PathBuf, SandboxError> {
    if super::super::config::path_escapes_sandbox(relative) {
        return Err(SandboxError::IsolationViolation(format!(
            "拒绝绝对路径/盘符/UNC/逃逸: {relative}"
        )));
    }
    Ok(base.join(relative))
}

/// 简单 shell 引用（包单引号，内部单引号转义）。
/// 仅 Unix 的 bash 包装使用；Windows 直接 spawn 不套 shell，故不编译。
#[cfg(not(target_os = "windows"))]
fn shell_quote(s: &str) -> String {
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 有界读取管道输出（v2.8.7）：最多读 MAX_OUTPUT_BYTES，超出截断
fn read_bounded<R: std::io::Read>(pipe: &mut Option<R>) -> Vec<u8> {
    use std::io::Read;
    let mut buf = Vec::new();
    if let Some(r) = pipe.take() {
        let mut limited = r.take(MAX_OUTPUT_BYTES as u64 + 1);
        let _ = limited.read_to_end(&mut buf);
        if buf.len() > MAX_OUTPUT_BYTES {
            buf.truncate(MAX_OUTPUT_BYTES);
        }
    }
    buf
}

/// 强杀子进程并回收（v2.8.7）。
///
/// Unix：exec 已把 bash 替换为目标程序，child.kill 即杀目标；wait 回收。
/// Windows：先 taskkill /T /F 杀整棵进程树，再 kill/wait 兜底。
/// （Windows 上另有 Job Object 做被动兜底，见 runtime/winjob.rs）
#[cfg(unix)]
fn kill_process_tree(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn kill_process_tree(child: &mut std::process::Child) {
    let pid = child.id().to_string();
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

// env_clear 后子进程需要 PATH 才能找到解释器。
// 在 build_command 中注入受限 PATH。
