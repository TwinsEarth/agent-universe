//! DHT（Kademlia）行为参数与 Eclipse（日蚀）防护。
//!
//! v3.9.17：K 桶加固。
//! - [`DhtConfig`]：把 libp2p-kad 的关键行为参数显式化（query timeout、
//!   kbucket 并发插入、最大包大小），并携带 Eclipse 防护配置与路由持久化路径；
//! - [`EclipseGuard`]：按「单 IP」与「/24（IPv4）/ /64（IPv6）子网」两个维度
//!   限制可进入 kbucket 的对端数量，防止恶意节点用大量伪身份把目标节点
//!   路由表中的真实邻居挤出去（Eclipse/日蚀攻击的第一阶段——路由表隔离）。
//!
//! 设计原则（与 v3.9.16 GossipConfig 一致）：参数可校验、失败快速暴露；
//! 只派生 `PartialEq`（含 `Duration` 与浮点无关，但统一不派生 `Eq`）。

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use libp2p::{Multiaddr, PeerId};

/// Eclipse 防护参数：控制同一来源可进入路由表的对端数量。
#[derive(Debug, Clone, PartialEq)]
pub struct EclipseGuardConfig {
    /// 同一 IP 地址允许进入 kbucket 的最大对端数（默认 4）。
    pub max_peers_per_ip: usize,
    /// 同一子网（IPv4 /24、IPv6 /64）允许进入 kbucket 的最大对端数（默认 8）。
    pub max_peers_per_subnet: usize,
    /// 防护跟踪表上限：防止攻击者用海量不同 IP 撑爆计数表（默认 256）。
    pub max_tracked: usize,
}

impl Default for EclipseGuardConfig {
    fn default() -> Self {
        Self {
            max_peers_per_ip: 4,
            max_peers_per_subnet: 8,
            max_tracked: 256,
        }
    }
}

/// DHT 行为配置。
#[derive(Debug, Clone, PartialEq)]
pub struct DhtConfig {
    /// 单次 Kademlia 查询超时（默认 30s）。
    pub query_timeout: Duration,
    /// 同一 kbucket 允许的并发插入数（防插入风暴，默认 10）。
    pub kbucket_inserts: usize,
    /// 单个 kad 消息最大字节数（默认 4096，防御放大攻击）。
    pub max_packet_size: usize,
    /// Eclipse 防护参数。
    pub eclipse: EclipseGuardConfig,
    /// 路由表持久化文件路径；`None` 表示不落盘（默认 None，由调用方按需开启）。
    pub routing_persist_path: Option<PathBuf>,
    /// bootstrap 完成后是否立即持久化一次路由表（默认 true）。
    pub persist_after_bootstrap: bool,
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            query_timeout: Duration::from_secs(30),
            kbucket_inserts: 10,
            max_packet_size: 4096,
            eclipse: EclipseGuardConfig::default(),
            routing_persist_path: None,
            persist_after_bootstrap: true,
        }
    }
}

impl DhtConfig {
    /// 链式开启路由持久化。
    pub fn with_routing_persist(mut self, path: PathBuf) -> Self {
        self.routing_persist_path = Some(path);
        self
    }

    /// 允许用环境变量覆盖 Eclipse 防护上限：
    /// `GSN_DHT_MAX_PEERS_PER_IP` / `GSN_DHT_MAX_PEERS_PER_SUBNET`。
    ///
    /// 默认（不调用本方法）仍使用内置上限（单 IP 4 / 单子网 8），行为不变；
    /// 仅在显式调用时读取环境变量。用于两类场景：
    /// - 真实集群中大量节点处于同一出口 IP / NAT 之后，内置上限会误伤正常邻居；
    /// - 单机多节点规模回归（全部节点同 IP）需要放宽上限才能验证真实 K 桶填充。
    ///
    /// 非法取值（非数字、0、或 ip > subnet）由 [`DhtConfig::validate`] 具名拒绝；
    /// 解析失败的环境变量被忽略（保持内置默认），不静默吞掉后续校验。
    pub fn with_eclipse_env_override(mut self) -> Self {
        if let Some(v) = std::env::var("GSN_DHT_MAX_PEERS_PER_IP")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        {
            self.eclipse.max_peers_per_ip = v;
        }
        if let Some(v) = std::env::var("GSN_DHT_MAX_PEERS_PER_SUBNET")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        {
            self.eclipse.max_peers_per_subnet = v;
        }
        self
    }

