//! Agent Sandbox 配置与策略（v2.7.6）
//!
//! 配置必须在创建沙箱前显式给出，默认值遵循"最小权限"：
//! 默认无网络、只读根、非 root、有界 CPU/内存/磁盘/进程数/超时。
//! 调用方需要放开某项能力时必须显式声明，不能依赖宽松默认值。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::capability::{Capability, Waiver};

/// 隔离级别（如实标注，不得静默升级/降级）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// 进程级隔离（独立临时目录 + 超时/资源约束），无 KVM/Docker 时的 fallback
    Process,
    /// 容器共享内核（Docker/Podman runc）
    ContainerSharedKernel,
    /// microVM 硬件虚拟化（Firecracker / Cloud Hypervisor / gVisor KVM）
    MicroVM,
}

impl IsolationLevel {
    pub fn label(&self) -> &'static str {
        match self {
            IsolationLevel::Process => "process",
            IsolationLevel::ContainerSharedKernel => "container-shared-kernel",
            IsolationLevel::MicroVM => "microvm",
        }
    }

    /// 隔离强度（数字越大越强），用于降级/升级决策
    pub fn strength(&self) -> u8 {
        match self {
            IsolationLevel::Process => 1,
            IsolationLevel::ContainerSharedKernel => 2,
            IsolationLevel::MicroVM => 3,
        }
    }
}

/// 资源上限（全部为硬上限，超限即终止并回收）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU 时间上限（毫秒；进程后端按 `ulimit -t` 向上取整到秒，容器按 cgroup）。
    /// 注意这是**累计 CPU 时间**而非"毫核速率"——速率配额在普通进程上无法强制。
    pub cpu_millis: u32,
    /// 内存/地址空间上限（MB）。
    /// 进程后端 Unix 用 `ulimit -v` 限制**虚拟地址空间**（不是 RSS）：
    /// 普通进程没有可用的 RSS 硬上限（`ulimit -m` 在现代 Linux 已不生效），
    /// 虚拟地址空间上限是唯一可跨 Unix 的原语。它对 Python 用 256MB 即可，
    /// 但 V8/Node 会预留较大的 CodeRange，实测 Node 需 ≥768MB 才能启动。
    /// Windows 用 Job Object 限制实际提交内存。
    pub mem_mb: u32,
    /// 临时磁盘上限（MB）——进程后端无文件系统配额，此条需容器或豁免
    pub disk_mb: u32,
    /// 最大子进程/线程数
    pub max_processes: u32,
    /// 最大文件句柄数
    pub max_open_files: u32,
    /// 执行墙钟超时（毫秒），到点强杀整棵进程树
    pub timeout_ms: u64,
}

impl Default for ResourceLimits {
    /// 最小权限默认值
    fn default() -> Self {
        Self {
            // 累计 CPU 时间上限 10 秒（配合 30 秒墙钟）；向上取整到秒
            cpu_millis: 10_000,
            // 虚拟地址空间上限 768MB：Python 256MB 即够，但 V8/Node 需预留
            // CodeRange（实测 ≥768MB 才能启动）；取 768 兼容两种运行时。
            mem_mb: 768,
            disk_mb: 128,
            // 现代运行时（V8/node 等）启动即需 worker 线程，64 会 abort；
            // 512 覆盖解释器自身线程，仍限制子进程无限扩张
            max_processes: 512,
            max_open_files: 256,
            timeout_ms: 30_000,
        }
    }
}

/// 网络策略（默认全部拒绝）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NetworkPolicy {
    /// 是否允许出站网络（默认 false）
    pub allow_egress: bool,
    /// 出站允许的 host:port 白名单（空 = 不放行任何目标）
    pub egress_allowlist: Vec<String>,
    /// 是否允许入站（默认 false，沙箱一般不接受外部主动连入）
    pub allow_ingress: bool,
    /// 是否禁止裸 IP（要求域名，便于审计；默认 false）
    pub deny_raw_ip: bool,
}

/// 文件系统策略
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemPolicy {
    /// 根文件系统只读（默认 true）
    pub read_only_root: bool,
    /// 是否可写（仅独立临时目录可写；默认 true，只在该目录内）
    pub writable_tmp: bool,
    /// 是否允许挂载宿主路径（默认 false）
    pub allow_host_mounts: bool,
    /// 允许只读挂载的宿主路径（白名单）
    pub read_only_host_paths: Vec<String>,
}

impl Default for FilesystemPolicy {
    fn default() -> Self {
        Self {
            read_only_root: true,
            writable_tmp: true,
            allow_host_mounts: false,
            read_only_host_paths: Vec::new(),
        }
    }
}

