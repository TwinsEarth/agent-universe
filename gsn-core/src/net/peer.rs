//! 真实 libp2p 对等节点
//!
//! 使用 libp2p 0.54 实现：TCP + Noise 加密 + Yamux 多路复用 + Kademlia DHT + GossipSub
//! v3.9.16: GossipSub 行为参数经 [`crate::net::gossip_config::GossipConfig`]
//! 参数化（mesh / 历史 / 心跳 / 转发 / 去重窗口），入站投递叠加按主题
//! 滑动窗口风暴防护（`GossipStormGuard`）。

use futures::StreamExt;
use libp2p::core::transport::ListenerId;
use libp2p::{
    autonat, dcutr,
    gossipsub::{
        self, ConfigBuilder as GossipsubConfigBuilder, IdentTopic, MessageAuthenticity,
        ValidationMode as GossipsubValidationMode,
    },
    identify::{self, Config as IdentifyConfig},
    identity,
    kad::{self, store::MemoryStore, Config as KadConfig, Quorum, Record, RecordKey},
    ping, relay,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour, Swarm, SwarmEvent},
    Multiaddr, PeerId, SwarmBuilder,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

pub use crate::net::dht_config::{DhtConfig, DhtConfigError, EclipseGuard, EclipseGuardConfig};
pub use crate::net::gossip_config::{GossipConfig, GossipConfigError, GossipStormGuard};
pub use crate::net::routing_persist::{
    ensure_parent_dir, sanitize_entries, RoutingEntry, RoutingTableStore,
};

/// 网络行为组合
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "PeerEvent")]
pub struct PeerBehaviour {
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    /// v2.5.4: Circuit Relay v2 客户端（通过中继节点跨 NAT）
    pub relay_client: relay::client::Behaviour,
    /// v2.5.4: AutoNAT（自动检测 NAT 类型）
    pub autonat: autonat::Behaviour,
    /// v2.5.4: DCUtR（TCP 打洞直连）
    pub dcutr: dcutr::Behaviour,
    /// v2.5.4: Ping（连接保活）
    pub ping: ping::Behaviour,
    /// 自建 Relay 中继服务端（可选）。`Some` 时本节点作为 Circuit Relay v2
    /// 中继（hop），为其他节点提供 reservation / circuit；`None` 时不承载中继。
    /// 使用 `Toggle<B>`：libp2p 0.54 以 `Toggle` 提供「可启停的 behaviour」，
    /// 未启用时该 behaviour 完全缺席、零开销。事件类型为
    /// `relay::Event`（Toggle 透传内部事件）。
    pub relay_server: Toggle<relay::Behaviour>,
}

#[derive(Debug)]
pub enum PeerEvent {
    Kademlia(kad::Event),
    Gossipsub(gossipsub::Event),
    Identify(identify::Event),
    RelayClient(relay::client::Event),
    AutoNat(autonat::Event),
    Dcutr(dcutr::Event),
    Ping(ping::Event),
    /// 自建中继服务端事件；未启用 relay server 时不产生。
    RelayServer(relay::Event),
}

impl From<kad::Event> for PeerEvent {
    fn from(e: kad::Event) -> Self {
        PeerEvent::Kademlia(e)
    }
}
impl From<gossipsub::Event> for PeerEvent {
    fn from(e: gossipsub::Event) -> Self {
        PeerEvent::Gossipsub(e)
    }
}
impl From<identify::Event> for PeerEvent {
    fn from(e: identify::Event) -> Self {
        PeerEvent::Identify(e)
    }
}
impl From<relay::client::Event> for PeerEvent {
    fn from(e: relay::client::Event) -> Self {
        PeerEvent::RelayClient(e)
    }
}
impl From<autonat::Event> for PeerEvent {
    fn from(e: autonat::Event) -> Self {
        PeerEvent::AutoNat(e)
    }
}
impl From<dcutr::Event> for PeerEvent {
    fn from(e: dcutr::Event) -> Self {
        PeerEvent::Dcutr(e)
    }
}
impl From<ping::Event> for PeerEvent {
    fn from(e: ping::Event) -> Self {
        PeerEvent::Ping(e)
    }
}
impl From<relay::Event> for PeerEvent {
    fn from(e: relay::Event) -> Self {
        PeerEvent::RelayServer(e)
    }
}

// ─────────────── P0-2: GossipSub 入站去重与投递 ───────────────

/// 入站 GossipSub 消息（应用层消费单位）。
///
/// swarm 事件流里 `Gossipsub(Event::Message)` 经去重后投递到此结构，
/// 由 daemon 层的消费者任务路由/记录。`source` 为传播者（forwarder）的 PeerId 字符串。
#[derive(Debug, Clone)]
pub struct InboundGossipMessage {
    /// 所属 topic（topic hash 的字符串形式）
    pub topic: String,
    /// 传播该消息的 peer（PeerId 字符串）
    pub source: String,
    /// 消息负载
    pub data: Vec<u8>,
}

/// GossipSub 消息去重器：基于 message id 的有界缓存。
///
/// 用 `HashSet` 判重 + `VecDeque` 保序淘汰：超过容量时丢弃最旧的 id，
/// 避免无界增长。只对「首次见到」的 id 返回 true（放行），重复返回 false（丢弃）。
pub struct GossipDedup {
    seen: HashSet<Vec<u8>>,
    order: VecDeque<Vec<u8>>,
    capacity: usize,
}

impl GossipDedup {
    /// 以指定容量创建去重器（如 4096）。
    pub fn new(capacity: usize) -> Self {
        Self {
            seen: HashSet::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// 记录并判定是否首次见到该消息 id。
    /// 返回 true = 首次（应放行投递）；false = 重复（应丢弃）。
    pub fn first_seen(&mut self, id: &[u8]) -> bool {
        if self.seen.contains(id) {
            return false;
        }
        self.seen.insert(id.to_vec());
        self.order.push_back(id.to_vec());
        // 超出容量淘汰最旧
        while self.order.len() > self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.seen.remove(&old);
            }
        }
        true
    }

    /// 去重器容量（v3.9.16 防风暴参数可观测）。
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// 真实 libp2p 对等节点
pub struct P2pPeer {
    pub peer_id: PeerId,
    pub swarm: Swarm<PeerBehaviour>,
    /// v2.5.3: 已发起过 bootstrap 连接的地址
    bootstrapped: Vec<String>,
    /// v2.5.5: 多 relay 通道映射：relay peer id → circuit listener。
    /// 方案 4：多个 relay 同时 listen、持有多组 reservation，续期前按 relay 主动移除旧 listener。
    relay_listeners: HashMap<String, ListenerId>,
    /// v2.7.5: DCUtR 直连升级成功的对端 peer（经 relay 打洞成功，升级为直连）。
    /// 直连比 relay 中继延迟低、吞吐高；失败时仍回退 relay。
    direct_peers: HashSet<PeerId>,
    /// P0-2: GossipSub 入站消息去重（按 message id）。
    gossip_dedup: GossipDedup,
    /// v3.9.16: 按主题滑动窗口风暴防护（去重之后、投递之前执行）。
    gossip_storm: GossipStormGuard,
    /// v3.9.17: DHT K 桶参数（K 桶大小、查询超时、Eclipse 防护等）。
    dht: DhtConfig,
    /// v3.9.17: Eclipse 防护——限制单一 IP / 子网可登记进 kbucket 的来源数。
    eclipse_guard: EclipseGuard,
}

impl P2pPeer {
    /// 创建新节点，从 ~/.gsn/identity.key 加载身份；不存在则生成并持久化
    pub async fn new() -> anyhow::Result<Self> {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        let dir = std::path::Path::new(&home).join(".gsn");
        let local_key = load_or_create_identity_in(&dir)?;
        Self::with_identity(local_key).await
    }

