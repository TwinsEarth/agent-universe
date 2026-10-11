//! v3.9.16: GossipSub 参数化配置与风暴防护
//!
//! libp2p-gossipsub 0.47 的 `ConfigBuilder` 提供 mesh / 历史 / 心跳 / 转发
//! 全套参数，但此前 gsn 只设置了 `heartbeat_interval`，其余全部走库默认。
//! 本模块把可调参数收敛为一个 `GossipConfig` 结构（纯数据，不依赖 libp2p
//! 类型），提供 `validate()` 在构造 Behaviour 前做合法性校验；同时提供
//! `GossipStormGuard`（按主题滑动窗口的入站限速），在应用层去重之后、
//! 投递到消费者之前执行，防御单主题突发风暴压垮 `inbound_tx` 通道。

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// GossipSub 消息验证模式（与 libp2p `ValidationMode` 一一对应，
/// 但保持在库面数据结构中，避免本模块直接依赖 libp2p 类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GossipValidationMode {
    /// 只接受 Signed 消息，签名无效直接丢弃（默认，抗伪造）。
    Strict,
    /// 接受 Anonymous 与 Signed，但 Anonymous 消息不转发。
    Anonymous,
    /// 接受任意来源消息（仅用于受控实验/演示，生产禁用）。
    Permissive,
}

/// GossipSub 行为参数集。
///
/// 默认值对齐 libp2p-gossipsub 0.47 推荐值，其中：
/// - `mesh_n / mesh_n_low / mesh_n_high`：每主题 mesh 目标/下限/上限
/// - `gossip_lazy / gossip_factor`：lazy gossip 扇出
/// - `heartbeat_interval`：mesh 维护心跳
/// - `history_length / history_gossip`：每主题消息缓存与重播条数
/// - `fanout_ttl`：fanout 主题存活时间
/// - `max_transmit_size`：单条消息上限（1 MiB，与库默认一致）
/// - `prune_peers`：PRUNE 保留的对端数（mesh 过载收缩目标）
/// - `opportunistic_graft_ticks`：机会性 GRAFT 触发心跳数
/// - `dedup_capacity`：应用层去重窗口（`GossipDedup` 容量）
/// - `storm_*`：按主题滑动窗口风暴防护参数
#[derive(Debug, Clone)]
pub struct GossipConfig {
    /// 每主题 mesh 目标邻居数
    pub mesh_n: usize,
    /// mesh 下限（低于则 GRAFT 补员）
    pub mesh_n_low: usize,
    /// mesh 上限（高于则 PRUNE 收缩）
    pub mesh_n_high: usize,
    /// lazy gossip 扇出数
    pub gossip_lazy: usize,
    /// lazy gossip 扇出比例（0.0~1.0）
    pub gossip_factor: f64,
    /// mesh 维护心跳间隔
    pub heartbeat_interval: Duration,
    /// 每主题消息缓存条数
    pub history_length: usize,
    /// 心跳时向新对端重播的历史消息条数
    pub history_gossip: usize,
    /// fanout 主题（无订阅者时的临时转发）存活时间
    pub fanout_ttl: Duration,
    /// 单条消息最大字节数
    pub max_transmit_size: usize,
    /// 消息验证模式
    pub validation_mode: GossipValidationMode,
    /// PRUNE 时保留的最大对端数
    pub prune_peers: usize,
    /// 机会性 GRAFT 触发间隔（心跳数）
    pub opportunistic_graft_ticks: u64,
    /// 应用层去重窗口容量
    pub dedup_capacity: usize,
    /// 风暴防护：滑动窗口时长
    pub storm_window: Duration,
    /// 风暴防护：每主题每窗口最大放行条数
    pub storm_max_per_topic: u32,
    /// 风暴防护：跟踪的最大主题数（防无界增长）
    pub storm_max_topics: usize,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            mesh_n: 6,
            mesh_n_low: 4,
            mesh_n_high: 12,
            gossip_lazy: 6,
            gossip_factor: 0.25,
            heartbeat_interval: Duration::from_secs(10),
            history_length: 5,
            history_gossip: 3,
            fanout_ttl: Duration::from_secs(60),
            max_transmit_size: 1024 * 1024,
            validation_mode: GossipValidationMode::Strict,
            prune_peers: 128,
            opportunistic_graft_ticks: 60,
            dedup_capacity: 4096,
            storm_window: Duration::from_secs(60),
            storm_max_per_topic: 1024,
            storm_max_topics: 256,
        }
    }
}

