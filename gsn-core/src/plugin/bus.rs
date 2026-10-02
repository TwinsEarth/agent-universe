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
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
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
    /// 随机 nonce（唯一，防重放；签名覆盖）。
    pub nonce: String,
    /// 发送方对本条消息（除本字段外）规范化字节的 HMAC-SHA256 签名（hex）。
    /// 见 [`PluginBus::sign_message`]。
    pub signature: String,
}

/// 会话密钥长度（HMAC-SHA256，32 字节）。
pub const SESSION_KEY_LEN: usize = 32;

/// 每个插件记忆的已用 nonce 上限（防重放窗口，超出按**签发时间**丢弃最旧，v3.5.2 AU-25）。
pub const MAX_SEEN_NONCES: usize = 1024;

/// 默认服务端时钟未注入（0）时，TTL 新鲜度检查暂不启用（确定性回放模式，向后兼容）。
/// 生产宿主须在启动后调用 [`PluginBus::set_server_clock_ms`] 注入真实时钟，
/// 一旦注入即对所有消息强制 `issued_at..issued_at+ttl_ms` 新鲜度断言。
pub const CLOCK_NOT_SET: u64 = 0;

/// 计算消息的签名载荷（除 `signature` 外的全部字段，顺序固定 → 跨语言可复现）。
fn signing_bytes(msg: &PmbMessage) -> PluginResult<Vec<u8>> {
    let view = serde_json::json!({
        "id": msg.id,
        "corr_id": msg.corr_id,
        "source": msg.source,
        "target": msg.target,
        "capability": msg.capability,
        "kind": msg.kind,
        "topic": msg.topic,
        "payload": msg.payload,
        "issued_at": msg.issued_at,
        "ttl_ms": msg.ttl_ms,
        "priority": msg.priority,
        "nonce": msg.nonce,
    });
    serde_json::to_vec(&view).map_err(|e| PluginError::Bus(format!("签名载荷序列化失败: {e}")))
}

/// 用会话密钥对消息（除 `signature` 外）计算 HMAC-SHA256，返回 hex 签名。
///
/// 签名覆盖随机 nonce 与签发时间，因此可同时校验完整性、来源与新鲜度。
pub fn compute_signature(msg: &PmbMessage, key: &[u8]) -> PluginResult<String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let bytes = signing_bytes(msg)?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key)
        .map_err(|e| PluginError::Bus(format!("签名失败: {e}")))?;
    mac.update(&bytes);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// 常量时间比较两个小写 hex 字符串是否相等（防时序侧信道）。