    /// P0-5: 创建节点，身份持久化到 `<data_dir>/identity.key`。
    ///
    /// 不同 `--data-dir` 的节点拥有不同的 PeerId（同机多节点不再共用同一身份）；
    /// 同一 data-dir 重复启动则确定性恢复同一 PeerId。
    pub async fn with_data_dir(data_dir: &std::path::Path) -> anyhow::Result<Self> {
        let local_key = load_or_create_identity_in(data_dir)?;
        Self::with_identity(local_key).await
    }

    /// 创建节点（可选自建 Relay 中继服务端）。
    ///
    /// `relay_server: Some(config)` 时，本节点同时作为 Circuit Relay v2 中继
    /// （hop）：其他节点可以对其发起 reservation 并经由它建立 circuit，实现
    /// 跨 NAT / 跨网络的 P2P 连接。`None` 时与 `with_data_dir` 完全一致
    /// （普通节点，不承载中继）。
    pub async fn with_data_dir_relay(
        data_dir: &std::path::Path,
        relay_server: Option<relay::Config>,
    ) -> anyhow::Result<Self> {
        let local_key = load_or_create_identity_in(data_dir)?;
        Self::with_identity_relay(local_key, relay_server).await
    }

    /// v3.9.16: 创建节点（可选自建 Relay 中继服务端，GossipSub 行为参数化）。
    ///
    /// 与 `with_data_dir_relay` 完全一致，额外接受 `GossipConfig` 与 `DhtConfig`。
    /// 上层（daemon / 测试）可借此调整 mesh 目标、去重窗口、风暴防护与 DHT K 桶参数。
    pub async fn with_data_dir_relay_gossip_dht(
        data_dir: &std::path::Path,
        relay_server: Option<relay::Config>,
        gossip: GossipConfig,
        dht: DhtConfig,
    ) -> anyhow::Result<Self> {
        let local_key = load_or_create_identity_in(data_dir)?;
        Self::with_identity_relay_gossip_dht(local_key, relay_server, gossip, dht).await
    }

    /// 用指定身份创建节点
    pub async fn with_identity(local_key: identity::Keypair) -> anyhow::Result<Self> {
        Self::with_identity_relay(local_key, None).await
    }

    /// 用指定身份创建节点（可选自建 Relay 中继服务端）。
    pub async fn with_identity_relay(
        local_key: identity::Keypair,
        relay_server: Option<relay::Config>,
    ) -> anyhow::Result<Self> {
        Self::with_identity_relay_gossip_dht(
            local_key,
            relay_server,
            GossipConfig::default(),
            DhtConfig::default(),
        )
        .await
    }