/// GossipConfig 校验错误（具名、可读，供上层转成 anyhow）。
/// 注意：`InvalidGossipFactor(f64)` 含 f64，不能派生 `Eq`（f64 未实现 Eq）。
#[derive(Debug, Clone, PartialEq)]
pub enum GossipConfigError {
    /// mesh 三参数不满足 low <= n <= high
    InvalidMeshBounds { low: usize, n: usize, high: usize },
    /// gossip_factor 超出 [0.0, 1.0]
    InvalidGossipFactor(f64),
    /// 该参数不允许为 0（历史/扇出/窗口等）
    ZeroValue(&'static str),
    /// max_transmit_size 超过 1 MiB（库上限）
    TransmitSizeTooLarge(usize),
    /// 风暴防护参数非法（窗口为 0 或每窗口放行为 0）
    InvalidStormGuard,
}

impl std::fmt::Display for GossipConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GossipConfigError::InvalidMeshBounds { low, n, high } => write!(
                f,
                "mesh 边界非法：要求 low <= n <= high，实际 low={low} n={n} high={high}"
            ),
            GossipConfigError::InvalidGossipFactor(v) => {
                write!(f, "gossip_factor 必须位于 [0.0, 1.0]，实际 {v}")
            }
            GossipConfigError::ZeroValue(name) => write!(f, "参数 {name} 不允许为 0"),
            GossipConfigError::TransmitSizeTooLarge(v) => {
                write!(f, "max_transmit_size={v} 超过 1 MiB 上限")
            }
            GossipConfigError::InvalidStormGuard => {
                write!(f, "风暴防护参数非法：窗口或每窗口放行数不能为 0")
            }
        }
    }
}

impl std::error::Error for GossipConfigError {}

impl GossipConfig {
    /// 校验参数合法性，非法返回具名错误（在构造 GossipsubConfig 前调用）。
    pub fn validate(&self) -> Result<(), GossipConfigError> {
        if self.mesh_n_low > self.mesh_n || self.mesh_n > self.mesh_n_high {
            return Err(GossipConfigError::InvalidMeshBounds {
                low: self.mesh_n_low,
                n: self.mesh_n,
                high: self.mesh_n_high,
            });
        }
        if !(0.0..=1.0).contains(&self.gossip_factor) {
            return Err(GossipConfigError::InvalidGossipFactor(self.gossip_factor));
        }
        for (name, v) in [
            ("mesh_n", self.mesh_n),
            ("gossip_lazy", self.gossip_lazy),
            ("history_length", self.history_length),
            ("history_gossip", self.history_gossip),
            ("prune_peers", self.prune_peers),
            ("dedup_capacity", self.dedup_capacity),
            ("storm_max_topics", self.storm_max_topics),
        ] {
            if v == 0 {
                return Err(GossipConfigError::ZeroValue(name));
            }
        }
        if self.max_transmit_size == 0 || self.max_transmit_size > 1024 * 1024 {
            return Err(GossipConfigError::TransmitSizeTooLarge(
                self.max_transmit_size,
            ));
        }
        if self.storm_window.is_zero() || self.storm_max_per_topic == 0 {
            return Err(GossipConfigError::InvalidStormGuard);
        }
        Ok(())
    }
}

/// 按主题滑动窗口的入站风暴防护。
///
/// 语义：每个主题维护 `(窗口起点, 窗口内已放行数)`；窗口过期则重置。
/// 窗口内放行数达到 `max_per_window` 后拒绝新消息，直到窗口滑动。
/// `max_topics` 限制跟踪主题数，避免恶意高频换主题导致计数表无界增长
/// （超限时拒绝**新**主题，已有主题不受影响）。
#[derive(Debug)]
pub struct GossipStormGuard {
    window: Duration,
    max_per_window: u32,
    max_topics: usize,
    counts: HashMap<String, (Instant, u32)>,
}

impl GossipStormGuard {
    /// 创建风暴防护器。
    pub fn new(window: Duration, max_per_window: u32, max_topics: usize) -> Self {
        Self {
            window,
            max_per_window,
            max_topics,
            counts: HashMap::with_capacity(max_topics),
        }
    }

    /// 由 GossipConfig 创建（已假定参数经 validate）。
    pub fn from_config(cfg: &GossipConfig) -> Self {
        Self::new(
            cfg.storm_window,
            cfg.storm_max_per_topic,
            cfg.storm_max_topics,
        )
    }

