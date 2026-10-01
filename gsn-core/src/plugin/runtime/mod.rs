//! 隔离运行时（Plugin Runtime）—— 后端能力声明与「无法强制即拒绝」
//!
//! # 每个后端必须声明它实际能强制哪些边界
//!
//! 用 [`RuntimeCapabilities`] 如实描述。加载一个插件时，宿主按其级别要求一组边界，
//! 逐条比对后端声明：
//!
//! - 后端能强制 → 接受；
//! - 后端不能强制，但清单 `waivers` 显式声明了该边界且插件**可信**（T1/T2）→
//!   带豁免接受（豁免被签名记录）；
//! - 否则**具名拒绝**（说出是哪条边界），绝不静默地不受限地跑。
//!
//! # 后端
//!
//! - [`native`]：T0 进程内（[`native::NativeRuntime`]），不隔离，仅系统插件；
//! - [`process`]：独立进程（[`process::ProcessRuntime`]），复用现有 sandbox；
//! - [`wasm`]：WASM（[`wasm::WasmRuntime`]）—— 本会话 wasmtime 未在依赖树，
//!   类型化拒绝（不假装能跑）。

pub mod native;
pub mod process;
pub mod wasm;

use crate::plugin::error::PluginResult;
use crate::plugin::manifest::PluginManifest;
use crate::plugin::tier::Tier;

/// 后端实际能强制的边界（诚实声明）。
///
/// 字段全部为「是否真的有强制原语」；默认全 false，由各后端按平台开启。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    /// 独立工作目录（文件系统隔离，不挂载宿主路径）。
    pub fs_isolation: bool,
    /// 阻止子进程读写宿主文件系统（子进程层面，而非仅 Rust helper）。
    pub fs_deny_host: bool,
    /// 内存上限。
    pub memory_limit: bool,
    /// CPU 时间上限。
    pub cpu_limit: bool,
    /// 磁盘配额。
    pub disk_quota: bool,
    /// 出站网络控制（防止子进程绕过总线直接联网）。
    pub network_egress: bool,
    /// 整棵进程树 kill。
    pub process_tree_kill: bool,
    /// 进程数上限。
    pub process_count_limit: bool,
    /// 单次输出上限。
    pub output_cap: bool,
}

impl Default for RuntimeCapabilities {
    /// 默认全 false（最诚实的起点：未声明即不支持）。
    fn default() -> Self {
        RuntimeCapabilities {
            fs_isolation: false,
            fs_deny_host: false,
            memory_limit: false,
            cpu_limit: false,
            disk_quota: false,
            network_egress: false,
            process_tree_kill: false,
            process_count_limit: false,
            output_cap: false,
        }
    }
}

/// 一条边界：用于按级别要求与豁免检查。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    FsIsolation,
    FsDenyHost,
    MemoryLimit,
    CpuLimit,
    DiskQuota,
    NetworkEgress,
    ProcessTreeKill,
    ProcessCountLimit,
    OutputCap,
}

impl Bound {
    /// 边界名（对应 waivers 的 key）。
    pub fn as_key(self) -> &'static str {
        match self {
            Bound::FsIsolation => "fs_isolation",
            Bound::FsDenyHost => "fs_deny_host",
            Bound::MemoryLimit => "memory_limit",
            Bound::CpuLimit => "cpu_limit",
            Bound::DiskQuota => "disk_quota",
            Bound::NetworkEgress => "network_egress",
            Bound::ProcessTreeKill => "process_tree_kill",
            Bound::ProcessCountLimit => "process_count_limit",
            Bound::OutputCap => "output_cap",
        }
    }

    /// 全部边界。
    pub const ALL: &[Bound] = &[
        Bound::FsIsolation,
        Bound::FsDenyHost,
        Bound::MemoryLimit,
        Bound::CpuLimit,
        Bound::DiskQuota,
        Bound::NetworkEgress,
        Bound::ProcessTreeKill,
        Bound::ProcessCountLimit,
        Bound::OutputCap,
    ];
}

/// 后端在该边界上是否能强制。
pub trait HasBound {
    fn provides(&self, bound: Bound) -> bool;
}

impl HasBound for RuntimeCapabilities {
    fn provides(&self, bound: Bound) -> bool {
        match bound {
            Bound::FsIsolation => self.fs_isolation,
            Bound::FsDenyHost => self.fs_deny_host,
            Bound::MemoryLimit => self.memory_limit,
            Bound::CpuLimit => self.cpu_limit,
            Bound::DiskQuota => self.disk_quota,
            Bound::NetworkEgress => self.network_egress,
            Bound::ProcessTreeKill => self.process_tree_kill,
            Bound::ProcessCountLimit => self.process_count_limit,
            Bound::OutputCap => self.output_cap,
        }
    }
}