    /// 用指定身份创建节点（可选自建 Relay 中继服务端，可参数化 GossipSub 与 DHT 行为）。
    pub async fn with_identity_relay_gossip_dht(
        local_key: identity::Keypair,
        relay_server: Option<relay::Config>,
        gossip: GossipConfig,
        dht: DhtConfig,
    ) -> anyhow::Result<Self> {
        // v3.9.16: 构造 Behaviour 前先校验参数，非法配置直接拒绝（fail-fast）。
        gossip.validate()?;
        // v3.9.17: DHT 参数同样 fail-fast。
        dht.validate()?;
        let peer_id = PeerId::from(local_key.public());

        let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            // v2.5.5: 叠加 QUIC（UDP），支持 UDP 打洞直连（方案 3），与 TCP 共存。
            // QUIC 属于 OtherTransport 层，必须在 with_dns / with_websocket 之前接入：
            // with_tcp 返回 QuicPhase，其上 with_quic 经 or_transport 叠加 QUIC（保留 TCP）。
            .with_quic()
            // v2.5.4: 启用 DNS 解析（/dns4、/dns6、/dnsaddr），公共 bootstrap 依赖。
            // OtherTransportPhase.with_dns 推进 phase 时保留已组合的 TCP+QUIC，再包上 DNS。
            .with_dns()?
            // v2.5.4: 叠加 WebSocket（/ws、/wss），与 TCP 共存（or_transport）。
            // wss 走 443 端口 + 域名，在受限/国内网络下抗干扰，用于经 relay 稳定建立 reservation。
            .with_websocket(libp2p::noise::Config::new, libp2p::yamux::Config::default)
            .await?
            // v2.5.4: 启用 Circuit Relay v2 客户端，支持通过中继跨 NAT
            .with_relay_client(libp2p::noise::Config::new, libp2p::yamux::Config::default)?
            .with_behaviour(|key, relay_client| {
                // Kademlia DHT（在 Config 上设置查询超时）
                let store = MemoryStore::new(peer_id);
                let mut kad_config = KadConfig::new(libp2p::StreamProtocol::new("/ipfs/kad/1.0.0"));
                // v3.9.17: K 桶加固——查询超时、K 桶插入开关与最大数据包由 DhtConfig 参数化。
                kad_config.set_query_timeout(dht.query_timeout);
                // 0=Manual（仅显式 add_address 入桶）/ >0=OnConnected（连接时自动入桶）。
                kad_config.set_kbucket_inserts(if dht.kbucket_inserts > 0 {
                    kad::BucketInserts::OnConnected
                } else {
                    kad::BucketInserts::Manual
                });
                kad_config.set_max_packet_size(dht.max_packet_size);
                let kademlia = kad::Behaviour::with_config(peer_id, store, kad_config);

                // GossipSub —— v3.9.16: 行为参数由 GossipConfig 提供。
                // mesh 三参数先经 validate() 保证 low <= n <= high；验证模式
                // 从库面枚举映射到 libp2p ValidationMode。
                let validation_mode = match gossip.validation_mode {
                    crate::net::gossip_config::GossipValidationMode::Strict => {
                        GossipsubValidationMode::Strict
                    }
                    crate::net::gossip_config::GossipValidationMode::Anonymous => {
                        GossipsubValidationMode::Anonymous
                    }
                    crate::net::gossip_config::GossipValidationMode::Permissive => {
                        GossipsubValidationMode::Permissive
                    }
                };
                let gossipsub_config = GossipsubConfigBuilder::default()
                    .heartbeat_interval(gossip.heartbeat_interval)
                    .mesh_n(gossip.mesh_n)
                    .mesh_n_low(gossip.mesh_n_low)
                    .mesh_n_high(gossip.mesh_n_high)
                    .gossip_lazy(gossip.gossip_lazy)
                    .gossip_factor(gossip.gossip_factor)
                    .history_length(gossip.history_length)
                    .history_gossip(gossip.history_gossip)
                    .fanout_ttl(gossip.fanout_ttl)
                    .max_transmit_size(gossip.max_transmit_size)
                    .validation_mode(validation_mode)
                    .prune_peers(gossip.prune_peers)
                    .opportunistic_graft_ticks(gossip.opportunistic_graft_ticks)
                    .build()
                    .map_err(|e| anyhow::anyhow!("invalid gossipsub config: {e}"))?;
                let gossipsub = gossipsub::Behaviour::new(
                    MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .map_err(|e| anyhow::anyhow!("invalid gossipsub: {e}"))?;

                // Identify
                let identify = identify::Behaviour::new(IdentifyConfig::new(
                    format!("/gsn/{}", env!("CARGO_PKG_VERSION")),
                    key.public(),
                ));

                // AutoNAT：自动检测 NAT 状态
                let autonat_config = autonat::Config {
                    retry_interval: Duration::from_secs(10),
                    refresh_interval: Duration::from_secs(30),
                    confidence_max: 1,
                    ..Default::default()
                };
                let autonat = autonat::Behaviour::new(peer_id, autonat_config);

                // DCUtR：TCP 打洞
                let dcutr = dcutr::Behaviour::new(peer_id);

                // Ping：连接保活
                let ping = ping::Behaviour::new(
                    ping::Config::new().with_interval(Duration::from_secs(15)),
                );

                // 自建 Relay 中继服务端（可选）：`relay_server` 由构造参数传入。
                // 注意：relay::Behaviour 即 Circuit Relay v2 服务端（libp2p-relay
                // 顶层类型）；Toggle 用于「未启用时行为完全缺席」。
                let relay_server =
                    Toggle::from(relay_server.map(|cfg| relay::Behaviour::new(peer_id, cfg)));

                Ok(PeerBehaviour {
                    kademlia,
                    gossipsub,
                    identify,
                    relay_client,
                    autonat,
                    dcutr,
                    ping,
                    relay_server,
                })
            })?
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        // v2.5.5: 同时监听 QUIC/UDP（随机端口），节点对外具备 UDP/QUIC 地址，可用于打洞
        swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;

        let mut me = Self {
            peer_id,
            swarm,
            bootstrapped: Vec::new(),
            relay_listeners: HashMap::new(),
            direct_peers: HashSet::new(),
            gossip_dedup: GossipDedup::new(gossip.dedup_capacity),
            gossip_storm: GossipStormGuard::from_config(&gossip),
            dht: dht.clone(),
            eclipse_guard: EclipseGuard::new(dht.eclipse.clone()),
        };
        // v3.9.17: 路由表回填——若配置了持久化路径，启动时把旧快照（经 K=8 卫生化）
        // 逐条 add_address 进 kbucket，使节点重启后无需从零收敛 DHT。
        if let Some(persist_path) = dht.routing_persist_path.clone() {
            match RoutingTableStore::new(persist_path.clone()).load() {
                Ok(raw) => {
                    let entries = sanitize_entries(raw, 8);
                    if !entries.is_empty() {
                        for e in entries {
                            for a in &e.addrs {
                                if let Ok(pid) = e.peer_id.parse::<PeerId>() {
                                    if let Ok(addr) = a.parse::<Multiaddr>() {
                                        let _ = me
                                            .swarm
                                            .behaviour_mut()
                                            .kademlia
                                            .add_address(&pid, addr);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(path = ?persist_path, err = %err, "dht: 路由表快照加载失败（忽略，从零自举）");
                }
            }
        }
        Ok(me)
    }

    /// v2.5.5: 在指定 UDP 端口监听 QUIC（与 TCP P2P 同端口，便于固定映射/打洞）
    pub fn listen_quic_port(&mut self, port: u16) -> anyhow::Result<Multiaddr> {
        let addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", port).parse()?;
        self.swarm.listen_on(addr.clone())?;
        Ok(addr)
    }

    /// 在指定端口监听
    pub fn listen_on_port(&mut self, port: u16) -> anyhow::Result<Multiaddr> {
        let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", port).parse()?;
        self.swarm.listen_on(addr.clone())?;
        Ok(addr)
    }

    /// 添加 bootstrap 节点（multiaddr）
    pub fn add_bootstrap(&mut self, addr: Multiaddr) -> anyhow::Result<()> {
        self.swarm.dial(addr)?;
        Ok(())
    }

    /// DHT 写入键值
    pub fn dht_put(&mut self, key: &[u8], value: Vec<u8>) -> anyhow::Result<kad::QueryId> {
        let record = Record::new(key.to_vec(), value);
        let qid = self
            .swarm
            .behaviour_mut()
            .kademlia
            .put_record(record, Quorum::One)?;
        Ok(qid)
    }

    /// DHT 查询键值
    pub fn dht_get(&mut self, key: &[u8]) -> kad::QueryId {
        self.swarm
            .behaviour_mut()
            .kademlia
            .get_record(RecordKey::new(&key))
    }

    /// 开始向 DHT 声明提供某内容
    pub fn start_providing(&mut self, key: &[u8]) -> anyhow::Result<()> {
        self.swarm
            .behaviour_mut()
            .kademlia
            .start_providing(RecordKey::new(&key))?;
        Ok(())
    }

    /// 订阅 GossipSub 主题
    pub fn subscribe(&mut self, topic: &str) -> anyhow::Result<()> {
        let t = IdentTopic::new(topic);
        self.swarm.behaviour_mut().gossipsub.subscribe(&t)?;
        Ok(())
    }

    /// 发布 GossipSub 消息
    pub fn publish(&mut self, topic: &str, data: Vec<u8>) -> anyhow::Result<()> {
        let t = IdentTopic::new(topic);
        self.swarm.behaviour_mut().gossipsub.publish(t, data)?;
        Ok(())
    }

    /// 发布 GossipSub 消息，但保留具体的 gossipsub 错误（供调用方区分
    /// `InsufficientPeers` 并做重试，而不是被 anyhow 抹平后无法判断）。
    pub fn try_publish(
        &mut self,
        topic: &str,
        data: Vec<u8>,
    ) -> Result<(), gossipsub::PublishError> {
        let t = IdentTopic::new(topic);
        self.swarm.behaviour_mut().gossipsub.publish(t, data)?;
        Ok(())
    }

    /// 本节点期望订阅的 GossipSub 主题（应用层固定，CRDT 主题为 "gsn/crdt"）。
    pub const GSN_GOSSIP_TOPICS: [&str; 3] = ["gsn/agents", "gsn/tasks", "gsn/crdt"];

    /// 连接建立后重放订阅，解决 GossipSub 冷启动死锁。
    ///
    /// 背景（libp2p-gossipsub 0.47）：节点启动、尚无邻居时调用 `subscribe()`，
    /// `join()` 因 `topic_peers` 为空选不到任何节点，**不会把主题放进 mesh**。
    /// 之后新连接建立时，`on_connection_established` 只对“已在 mesh”的主题
    /// 重发 SUBSCRIBE（见 gossipsub behaviour.rs 中 `for topic in self.mesh`），
    /// 于是双方都不向对方宣告主题，`topic_peers` 永远为空，`publish` 返回
    /// `InsufficientPeers`——典型的“连得上、消息发不出”冷启动死锁。
    ///
    /// 在每条新连接建立后重新调用 `subscribe`（幂等：已在 mesh 返回 Ok(false)）：
    /// 若尚未入 mesh，会向新连接对端发送 SUBSCRIBE（对端把本节点记入 topic_peers），
    /// 随后 `join()` 即可从 topic_peers 选中该对端并 GRAFT 组建 mesh。
    pub fn refresh_gossipsub_subscriptions(&mut self) {
        for t in Self::GSN_GOSSIP_TOPICS {
            if let Err(e) = self.subscribe(t) {
                tracing::debug!(topic = t, error = %e, "refresh subscribe 失败");
            }
        }
    }

    /// 主动查找节点（DHT 节点发现）
    pub fn find_peer(&mut self, peer: PeerId) {
        self.swarm.behaviour_mut().kademlia.get_closest_peers(peer);
    }

    /// v5.2.2: 随机 key 的 GetClosestPeers 查找。
    ///
    /// 以自身 PeerId 为 key 时，每次返回的都是"距自身 XOR 最近"的同一批
    /// K（=20）个节点，路由表扩展一轮即饱和（50 节点实测 max=19 停住）；
    /// 改用随机 key 使每轮查询返回不同的节点子集，冷启动多轮自举可累积
    /// 覆盖全网，配合 `dial_learned_peers` 把学到的节点逐一补齐连接。
    pub fn find_random_peer(&mut self) {
        let mut key = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut key);
        tracing::info!("dht: 发起随机 key GetClosestPeers 查询（迭代扩展路由）");
        self.swarm
            .behaviour_mut()
            .kademlia
            .get_closest_peers(key.to_vec());
    }

    /// v5.2.2: 以「kbucket 中随机一个已知 peer」为 key 的 GetClosestPeers 查询。
    ///
    /// 纯随机 key 查询只会问"距随机 key XOR 最近"的 3 个节点，冷启动时它们
    /// 常常都不含信息枢纽（boot），导致每轮只学到同一批直连邻居、路由表
    /// 停在 ~8 条不扩展（12/50 节点实测 learned 恒定 7/8 的死锁）。
    /// 以已知 peer 的 PeerId 字节作 key 时，该 peer 距自己 XOR 距离为 0、
    /// 必然被选为初始查询目标，于是每轮都向"某个已连接节点"索取它的整张
    /// 路由表；配合 `dial_learned_peers` 把返回的节点补齐连接，多轮覆盖
    /// 全网。kbucket 为空时静默跳过（冷启动首轮由纯随机 key 查询兜底）。
    pub fn find_peer_query(&mut self) {
        use rand::seq::IteratorRandom;
        let mut rng = rand::thread_rng();
        let mut picked: Option<libp2p::PeerId> = None;
        for bucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
            if let Some(entry) = bucket.iter().choose(&mut rng) {
                picked = Some(*entry.node.key.preimage());
                break;
            }
        }
        if let Some(peer_id) = picked {
            tracing::info!(%peer_id, "dht: 以已知 peer 为 key 发起 GetClosestPeers 查询（信息富集）");
            self.swarm
                .behaviour_mut()
                .kademlia
                .get_closest_peers(peer_id.to_bytes());
        } else {
            tracing::info!("dht: kbucket 为空，跳过 peer-key 查询（由随机 key 查询兜底）");
        }
    }

    /// P0-1: 把一个已连接 peer 的对端地址登记进 Kademlia kbucket。
    ///
    /// dial 成功只会建立传输连接，**不会**自动让 Kademlia 把对端纳入路由表；
    /// 必须显式 `kademlia.add_address(peer, addr)`，否则 kbucket 恒空、
    /// `get_closest_peers` 无路由可走。这里在 ConnectionEstablished / Identify.Received
    /// 时调用。返回值（RoutingUpdate）可忽略，仅记 debug。
    pub fn add_kad_address(&mut self, peer_id: PeerId, addr: Multiaddr) {
        // v3.9.17: Eclipse 防护——超限来源（单 IP / 子网）保持连接但不登记进 kbucket。
        if !self.eclipse_guard.accept(&peer_id, &addr) {
            tracing::debug!(%peer_id, %addr, "dht: Eclipse 防护拒绝登记该来源（同 IP/子网已达上限）");
            return;
        }
        let update = self
            .swarm
            .behaviour_mut()
            .kademlia
            .add_address(&peer_id, addr.clone());
        tracing::debug!(%peer_id, %addr, ?update, "kademlia add_address");
    }

    /// v5.2.2: Identify 失败补救——断开已建立的连接并按 kbucket 已知地址重拨，
    /// 触发新一轮 Identify（50 节点冷启动并发风暴下部分入站 Identify 流超时/断连）。
    /// kbucket 里无该 peer 地址时静默跳过（等对方重拨/自举再学）。
    pub fn redial_peer(&mut self, peer_id: PeerId) {
        let mut addr: Option<Multiaddr> = None;
        for bucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
            for entry in bucket.iter() {
                if *entry.node.key.preimage() == peer_id {
                    if let Some(a) = entry.node.value.iter().next() {
                        addr = Some(a.clone());
                    }
                    break;
                }
            }
            if addr.is_some() {
                break;
            }
        }
        if let Some(a) = addr {
            let _ = self.swarm.disconnect_peer_id(peer_id);
            match self.swarm.dial(a) {
                Ok(()) => tracing::info!(%peer_id, "dht: identify 补救重拨已发起"),
                Err(e) => tracing::debug!(%peer_id, error = %e, "dht: identify 补救重拨未发起"),
            }
        }
    }
    ///
    /// 迭代 FIND_NODE 每跳会从响应里学到新节点（PeerInfo 自带其可拨地址），
    /// 但 libp2p 的 Kademlia 查询只向**已有连接**的对端发请求；学到的节点
    /// 若不主动 dial，后续跳数无法向它们追问，路由表会停留在"直连 + 一跳"
    /// 而不会扩展为全网 K 邻居。这正是规模回归（50 节点 K=20）观察到的现象：
    /// 每节点路由表仅 9~17 条且不再增长。本方法对未连接的学习节点发起 dial，
    /// 连接建立后由 ConnectionEstablished / Identify 臂补 add_address，迭代得以
    /// 继续扩展；已在连接中的节点直接跳过。
    pub fn dial_learned_peers(&mut self, peers: &[libp2p::kad::PeerInfo]) {
        // v5.2.2: 每轮最多拨 8 个新 peer——50 节点冷启动时一次学 49 个
        // 若全量并发 dial 会形成拨号风暴（实测 50 节点 median=1：几乎全失败、
        // 路由表不扩展）；改成每轮小步拨号 + 跳过已连接，靠多轮自举
        // （15s/30s/60s/120s）分批累积，网络压力小、成功率升。
        const MAX_DIAL_PER_ROUND: usize = 4;
        let mut dialed = 0usize;
        'outer: for info in peers {
            if dialed >= MAX_DIAL_PER_ROUND {
                break;
            }
            if self.swarm.is_connected(&info.peer_id) {
                continue;
            }
            for addr in &info.addrs {
                // 只为可拨打的 TCP 地址发起 dial（避免把 /p2p-circuit 等
                // 无中继段地址当直连目标）；失败静默，交给下一跳重学。
                if addr
                    .iter()
                    .any(|p| matches!(p, libp2p::multiaddr::Protocol::Tcp(_)))
                {
                    // v5.2.2: 先登记进 kbucket（Eclipse 防护内、尽力而为）——
                    // 让下一轮 GetClosestPeers 查询能把请求发到这些节点，迭代
                    // 扩展不依赖本轮 dial 是否成功；dial 成功后 OnConnected 会再入桶。
                    self.add_kad_address(info.peer_id, addr.clone());
                    match self.swarm.dial(addr.clone()) {
                        Ok(()) => {
                            dialed += 1;
                            // 该 peer 已发起拨号，跳过其其余地址，处理下一个 peer。
                            continue 'outer;
                        }
                        Err(e) => {
                            tracing::debug!(%addr, error = %e, "dht: 学习节点 dial 未建立（下一跳重学）");
                        }
                    }
                }
            }
        }
        if dialed > 0 {
            tracing::info!(count = dialed, "dht: 已对学习节点发起补齐连接");
        }
    }

    /// v3.9.17: 导出路由表快照（peer id → 已知地址），用于持久化。
    pub fn routing_table_snapshot(&mut self) -> Vec<RoutingEntry> {
        let mut entries = Vec::new();
        for bucket in self.swarm.behaviour_mut().kademlia.kbuckets() {
            for pid in bucket.iter() {
                let addrs: Vec<String> = pid.node.value.iter().map(|a| a.to_string()).collect();
                if addrs.is_empty() {
                    continue;
                }
                entries.push(RoutingEntry {
                    peer_id: pid.node.key.preimage().to_string(),
                    addrs,
                });
            }
        }
        entries
    }

    /// v3.9.17: 把当前路由表快照原子落盘。未配置持久化路径时静默返回 Ok。
    pub fn persist_routing_table(&mut self) -> Result<(), String> {
        let Some(path) = self.dht.routing_persist_path.clone() else {
            return Ok(());
        };
        ensure_parent_dir(&path)?;
        let store = RoutingTableStore::new(path);
        let entries = self.routing_table_snapshot();
        store.save(&entries)
    }

    /// P0-2: 查询 GossipDedup 是否首次见到该 message id（首次记录并返回 true=放行）。
    pub fn gossip_dedup_first_seen(&mut self, message_id: &gossipsub::MessageId) -> bool {
        self.gossip_dedup.first_seen(&message_id.0)
    }

    /// v3.9.16: 按主题滑动窗口风暴防护。去重放行后调用；返回 false 表示
    /// 该主题当前窗口已超限，应丢弃（防御单主题突发风暴）。
    pub fn gossip_storm_allow(&mut self, topic: &str) -> bool {
        self.gossip_storm.allow(topic)
    }

    /// v2.5.5: 触发一次 DHT 随机节点发现（随机 Peer 查 closest peers），
    /// 扩充 kbucket；新节点经 Identify 上报协议，支持 hop 的会被纳入 relay 候选池。
    pub fn discover_random_peers(&mut self) {
        self.swarm
            .behaviour_mut()
            .kademlia
            .get_closest_peers(PeerId::random());
    }

    /// P0-1: 运行 Kademlia 标准自举（bootstrap）。
    ///
    /// 仅仅 `add_address(boot)` 只会让本节点 kbucket 里有“引导节点”这一条；
    /// 必须再调一次 `kademlia.bootstrap()`，它以本节点 PeerId 为目标发起
    /// 迭代 FIND_NODE 查询：向引导节点问“离我最近的节点”，拿到更近的
    /// 节点后继续追问，并把每一跳成功学到的节点纳入 kbucket——这才让
    /// 每个新节点的路由表从“只认识 boot”扩展为“持有全网 K 个邻居”。
    /// 须在 kbucket 已至少有 1 个 peer（add_address 之后）调用，否则返回
    /// `NoKnownPeers`。这里把该错误转成字符串，调用方据此决定稍后重试。
    pub fn dht_bootstrap(&mut self) -> Result<(), String> {
        match self.swarm.behaviour_mut().kademlia.bootstrap() {
            Ok(_qid) => Ok(()),
            Err(e) => Err(format!("{e:?}")),
        }
    }

    /// P0-1: 把 Kademlia 显式置为 Server 模式，使本节点接受入站 kad 查询并
    /// 在 Identify 中广播 `/ipfs/kad/1.0.0`。
    ///
    /// libp2p-kad 0.46 默认是 **Client** 模式，只有在“确认了外部地址”
    /// （`ExternalAddrConfirmed`）后才自动切换为 Server；Client 模式下
    /// 所有 kad handler 的 `listen_protocol` 用 `DeniedUpgrade`——既不响应
    /// 入站 FIND_NODE，也不向 Identify 注册 kad 协议，于是任何 DHT 查询都
    /// 返回 "protocol not supported"，路由表永远无法在节点间扩散。
    /// 对全节点/归档节点（应当服务 DHT），在启动时显式置 Server 是标准做法。
    /// 轻/边缘节点可保持 Client（只发起查询、不服务）。
    pub fn set_kad_server_mode(&mut self) {
        self.swarm
            .behaviour_mut()
            .kademlia
            .set_mode(Some(kad::Mode::Server));
    }

    /// 对任意 key 发起迭代 Kademlia 查找（get_closest_peers），返回该查询的
    /// QueryId。结果通过 kad::Event::OutboundQueryProgressed
    /// （QueryResult::GetClosestPeers）异步返回。这是“跨节点查找”，会沿
    /// kbucket 多跳迭代——Kademlia 查找的迭代次数对节点数 n 有 O(log n) 上界。
    pub fn find_closest_peers(&mut self, key: &str) -> kad::QueryId {
        self.swarm
            .behaviour_mut()
            .kademlia
            .get_closest_peers(key.as_bytes().to_vec())
    }

    /// 轮询一次 swarm 事件（swarm 由 &mut self 持有，事件流结束仅理论上可能）
    pub async fn next_event(&mut self) -> Option<SwarmEvent<PeerEvent>> {
        self.swarm.next().await
    }

    /// 当前已连接的 peer 数
    pub fn connected_peers(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// DHT kbucket 条目数
    pub fn routing_table_size(&mut self) -> usize {
        self.swarm
            .behaviour_mut()
            .kademlia
            .kbuckets()
            .map(|b| b.num_entries())
            .sum()
    }

    // ─────────────── v2.5.3 网络增强 ───────────────

    /// 本机监听地址列表
    pub fn listen_addrs(&self) -> Vec<String> {
        self.swarm.listeners().map(|a| a.to_string()).collect()
    }

    /// 已发起 bootstrap 连接的地址列表
    pub fn bootstrapped(&self) -> Vec<String> {
        self.bootstrapped.clone()
    }

    /// 从字符串 multiaddr 发起 bootstrap 连接
    pub fn add_bootstrap_from_str(&mut self, addr: &str) -> Result<(), String> {
        let multi: Multiaddr = addr
            .parse()
            .map_err(|e| format!("multiaddr 解析失败: {}", e))?;
        self.swarm
            .dial(multi)
            .map_err(|e| format!("dial 失败: {}", e))?;
        self.bootstrapped.push(addr.to_string());
        Ok(())
    }

    /// v2.5.5: 经 Circuit Relay v2 中继监听（多通道版）。
    ///
    /// 以 relay 的 Peer ID 为 key：同一 relay 重复调用（80s 续期）会先移除该 relay
    /// 的旧 circuit listener 再重新 listen，避免地址累积；不同 relay 各自独立持有
    /// listener 与 reservation，从而同时在线多条中继通道（方案 4）。
    ///
    /// 入参 `relay_addr` 为中继节点 multiaddr（含 /p2p/<relay_peer>），
    /// 方法自动追加 `/p2p-circuit`。
    pub fn listen_via_relay(&mut self, relay_addr: &str) -> Result<(), String> {
        let base: Multiaddr = relay_addr
            .parse()
            .map_err(|e| format!("relay multiaddr 解析失败: {}", e))?;
        let relay_key = extract_relay_peer_id(&base).unwrap_or_else(|| relay_addr.to_string());
        let circuit = base.with(libp2p::multiaddr::Protocol::P2pCircuit);

        // 该 relay 已有 listener（续期场景）→ 先移除，保证每个 relay 恒为 1 个 listener
        if let Some(old) = self.relay_listeners.remove(&relay_key) {
            let removed = self.swarm.remove_listener(old);
            eprintln!(
                "🧹 移除 relay {} 旧 listener (removed={})",
                relay_key, removed
            );
        }
        let listener_id = self
            .swarm
            .listen_on(circuit)
            .map_err(|e| format!("relay listen 失败: {}", e))?;
        self.relay_listeners.insert(relay_key, listener_id);
        Ok(())
    }

    /// v2.5.5: 关闭并移除某一条 relay 通道（传入 relay Peer ID 或完整 multiaddr）
    pub fn remove_relay(&mut self, relay_addr_or_id: &str) -> bool {
        if let Some(old) = self.relay_listeners.remove(relay_addr_or_id) {
            return self.swarm.remove_listener(old);
        }
        if let Ok(base) = relay_addr_or_id.parse::<Multiaddr>() {
            if let Some(key) = extract_relay_peer_id(&base) {
                if let Some(old) = self.relay_listeners.remove(&key) {
                    return self.swarm.remove_listener(old);
                }
            }
        }
        false
    }

    /// v2.5.5: 仅 dial 候选 relay（不 listen、不建 reservation），用于健康探测；
    /// dial 成功后 Identify 事件会报告其支持的协议，据此判断是否提供 hop。
    pub fn probe_relay(&mut self, relay_addr: &str) -> Result<(), String> {
        let base: Multiaddr = relay_addr
            .parse()
            .map_err(|e| format!("relay multiaddr 解析失败: {}", e))?;
        self.swarm
            .dial(base)
            .map_err(|e| format!("probe dial 失败: {}", e))?;
        Ok(())
    }

    /// v2.5.5: 当前持有 circuit listener 的 relay Peer ID 列表（多通道）
    pub fn active_relay_ids(&self) -> Vec<String> {
        self.relay_listeners.keys().cloned().collect()
    }

    /// v2.7.5: 记录 DCUtR 事件——result.is_ok() 表示经 relay 打洞成功、升级为直连。
    /// 返回 true 表示本次是新升级成功（用于日志/指标）。
    pub fn note_dcutr_event(&mut self, e: &dcutr::Event) -> bool {
        match &e.result {
            Ok(_conn_id) => self.direct_peers.insert(e.remote_peer_id),
            Err(_) => false,
        }
    }

    /// v2.7.5: 已 DCUtR 直连升级成功的对端 peer 列表
    pub fn direct_peer_ids(&self) -> Vec<String> {
        self.direct_peers.iter().map(|p| p.to_string()).collect()
    }

    /// 已连接对等节点的 peer_id 列表
    pub fn connected_peer_ids(&self) -> Vec<String> {
        self.swarm
            .connected_peers()
            .map(|peer_id| peer_id.to_string())
            .collect()
    }

    /// v3.9.21: 当前持有的 relay circuit listener 通道数（relay_pool 多冗余 failover 实况）。
    pub fn relay_channel_count(&self) -> usize {
        self.relay_listeners.len()
    }

    /// v3.9.21: DCUtR 直连升级成功的对端数（NAT 穿透效果可观测）。
    pub fn dcutr_direct_count(&self) -> usize {
        self.direct_peers.len()
    }

    /// v3.9.21: GossipSub 去重器容量（防风暴参数可观测）。
    pub fn gossip_dedup_capacity(&self) -> usize {
        self.gossip_dedup.capacity()
    }

    // ─────────────── v2.5.4 NAT 穿透增强 ───────────────

    /// 连接公共 libp2p 网络：bootstrap 节点 + Circuit Relay 中继
    ///
    /// 公共节点同时支持 DHT 路由发现和 relay 中继，
    /// 节点在 NAT 后可通过中继被其他节点 dial 到。
    pub fn bootstrap_public_network(&mut self) -> Vec<String> {
        // libp2p 官方公共 bootstrap/relay 节点
        let public_addrs: Vec<&str> = vec![
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
        ];

        let mut initiated = Vec::new();
        for addr_str in public_addrs {
            if let Ok(multi) = addr_str.parse::<Multiaddr>() {
                if self.swarm.dial(multi).is_ok() {
                    initiated.push(addr_str.to_string());
                    self.bootstrapped.push(addr_str.to_string());
                }
            }
        }
        initiated
    }

    /// AutoNAT 检测到的 NAT 状态
    pub fn nat_status(&self) -> String {
        match self.swarm.behaviour().autonat.nat_status() {
            autonat::NatStatus::Unknown => "unknown".to_string(),
            autonat::NatStatus::Public(_) => "public".to_string(),
            autonat::NatStatus::Private => "private_nat".to_string(),
        }
    }

    /// 已连接的中继节点数量（relay client 持有的 reservation）
    pub fn connected_relays(&self) -> Vec<String> {
        // relay client 通过 reservation 保持与中继的连接，
        // 这些 peer 出现在 swarm connected_peers 中
        self.swarm
            .connected_peers()
            .map(|peer_id| peer_id.to_string())
            .collect()
    }
}

/// v2.5.5: 从 multiaddr 中提取 relay 的 Peer ID（/p2p/<PeerId>）
pub fn extract_relay_peer_id(addr: &Multiaddr) -> Option<String> {
    addr.iter().find_map(|p| match p {
        libp2p::multiaddr::Protocol::P2p(pid) => Some(pid.to_string()),
        _ => None,
    })
}

/// v2.5.3: 从磁盘加载 libp2p 身份密钥；不存在则生成 Ed25519 并保存。
///
/// 持久化路径: `~/.gsn/identity.key`（protobuf 编码）。向后兼容；
/// 新代码请用 [`load_or_create_identity_in`] 以跟随 `--data-dir`。
/// 保证同一台机器跨重启 Peer ID 稳定，DID 绑定可保持。
pub fn load_or_create_identity() -> anyhow::Result<identity::Keypair> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = std::path::Path::new(&home).join(".gsn");
    load_or_create_identity_in(&dir)
}

/// P0-5: 从指定目录加载 libp2p 身份密钥；不存在则生成 Ed25519 并保存到
/// `<dir>/identity.key`（protobuf 编码）。
///
/// - 目录不存在会 `create_dir_all` 创建；
/// - 同一目录重复调用确定性恢复同一 PeerId；不同目录得到不同 PeerId；
/// - Unix 下新建文件权限设为 `0600`（仅属主可读写，私钥不暴露）。
///
/// 密码学说明：仅复用 libp2p/ed25519 自带的密钥生成与 protobuf 编解码，
/// 未自造任何密码学原语。【密钥持久化改动需外部审计】
pub fn load_or_create_identity_in(dir: &std::path::Path) -> anyhow::Result<identity::Keypair> {
    std::fs::create_dir_all(dir)?;
    let key_path = dir.join("identity.key");

    if key_path.exists() {
        let bytes = std::fs::read(&key_path)?;
        match identity::Keypair::from_protobuf_encoding(&bytes) {
            Ok(kp) => return Ok(kp),
            Err(e) => {
                eprintln!("⚠️ identity.key 损坏，重新生成: {}", e);
            }
        }
    }

    let kp = identity::Keypair::generate_ed25519();
    let encoded = kp.to_protobuf_encoding()?;
    std::fs::write(&key_path, &encoded)?;
    // Unix 下收紧权限到 0600（私钥不应被同机其他用户读取）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(kp)
}

// ───────────────────────── P0 修复的本地验证测试 ─────────────────────────

#[cfg(test)]
mod p0_tests {
    use super::*;
    use libp2p::gossipsub::Event as GsEvent;
    use libp2p::swarm::SwarmEvent;
    use std::time::{Duration, Instant, SystemTime};

    /// 从 Multiaddr 中取出 TCP 端口（QUIC 走 UDP，不会命中）。
    fn tcp_port_of(addr: &Multiaddr) -> Option<u16> {
        addr.iter().find_map(|p| match p {
            libp2p::multiaddr::Protocol::Tcp(port) => Some(port),
            _ => None,
        })
    }

    /// 轮询 swarm 直到拿到 TCP listener 的真实绑定地址（端口 0 由内核分配，
    /// 必须经 NewListenAddr 事件才能得知，不能同步读 listeners()）。
    async fn wait_tcp_listen_addr(a: &mut P2pPeer) -> Multiaddr {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match a.next_event().await {
                Some(SwarmEvent::NewListenAddr { address, .. }) => {
                    if tcp_port_of(&address).is_some() {
                        return address;
                    }
                }
                Some(_) => {}
                None => {}
            }
        }
        panic!("等待 TCP NewListenAddr 超时");
    }

    // ── P0-1: dial 成功 + add_address → kbucket 非空 ──
    #[tokio::test]
    async fn p01_dht_add_address_populates_kbucket() {
        let mut a = P2pPeer::with_identity(identity::Keypair::generate_ed25519())
            .await
            .unwrap();
        let mut b = P2pPeer::with_identity(identity::Keypair::generate_ed25519())
            .await
            .unwrap();
        // 先等 A 的 TCP 端口绑定完成
        let listen = wait_tcp_listen_addr(&mut a).await;
        let port = tcp_port_of(&listen).unwrap();
        let a_peer = a.peer_id;

        let dial: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", port, a_peer)
            .parse()
            .unwrap();
        b.add_bootstrap(dial).unwrap();

        let deadline = Instant::now() + Duration::from_secs(15);
        while b.routing_table_size() < 1 && Instant::now() < deadline {
            tokio::select! {
                ev = a.next_event() => {
                    if let Some(SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. }) = ev {
                        a.add_kad_address(peer_id, endpoint.get_remote_address().clone());
                    }
                }
                ev = b.next_event() => {
                    if let Some(SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. }) = ev {
                        b.add_kad_address(peer_id, endpoint.get_remote_address().clone());
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }
        assert!(
            b.routing_table_size() >= 1,
            "B 的 kbucket 应在 add_address 后包含 A，实际 size={}",
            b.routing_table_size()
        );
    }

    // ── P0-1(Server 模式): 两端均 Server，B 对 A 的 bootstrap 查询必须成功
    //    （而非 Client 模式下的 "protocol not supported"）。
    #[tokio::test]
    async fn p01_server_mode_serves_inbound_kad_query() {
        let mut a = P2pPeer::with_identity(identity::Keypair::generate_ed25519())
            .await
            .unwrap();
        let mut b = P2pPeer::with_identity(identity::Keypair::generate_ed25519())
            .await
            .unwrap();
        // 两端都显式 Server（生产中 full/archive 节点在 run_daemon 里这样设置）。
        a.set_kad_server_mode();
        b.set_kad_server_mode();
        let listen = wait_tcp_listen_addr(&mut a).await;
        let port = tcp_port_of(&listen).unwrap();
        let a_peer = a.peer_id;
        let dial: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", port, a_peer)
            .parse()
            .unwrap();
        b.add_bootstrap(dial).unwrap();

        // 驱动连接，双方都把对端登记进 kbucket。
        // 必须用 select 并发驱动 A、B：任一侧暂时没有事件都不能阻塞另一侧，
        // 否则在高负载 CI 上会把对端已到达的事件饿死（对齐同模块
        // p01_dht_add_address_populates_kbucket 的成熟写法）。
        let conn_deadline = Instant::now() + Duration::from_secs(15);
        while b.routing_table_size() < 1 && Instant::now() < conn_deadline {
            tokio::select! {
                ev = b.next_event() => {
                    if let Some(SwarmEvent::ConnectionEstablished {
                        peer_id, endpoint, ..
                    }) = ev
                    {
                        b.add_kad_address(peer_id, endpoint.get_remote_address().clone());
                    }
                }
                ev = a.next_event() => {
                    if let Some(SwarmEvent::ConnectionEstablished {
                        peer_id, endpoint, ..
                    }) = ev
                    {
                        if endpoint.is_dialer() {
                            a.add_kad_address(peer_id, endpoint.get_remote_address().clone());
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }
        assert!(b.routing_table_size() >= 1, "B 应先与 A 建立连接");

        // B 发起 bootstrap 查询。同样用 select 并发驱动 A（让它响应入站查询）
        // 与 B（收查询结果）：不能先阻塞等 A 的事件再读 B，否则 A 安静时
        // B 已到达的 Bootstrap(Ok) 会被饿死，在高负载 runner 上表现为偶发超时。
        b.dht_bootstrap().unwrap();
        let mut saw_success = false;
        let q_deadline = Instant::now() + Duration::from_secs(20);
        while !saw_success && Instant::now() < q_deadline {
            tokio::select! {
                _ = a.next_event() => {}
                ev = b.next_event() => {
                    if let Some(SwarmEvent::Behaviour(PeerEvent::Kademlia(
                        libp2p::kad::Event::OutboundQueryProgressed {
                            result: libp2p::kad::QueryResult::Bootstrap(Ok(_)),
                            ..
                        },
                    ))) = ev
                    {
                        saw_success = true;
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }
        assert!(saw_success, "Server 模式下 B 对 A 的 bootstrap 查询应成功");
    }

    // ── P0-2: 两个真实 swarm 互发 GossipSub，B 收到且 data 一致 ──
    #[tokio::test]
    async fn p02_gossipsub_delivers_between_real_swarms() {
        let mut a = P2pPeer::with_identity(identity::Keypair::generate_ed25519())
            .await
            .unwrap();
        let mut b = P2pPeer::with_identity(identity::Keypair::generate_ed25519())
            .await
            .unwrap();
        let listen = wait_tcp_listen_addr(&mut a).await;
        let port = tcp_port_of(&listen).unwrap();
        let a_peer = a.peer_id;
        let topic = "gsn.test.p02.delivery";
        a.subscribe(topic).unwrap();
        b.subscribe(topic).unwrap();
        let dial: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", port, a_peer)
            .parse()
            .unwrap();
        b.add_bootstrap(dial).unwrap();

        let payload = b"p02-unique-payload-0xDeadBeef".to_vec();
        let mut received: Option<Vec<u8>> = None;
        let deadline = Instant::now() + Duration::from_secs(20);
        while received.is_none() && Instant::now() < deadline {
            tokio::select! {
                ev = a.next_event() => {
                    if let Some(SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. }) = ev {
                        a.add_kad_address(peer_id, endpoint.get_remote_address().clone());
                    }
                }
                ev = b.next_event() => {
                    if let Some(e) = ev {
                        if let SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } = &e {
                            b.add_kad_address(*peer_id, endpoint.get_remote_address().clone());
                        }
                        if let SwarmEvent::Behaviour(PeerEvent::Gossipsub(GsEvent::Message { message, .. })) = &e {
                            received = Some(message.data.clone());
                        }
                    }
                }
                // 周期性重发：等双方订阅关系交换后，下一次 publish 即被 B 收到
                _ = tokio::time::sleep(Duration::from_millis(300)) => {
                    let _ = a.publish(topic, payload.clone());
                }
            }
        }
        let received = received.expect("B 应在 mesh 建立后收到 A 发布的 gossip 消息");
        assert_eq!(received, payload, "收到的 data 必须与发布一致");
    }

    // ── P0-2(冷启动): 先订阅(无邻居)→连接建立时 refresh 重放订阅→publish 一次即送达 ──
    #[tokio::test]
    async fn p02_coldstart_subscribe_before_connect_refresh_then_publish() {
        let mut a = P2pPeer::with_identity(identity::Keypair::generate_ed25519())
            .await
            .unwrap();
        let mut b = P2pPeer::with_identity(identity::Keypair::generate_ed25519())
            .await
            .unwrap();
        let listen = wait_tcp_listen_addr(&mut a).await;
        let port = tcp_port_of(&listen).unwrap();
        let a_peer = a.peer_id;
        // 关键：双方在“没有任何邻居”时先订阅（模拟 run_daemon 启动顺序）
        let topic = "gsn.test.coldstart";
        a.subscribe(topic).unwrap();
        b.subscribe(topic).unwrap();
        let dial: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", port, a_peer)
            .parse()
            .unwrap();
        b.add_bootstrap(dial).unwrap();

        // 连接建立时用 refresh 重放订阅（对应 process_swarm_event 中的调用）
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut connected = 0;
        while connected < 2 && Instant::now() < deadline {
            tokio::select! {
                ev = a.next_event() => {
                    if let Some(SwarmEvent::ConnectionEstablished { .. }) = ev {
                        a.refresh_gossipsub_subscriptions();
                        connected += 1;
                    }
                }
                ev = b.next_event() => {
                    if let Some(SwarmEvent::ConnectionEstablished { .. }) = ev {
                        b.refresh_gossipsub_subscriptions();
                        connected += 1;
                    }
                }
            }
        }
        assert_eq!(connected, 2, "双方都应建立连接");

        // 等 mesh 组建（心跳），然后 publish 一次
        let payload = b"coldstart-unique-payload-0xC0FFEE".to_vec();
        let mut received: Option<Vec<u8>> = None;
        let deadline = Instant::now() + Duration::from_secs(20);
        while received.is_none() && Instant::now() < deadline {
            tokio::select! {
                ev = a.next_event() => { let _ = ev; }
                ev = b.next_event() => {
                    if let Some(SwarmEvent::Behaviour(PeerEvent::Gossipsub(GsEvent::Message { message, .. }))) = ev {
                        received = Some(message.data.clone());
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(400)) => {
                    let _ = a.publish(topic, payload.clone());
                }
            }
        }
        let received = received.expect("refresh 后 mesh 应组建，B 收到一次 publish");
        assert_eq!(received, payload);
    }

    // ── P0-2: GossipDedup 仅首次放行，重复丢弃，容量超限淘汰最旧 ──
    #[test]
    fn p02_gossip_dedup_filters_duplicates_and_evicts() {
        let mut d = GossipDedup::new(2);
        assert!(d.first_seen(b"a"), "首次应放行");
        assert!(!d.first_seen(b"a"), "重复应丢弃");
        assert!(d.first_seen(b"b")); // 窗口 [a,b] 满
                                     // 再入 c → 淘汰最旧 a；窗口 [b,c]
        assert!(d.first_seen(b"c"));
        assert!(!d.first_seen(b"b"), "b 仍在窗口内，应判重");
        assert!(!d.first_seen(b"c"), "c 仍在窗口内，应判重");
        // a 已被淘汰 → 重新视为首次；这次插入会淘汰 b
        assert!(d.first_seen(b"a"), "a 已被淘汰，应重新放行");
        assert!(!d.first_seen(b"c"), "c 仍在窗口内，应判重");
    }

    // ── P0-5: 身份随目录持久化：不同目录 PeerId 不同，同目录恢复一致，文件 0600 ──
    #[test]
    fn p05_identity_persists_per_data_dir() {
        let base = std::env::temp_dir().join(format!(
            "gsn-id-p05-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let d1 = base.join("a");
        let d2 = base.join("b");

        let k1a = load_or_create_identity_in(&d1).unwrap();
        let k1b = load_or_create_identity_in(&d1).unwrap();
        let k2 = load_or_create_identity_in(&d2).unwrap();

        let p1a = PeerId::from(k1a.public());
        let p1b = PeerId::from(k1b.public());
        let p2 = PeerId::from(k2.public());

        assert_eq!(p1a, p1b, "同一目录两次加载必须恢复同一 PeerId");
        assert_ne!(p1a, p2, "不同 data-dir 必须得到不同 PeerId");

        let key_path = d1.join("identity.key");
        assert!(key_path.exists(), "identity.key 应落盘");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&key_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "identity.key 权限应为 0600");
        }

        let _ = std::fs::remove_dir_all(&base);
    }
}
