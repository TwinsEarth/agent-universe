//! 插件体系统一错误类型 —— v3.0.0
//!
//! 所有失败都是**类型化拒绝**，不是静默降级：
//! 清单错、签名错、非法状态转移、未授权、黑名单命中、运行时无法强制、配额超限……

use thiserror::Error;

/// 插件体系错误。
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// 清单解析/校验失败（含未知字段、必填项缺失、能力名非法）。
    #[error("清单解析失败: {0}")]
    Manifest(String),

    /// 签名 / 摘要 / 副签校验失败。
    #[error("签名校验失败: {0}")]
    Signature(String),

    /// 非法状态转移。
    #[error("非法状态转移: {from} → {to}")]
    InvalidTransition { from: String, to: String },

    /// 能力未被授予（总线入口拒绝）。
    #[error("能力 {capability} 未被授予插件 {plugin_id}")]
    Unauthorized {
        plugin_id: String,
        capability: String,
    },

    /// 黑名单命中（禁止加载）。
    #[error("黑名单命中: {0}")]
    Blacklisted(String),

    /// 隔离运行时无法强制所请求的边界（具名拒绝）。
    #[error("运行时无法强制: {0}")]
    Runtime(String),

    /// 插件未找到 / 未注册。
    #[error("插件未找到: {0}")]
    NotFound(String),

    /// 超出资源 / 速率配额。
    #[error("超出配额: {0}")]
    Quota(String),

    /// 总线消息被拒绝（投递检查失败 / 目标不可达）。
    #[error("总线消息被拒绝: {0}")]
    Bus(String),

    /// 信任 / 发布者相关（未信任的第三方发布者）。
    #[error("信任错误: {0}")]
    Trust(String),

    /// 热更新失败（健康检查失败 → 自动回滚；无法回滚时上抛）。
    #[error("热更新失败: {0}")]
    HotSwap(String),
}

impl PluginError {
    /// 建议的 HTTP 状态码（供 REST 层映射）。
    pub fn http_status(&self) -> u16 {
        match self {
            PluginError::NotFound(_) => 404,
            PluginError::Unauthorized { .. }
            | PluginError::Signature(_)
            | PluginError::Trust(_) => 401,
            PluginError::Blacklisted(_) => 403,
            PluginError::Manifest(_)
            | PluginError::InvalidTransition { .. }
            | PluginError::Runtime(_) => 422,
            PluginError::Quota(_) | PluginError::Bus(_) | PluginError::HotSwap(_) => 400,
        }
    }
}

/// 插件体系专用 Result。
pub type PluginResult<T> = Result<T, PluginError>;
