//! 插件总线（Plugin Message Bus, PMB）—— 唯一通信通道
//!
//! # 插件不能直接调用宿主
//!
//! 唯一通道是总线，因此策略执行点是**集中的、可审计的、可拦截的**。
//!
//! # 投递前五道检查（任一不过即丢弃并记违规）
//!
//! 1. 发送方在 RUNNING；
//! 2. 发送方令牌持有消息声明的 `capability`；
//! 3. 消息 `source` 与令牌绑定的 `plugin_id` 一致（防伪造 source）；
//! 4. 目标在 RUNNING 且未被隔离（广播时逐目标检查）；
//! 5. 速率未超配额。
//!
//! # 用规范化 JSON
//!
//! 跨语言、可审计、可复现；用 [`PluginBus::max_message_bytes`] 与速率限制兜住体积。
//!
//! 路由表是**可替换的映射**（[`PluginBus::route_table`]），切换是单点操作，
//! 是热更新原子切换的落点。

use crate::plugin::capability::{Capability, CapabilityToken};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::lifecycle::PluginState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};

/// 消息类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    /// 请求。
    Request,
    /// 响应。
    Response,
    /// 事件。
    Event,
}

/// 优先级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Critical,
    High,
    Normal,
    Low,
}

/// 消息目标。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// 点对插件。
    Plugin(String),
    /// 广播（投递给所有 RUNNING 插件，除发送方）。
    Broadcast,
    /// 宿主。
    Host,
}

/// PMB 消息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmbMessage {
    /// 消息 id。
    pub id: String,
    /// 关联 id（响应对应请求的 id）。
    #[serde(default)]
    pub corr_id: Option<String>,
    /// 发送方插件 id。
    pub source: String,
    /// 目标。
    pub target: Target,
    /// 发送方声明的能力。
    pub capability: String,
    /// 消息类别。
    pub kind: MessageKind,
    /// 主题（发布/订阅）。
    #[serde(default)]
    pub topic: Option<String>,
    /// 负载。
    pub payload: serde_json::Value,
    /// 签发时间（Unix 秒）。
    pub issued_at: u64,
    /// 生存时间（毫秒）。
    pub ttl_ms: u64,
    /// 优先级。
    pub priority: Priority,
}

/// 滑动窗口速率限制。
#[derive(Debug, Clone)]
struct RateLimit {
    window_ms: u64,
    max: usize,
    stamps: VecDeque<u64>,
}

impl RateLimit {
    fn new(window_ms: u64, max: usize) -> Self {
        RateLimit {
            window_ms,
            max,
            stamps: VecDeque::new(),
        }
    }

    /// 检查是否超限（基于消息的 issued_at，毫秒）。
    /// 未超限则记录时间戳。
    fn check(&mut self, now_ms: u64) -> bool {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        while let Some(front) = self.stamps.front() {
            if *front < cutoff {
                self.stamps.pop_front();
            } else {
                break;
            }
        }
        if self.stamps.len() >= self.max {
            return false;
        }
        self.stamps.push_back(now_ms);
        true
    }
}

/// 路由项：插件 id → 投递队列 + 状态 + 令牌 + 速率 + 违规计数。
struct RouteEntry {
    tx: Sender<PmbMessage>,
    state: PluginState,
    token: Option<CapabilityToken>,
    rate: RateLimit,
    violations: u32,
}

/// 投递审计记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub msg_id: String,
    pub source: String,
    /// 目标（序列化）。
    pub target: String,
    pub accepted: bool,
    /// 拒绝原因（被拒时）。
    pub reason: Option<String>,
}

/// 违规累计多少次后隔离。
pub const QUARANTINE_THRESHOLD: u32 = 3;

/// 插件总线。
///
/// 注意：`RouteEntry`/`Sender` 不实现 `Debug`（mpsc 发送端），故手动实现。
pub struct PluginBus {
    routes: BTreeMap<String, RouteEntry>,
    /// 宿主队列。
    host_tx: Option<Sender<PmbMessage>>,
    /// 审计日志。
    audit: Vec<AuditRecord>,
    /// 单消息上限（字节）。
    max_message_bytes: usize,
}

