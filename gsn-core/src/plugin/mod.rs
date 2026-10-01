//! 一切插件化架构 —— v3.0.0
//!
//! # 总原则
//!
//! 1. **一切皆插件，但内核不是插件**。内核只做注册、路由、仲裁、生命周期；
//! 2. **插件不能直接调用宿主**，唯一通道是 [`bus::PluginBus`]（PMB）；
//! 3. **权限是令牌**（[`capability::CapabilityToken`]），不是约定；
//! 4. **隔离级别随信任级别递减**；
//! 5. **无法强制的隔离拒绝加载**，不是静默放行；
//! 6. **清单是签名载荷**（[`manifest::PluginManifest`]）。
//!
//! # 模块
//!
//! - [`tier`]：五级分类（信任级别），仅由 name 前缀决定；
//! - [`capability`]：能力模型与能力矩阵；
//! - [`error`]：类型化错误；
//! - [`lifecycle`]：生命周期状态机；
//! - [`manifest`]：清单解析、签名、摘要；
//! - [`blacklist`]：黑名单、取证、申诉、解封；
//! - [`bus`]：插件总线（PMB）；
//! - [`registry`]：注册中心；
//! - [`arbiter`]：权限仲裁、令牌签发；
//! - [`runtime`]：隔离运行时（[`runtime::PluginRuntime`]）；
//! - [`system`]：系统插件（T0）；
//! - [`official`]：官方插件（T1）；
//! - [`host`]：宿主装配器（[`host::PluginHost`]）。

pub mod arbiter;
pub mod blacklist;
pub mod bus;
pub mod capability;
pub mod error;
pub mod lifecycle;
pub mod manifest;
pub mod registry;
pub mod tier;

pub mod host;
pub mod official;
pub mod runtime;
pub mod system;

// 常用类型 re-export。
pub use arbiter::Arbiter;
pub use blacklist::{Blacklist, BlacklistEntry, BlacklistReason};
pub use bus::{PluginBus, PmbMessage, Target};
pub use capability::{Capability, CapabilityToken};
pub use error::{PluginError, PluginResult};
pub use host::PluginHost;
pub use lifecycle::{PluginLifecycle, PluginState};
pub use manifest::PluginManifest;
pub use registry::PluginRegistry;
pub use runtime::{PluginInstance, PluginRuntime, RuntimeCapabilities};
