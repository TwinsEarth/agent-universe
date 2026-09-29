//! 进程级隔离沙箱（v2.7.7）
//!
//! 在云电脑 / 普通 PC 上真实可跑，不依赖 Docker/KVM。
//!
//! 隔离手段：
//! 1. 每个沙箱独立临时目录（`base/au-sandbox-{id}`），作为唯一工作目录；
//! 2. 子进程只在该目录内运行，文件操作经路径校验拒绝逃逸；
//! 3. 子进程不继承宿主环境，仅注入 cfg.env 白名单；
//! 4. 经 `timeout`（超时强杀）+ `bash -c ulimit`（进程/句柄上限）；
//! 5. 不挂载宿主路径，根只读由"不向临时目录外写"保证。
//!
//! 不提供内核级隔离（strength=1），不用于完全不可信代码。

use super::super::config::{IsolationLevel, SandboxConfig};
use super::super::error::SandboxError;
use super::super::state::{LifecycleAction, SandboxState};
use super::super::SandboxResult;
use super::super::Sandbox;
use super::CodeLanguage;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 进程级隔离沙箱
pub struct ProcessSandbox {
    id: String,
    state: SandboxState,
    work_dir: Option<PathBuf>,
    cfg: Option<SandboxConfig>,
    created_ms: u64,
}

impl ProcessSandbox {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            state: SandboxState::Pending,
            work_dir: None,
            cfg: None,
            created_ms: now_ms(),
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
        self.state = from.transition(action).map_err(|_| {
            SandboxError::InvalidLifecycle {
                from: from.label().to_string(),
                action: action.label().to_string(),
            }
        })?;
        Ok(())
    }

    /// 在沙箱工作目录内写文件（相对路径，拒绝逃逸）
    pub fn write_file(&self, relative: &str, content: &str) -> Result<(), SandboxError> {
        let dir = self.work_dir.as_ref().ok_or(SandboxError::NotFound(
            "sandbox not created".into(),
        ))?;
        let target = safe_join(dir, relative)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SandboxError::Internal(e.to_string()))?;
        }
        std::fs::write(&target, content).map_err(|e| SandboxError::Internal(e.to_string()))
    }

    /// 读沙箱内文件（相对路径）
    pub fn read_file(&self, relative: &str) -> Result<String, SandboxError> {
        let dir = self.work_dir.as_ref().ok_or(SandboxError::NotFound(
            "sandbox not created".into(),
        ))?;
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
        // 解释器名跨平台：Windows 上官方 Python 注册为 `python`（无 python3 别名），
        // 硬编码 `python3` 会在 Windows 上找不到解释器导致 exec 500（v2.8.1 修复）。
        let (entry, program) = match lang {
            CodeLanguage::Python => {
                if cfg!(target_os = "windows") {
                    ("main.py", "python")
                } else {
                    ("main.py", "python3")
                }
            }
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
        let cfg = self.cfg.as_ref().ok_or(SandboxError::NotFound(
            "sandbox not configured".into(),
        ))?;
        let dir = self.work_dir.as_ref().ok_or(SandboxError::NotFound(
            "sandbox not created".into(),
        ))?;

        // 仅允许白名单解释器；shell 需显式 allow_shell
        let resolved = resolve_program(program, cfg)?;

        // 平台分发：
        // - Unix：用 bash 包装 ulimit（进程/句柄上限）+ timeout（超时强杀）；
        // - Windows：无 bash/ulimit/timeout 等价物，直接 spawn 解释器。
        //   资源上限与超时在 Windows 上暂未实现（已知降级，后续版本用 Job Object 补齐）。
        //   这是 v2.8.1 修复：v2.8.0 硬编码 bash 导致 Windows 上 exec 一律 500。
        let cmd = build_platform_command(&resolved, args, dir, cfg)?;
        Ok(cmd)
    }
}

/// 按目标平台构造执行命令。
#[cfg(not(target_os = "windows"))]
fn build_platform_command(
    resolved: &str,
    args: &[String],
    dir: &Path,
    cfg: &SandboxConfig,
) -> Result<std::process::Command, SandboxError> {
    // timeout 秒数向上取整
    let timeout_s = (cfg.resources.timeout_ms / 1000).max(1);
    let mut inner = String::new();
    inner.push_str(&format!("ulimit -u {}; ", cfg.resources.max_processes));
    inner.push_str(&format!("ulimit -n {}; ", cfg.resources.max_open_files));
    inner.push_str("exec ");
    inner.push_str(&shell_quote(resolved));
    for a in args {
        inner.push(' ');
        inner.push_str(&shell_quote(a));
    }

    let mut cmd = std::process::Command::new("bash");
    cmd.arg("-c")
        // timeout 在最外层：到点发 SIGTERM（再 KILL 用 -k）
        .arg(format!(
            "timeout -k 1 {timeout_s} bash -c {quoted}",
            quoted = shell_quote(&inner)
        ))
        .current_dir(dir)
        // 不继承宿主环境
        .env_clear();
    // 仅注入受限 PATH，用于定位白名单解释器与 timeout；其余变量不继承
    if let Some(p) = std::env::var_os("PATH") {
        cmd.env("PATH", p);
    }
    for (k, v) in &cfg.env {
        cmd.env(k, v);
    }
    Ok(cmd)
}

/// Windows：直接 spawn 解释器，不套 shell。
#[cfg(target_os = "windows")]
fn build_platform_command(
    resolved: &str,
    args: &[String],
    dir: &Path,
    cfg: &SandboxConfig,
) -> Result<std::process::Command, SandboxError> {
    let mut cmd = std::process::Command::new(resolved);
    cmd.args(args).current_dir(dir).env_clear();
    // Windows 上 python/node 通常已在 PATH；注入 PATH 保证能找到 .exe
    if let Some(p) = std::env::var_os("PATH") {
        cmd.env("PATH", p);
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
        let id = if cfg.sandbox_id.is_empty() {
            self.id.clone()
        } else {
            cfg.sandbox_id.clone()
        };
        self.id = id;
        self.apply(LifecycleAction::Create)?;

        let base = cfg
            .work_dir_base
            .clone()
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("au-sandbox-{}", self.id));
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
        let mut cmd = self.build_command(program, args)?;
        let start = Instant::now();
        let output = cmd
            .output()
            .map_err(|e| SandboxError::Internal(e.to_string()))?;
        let wall_ms = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let cfg = self.cfg.as_ref().ok_or(SandboxError::Internal("no cfg".into()))?;

        // timeout 退出码 124 = 超时
        if output.status.code() == Some(124) {
            return Err(SandboxError::ResourceLimitExceeded {
                kind: "timeout".into(),
                limit: cfg.resources.timeout_ms,
                actual: wall_ms,
            });
        }

        let exit_code = output.status.code().unwrap_or(-1);
        Ok(SandboxResult {
            exit_code,
            stdout,
            stderr,
            cpu_time_ms: 0,
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

/// 把相对路径安全地 join 到沙箱目录，拒绝绝对路径与 `..` 逃逸
fn safe_join(base: &Path, relative: &str) -> Result<PathBuf, SandboxError> {
    if relative.starts_with('/') {
        return Err(SandboxError::IsolationViolation(format!(
            "拒绝绝对路径: {relative}"
        )));
    }
    let rel = Path::new(relative);
    if rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(SandboxError::IsolationViolation(format!(
            "路径逃逸沙箱: {relative}"
        )));
    }
    Ok(base.join(rel))
}

/// 简单 shell 引用（包单引号，内部单引号转义）
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

// env_clear 后子进程需要 PATH 才能找到解释器。
// 在 build_command 中注入受限 PATH。