impl std::fmt::Debug for PluginBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginBus")
            .field("routes", &self.routes.len())
            .field("has_host", &self.host_tx.is_some())
            .field("audit", &self.audit.len())
            .field("max_message_bytes", &self.max_message_bytes)
            .finish()
    }
}

impl PluginBus {
    /// 新总线（默认 1 MiB 消息上限）。
    pub fn new() -> Self {
        PluginBus {
            routes: BTreeMap::new(),
            host_tx: None,
            audit: Vec::new(),
            max_message_bytes: 1024 * 1024,
        }
    }

    /// 设置消息上限。
    pub fn set_max_message_bytes(&mut self, bytes: usize) {
        self.max_message_bytes = bytes;
    }

    /// 注册路由，返回该插件的接收端（同步、进程内）。
    pub fn register(&mut self, plugin_id: &str) -> Receiver<PmbMessage> {
        let (tx, rx) = channel();
        self.routes.insert(
            plugin_id.to_string(),
            RouteEntry {
                tx,
                state: PluginState::Discovered,
                token: None,
                // 默认每秒 100 条。
                rate: RateLimit::new(1000, 100),
                violations: 0,
            },
        );
        rx
    }

    /// 注册宿主接收端。
    pub fn register_host(&mut self) -> Receiver<PmbMessage> {
        let (tx, rx) = channel();
        self.host_tx = Some(tx);
        rx
    }

    /// 更新插件状态（生命周期变化时调用）。
    pub fn set_state(&mut self, plugin_id: &str, state: PluginState) -> PluginResult<()> {
        let entry = self
            .routes
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        entry.state = state;
        Ok(())
    }

    /// 绑定能力令牌（加载时）。
    pub fn set_token(&mut self, plugin_id: &str, token: CapabilityToken) -> PluginResult<()> {
        let entry = self
            .routes
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        entry.token = Some(token);
        Ok(())
    }

    /// 插件当前违规次数。
    pub fn violations(&self, plugin_id: &str) -> u32 {
        self.routes
            .get(plugin_id)
            .map(|e| e.violations)
            .unwrap_or(0)
    }

    /// 审计日志（只读）。
    pub fn audit_log(&self) -> &[AuditRecord] {
        &self.audit
    }

    /// 路由表快照（插件 id → 状态），用于展示/核对。
    pub fn route_table(&self) -> BTreeMap<String, PluginState> {
        self.routes
            .iter()
            .map(|(id, e)| (id.clone(), e.state))
            .collect()
    }

    /// 检查消息大小（规范化 JSON 字节）。
    fn check_size(&self, msg: &PmbMessage) -> PluginResult<()> {
        let bytes = serde_json::to_vec(msg)
            .map_err(|e| PluginError::Bus(format!("消息序列化失败: {e}")))?;
        if bytes.len() > self.max_message_bytes {
            return Err(PluginError::Quota(format!(
                "消息 {} 字节超过上限 {}",
                bytes.len(),
                self.max_message_bytes
            )));
        }
        Ok(())
    }

    /// 对发送方做检查并记录一次违规（返回拒绝错误）。
    fn reject(&mut self, msg: &PmbMessage, reason: String) -> PluginError {
        if let Some(entry) = self.routes.get_mut(&msg.source) {
            entry.violations += 1;
        }
        self.audit.push(AuditRecord {
            msg_id: msg.id.clone(),
            source: msg.source.clone(),
            target: serde_json::to_string(&msg.target).unwrap_or_default(),
            accepted: false,
            reason: Some(reason.clone()),
        });
        PluginError::Bus(reason)
    }