/// 沙箱创建配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// 调用方指定的沙箱 ID（空则自动生成）
    pub sandbox_id: String,
    /// 模板/镜像（进程级为运行时名，如 "python:3.11"；容器/microVM 为镜像名）
    pub template: String,
    /// 期望隔离级别（实际能达到的级别由运行时探测决定，不得静默冒充）
    pub isolation: IsolationLevel,
    /// 资源上限
    pub resources: ResourceLimits,
    /// 网络策略
    pub network: NetworkPolicy,
    /// 文件系统策略
    pub filesystem: FilesystemPolicy,
    /// 工作目录内初始文件（相对路径 → 内容）
    pub initial_files: Vec<(String, String)>,
    /// 是否允许执行任意 shell（默认 false，仅允许指定语言的代码）
    pub allow_shell: bool,
    /// 环境变量（白名单；不继承宿主环境，长期密钥不得进入）
    pub env: Vec<(String, String)>,
    /// 沙箱工作目录的父目录（None = 系统临时目录）。管理器统一编排时指定。
    pub work_dir_base: Option<PathBuf>,
    /// 显式豁免的边界（必须带非空理由，记入审计）。
    /// 进程后端无法强制网络拒绝/文件系统子树限制/磁盘配额，默认配置要求这些时
    /// 会被拒绝，除非在此显式声明放弃——这迫使调用方明确"这段代码可信"。
    pub waivers: Vec<Waiver>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            sandbox_id: String::new(),
            template: "process:generic".to_string(),
            isolation: IsolationLevel::Process,
            resources: ResourceLimits::default(),
            network: NetworkPolicy::default(),
            filesystem: FilesystemPolicy::default(),
            initial_files: Vec::new(),
            allow_shell: false,
            env: Vec::new(),
            work_dir_base: None,
            waivers: Vec::new(),
        }
    }
}

impl SandboxConfig {
    /// 校验配置合法性（不合法返回原因列表，不进入执行）
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errs = Vec::new();
        if self.resources.cpu_millis == 0 {
            errs.push("cpu_millis 必须 > 0".to_string());
        }
        if self.resources.mem_mb == 0 {
            errs.push("mem_mb 必须 > 0".to_string());
        }
        if self.resources.timeout_ms == 0 {
            errs.push("timeout_ms 必须 > 0".to_string());
        }
        if self.network.allow_egress && self.network.egress_allowlist.is_empty() {
            errs.push(
                "允许出站但白名单为空，等同全放开；至少给一个目标或显式标 deny_raw_ip".to_string(),
            );
        }
        if self.template.trim().is_empty() {
            errs.push("template 不能为空".to_string());
        }
        // 初始文件路径不得逃逸沙箱（v3.5.2 AU-26：跨平台，覆盖 Windows 盘符/UNC）
        for (p, _) in &self.initial_files {
            if path_escapes_sandbox(p) {
                errs.push(format!("initial_files 路径逃逸沙箱: {p}"));
            }
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }

    /// 该配置隐含的**必须由后端强制**的边界集合。
    ///
    /// 后端在 create 时逐条 [`CapabilityDeclaration::check`]：强制了，或被带理由
    /// 的豁免覆盖，否则具名拒绝。这保证"请求了隔离"不会被一个做不到隔离的后端
    /// 静默接受。
    pub fn required_boundaries(&self) -> Vec<Capability> {
        use Capability::*;
        let mut required = Vec::new();

        // 所有沙箱都强制的基础边界
        required.push(EnvAllowlist);
        required.push(OutputCap);
        required.push(WorkDirIsolation);

        // 网络
        if self.network.allow_egress {
            if !self.network.egress_allowlist.is_empty() {
                required.push(NetworkAllowList);
            }
        } else {
            // 默认拒绝出站 → 需要真正的网络拒绝能力
            required.push(NetworkDenyAll);
        }

        // 文件系统：默认只读根（仅 tmp 可写）→ 需要子树限制
        if self.filesystem.read_only_root {
            required.push(FilesystemConfinement);
        }

        // 资源上限
        let r = &self.resources;
        if r.timeout_ms > 0 {
            required.push(Timeout);
        }
        if r.mem_mb > 0 {
            required.push(MemoryLimit);
        }
        if r.cpu_millis > 0 {
            required.push(CpuLimit);
        }
        if r.disk_mb > 0 {
            required.push(DiskQuota);
        }
        if r.max_processes > 0 {
            required.push(ProcessCountLimit);
        }
        if r.max_open_files > 0 {
            required.push(OpenFileLimit);
        }

        required
    }