    /// 校验参数合法性。非法配置直接返回具名错误（fail-fast）。
    pub fn validate(&self) -> Result<(), DhtConfigError> {
        if self.query_timeout.is_zero() {
            return Err(DhtConfigError::QueryTimeoutZero);
        }
        if self.kbucket_inserts == 0 {
            return Err(DhtConfigError::KbucketInsertsZero);
        }
        if self.max_packet_size == 0 || self.max_packet_size > 16 * 1024 * 1024 {
            return Err(DhtConfigError::MaxPacketSizeOutOfRange(
                self.max_packet_size,
            ));
        }
        if self.eclipse.max_peers_per_ip == 0 {
            return Err(DhtConfigError::EclipsePerIpZero);
        }
        if self.eclipse.max_peers_per_subnet == 0 {
            return Err(DhtConfigError::EclipsePerSubnetZero);
        }
        if self.eclipse.max_tracked == 0 {
            return Err(DhtConfigError::EclipseTrackedZero);
        }
        if self.eclipse.max_peers_per_ip > self.eclipse.max_peers_per_subnet {
            return Err(DhtConfigError::EclipseIpGtSubnet);
        }
        Ok(())
    }
}

/// DHT 配置校验错误（具名、可枚举）。
#[derive(Debug, Clone, PartialEq)]
pub enum DhtConfigError {
    /// query_timeout 为 0。
    QueryTimeoutZero,
    /// kbucket_inserts 为 0。
    KbucketInsertsZero,
    /// max_packet_size 越界（>16MiB 视为放大风险）。
    MaxPacketSizeOutOfRange(usize),
    /// Eclipse 单 IP 上限为 0。
    EclipsePerIpZero,
    /// Eclipse 单子网上限为 0。
    EclipsePerSubnetZero,
    /// Eclipse 跟踪表上限为 0。
    EclipseTrackedZero,
    /// 单 IP 上限大于单子网上限（语义矛盾）。
    EclipseIpGtSubnet,
}

impl std::fmt::Display for DhtConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueryTimeoutZero => write!(f, "query_timeout 必须大于 0"),
            Self::KbucketInsertsZero => write!(f, "kbucket_inserts 必须大于 0"),
            Self::MaxPacketSizeOutOfRange(v) => {
                write!(f, "max_packet_size {v} 越界（应位于 1..=16MiB）")
            }
            Self::EclipsePerIpZero => write!(f, "eclipse.max_peers_per_ip 必须大于 0"),
            Self::EclipsePerSubnetZero => write!(f, "eclipse.max_peers_per_subnet 必须大于 0"),
            Self::EclipseTrackedZero => write!(f, "eclipse.max_tracked 必须大于 0"),
            Self::EclipseIpGtSubnet => {
                write!(f, "eclipse.max_peers_per_ip 不能大于 max_peers_per_subnet")
            }
        }
    }
}

impl std::error::Error for DhtConfigError {}

/// 把 Multiaddr 中的 IP 地址提取出来；无法解析 IP 时返回 None
/// （如纯 DNS 地址 `/dns4/…`，此时不做 Eclipse 计数，交给子网/连接层约束）。
fn extract_ip(addr: &Multiaddr) -> Option<IpAddr> {
    for p in addr.iter() {
        use libp2p::multiaddr::Protocol;
        match p {
            Protocol::Ip4(a) => return Some(IpAddr::V4(a)),
            Protocol::Ip6(a) => return Some(IpAddr::V6(a)),
            _ => {}
        }
    }
    None
}

/// 子网 key：IPv4 取 /24，IPv6 取 /64（前 8 字节）。
fn subnet_key(ip: &IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            format!("{}.{}.{}.0/24", o[0], o[1], o[2])
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
    }
}