    /// 校验发送方（五道检查的前 3 道 + 速率），返回发送方能力所需。
    fn validate_sender(&mut self, msg: &PmbMessage) -> PluginResult<()> {
        // 大小。
        self.check_size(msg)?;

        let entry = self
            .routes
            .get(&msg.source)
            .ok_or_else(|| PluginError::Bus(format!("发送方 {} 未注册", msg.source)))?;

        // 1. 发送方 RUNNING。
        if entry.state != PluginState::Running {
            return Err(self.reject(
                msg,
                format!("发送方 {} 不在 RUNNING（{:?}）", msg.source, entry.state),
            ));
        }

        // 2/3. 令牌：存在、source 一致、含 capability。
        let token = entry
            .token
            .clone()
            .ok_or_else(|| self.reject(msg, format!("发送方 {} 无能力令牌", msg.source)))?;
        if token.plugin_id != msg.source {
            return Err(self.reject(
                msg,
                format!(
                    "伪造 source：令牌绑定 {} 但消息声称 {}",
                    token.plugin_id, msg.source
                ),
            ));
        }
        let cap = Capability::parse(&msg.capability)
            .ok_or_else(|| self.reject(msg, format!("未知能力名 {}", msg.capability)))?;
        if !token.has(cap) {
            return Err(self.reject(
                msg,
                format!("能力 {} 未被授予插件 {}", msg.capability, msg.source),
            ));
        }

        // 5. 速率（issued_at 毫秒）。
        let now_ms = msg.issued_at.saturating_mul(1000);
        let ok = self
            .routes
            .get_mut(&msg.source)
            .map(|e| e.rate.check(now_ms))
            .unwrap_or(false);
        if !ok {
            return Err(self.reject(msg, format!("发送方 {} 超出速率配额", msg.source)));
        }
        Ok(())
    }

    /// 投递一条消息（五道检查 + 路由）。
    ///
    /// 广播会投递给所有 RUNNING 插件（除发送方）；点对插件检查目标 RUNNING；
    /// host 投递到宿主队列。
    pub fn dispatch(&mut self, msg: &PmbMessage) -> PluginResult<()> {
        self.validate_sender(msg)?;

        match &msg.target {
            Target::Host => {
                if let Some(tx) = &self.host_tx {
                    tx.send(msg.clone())
                        .map_err(|e| PluginError::Bus(format!("宿主投递失败: {e}")))?;
                } else {
                    return Err(self.reject(msg, "宿主未注册接收端".to_string()));
                }
            }
            Target::Plugin(id) => {
                let target_state = self
                    .routes
                    .get(id)
                    .map(|e| e.state)
                    .ok_or_else(|| PluginError::Bus(format!("目标 {} 未注册", id)))?;
                // 4. 目标 RUNNING 且未隔离。
                if target_state != PluginState::Running {
                    return Err(self.reject(
                        msg,
                        format!("目标 {} 不在 RUNNING（{:?}）", id, target_state),
                    ));
                }
                let tx = self.routes.get(id).unwrap().tx.clone();
                tx.send(msg.clone())
                    .map_err(|e| PluginError::Bus(format!("目标投递失败: {e}")))?;
            }
            Target::Broadcast => {
                // 收集所有 RUNNING 插件（除发送方）。
                let targets: Vec<String> = self
                    .routes
                    .iter()
                    .filter(|(id, e)| **id != msg.source && e.state == PluginState::Running)
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in targets {
                    if let Some(tx) = self.routes.get(&id).map(|e| e.tx.clone()) {
                        // 广播尽力而为：单个目标失败不影响其余。
                        let _ = tx.send(msg.clone());
                    }
                }
            }
        }

        self.audit.push(AuditRecord {
            msg_id: msg.id.clone(),
            source: msg.source.clone(),
            target: serde_json::to_string(&msg.target).unwrap_or_default(),
            accepted: true,
            reason: None,
        });
        Ok(())
    }

    /// 是否应因违规达到阈值而隔离某插件。
    pub fn should_quarantine(&self, plugin_id: &str) -> bool {
        self.violations(plugin_id) >= QUARANTINE_THRESHOLD
    }
}

impl Default for PluginBus {
    fn default() -> Self {
        PluginBus::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::capability::CapabilityToken;

    fn msg(source: &str, target: Target, cap: &str, issued_at: u64) -> PmbMessage {
        PmbMessage {
            id: format!("m-{source}-{issued_at}"),
            corr_id: None,
            source: source.to_string(),
            target,
            capability: cap.to_string(),
            kind: MessageKind::Request,
            topic: None,
            payload: serde_json::json!({}),
            issued_at,
            ttl_ms: 5000,
            priority: Priority::Normal,
        }
    }

    fn token_for(id: &str, caps: &[Capability]) -> CapabilityToken {
        CapabilityToken {
            plugin_id: id.to_string(),
            granted: caps.to_vec(),
            issued_at: 1,
            manifest_digest: "d".to_string(),
        }
    }

    /// 注册插件并推进到 RUNNING、绑定令牌。
    fn bring_up(bus: &mut PluginBus, id: &str, caps: &[Capability]) -> Receiver<PmbMessage> {
        let rx = bus.register(id);
        bus.set_state(id, PluginState::Running).unwrap();
        bus.set_token(id, token_for(id, caps)).unwrap();
        rx
    }

    #[test]
    fn delivers_point_to_point() {
        let mut bus = PluginBus::new();
        let rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        bus.dispatch(&msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        ))
        .unwrap();
        let got = rx_b
            .recv_timeout(std::time::Duration::from_millis(100))
            .unwrap();
        assert_eq!(got.source, "a");
    }