    /// 构造一个"本地可信执行"配置：显式豁免进程后端在所有平台都无法强制的
    /// 三条边界（网络拒绝、文件系统子树限制、磁盘配额）。
    ///
    /// 仅在 operator 明确信任宿主机与待执行代码时使用（例如本机开发、单机部署，
    /// 且 API 已要求认证 + 所有权）。豁免会被写入审计日志，**绝不能作为默认值**。
    pub fn trusted_local(justification: &str) -> Self {
        let mut cfg = Self::default();
        let reason = |what: &str| {
            format!(
                "process backend has no primitive for {what} on this platform; \
                 operator accepts a host-level boundary [{justification}]"
            )
        };
        cfg.waivers = vec![
            Waiver::new(Capability::NetworkDenyAll, reason("network egress denial")),
            Waiver::new(
                Capability::FilesystemConfinement,
                reason("filesystem subtree confinement"),
            ),
            Waiver::new(Capability::DiskQuota, reason("disk quota")),
        ];
        // macOS 无 RLIMIT_AS，Python 内存无强制原语 → 显式 waiver（带理由）。
        if cfg!(target_os = "macos") {
            cfg.waivers.push(Waiver::new(
                Capability::MemoryLimit,
                reason("memory (RLIMIT_AS)"),
            ));
        }
        // Windows Job Object 对普通进程不强制 CPU 时间与句柄数（仅内存/进程数），
        // 而 default 资源配置会请求 CpuLimit/OpenFileLimit → 必须显式 waiver，
        // 否则 create 在 Windows 上 PolicyNotEnforceable（v2.9.2 CI 首次暴露）。
        if cfg!(target_os = "windows") {
            cfg.waivers.push(Waiver::new(
                Capability::CpuLimit,
                reason("CPU time (Job Object)"),
            ));
            cfg.waivers.push(Waiver::new(
                Capability::OpenFileLimit,
                reason("open handle count (Job Object)"),
            ));
        }
        cfg
    }
}

/// 跨平台判断一个**相对**路径是否试图逃逸沙箱（v3.5.2，AU-26）。
///
/// 旧判定只挡 Unix `/` 前缀与 `..`，在 Windows 上漏放：
/// - 盘符绝对路径 `C:\...` / `C:/...`（不以 `/` 开头、不含 `..`）；
/// - UNC 路径 `\\server\share\...`（以反斜杠开头）。
///
/// 由于 `std::path::Path::is_absolute()` 的语义随编译平台变化（在 Linux 上构造
/// `C:\foo` 会被判为相对路径），这里**显式按字符串**同时检查：
/// 前导 `/`、前导 `\`、`X:` 盘符（X 为 ASCII 字母），并以 `is_absolute()` 兜底。
pub fn path_escapes_sandbox(rel: &str) -> bool {
    let r = rel.trim();
    if r.is_empty() {
        return false;
    }
    // Unix 根路径 / Windows UNC（\\server）/ Windows 反斜杠分隔的绝对写法
    if r.starts_with('/') || r.starts_with('\\') {
        return true;
    }
    // Windows 盘符前缀 "X:"（X 为 ASCII 字母）——无论后跟 \ 还是 / 一律拒绝
    let b = r.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return true;
    }
    // 平台原生绝对路径兜底
    if Path::new(r).is_absolute() {
        return true;
    }
    Path::new(r)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AU-26：旧判定（仅 `/` 前缀 + `..`）会漏放 Windows 盘符与 UNC；
    /// 新判定在任何编译平台都按字符串显式拒绝这些形态。
    #[test]
    fn rejects_windows_drive_and_unc_and_parent() {
        // 相对合法路径 → 不逃逸
        assert!(!path_escapes_sandbox("main.py"));
        assert!(!path_escapes_sandbox("tmp/out.jsonl"));
        assert!(!path_escapes_sandbox("sub/dir/file.txt"));

        // Unix 绝对路径
        assert!(path_escapes_sandbox("/etc/passwd"));
        // `..` 逃逸
        assert!(path_escapes_sandbox("../escape"));
        assert!(path_escapes_sandbox("a/../../b"));

        // Windows 盘符（反斜杠 / 正斜杠）——旧实现漏放
        assert!(path_escapes_sandbox("C:\\Windows\\system32"));
        assert!(path_escapes_sandbox("C:/Users/x"));
        // UNC
        assert!(path_escapes_sandbox("\\\\server\\share\\x"));
        assert!(path_escapes_sandbox("\\unc\\path"));
    }

    /// AU-26：validate() 对 initial_files 的 Windows 绝对路径报错。
    #[test]
    fn validate_rejects_windows_absolute_initial_file() {
        let cfg = SandboxConfig {
            initial_files: vec![("C:\\Windows\\evil.exe".to_string(), "x".to_string())],
            ..SandboxConfig::default()
        };
        let errs = cfg.validate().expect_err("应拒绝 Windows 盘符路径");
        assert!(errs.iter().any(|e| e.contains("逃逸沙箱")));

        let ok = SandboxConfig {
            initial_files: vec![("main.py".to_string(), "print(1)".to_string())],
            ..SandboxConfig::default()
        };
        assert!(ok.validate().is_ok());
    }
}
