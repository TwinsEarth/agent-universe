//! 插件生命周期状态机 —— v3.0.0
//!
//! # 状态转移是唯一入口
//!
//! 任何状态变更都必须经由 [`PluginLifecycle::transition`]。状态字段私有，
//! 全文件**只有一个赋值点**（`transition` 内部）；构造外无法直接改写状态。
//!
//! # 终态锁定
//!
//! [`PluginState::Refused`]、[`PluginState::Quarantined`]、[`PluginState::Archived`]
//! 是终态，**没有任何合法转出边**：一旦进入即锁定，无法再被改变。
//!
//! # 状态图
//!
//! ```text
//! DISCOVERED → VERIFIED → LOADED → RUNNING → STOPPING → STOPPED → ARCHIVED
//!     │            │          │         │          ▲
//!     ▼            ▼          ▼         ▼          │
//!  REFUSED      REFUSED    REFUSED  UNHEALTHY ─────┘ (恢复)
//!                              │         │
//!                              ▼         ▼
//!                         QUARANTINED  (违规 3 次 / 黑名单)
//! ```

use crate::plugin::error::{PluginError, PluginResult};
use serde::{Deserialize, Serialize};

/// 插件生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// 已发现：清单已解析、待校验。
    Discovered,
    /// 已校验：签名/摘要/依赖/黑名单四方校验通过。
    Verified,
    /// 已加载：模块已载入、待初始化。
    Loaded,
    /// 运行中：可收发消息。
    Running,
    /// 停止中：优雅退出、等待在途请求。
    Stopping,
    /// 已停止：不再处理消息，可归档或重启。
    Stopped,
    /// 已归档：终态。
    Archived,
    /// 已拒绝：校验失败，终态。
    Refused,
    /// 不健康：健康检查失败，待恢复或隔离。
    Unhealthy,
    /// 已隔离：违规累计 3 次 / 黑名单命中，终态。
    Quarantined,
}

impl PluginState {
    /// 稳定标识（用于审计/错误信息）。
    pub fn as_str(self) -> &'static str {
        match self {
            PluginState::Discovered => "discovered",
            PluginState::Verified => "verified",
            PluginState::Loaded => "loaded",
            PluginState::Running => "running",
            PluginState::Stopping => "stopping",
            PluginState::Stopped => "stopped",
            PluginState::Archived => "archived",
            PluginState::Refused => "refused",
            PluginState::Unhealthy => "unhealthy",
            PluginState::Quarantined => "quarantined",
        }
    }

    /// 是否为终态（无合法转出边）。
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            PluginState::Refused | PluginState::Quarantined | PluginState::Archived
        )
    }

    /// 是否可在总线上收发消息（投递检查用）。
    pub fn is_active(self) -> bool {
        matches!(self, PluginState::Running)
    }
}

/// 插件生命周期句柄：持有状态，唯一经 `transition` 改变。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLifecycle {
    plugin_id: String,
    state: PluginState,
}

impl PluginLifecycle {
    /// 新建：以 [`PluginState::Discovered`] 为起点。
    pub fn new(plugin_id: &str) -> Self {
        PluginLifecycle {
            plugin_id: plugin_id.to_string(),
            state: PluginState::Discovered,
        }
    }

    /// 当前状态。
    pub fn state(&self) -> PluginState {
        self.state
    }

    /// 所属插件 id。
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// 判断是否允许 `from → to` 转移。
    fn allowed(from: PluginState, to: PluginState) -> bool {
        use PluginState::*;
        // 同状态不算转移。
        if from == to {
            return false;
        }
        // 终态无任何转出边。
        if from.is_terminal() {
            return false;
        }
        match (from, to) {
            // 校验阶段：可前进或拒绝。
            (Discovered, Verified) | (Discovered, Refused) => true,
            (Verified, Loaded) | (Verified, Refused) | (Verified, Quarantined) => true,
            (Loaded, Running) | (Loaded, Refused) => true,
            // 运行中：可优雅停止、转不健康、或被隔离。
            (Running, Stopping) | (Running, Unhealthy) | (Running, Quarantined) => true,
            // 停止中 → 已停止。
            (Stopping, Stopped) => true,
            // 不健康：恢复为运行、或被隔离。
            (Unhealthy, Running) | (Unhealthy, Quarantined) => true,
            // 已停止：归档、或重新启动（热插拔重新加载）。
            (Stopped, Archived) | (Stopped, Running) => true,
            _ => false,
        }
    }

    /// 执行状态转移（唯一赋值点）。
    ///
    /// 非法转移（含从终态转出）返回 [`PluginError::InvalidTransition`]，状态不变。
    pub fn transition(&mut self, to: PluginState) -> PluginResult<()> {
        if Self::allowed(self.state, to) {
            self.state = to;
            Ok(())
        } else {
            Err(PluginError::InvalidTransition {
                from: self.state.as_str().to_string(),
                to: to.as_str().to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PluginState::*;

    #[test]
    fn happy_path_to_running() {
        let mut lc = PluginLifecycle::new("p");
        assert_eq!(lc.state(), Discovered);
        lc.transition(Verified).unwrap();
        lc.transition(Loaded).unwrap();
        lc.transition(Running).unwrap();
        assert_eq!(lc.state(), Running);
    }

    #[test]
    fn full_path_to_archived() {
        let mut lc = PluginLifecycle::new("p");
        for s in [Verified, Loaded, Running, Stopping, Stopped, Archived] {
            lc.transition(s).unwrap();
        }
        assert_eq!(lc.state(), Archived);
    }

    #[test]
    fn terminal_states_have_no_outgoing_edges() {
        let all = [
            Discovered,
            Verified,
            Loaded,
            Running,
            Stopping,
            Stopped,
            Archived,
            Refused,
            Unhealthy,
            Quarantined,
        ];
        // 终态对任何目标都不允许转出。
        for terminal in [Archived, Refused, Quarantined] {
            for target in all {
                assert!(
                    !PluginLifecycle::allowed(terminal, target),
                    "{terminal:?} 不应有转出边到 {target:?}"
                );
            }
        }
    }

    #[test]
    fn cannot_skip_verification() {
        let mut lc = PluginLifecycle::new("p");
        // Discovered 不能直接跳到 Loaded/Running（必须先 Verified）。
        assert!(lc.transition(Loaded).is_err());
        assert!(lc.transition(Running).is_err());
    }

    #[test]
    fn unhealthy_can_recover() {
        let mut lc = PluginLifecycle::new("p");
        lc.transition(Verified).unwrap();
        lc.transition(Loaded).unwrap();
        lc.transition(Running).unwrap();
        lc.transition(Unhealthy).unwrap();
        lc.transition(Running).unwrap();
        assert_eq!(lc.state(), Running);
    }

    #[test]
    fn stopped_can_restart_or_archive() {
        let mut lc = PluginLifecycle::new("p");
        for s in [Verified, Loaded, Running, Stopping, Stopped] {
            lc.transition(s).unwrap();
        }
        // 热插拔重新加载。
        lc.transition(Running).unwrap();
        assert_eq!(lc.state(), Running);
    }

    #[test]
    fn same_state_is_not_a_transition() {
        let mut lc = PluginLifecycle::new("p");
        assert!(lc.transition(Discovered).is_err());
    }
}
