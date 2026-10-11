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

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use libp2p::Multiaddr;

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
#[derive(Debug, Clone)]
pub struct EclipseGuard {
    cfg: EclipseGuardConfig,
    per_ip: HashMap<String, usize>,
    per_subnet: HashMap<String, usize>,
    total: usize,
}

impl EclipseGuard {
    /// 创建守卫。
    pub fn new(cfg: EclipseGuardConfig) -> Self {
        Self {
            cfg,
            per_ip: HashMap::new(),
            per_subnet: HashMap::new(),
            total: 0,
        }
    }

    /// 判定该地址是否允许进入 kbucket；允许则计入统计并返回 true。
    ///
    /// - 无 IP（DNS 等）→ 放行（不计数）；
    /// - 单 IP 计数 >= max_peers_per_ip → 拒绝；
    /// - 单子网计数 >= max_peers_per_subnet → 拒绝；
    /// - 跟踪表已满（total >= max_tracked）且该 IP 未见过的 → 拒绝（保护表大小）。
    pub fn accept(&mut self, addr: &Multiaddr) -> bool {
        let Some(ip) = extract_ip(addr) else {
            return true;
        };
        let ip_key = ip.to_string();
        let sub_key = subnet_key(&ip);
        let ip_n = self.per_ip.get(&ip_key).copied().unwrap_or(0);
        let sub_n = self.per_subnet.get(&sub_key).copied().unwrap_or(0);
        if ip_n >= self.cfg.max_peers_per_ip {
            return false;
        }
        if sub_n >= self.cfg.max_peers_per_subnet {
            return false;
        }
        // 新来源但表已满 → 拒绝，防止跟踪表被海量 IP 撑爆。
        if ip_n == 0 && self.total >= self.cfg.max_tracked {
            return false;
        }
        *self.per_ip.entry(ip_key).or_insert(0) += 1;
        *self.per_subnet.entry(sub_key).or_insert(0) += 1;
        self.total += 1;
        true
    }

    /// 路由表回填（重启后从持久化恢复）时，直接计数而不判定。
    /// 返回是否成功计数（表满时返回 false，调用方跳过该条目）。
    pub fn count(&mut self, addr: &Multiaddr) -> bool {
        let Some(ip) = extract_ip(addr) else {
            return true;
        };
        let ip_key = ip.to_string();
        let sub_key = subnet_key(&ip);
        if self.per_ip.get(&ip_key).copied().unwrap_or(0) >= self.cfg.max_peers_per_ip {
            return false;
        }
        if self.per_subnet.get(&sub_key).copied().unwrap_or(0) >= self.cfg.max_peers_per_subnet {
            return false;
        }
        *self.per_ip.entry(ip_key).or_insert(0) += 1;
        *self.per_subnet.entry(sub_key).or_insert(0) += 1;
        self.total += 1;
        true
    }

    /// 当前跟踪的独立 IP 数量（含子网维度总量）。
    pub fn tracked(&self) -> usize {
        self.total
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

    #[test]
    fn same_ip_limited_per_ip() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 2,
            max_peers_per_subnet: 8,
            max_tracked: 256,
        });
        let a = m("/ip4/10.0.0.1/tcp/4001");
        let b = m("/ip4/10.0.0.1/tcp/4002");
        let c = m("/ip4/10.0.0.1/tcp/4003");
        assert!(g.accept(&a));
        assert!(g.accept(&b));
        assert!(!g.accept(&c), "同 IP 第三条应被拒绝");
    }

    #[test]
    fn same_subnet_limited_per_subnet() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 2,
            max_peers_per_subnet: 3,
            max_tracked: 256,
        });
        // 10.0.0.x 同一 /24；10.0.1.x 不同 /24。
        assert!(g.accept(&m("/ip4/10.0.0.1/tcp/1")));
        assert!(g.accept(&m("/ip4/10.0.0.2/tcp/1")));
        assert!(g.accept(&m("/ip4/10.0.0.3/tcp/1")));
        assert!(
            !g.accept(&m("/ip4/10.0.0.4/tcp/1")),
            "同 /24 第四条应被拒绝"
        );
        assert!(g.accept(&m("/ip4/10.0.1.1/tcp/1")), "不同子网放行");
    }

    #[test]
    fn dns_addr_not_counted() {
        let mut g = EclipseGuard::new(EclipseGuardConfig::default());
        assert!(g.accept(&m("/dns4/boot.example.com/tcp/4001")));
        assert_eq!(g.tracked(), 0);
    }

    #[test]
    fn ipv6_subnet_isolation() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 2,
            max_peers_per_subnet: 2,
            max_tracked: 256,
        });
        assert!(g.accept(&m("/ip6/2001:db8::1/tcp/1")));
        assert!(g.accept(&m("/ip6/2001:db8::2/tcp/1")));
        assert!(
            !g.accept(&m("/ip6/2001:db8::3/tcp/1")),
            "同 /64 第三条应被拒绝"
        );
        assert!(g.accept(&m("/ip6/2001:db9::1/tcp/1")), "不同 /64 放行");
    }

    #[test]
    fn tracked_table_cap() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 2,
            max_peers_per_subnet: 2,
            max_tracked: 2,
        });
        assert!(g.accept(&m("/ip4/10.0.0.1/tcp/1")));
        assert!(g.accept(&m("/ip4/10.0.1.1/tcp/1")));
        // 表已满（total=2），新 IP 应拒绝。
        assert!(!g.accept(&m("/ip4/10.0.2.1/tcp/1")));
        assert_eq!(g.tracked(), 2);
    }

    #[test]
    fn count_backfill_respects_limits() {
        let mut g = EclipseGuard::new(EclipseGuardConfig {
            max_peers_per_ip: 2,
            max_peers_per_subnet: 8,
            max_tracked: 256,
        });
        assert!(g.count(&m("/ip4/10.0.0.1/tcp/1")));
        assert!(g.count(&m("/ip4/10.0.0.1/tcp/2")));
        assert!(!g.count(&m("/ip4/10.0.0.1/tcp/3")));
    }
}
