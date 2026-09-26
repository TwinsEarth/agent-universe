//! 真实 libp2p 对等节点
//!
//! 使用 libp2p 0.54 实现：TCP + Noise 加密 + Yamux 多路复用 + Kademlia DHT + GossipSub

use futures::StreamExt;
use libp2p::{
    autonat, dcutr,
    gossipsub::{
        self, ConfigBuilder as GossipsubConfigBuilder, IdentTopic, MessageAuthenticity,
    },
    identify::{self, Config as IdentifyConfig},
    identity,
    kad::{self, store::MemoryStore, Config as KadConfig, Quorum, Record, RecordKey},
    ping, relay,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    Multiaddr, PeerId, SwarmBuilder,
};
use std::time::Duration;

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

/// 真实 libp2p 对等节点
pub struct P2pPeer {
    pub peer_id: PeerId,
    pub swarm: Swarm<PeerBehaviour>,
    /// v2.5.3: 已发起过 bootstrap 连接的地址
    bootstrapped: Vec<String>,
}

impl P2pPeer {
    /// 创建新节点，从 ~/.gsn/identity.key 加载身份；不存在则生成并持久化
    pub fn new() -> anyhow::Result<Self> {
        let local_key = load_or_create_identity()?;
        Self::with_identity(local_key)
    }

    /// 用指定身份创建节点
    pub fn with_identity(local_key: identity::Keypair) -> anyhow::Result<Self> {
        let peer_id = PeerId::from(local_key.public());

        let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            // v2.5.4: 启用 DNS 解析（/dns4、/dns6、/dnsaddr），公共 bootstrap 依赖
            .with_dns()?
            // v2.5.4: 启用 Circuit Relay v2 客户端，支持通过中继跨 NAT
            .with_relay_client(
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            .with_behaviour(|key, relay_client| {
                // Kademlia DHT（在 Config 上设置查询超时）
                let store = MemoryStore::new(peer_id);
                let mut kad_config =
                    KadConfig::new(libp2p::StreamProtocol::new("/ipfs/kad/1.0.0"));
                kad_config.set_query_timeout(Duration::from_secs(30));
                let kademlia = kad::Behaviour::with_config(peer_id, store, kad_config);

                // GossipSub
                let gossipsub_config = GossipsubConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .build()
                    .expect("valid gossipsub config");
                let gossipsub = gossipsub::Behaviour::new(
                    MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .expect("valid gossipsub");

                // Identify
                let identify = identify::Behaviour::new(IdentifyConfig::new(
                    "/gsn/0.2.54".to_string(),
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

                PeerBehaviour {
                    kademlia,
                    gossipsub,
                    identify,
                    relay_client,
                    autonat,
                    dcutr,
                    ping,
                }
            })?
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(Self { peer_id, swarm, bootstrapped: Vec::new() })
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

    /// 主动查找节点（DHT 节点发现）
    pub fn find_peer(&mut self, peer: PeerId) {
        self.swarm
            .behaviour_mut()
            .kademlia
            .get_closest_peers(peer);
    }

    /// 轮询一次 swarm 事件
    pub async fn next_event(&mut self) -> SwarmEvent<PeerEvent> {
        self.swarm.next().await.expect("swarm stream never ends")
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
        self.swarm
            .listeners()
            .map(|a| a.to_string())
            .collect()
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

    /// 已连接对等节点的 peer_id 列表
    pub fn connected_peer_ids(&self) -> Vec<String> {
        self.swarm
            .connected_peers()
            .map(|peer_id| peer_id.to_string())
            .collect()
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
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19UokH",
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

/// v2.5.3: 从磁盘加载 libp2p 身份密钥；不存在则生成 Ed25519 并保存
///
/// 持久化路径: `~/.gsn/identity.key`（protobuf 编码）
/// 保证同一台机器跨重启 Peer ID 稳定，DID 绑定可保持
pub fn load_or_create_identity() -> anyhow::Result<identity::Keypair> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = std::path::Path::new(&home).join(".gsn");
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
    std::fs::create_dir_all(&dir)?;
    let encoded = kp.to_protobuf_encoding()?;
    std::fs::write(&key_path, encoded)?;
    Ok(kp)
}
