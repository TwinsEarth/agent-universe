//! Agent Sandbox 生命周期状态机（v2.7.6）
//!
//! 状态（沙箱自身运行状态）：
//!
//! ```text
//!   Pending ──create──▶ Creating ──start──▶ Starting ──▶ Running
//!                            │                   │           │
//!                            │                   │      pause│
//!                            │                   │           ▼
//!                            │                   │       Pausing ──▶ Paused
//!                            │                   │           │        │
//!                            │                   │           │     resume
//!                            │                   │           ▼        ▼
//!                            │                   │      Resuming ◀────┘
//!                            │                   │
//!                         stop│              stop │
//!                            ▼                   ▼
//!                       Stopping ◀────────────────
//!                            │
//!                            ▼
//!                         Stopped
//!
//!   任意运行态 ──错误/超限──▶ Failed
//! ```
//!
//! 注意：这里只管理沙箱自身的运行状态（内存/文件/进程）。
//! 业务状态（任务进度、DB 写入、外部系统）由上层 Agent/编排记录，
//! 沙箱不负责保存业务状态——这是 v2.7.6 的关键边界。

use serde::{Deserialize, Serialize};

/// 沙箱生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxState {
    /// 已登记配置，尚未分配资源
    Pending,
    /// 正在创建（准备运行时/文件系统/网络）
    Creating,
    /// 正在启动（拉起进程/VM）
    Starting,
    /// 运行中
    Running,
    /// 正在暂停
    Pausing,
    /// 已暂停（资源已释放，状态已保存，可唤醒）
    Paused,
    /// 正在恢复
    Resuming,
    /// 正在停止
    Stopping,
    /// 已停止（保留或待回收）
    Stopped,
    /// 失败/超限（终态，需回收）
    Failed,
}

impl SandboxState {
    pub fn label(&self) -> &'static str {
        match self {
            SandboxState::Pending => "pending",
            SandboxState::Creating => "creating",
            SandboxState::Starting => "starting",
            SandboxState::Running => "running",
            SandboxState::Pausing => "pausing",
            SandboxState::Paused => "paused",
            SandboxState::Resuming => "resuming",
            SandboxState::Stopping => "stopping",
            SandboxState::Stopped => "stopped",
            SandboxState::Failed => "failed",
        }
    }

    /// 从 label 反解析（持久化/恢复用），未知回 None，调用方决定降级
    pub fn from_label(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => SandboxState::Pending,
            "creating" => SandboxState::Creating,
            "starting" => SandboxState::Starting,
            "running" => SandboxState::Running,
            "pausing" => SandboxState::Pausing,
            "paused" => SandboxState::Paused,
            "resuming" => SandboxState::Resuming,
            "stopping" => SandboxState::Stopping,
            "stopped" => SandboxState::Stopped,
            "failed" => SandboxState::Failed,
            _ => return None,
        })
    }

    /// 是否终态（不再有合法迁移）
    pub fn is_terminal(&self) -> bool {
        matches!(self, SandboxState::Stopped | SandboxState::Failed)
    }

    /// 是否持有运行资源（CPU/内存/进程）
    pub fn holds_resources(&self) -> bool {
        matches!(
            self,
            SandboxState::Starting
                | SandboxState::Running
                | SandboxState::Pausing
                | SandboxState::Resuming
                | SandboxState::Stopping
        )
    }

    /// 状态是否允许执行动作（合法迁移）。
    /// 返回 Ok(迁移后的状态) 或 Err(原因)。
    pub fn transition(&self, action: LifecycleAction) -> Result<SandboxState, String> {
        use LifecycleAction::*;
        use SandboxState::*;
        let next = match (self, action) {
            (Pending, Create) => Creating,
            (Creating, Start) => Starting,
            (Starting, MarkReady) => Running,
            (Running, Pause) => Pausing,
            (Pausing, MarkPaused) => Paused,
            (Paused, Resume) => Resuming,
            (Resuming, MarkReady) => Running,
            (Running, Stop) => Stopping,
            (Starting, Stop) => Stopping,
            (Pausing, Stop) => Stopping,
            (Paused, Stop) => Stopping,
            (Resuming, Stop) => Stopping,
            (Stopping, MarkStopped) => Stopped,
            // 任意非终态可失败
            (s, Fail) if !s.is_terminal() => Failed,
            (from, action) => {
                return Err(format!(
                    "非法迁移: {} 不允许 {}",
                    from.label(),
                    action.label()
                ))
            }
        };
        Ok(next)
    }
}

/// 生命周期动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    Create,
    Start,
    Pause,
    Resume,
    Stop,
    MarkReady,
    MarkPaused,
    MarkStopped,
    Fail,
}

impl LifecycleAction {
    pub fn label(&self) -> &'static str {
        match self {
            LifecycleAction::Create => "create",
            LifecycleAction::Start => "start",
            LifecycleAction::Pause => "pause",
            LifecycleAction::Resume => "resume",
            LifecycleAction::Stop => "stop",
            LifecycleAction::MarkReady => "mark-ready",
            LifecycleAction::MarkPaused => "mark-paused",
            LifecycleAction::MarkStopped => "mark-stopped",
            LifecycleAction::Fail => "fail",
        }
    }
}

/// 生命周期事件（审计/记录用）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub sandbox_id: String,
    pub from: String,
    pub action: String,
    pub to: String,
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions() {
        let mut s = SandboxState::Pending;
        s = s.transition(LifecycleAction::Create).unwrap();
        s = s.transition(LifecycleAction::Start).unwrap();
        s = s.transition(LifecycleAction::MarkReady).unwrap();
        assert_eq!(s, SandboxState::Running);
        s = s.transition(LifecycleAction::Pause).unwrap();
        s = s.transition(LifecycleAction::MarkPaused).unwrap();
        assert_eq!(s, SandboxState::Paused);
        s = s.transition(LifecycleAction::Resume).unwrap();
        s = s.transition(LifecycleAction::MarkReady).unwrap();
        assert_eq!(s, SandboxState::Running);
        s = s.transition(LifecycleAction::Stop).unwrap();
        s = s.transition(LifecycleAction::MarkStopped).unwrap();
        assert_eq!(s, SandboxState::Stopped);
    }

    #[test]
    fn invalid_transition_rejected() {
        // Stopped 是终态，不能再 pause
        let r = SandboxState::Stopped.transition(LifecycleAction::Pause);
        assert!(r.is_err());
        // Pending 不能直接 pause
        assert!(SandboxState::Pending
            .transition(LifecycleAction::Pause)
            .is_err());
    }

    #[test]
    fn fail_from_any_nonterminal() {
        assert_eq!(
            SandboxState::Running
                .transition(LifecycleAction::Fail)
                .unwrap(),
            SandboxState::Failed
        );
        assert_eq!(
            SandboxState::Paused
                .transition(LifecycleAction::Fail)
                .unwrap(),
            SandboxState::Failed
        );
    }

    #[test]
    fn label_roundtrip() {
        for s in [
            SandboxState::Pending,
            SandboxState::Creating,
            SandboxState::Starting,
            SandboxState::Running,
            SandboxState::Pausing,
            SandboxState::Paused,
            SandboxState::Resuming,
            SandboxState::Stopping,
            SandboxState::Stopped,
            SandboxState::Failed,
        ] {
            assert_eq!(SandboxState::from_label(s.label()), Some(s));
        }
        assert_eq!(SandboxState::from_label("garbage"), None);
    }
}
