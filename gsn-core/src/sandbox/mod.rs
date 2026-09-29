//! Agent Sandbox — 智能体沙箱（v2.7.6 起大版本重构）
//!
//! # 定位
//!
//! Agent Sandbox 位于 Agent Infra 的**执行层**，是"给 Agent 使用的一台计算机"。
//! 模型负责推理并生成下一步行动，编排系统负责组织任务/协调工具，当任务需要
//! 运行代码、访问文件、操作浏览器或调用外部 API 时，由沙箱承载这些真实执行。
//!
//! # 核心职责
//!
//! 1. 为进程、文件、网络、存储、系统权限设定边界，任务之间互不干扰；
//! 2. 管理完整生命周期：创建 → 启动 → 暂停 → 状态保存 → 恢复 → 回收；
//! 3. 区分**沙箱运行状态**（自身内存/文件/进程）与**业务状态**（任务进度/DB/
//!    外部系统，由上层记录），沙箱只负责前者；
//! 4. 承载两类工作负载：
//!    - **训练评测**（Agentic RL / RSI 自我进化）：大量并行隔离环境 + Rollout/Reward
//!      + 状态复用（Reset/Fork/Replay/内存级 Checkpoint）；
//!    - **生产应用**（Agent Serving / Use）：快速启动、工具/代码/浏览器执行、
//!      安全边界、休眠唤醒恢复。
//!
//! # 模块结构
//!
//! - [`error`]：统一错误类型（环境不具备/隔离破坏/执行失败/超限/非法配置/网络拒绝）
//! - [`config`]：SandboxConfig、ResourceLimits、NetworkPolicy、FilesystemPolicy、IsolationLevel
//! - [`state`]：生命周期状态机（SandboxState + LifecycleAction）
//! - [`identity`]：沙箱临时身份、Agent 长期身份、短时执行令牌
//! - [`docker`]：容器共享内核后端
//! - [`firecracker`]：microVM 强隔离后端
//! - `runtime`（v2.7.7）：进程级隔离运行时
//! - `manager`（v2.7.8）：生命周期管理与弹性供给
//! - `security`（v2.7.9）：网络策略、审计、权限
//!
//! # 安全默认（最小权限）
//!
//! 默认无网络、只读根文件系统、非 root、有界 CPU/内存/磁盘/进程数/超时，
//! 长期密钥不进入沙箱。放开能力必须显式声明。

pub mod config;
pub mod error;
pub mod identity;
pub mod state;

pub mod docker;
pub mod firecracker;

pub mod runtime;

// 兼容旧导出（v2.4.0 名称），逐步迁移到 config:: 命名空间
pub use config::IsolationLevel;
pub use error::SandboxError;

/// 沙箱 ID
pub type SandboxId = String;

/// 沙箱执行结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub cpu_time_ms: u64,
    pub wall_ms: u64,
    pub isolation: IsolationLevel,
    pub state: state::SandboxState,
}

/// 沙箱核心 trait：所有隔离后端（进程/容器/microVM）实现这一接口。
pub trait Sandbox {
    /// 当前实际隔离级别（如实标注，不得静默冒充）
    fn isolation(&self) -> IsolationLevel;

    /// 当前生命周期状态
    fn state(&self) -> state::SandboxState;

    /// 沙箱 ID
    fn id(&self) -> &str;

    /// 创建（分配资源、准备文件系统/网络）
    fn create(&mut self, cfg: &config::SandboxConfig) -> Result<(), SandboxError>;

    /// 启动（拉起运行时）
    fn start(&mut self) -> Result<(), SandboxError>;

    /// 在沙箱内执行（代码/命令）
    fn exec(&mut self, program: &str, args: &[String]) -> Result<SandboxResult, SandboxError>;

    /// 暂停（保存状态、释放 CPU/内存）
    fn pause(&mut self) -> Result<(), SandboxError>;

    /// 恢复
    fn resume(&mut self) -> Result<(), SandboxError>;

    /// 停止并回收
    fn destroy(&mut self) -> Result<(), SandboxError>;
}