    #[test]
    fn rejects_without_capability() {
        let mut bus = PluginBus::new();
        bring_up(&mut bus, "b", &[Capability::MessageSend]);
        // a 没有 MessageSend 能力。
        bring_up(&mut bus, "a", &[]);
        let r = bus.dispatch(&msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        ));
        assert!(r.is_err());
        assert_eq!(bus.violations("a"), 1);
    }

    #[test]
    fn rejects_forged_source() {
        let mut bus = PluginBus::new();
        // 保留 b 的接收端，避免 channel 关闭。
        let _rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        // 注册 a，令牌绑定 a，但用 c 的名义发送。
        let _rx_a = bring_up(&mut bus, "a", &[Capability::MessageSend]);
        let r = bus.dispatch(&msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        ));
        // 正常 a→b 应成功；伪造场景：令牌 plugin_id 与 source 不一致。
        assert!(r.is_ok());
        // 手动构造一个 source 与令牌不一致的情况：
        bus.register("c");
        bus.set_state("c", PluginState::Running).unwrap();
        bus.set_token("c", token_for("different", &[Capability::MessageSend]))
            .unwrap();
        let r2 = bus.dispatch(&msg(
            "c",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            2,
        ));
        assert!(r2.is_err());
    }

    #[test]
    fn rejects_non_running_sender() {
        let mut bus = PluginBus::new();
        bus.register("a");
        // 不推进到 RUNNING。
        bus.set_token("a", token_for("a", &[Capability::MessageSend]))
            .unwrap();
        let r = bus.dispatch(&msg("a", Target::Host, "plugin:message:send", 1));
        assert!(r.is_err());
    }

    #[test]
    fn rejects_non_running_target() {
        let mut bus = PluginBus::new();
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        bus.register("b"); // b 未 RUNNING。
        let r = bus.dispatch(&msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        ));
        assert!(r.is_err());
    }

    #[test]
    fn broadcasts_to_all_running() {
        let mut bus = PluginBus::new();
        let rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        let rx_c = bring_up(&mut bus, "c", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        bus.dispatch(&msg("a", Target::Broadcast, "plugin:message:send", 1))
            .unwrap();
        assert!(rx_b
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_ok());
        assert!(rx_c
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_ok());
    }

    #[test]
    fn quarantine_after_three_violations() {
        let mut bus = PluginBus::new();
        bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[]); // a 无能力
        for t in 0..3 {
            let _ = bus.dispatch(&msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1 + t,
            ));
        }
        assert!(bus.should_quarantine("a"));
    }

    #[test]
    fn rate_limit_enforced() {
        let mut bus = PluginBus::new();
        // 用极小速率：窗口 1000ms 内 2 条。
        let rx = bus.register("b");
        bus.set_state("b", PluginState::Running).unwrap();
        bus.set_token("b", token_for("b", &[Capability::MessageSend]))
            .unwrap();
        let rx_a = bus.register("a");
        bus.set_state("a", PluginState::Running).unwrap();
        bus.set_token("a", token_for("a", &[Capability::MessageSend]))
            .unwrap();
        // 直接改 a 的速率为 2 条/窗口。
        bus.routes.get_mut("a").unwrap().rate = RateLimit::new(1000, 2);
        let _ = bus.dispatch(&msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        ));
        let _ = bus.dispatch(&msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        ));
        // 同一秒第 3 条应被限流。
        let r3 = bus.dispatch(&msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        ));
        assert!(r3.is_err());
        let _ = rx;
        let _ = rx_a;
    }
}