/// Eclipse（日蚀）防护：按单 IP 与子网两个维度限制可进入 kbucket 的对端。
///
/// 攻击模型：攻击者批量生成 PeerId 并从同一 IP / 同一子网发起连接，把
/// 目标节点 kbucket 填满，使真实邻居被挤出路由表（Eclipse 攻击前提）。
/// 本守卫在 [`super::peer::P2pPeer::add_kad_address`] 之前执行：
/// 超限来源的对端「保持连接但不登记进 kbucket」。
///
/// v5.2.2：计数维度从「地址」修正为「对端身份（PeerId）」——同一 peer 的
/// 多个地址（TCP/QUIC/IPv4/IPv6 listen_addrs）只计 1 次。修复 50 节点单机
/// 回归实测暴露的缺陷：每个 peer 平均 8 个地址，按地址计数会把同一 IP 的
/// 配额（默认 4，实测放宽到 50）提前吃光，导致后续新连接全部被拒入桶
/// （实测 min=0，路由表反而退化为 0）。现在「单 IP 最多 N 个不同 peer」，
/// 语义与 K 桶加固目标一致。
#[derive(Debug, Clone)]
pub struct EclipseGuard {
    cfg: EclipseGuardConfig,
    per_ip: HashMap<String, HashSet<PeerId>>,
    per_subnet: HashMap<String, HashSet<PeerId>>,
    seen: HashSet<PeerId>,
}

impl EclipseGuard {
    /// 创建守卫。
    pub fn new(cfg: EclipseGuardConfig) -> Self {
        Self {
            cfg,
            per_ip: HashMap::new(),
            per_subnet: HashMap::new(),
            seen: HashSet::new(),
        }
    }

    /// 判定该对端（peer_id）的该地址是否允许进入 kbucket；允许则计入统计并返回 true。
    ///
    /// - 无 IP（DNS 等）→ 放行（不计数）；
    /// - 同 peer 已在同一 IP/子网登记过 → 放行（多地址只计 1 次）；
    /// - 该 IP 已登记的不同 peer 数 >= max_peers_per_ip → 拒绝；
    /// - 该子网已登记的不同 peer 数 >= max_peers_per_subnet → 拒绝；
    /// - 从未见过的 peer 且跟踪表已满（seen.len() >= max_tracked）→ 拒绝（保护表大小）。
    pub fn accept(&mut self, peer_id: &PeerId, addr: &Multiaddr) -> bool {
        let Some(ip) = extract_ip(addr) else {
            return true;
        };
        let ip_key = ip.to_string();
        let sub_key = subnet_key(&ip);
        if let Some(set) = self.per_ip.get(&ip_key) {
            if set.contains(peer_id) {
                return true;
            }
            if set.len() >= self.cfg.max_peers_per_ip {
                return false;
            }
        }
        if let Some(set) = self.per_subnet.get(&sub_key) {
            if set.contains(peer_id) {
                return true;
            }
            if set.len() >= self.cfg.max_peers_per_subnet {
                return false;
            }
        }
        // 新来源但表已满 → 拒绝，防止跟踪表被海量 IP/对端撑爆。
        if !self.seen.contains(peer_id) && self.seen.len() >= self.cfg.max_tracked {
            return false;
        }
        self.per_ip
            .entry(ip_key.clone())
            .or_default()
            .insert(*peer_id);
        self.per_subnet.entry(sub_key).or_default().insert(*peer_id);
        self.seen.insert(*peer_id);
        true
    }

    /// 路由表回填（重启后从持久化恢复）时，直接计数而不判定。
    /// 返回是否成功计数（表满时返回 false，调用方跳过该条目）。
    pub fn count(&mut self, peer_id: &PeerId, addr: &Multiaddr) -> bool {
        let Some(ip) = extract_ip(addr) else {
            return true;
        };
        let ip_key = ip.to_string();
        let sub_key = subnet_key(&ip);
        if let Some(set) = self.per_ip.get(&ip_key) {
            if set.contains(peer_id) {
                return true;
            }
            if set.len() >= self.cfg.max_peers_per_ip {
                return false;
            }
        }
        if let Some(set) = self.per_subnet.get(&sub_key) {
            if set.contains(peer_id) {
                return true;
            }
            if set.len() >= self.cfg.max_peers_per_subnet {
                return false;
            }
        }
        self.per_ip
            .entry(ip_key.clone())
            .or_default()
            .insert(*peer_id);
        self.per_subnet.entry(sub_key).or_default().insert(*peer_id);
        self.seen.insert(*peer_id);
        true
    }