    /// 判定某主题当前是否放行（放行则计入计数）。使用系统时钟。
    pub fn allow(&mut self, topic: &str) -> bool {
        self.allow_at(topic, Instant::now())
    }

    /// 判定某主题当前是否放行（可注入时间，供确定性单测）。
    pub fn allow_at(&mut self, topic: &str, now: Instant) -> bool {
        match self.counts.get_mut(topic) {
            Some(entry) => {
                if now.duration_since(entry.0) >= self.window {
                    *entry = (now, 1);
                    return true;
                }
                if entry.1 >= self.max_per_window {
                    return false;
                }
                entry.1 += 1;
                true
            }
            None => {
                // 新主题：跟踪表已满则拒绝，防无界增长
                if self.counts.len() >= self.max_topics {
                    return false;
                }
                self.counts.insert(topic.to_string(), (now, 1));
                true
            }
        }
    }

    /// 当前跟踪的主题数（诊断/测试用）。
    pub fn tracked_topics(&self) -> usize {
        self.counts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> GossipConfig {
        GossipConfig::default()
    }

    #[test]
    fn default_config_validates() {
        assert!(cfg().validate().is_ok());
    }

    #[test]
    fn rejects_invalid_mesh_bounds() {
        let mut c = cfg();
        c.mesh_n_low = 8;
        c.mesh_n = 6;
        assert_eq!(
            c.validate(),
            Err(GossipConfigError::InvalidMeshBounds {
                low: 8,
                n: 6,
                high: 12
            })
        );
        let mut c = cfg();
        c.mesh_n = 20;
        assert!(matches!(
            c.validate(),
            Err(GossipConfigError::InvalidMeshBounds { .. })
        ));
    }

    #[test]
    fn rejects_invalid_gossip_factor() {
        let mut c = cfg();
        c.gossip_factor = 1.5;
        assert_eq!(
            c.validate(),
            Err(GossipConfigError::InvalidGossipFactor(1.5))
        );
        let mut c = cfg();
        c.gossip_factor = -0.1;
        assert!(c.validate().is_err());
    }

    #[test]
    fn rejects_zero_and_oversize() {
        let mut c = cfg();
        c.dedup_capacity = 0;
        assert_eq!(
            c.validate(),
            Err(GossipConfigError::ZeroValue("dedup_capacity"))
        );
        let mut c = cfg();
        c.max_transmit_size = 2 * 1024 * 1024;
        assert_eq!(
            c.validate(),
            Err(GossipConfigError::TransmitSizeTooLarge(2 * 1024 * 1024))
        );
    }

    #[test]
    fn rejects_invalid_storm_guard() {
        let mut c = cfg();
        c.storm_max_per_topic = 0;
        assert_eq!(c.validate(), Err(GossipConfigError::InvalidStormGuard));
    }

    #[test]
    fn storm_guard_burst_then_window_rolls() {
        let mut g = GossipStormGuard::new(Duration::from_secs(60), 3, 16);
        let t0 = Instant::now();
        assert!(g.allow_at("gsn/agents", t0), "第 1 条放行");
        assert!(g.allow_at("gsn/agents", t0 + Duration::from_millis(10)));
        assert!(g.allow_at("gsn/agents", t0 + Duration::from_millis(20)));
        assert!(
            !g.allow_at("gsn/agents", t0 + Duration::from_millis(30)),
            "窗口内超出 burst 拒绝"
        );
        // 窗口滑动后恢复
        assert!(g.allow_at("gsn/agents", t0 + Duration::from_secs(61)));
        assert!(g.tracked_topics() == 1);
    }

    #[test]
    fn storm_guard_is_per_topic() {
        let mut g = GossipStormGuard::new(Duration::from_secs(60), 2, 16);
        let t0 = Instant::now();
        assert!(g.allow_at("gsn/agents", t0));
        assert!(g.allow_at("gsn/agents", t0));
        assert!(!g.allow_at("gsn/agents", t0));
        // 不同主题独立计数
        assert!(g.allow_at("gsn/crdt", t0));
        assert!(g.tracked_topics() == 2);
    }

    #[test]
    fn storm_guard_rejects_new_topic_when_table_full() {
        let mut g = GossipStormGuard::new(Duration::from_secs(60), 1024, 2);
        let t0 = Instant::now();
        assert!(g.allow_at("a", t0));
        assert!(g.allow_at("b", t0));
        assert!(!g.allow_at("c", t0), "跟踪表已满，新主题拒绝");
        // 已有主题仍放行
        assert!(g.allow_at("a", t0 + Duration::from_secs(61)));
    }
}
