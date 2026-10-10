//! 网络层
//!
//! - [`peer`] / [`P2pPeer`]：**真实** libp2p 网络（传输、加密、DHT、GossipSub、中继/DCUtR）。
//! - [`memory_node`] / [`memory_dht`] / [`memory_gossip`]：**进程内替身**（in-memory），
//!   仅用于单元测试与离线演示，不产生任何网络流量；勿用于断言真实联网。
//! - [`proxy`] / [`pac`] / [`subscription`] / [`routing`] / [`probe`]（v3.9.14，
//!   参照 v2rayN）：系统代理、本地 PAC 服务、节点订阅、路由规则引擎与连通性探测。

pub mod memory_dht;
pub mod memory_gossip;
pub mod memory_node;
pub mod pac;
pub mod peer;
pub mod probe;
pub mod proxy;
pub mod root_seed;
pub mod routing;
pub mod subscription;

pub use memory_dht::InMemoryKademlia;
pub use memory_gossip::InMemoryGossip;
pub use memory_node::InMemoryNode;
pub use pac::{serve_pac, PacConfig};
pub use peer::P2pPeer;
pub use probe::{
    measure_latency_via_proxy, query_exit_info, socks5_handshake, ProbeConfig, ProbeResult,
};
pub use proxy::{
    real_manager, ProxyScheme, RealCommandRunner, SystemProxyConfig, SystemProxyManager,
};
pub use root_seed::{RootSeedConfig, SeedMode};
pub use routing::{ip_in_cidr, parse_rule_line, port_in_range, RouteAction, RouteRule, RouteSet};
pub use subscription::{
    decode_lines, parse_node_line, parse_nodes, SubscribedNode, SubscriptionEntry,
};