    /// 当前跟踪的独立对端数量。
    pub fn tracked(&self) -> usize {
        self.seen.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(addr: &str) -> Multiaddr {
        addr.parse().expect("valid multiaddr")
    }

    #[test]
    fn default_config_is_valid() {
        DhtConfig::default().validate().expect("default valid");
    }

    #[test]
    fn zero_query_timeout_rejected() {
        let c = DhtConfig {
            query_timeout: Duration::ZERO,
            ..DhtConfig::default()
        };
        assert_eq!(c.validate(), Err(DhtConfigError::QueryTimeoutZero));
    }

    #[test]
    fn zero_kbucket_inserts_rejected() {
        let c = DhtConfig {
            kbucket_inserts: 0,
            ..DhtConfig::default()
        };
        assert_eq!(c.validate(), Err(DhtConfigError::KbucketInsertsZero));
    }

    #[test]
    fn packet_size_out_of_range_rejected() {
        let c = DhtConfig {
            max_packet_size: 17 * 1024 * 1024,
            ..DhtConfig::default()
        };
        assert!(matches!(
            c.validate(),
            Err(DhtConfigError::MaxPacketSizeOutOfRange(_))
        ));
    }

    #[test]
    fn ip_greater_than_subnet_rejected() {
        let mut c = DhtConfig::default();
        c.eclipse.max_peers_per_ip = 8;
        c.eclipse.max_peers_per_subnet = 4;
        assert_eq!(c.validate(), Err(DhtConfigError::EclipseIpGtSubnet));
    }

    // ─────────── v5.2.2: 环境变量覆盖 Eclipse 上限 ───────────

    /// 三个环境变量场景合并为一个串行测试：
    /// env 是进程级全局状态，拆成多个并行测试会互相污染（Rust 测试默认并行）。
    #[test]
    fn env_override_behavior_serial() {
        // 1) 不设环境变量 → 保持内置默认 4/8。
        std::env::remove_var("GSN_DHT_MAX_PEERS_PER_IP");
        std::env::remove_var("GSN_DHT_MAX_PEERS_PER_SUBNET");
        let c = DhtConfig::default().with_eclipse_env_override();
        assert_eq!(c.eclipse.max_peers_per_ip, 4);
        assert_eq!(c.eclipse.max_peers_per_subnet, 8);
        c.validate().expect("default still valid");

        // 2) 设置合法值 → 覆盖生效。
        std::env::set_var("GSN_DHT_MAX_PEERS_PER_IP", "50");
        std::env::set_var("GSN_DHT_MAX_PEERS_PER_SUBNET", "64");
        let c = DhtConfig::default().with_eclipse_env_override();
        std::env::remove_var("GSN_DHT_MAX_PEERS_PER_IP");
        std::env::remove_var("GSN_DHT_MAX_PEERS_PER_SUBNET");
        assert_eq!(c.eclipse.max_peers_per_ip, 50);
        assert_eq!(c.eclipse.max_peers_per_subnet, 64);
        c.validate().expect("overridden config valid");

        // 3) 非法值（非数字 / 空串）被忽略；0 通过 override 写入后被 validate 拒绝。
        std::env::set_var("GSN_DHT_MAX_PEERS_PER_IP", "not-a-number");
        std::env::set_var("GSN_DHT_MAX_PEERS_PER_SUBNET", "");
        let c = DhtConfig::default().with_eclipse_env_override();
        std::env::remove_var("GSN_DHT_MAX_PEERS_PER_IP");
        std::env::remove_var("GSN_DHT_MAX_PEERS_PER_SUBNET");
        assert_eq!(c.eclipse.max_peers_per_ip, 4, "非法值忽略");
        assert_eq!(c.eclipse.max_peers_per_subnet, 8, "空串忽略");

        std::env::set_var("GSN_DHT_MAX_PEERS_PER_IP", "0");
        let z = DhtConfig::default().with_eclipse_env_override();
        std::env::remove_var("GSN_DHT_MAX_PEERS_PER_IP");
        assert_eq!(z.validate(), Err(DhtConfigError::EclipsePerIpZero));
    }

    #[test]
    fn same_ip_limited_per_ip() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 2,
            max_peers_per_subnet: 8,
            max_tracked: 256,
        });
        let a = m("/ip4/10.0.0.1/tcp/4001");
        let p1 = PeerId::random();
        let p2 = PeerId::random();
        let p3 = PeerId::random();
        assert!(g.accept(&p1, &a));
        // v5.2.2: 同一 peer 的第二个地址（不同端口）只计 1 次，必须放行。
        assert!(g.accept(&p1, &m("/ip4/10.0.0.1/tcp/4002")));
        assert!(g.accept(&p2, &m("/ip4/10.0.0.1/tcp/4003")));
        assert!(
            !g.accept(&p3, &m("/ip4/10.0.0.1/tcp/4004")),
            "同 IP 第三个不同 peer 应被拒绝"
        );
    }

    #[test]
    fn same_subnet_limited_per_subnet() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 8,
            max_peers_per_subnet: 3,
            max_tracked: 256,
        });
        // 10.0.0.x 同一 /24；10.0.1.x 不同 /24。
        assert!(g.accept(&PeerId::random(), &m("/ip4/10.0.0.1/tcp/1")));
        assert!(g.accept(&PeerId::random(), &m("/ip4/10.0.0.2/tcp/1")));
        assert!(g.accept(&PeerId::random(), &m("/ip4/10.0.0.3/tcp/1")));
        assert!(
            !g.accept(&PeerId::random(), &m("/ip4/10.0.0.4/tcp/1")),
            "同 /24 第四个不同 peer 应被拒绝"
        );
        assert!(
            g.accept(&PeerId::random(), &m("/ip4/10.0.1.1/tcp/1")),
            "不同子网放行"
        );
    }

    #[test]
    fn dns_addr_not_counted() {
        let mut g = EclipseGuard::new(EclipseGuardConfig::default());
        assert!(g.accept(&PeerId::random(), &m("/dns4/boot.example.com/tcp/4001")));
        assert_eq!(g.tracked(), 0);
    }

    #[test]
    fn ipv6_subnet_isolation() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 2,
            max_peers_per_subnet: 2,
            max_tracked: 256,
        });
        assert!(g.accept(&PeerId::random(), &m("/ip6/2001:db8::1/tcp/1")));
        assert!(g.accept(&PeerId::random(), &m("/ip6/2001:db8::2/tcp/1")));
        assert!(
            !g.accept(&PeerId::random(), &m("/ip6/2001:db8::3/tcp/1")),
            "同 /64 第三个不同 peer 应被拒绝"
        );
        assert!(
            g.accept(&PeerId::random(), &m("/ip6/2001:db9::1/tcp/1")),
            "不同 /64 放行"
        );
    }

    #[test]
    fn tracked_table_cap() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 8,
            max_peers_per_subnet: 8,
            max_tracked: 2,
        });
        assert!(g.accept(&PeerId::random(), &m("/ip4/10.0.0.1/tcp/1")));
        assert!(g.accept(&PeerId::random(), &m("/ip4/10.0.1.1/tcp/1")));
        // 表已满（tracked=2），新 peer 应拒绝。
        assert!(!g.accept(&PeerId::random(), &m("/ip4/10.0.2.1/tcp/1")));
        assert_eq!(g.tracked(), 2);
    }

    #[test]
    fn count_backfill_respects_limits() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 2,
            max_peers_per_subnet: 8,
            max_tracked: 256,
        });
        assert!(g.count(&PeerId::random(), &m("/ip4/10.0.0.1/tcp/1")));
        assert!(g.count(&PeerId::random(), &m("/ip4/10.0.0.1/tcp/2")));
        assert!(!g.count(&PeerId::random(), &m("/ip4/10.0.0.1/tcp/3")));
    }
}
