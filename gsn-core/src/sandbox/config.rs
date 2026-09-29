//! Agent Sandbox 配置与策略（v2.7.6）
//!
//! 配置必须在创建沙箱前显式给出，默认值遵循"最小权限"：
//! 默认无网络、只读根、非 root、有界 CPU/内存/磁盘/进程数/超时。
//! 调用方需要放开某项能力时必须显式声明，不能依赖宽松默认值。

use serde::{Deserialize, Serialize};

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
    /// CPU 配额（毫核，1000 = 1 vCPU）
    pub cpu_millis: u32,
    /// 内存上限（MB）
    pub mem_mb: u32,
    /// 临时磁盘上限（MB）
    pub disk_mb: u32,
    /// 最大子进程/线程数
    pub max_processes: u32,
    /// 最大文件句柄数
    pub max_open_files: u32,
    /// 执行超时（毫秒），到点强杀
    pub timeout_ms: u64,
}

impl Default for ResourceLimits {
    /// 最小权限默认值
    fn default() -> Self {
        Self {
            cpu_millis: 1000,
            mem_mb: 256,
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
            errs.push("允许出站但白名单为空，等同全放开；至少给一个目标或显式标 deny_raw_ip".to_string());
        }
        if self.template.trim().is_empty() {
            errs.push("template 不能为空".to_string());
        }
        // 初始文件路径不得逃逸沙箱（禁止绝对路径 / ..）
        for (p, _) in &self.initial_files {
            if p.starts_with('/') || p.contains("..") {
                errs.push(format!("initial_files 路径逃逸沙箱: {p}"));
            }
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }
}