/// 按级别要求一组「必需边界」。
///
/// - T1/T2 可信：基础隔离必须满足；`fs_deny_host`/`network_egress`/`disk_quota`
///   这类无法在所有平台强制的，允许以显式 waiver 接受（waiver 被签名）；
/// - T3 不可信：所有关键边界都必须由后端强制，**无 waiver 例外**。
pub fn required_bounds(tier: Tier) -> Vec<Bound> {
    match tier {
        Tier::Official | Tier::Certified => {
            vec![Bound::FsIsolation, Bound::CpuLimit, Bound::OutputCap]
        }
        Tier::ThirdParty => vec![
            Bound::FsIsolation,
            Bound::FsDenyHost,
            Bound::MemoryLimit,
            Bound::CpuLimit,
            Bound::DiskQuota,
            Bound::NetworkEgress,
            Bound::ProcessTreeKill,
            Bound::ProcessCountLimit,
            Bound::OutputCap,
        ],
        // T0 进程内不要求隔离。
        Tier::System | Tier::Blacklist => vec![],
    }
}

/// 某条边界是否允许以 waiver 形式豁免（按级别）。
///
/// 关键安全属性：T3 任何关键边界都不允许豁免。
pub fn waiver_allowed(tier: Tier, bound: Bound) -> bool {
    match tier {
        // T1/T2 仅允许豁免这几个「所有平台都无原语」的边界。
        Tier::Official | Tier::Certified => matches!(
            bound,
            Bound::FsDenyHost | Bound::NetworkEgress | Bound::DiskQuota
        ),
        Tier::ThirdParty => false,
        Tier::System | Tier::Blacklist => false,
    }
}

/// 隔离运行时后端。
pub trait PluginRuntime {
    /// 后端名称。
    fn name(&self) -> &str;

    /// 后端能力声明。
    fn declares(&self) -> RuntimeCapabilities;

    /// 判定该后端能否承载给定清单（按级别边界 + waiver）。
    ///
    /// 无法满足且无豁免时，返回具名错误（[`crate::plugin::error::PluginError::Runtime`]）。
    fn supports(&self, manifest: &PluginManifest) -> PluginResult<()> {
        let tier = Tier::from_name(&manifest.plugin.name);
        let caps = self.declares();
        for bound in required_bounds(tier) {
            if caps.provides(bound) {
                continue;
            }
            // 后端不支持：看是否有显式 waiver，且该级别允许豁免。
            let has_waiver = manifest.waivers.contains_key(bound.as_key());
            if has_waiver && waiver_allowed(tier, bound) {
                continue;
            }
            return Err(crate::plugin::error::PluginError::Runtime(format!(
                "后端 {} 无法强制边界 {}（插件 {}，级别 {}）；需显式 waiver 或更换后端",
                self.name(),
                bound.as_key(),
                manifest.plugin.name,
                tier.as_str()
            )));
        }
        Ok(())
    }

    /// 加载并返回一个运行实例。
    fn spawn(&mut self, manifest: &PluginManifest) -> PluginResult<Box<dyn PluginInstance>>;
}

/// 运行中的插件实例。
///
/// 必须是 `Send`：实例存放在宿主中，可能被 `spawn_blocking` 工作线程访问。
pub trait PluginInstance: Send {
    /// 调用一个方法（method + JSON 负载字节），返回 JSON 结果字节。
    fn call(&mut self, method: &str, payload: &[u8]) -> PluginResult<Vec<u8>>;

    /// 停止并释放。
    fn stop(&mut self) -> PluginResult<()>;

    /// 是否仍存活。
    fn is_alive(&mut self) -> bool;

    /// 取走插件自上次调用以来**主动产生**的消息（outbox，B2）。
    ///
    /// 默认返回空：不支持主动通信的后端（native/wasm）无需实现。宿主在每次
    /// `call` 后取出并代表插件投递 PMB（宿主仍是唯一投递点，保持七道检查）。
    fn drain_outbox(&mut self) -> PluginResult<Vec<OutboxMessage>> {
        let _ = self;
        Ok(Vec::new())
    }
}

/// 插件主动产生、经 outbox 上送的消息（B2 进程插件主动通信）。
#[derive(Debug, Clone)]
pub struct OutboxMessage {
    /// `"send"`（点对点）或 `"publish"`（广播）。
    pub kind: String,
    /// `send` 时的目标插件 id。
    pub target: Option<String>,
    /// 声明的能力（投递 PMB 时作为 capability）。
    pub capability: String,
    /// 业务负载。
    pub payload: serde_json::Value,
}

/// 插件实例的元信息（注册到总线时使用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstanceInfo {
    /// 插件 id（= 清单 name）。
    pub id: String,
    /// 版本。
    pub version: String,
    /// 级别。
    pub tier: Tier,
}

impl PluginInstanceInfo {
    pub fn from_manifest(manifest: &PluginManifest) -> Self {
        PluginInstanceInfo {
            id: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            tier: Tier::from_name(&manifest.plugin.name),
        }
    }
}