/// 长度不同或含非法 hex 字符时返回 false。
fn constant_time_eq_hex(a: &str, b: &str) -> bool {
    let da = match hex::decode(a) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let db = match hex::decode(b) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if da.len() != db.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in da.iter().zip(db.iter()) {
        diff |= x ^ y;
    }
    diff == 0
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

/// 路由项：插件 id → 投递队列 + 状态 + 令牌 + 速率 + 会话密钥 + 违规计数。
struct RouteEntry {
    tx: Sender<PmbMessage>,
    state: PluginState,
    token: Option<CapabilityToken>,
    rate: RateLimit,
    /// 会话密钥（注册时随机生成，HMAC 签名用）。
    key: [u8; SESSION_KEY_LEN],
    /// 已见 nonce（防重放，按内容 membership）。v3.5.2（AU-25）：从 BTreeSet<String>
    /// 改为 HashSet 配合 `nonce_recency`，使超窗淘汰按签发时间而非字典序。
    seen_nonces: HashSet<String>,
    /// nonce 按 (签发毫秒, nonce) 时间序排列，超窗时淘汰最旧（而非字典序最小）。
    nonce_recency: BTreeSet<(u64, String)>,
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
    /// 服务端当前时钟（毫秒，Unix epoch）。v3.5.2（AU-21）：用于 TTL 新鲜度断言。
    /// 0 = 未注入（确定性回放模式，新鲜度检查暂关，向后兼容）；生产宿主须调用
    /// [`PluginBus::set_server_clock_ms`] 注入真实时钟。
    server_now_ms: u64,
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
            server_now_ms: 0,
        }
    }

    /// 注入服务端当前时钟（毫秒，Unix epoch），启用 TTL 新鲜度断言（v3.5.2，AU-21）。
    ///
    /// 生产宿主在启动后用真实墙钟调用一次；此后每条消息必须落在
    /// `[issued_at, issued_at + ttl_ms]` 内，否则以 `PMB_MESSAGE_EXPIRED` 拒绝。
    /// 测试用此注入可控时钟以验证过期拒绝。
    pub fn set_server_clock_ms(&mut self, now_ms: u64) {
        self.server_now_ms = now_ms;
    }

    /// 设置消息上限。
    pub fn set_max_message_bytes(&mut self, bytes: usize) {
        self.max_message_bytes = bytes;
    }

    /// 注册路由，返回该插件的接收端（同步、进程内）。
    ///
    /// 注册时为插件随机生成会话密钥（HMAC 签名用）；插件可用
    /// [`PluginBus::sign_message`] 对要发出的消息签名，或用
    /// [`PluginBus::session_key`] 取出密钥自行签名。
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
                key: Self::random_key(),
                seen_nonces: HashSet::new(),
                nonce_recency: BTreeSet::new(),
                violations: 0,
            },
        );
        rx
    }

    /// 生成随机会话密钥。
    fn random_key() -> [u8; SESSION_KEY_LEN] {
        use rand::RngCore;
        let mut k = [0u8; SESSION_KEY_LEN];
        rand::thread_rng().fill_bytes(&mut k);
        k
    }

    /// 取某插件的会话密钥（仅用于该插件自行签名；插件拿不到其它插件的密钥）。
    pub fn session_key(&self, plugin_id: &str) -> Option<[u8; SESSION_KEY_LEN]> {
        self.routes.get(plugin_id).map(|e| e.key)
    }

    /// 用消息 source 对应插件的会话密钥就地签名（填入 nonce 与 signature）。
    ///
    /// 若消息已带 nonce 则沿用、否则生成随机 nonce；source 未注册时返回错误。
    /// 这是插件发出消息前的标准步骤。
    pub fn sign_message(&mut self, msg: &mut PmbMessage) -> PluginResult<()> {
        let key =
            self.routes.get(&msg.source).map(|e| e.key).ok_or_else(|| {
                PluginError::Bus(format!("发送方 {} 未注册，无法签名", msg.source))
            })?;
        if msg.nonce.is_empty() {
            msg.nonce = uuid::Uuid::new_v4().to_string();
        }
        msg.signature = compute_signature(msg, &key)?;
        Ok(())
    }

    /// 注册宿主接收端。
    pub fn register_host(&mut self) -> Receiver<PmbMessage> {
        let (tx, rx) = channel();
        self.host_tx = Some(tx);
        rx
    }

    /// 更新插件状态（生命周期变化时调用）。
    ///
    /// v3.5.2（AU-06）：复用 [`PluginState::is_terminal`]（与
    /// [`crate::plugin::lifecycle::PluginLifecycle`] 同一终态谓词）做**终态锁定后门**——
    /// 一旦插件处于终态（Refused/Quarantined/Archived），任何再调用本方法把它改写到
    /// 其它状态（尤其回 Running）都返回 [`PluginError::InvalidTransition`]，不再静默
    /// 直改。粒度更细的状态图行走（Discovered→Verified→…）仍是宿主
    /// `PluginLifecycle::transition` 的职责；此处作为总线侧的防御性兜底。
    pub fn set_state(&mut self, plugin_id: &str, state: PluginState) -> PluginResult<()> {
        let entry = self
            .routes
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        // 终态锁定：终态插件不可被重新激活/改写状态。
        if entry.state.is_terminal() && state != entry.state {
            return Err(PluginError::InvalidTransition {
                from: entry.state.as_str().to_string(),
                to: state.as_str().to_string(),
            });
        }
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

    /// 移除某插件的整条路由（卸载时调用，v3.5.2，AU-24）。
    ///
    /// 旧实现卸载后仍保留 RouteEntry（含 Sender/状态/令牌），形成总线上的孤儿路由；
    /// 移除后该插件不再占用路由表、也不再可被投递。返回是否原本存在。
    pub fn remove_route(&mut self, plugin_id: &str) -> bool {
        self.routes.remove(plugin_id).is_some()
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
        let session_key = entry.key;

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

        // 6. 签名：用发送方会话密钥重算 HMAC，必须与消息携带的 signature 一致。
        //    任何字段被篡改、伪造 source 或换密钥都会在此失败。
        //    v3.5.2（AU-25）：签名/nonce/新鲜度一律**先于**速率桶校验——否则一个
        //    坏签名或重放攻击者就能白嫖掉合法插件的速率配额。
        let expected = compute_signature(msg, &session_key)?;
        if msg.signature.is_empty() || !constant_time_eq_hex(&expected, &msg.signature) {
            return Err(self.reject(msg, "消息签名无效或缺失（可能被篡改/伪造）".to_string()));
        }

        // 7. nonce 防重放：同一 nonce 只接受一次（签名覆盖 nonce，故攻击者无法
        //    复用签名后只改 nonce）。窗口满时按**签发时间**淘汰最旧（AU-25）。
        if msg.nonce.is_empty() {
            return Err(self.reject(msg, "消息缺少 nonce".to_string()));
        }
        let issued_ms = msg.issued_at.saturating_mul(1000);
        let entry = self.routes.get_mut(&msg.source).ok_or_else(|| {
            PluginError::Bus(format!("发送方 {} 未注册（nonce 检查）", msg.source))
        })?;
        if !entry.seen_nonces.insert(msg.nonce.clone()) {
            return Err(self.reject(msg, "重放消息：nonce 已被使用".to_string()));
        }
        entry.nonce_recency.insert((issued_ms, msg.nonce.clone()));
        if entry.seen_nonces.len() > MAX_SEEN_NONCES {
            // 按时间序淘汰最旧（BTreeSet 按 (issued_ms, nonce) 升序，first=最旧），
            // 而非旧实现的字典序最小——字典序可能误删刚签发的合法 nonce。
            if let Some((old_issued, old_nonce)) = entry.nonce_recency.iter().next().cloned() {
                entry.nonce_recency.remove(&(old_issued, old_nonce.clone()));
                entry.seen_nonces.remove(&old_nonce);
            }
        }

        // 8. TTL 新鲜度（v3.5.2，AU-21）：ttl_ms 已在签名内（不可改），旧实现从不
        //    比对时钟。一旦宿主注入服务端时钟，要求 now ∈ [issued_at, issued_at+ttl_ms]。
        if self.server_now_ms != CLOCK_NOT_SET {
            let valid_until = issued_ms.saturating_add(msg.ttl_ms);
            if self.server_now_ms > valid_until {
                return Err(self.reject(
                    msg,
                    format!(
                        "PMB_MESSAGE_EXPIRED: 消息签发于 {}ms，TTL {}ms，已于 {}ms 过期",
                        issued_ms, msg.ttl_ms, self.server_now_ms
                    ),
                ));
            }
        }

        // 5. 速率（issued_at 毫秒）——v3.5.2（AU-25）移到签名/nonce/新鲜度之后，
        //    坏签名/重放/过期消息不消耗合法插件的速率配额。
        let ok = self
            .routes
            .get_mut(&msg.source)
            .map(|e| e.rate.check(issued_ms))
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
                let entry = self
                    .routes
                    .get(id)
                    .ok_or_else(|| PluginError::Bus(format!("目标 {} 未注册", id)))?;
                // 4. 目标 RUNNING 且未隔离。
                if entry.state != PluginState::Running {
                    return Err(self.reject(
                        msg,
                        format!("目标 {} 不在 RUNNING（{:?}）", id, entry.state),
                    ));
                }
                let tx = entry.tx.clone();
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
            nonce: String::new(),
            signature: String::new(),
        }
    }

    /// 用 source 对应插件的密钥对消息签名（dispatch 前的标准步骤）。
    fn signed(bus: &mut PluginBus, mut m: PmbMessage) -> PmbMessage {
        bus.sign_message(&mut m).unwrap();
        m
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
        let m = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        bus.dispatch(&m).unwrap();
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
        // 即使消息已正确签名，也因能力不足被拒。
        let m = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        let r = bus.dispatch(&m);
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
        // 正常 a→b 应成功（已签名）。
        let m = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        let r = bus.dispatch(&m);
        assert!(r.is_ok());
        // 手动构造一个 source 与令牌不一致的情况：
        bus.register("c");
        bus.set_state("c", PluginState::Running).unwrap();
        bus.set_token("c", token_for("different", &[Capability::MessageSend]))
            .unwrap();
        // 用 c 的密钥签名，但令牌绑定 different → 第3道 source 校验失败。
        let m2 = signed(
            &mut bus,
            msg(
                "c",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                2,
            ),
        );
        let r2 = bus.dispatch(&m2);
        assert!(r2.is_err());
    }

    #[test]
    fn rejects_non_running_sender() {
        let mut bus = PluginBus::new();
        bus.register("a");
        // 不推进到 RUNNING。
        bus.set_token("a", token_for("a", &[Capability::MessageSend]))
            .unwrap();
        let m = signed(&mut bus, msg("a", Target::Host, "plugin:message:send", 1));
        let r = bus.dispatch(&m);
        assert!(r.is_err());
    }

    #[test]
    fn rejects_non_running_target() {
        let mut bus = PluginBus::new();
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        bus.register("b"); // b 未 RUNNING。
        let m = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        let r = bus.dispatch(&m);
        assert!(r.is_err());
    }

    #[test]
    fn broadcasts_to_all_running() {
        let mut bus = PluginBus::new();
        let rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        let rx_c = bring_up(&mut bus, "c", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        let m = signed(
            &mut bus,
            msg("a", Target::Broadcast, "plugin:message:send", 1),
        );
        bus.dispatch(&m).unwrap();
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
            let m = signed(
                &mut bus,
                msg(
                    "a",
                    Target::Plugin("b".to_string()),
                    "plugin:message:send",
                    1 + t,
                ),
            );
            let _ = bus.dispatch(&m);
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
        let m1 = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        let _ = bus.dispatch(&m1);
        let m2 = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        let _ = bus.dispatch(&m2);
        // 同一窗口第 3 条应被限流（即使签名正确）。
        let m3 = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        let r3 = bus.dispatch(&m3);
        assert!(r3.is_err());
        let _ = rx;
        let _ = rx_a;
    }

    #[test]
    fn rejects_tampered_payload() {
        let mut bus = PluginBus::new();
        let _rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        // 签名后篡改 payload → 签名失配。
        let mut m = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        m.payload = serde_json::json!({"evil": true});
        let r = bus.dispatch(&m);
        assert!(r.is_err());
        assert_eq!(bus.violations("a"), 1);
    }

    #[test]
    fn rejects_unsigned_message() {
        let mut bus = PluginBus::new();
        let _rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        // 不签名直接投递（signature 为空）。
        let m = msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        );
        let r = bus.dispatch(&m);
        assert!(r.is_err());
    }

    #[test]
    fn rejects_replayed_nonce() {
        let mut bus = PluginBus::new();
        let _rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        // 第一条合法送达。
        let m1 = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        bus.dispatch(&m1).unwrap();
        // 完全相同的消息（含 nonce/signature）再次投递 → 重放被拒。
        let r2 = bus.dispatch(&m1);
        assert!(r2.is_err());
    }

    #[test]
    fn rejects_signature_from_other_plugin() {
        let mut bus = PluginBus::new();
        let _rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        bring_up(&mut bus, "x", &[Capability::MessageSend]);
        // 构造一条声称来自 a 的消息，却用 x 的密钥签名。
        let mut m = msg(
            "a",
            Target::Plugin("b".to_string()),
            "plugin:message:send",
            1,
        );
        let x_key = bus.session_key("x").unwrap();
        m.nonce = uuid::Uuid::new_v4().to_string();
        m.signature = compute_signature(&m, &x_key).unwrap();
        let r = bus.dispatch(&m);
        assert!(r.is_err());
    }

    // ── v3.5.2（AU-06）：终态锁定后门 ──────────────────────────────
    #[test]
    fn terminal_state_cannot_be_revived_to_running() {
        let mut bus = PluginBus::new();
        bus.register("p");
        // 进入终态 Quarantined（Discovered→Quarantined 非终态源，允许）。
        bus.set_state("p", PluginState::Quarantined).unwrap();
        // 终态插件不可被重新激活回 Running。
        let r = bus.set_state("p", PluginState::Running);
        assert!(r.is_err(), "终态 Quarantined 不可回 Running");
        // 也不可改写到任何其它非终态。
        assert!(bus.set_state("p", PluginState::Stopped).is_err());
        assert!(bus.set_state("p", PluginState::Discovered).is_err());
        // 状态保持终态。
        assert_eq!(bus.route_table().get("p"), Some(&PluginState::Quarantined));
    }

    // ── v3.5.2（AU-21）：TTL 新鲜度 ───────────────────────────────
    #[test]
    fn expired_message_rejected_when_server_clock_set() {
        let mut bus = PluginBus::new();
        let _rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        // 消息签发于 issued_at=1s（1000ms），ttl=5000ms → 有效至 6000ms。
        let m = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        // 未注入时钟（回放模式）→ 旧行为：新鲜度不拦，正常送达。
        bus.dispatch(&m).unwrap();
        // 注入服务端时钟到很远的未来（10,000s）→ 该消息早已过期。
        bus.set_server_clock_ms(10_000_000);
        let m2 = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        let err = bus.dispatch(&m2).unwrap_err();
        assert!(
            err.to_string().contains("PMB_MESSAGE_EXPIRED"),
            "过期消息必须被拒，got {err}"
        );
    }

    // ── v3.5.2（AU-25）：nonce 按时间淘汰 + 速率后置 ───────────────
    #[test]
    fn nonce_window_evicts_oldest_by_time_not_lexicographic() {
        let mut bus = PluginBus::new();
        let _rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        // 把 a 的 nonce 窗口填到 MAX 条，刻意让"字典序最小"的 aaa 反而是最新签发，
        // "时间最旧"的 zzz 反而是字典序最大——旧 BTreeSet 字典序淘汰会误删 aaa。
        {
            let e = bus.routes.get_mut("a").unwrap();
            for i in 0..(MAX_SEEN_NONCES - 2) {
                let n = format!("n-{i:05}");
                e.seen_nonces.insert(n.clone());
                e.nonce_recency.insert((1000 + i as u64, n));
            }
            // 字典序最小、但时间最新
            e.seen_nonces.insert("aaa".to_string());
            e.nonce_recency.insert((10_000_000u64, "aaa".to_string()));
            // 字典序最大、但时间最旧
            e.seen_nonces.insert("zzz".to_string());
            e.nonce_recency.insert((500u64, "zzz".to_string()));
            assert_eq!(e.seen_nonces.len(), MAX_SEEN_NONCES);
        }
        // 再投递一条合法新消息 → 窗口溢出，淘汰最旧。
        let m = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                5000,
            ),
        );
        bus.dispatch(&m).unwrap();
        let e = bus.routes.get("a").unwrap();
        // 时间序：zzz(500) 最旧 → 应被淘汰（可重放其 nonce）；aaa(10M) 最新 → 必须保留。
        assert!(
            !e.seen_nonces.contains("zzz"),
            "时间最旧的 zzz 应被淘汰，got recency={:?}",
            e.nonce_recency
        );
        assert!(
            e.seen_nonces.contains("aaa"),
            "时间最新的 aaa 不应被字典序淘汰"
        );
    }

    #[test]
    fn bad_signature_does_not_consume_rate_quota() {
        let mut bus = PluginBus::new();
        let rx_b = bring_up(&mut bus, "b", &[Capability::MessageSend]);
        bring_up(&mut bus, "a", &[Capability::MessageSend]);
        // a 的速率：窗口 1000ms 内仅 1 条。
        bus.routes.get_mut("a").unwrap().rate = RateLimit::new(1000, 1);
        // 第 1 条合法送达（消耗唯一配额）。
        let m1 = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                1,
            ),
        );
        bus.dispatch(&m1).unwrap();
        // 第 2 条：篡改 payload → 坏签名。旧实现速率在前会把它当"限流"拒绝并占坑语义；
        // 新实现签名先于速率，坏签名必须在签名步被拒，且**不消耗速率配额**。
        let mut m2 = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                2,
            ),
        );
        m2.payload = serde_json::json!({"evil": true});
        let r2 = bus.dispatch(&m2);
        assert!(r2.is_err());
        assert!(
            r2.as_ref().unwrap_err().to_string().contains("签名"),
            "坏签名应在签名步被拒，而非限流，got {r2:?}"
        );
        // 第 3 条：全新合法消息（issued_at=3，落在下一窗口）。速率配额未被坏签名消耗。
        let m3 = signed(
            &mut bus,
            msg(
                "a",
                Target::Plugin("b".to_string()),
                "plugin:message:send",
                3,
            ),
        );
        let r3 = bus.dispatch(&m3);
        assert!(r3.is_ok(), "坏签名不应消耗速率配额，got {r3:?}");
        let _ = rx_b;
    }
}
