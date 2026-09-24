//! 真实 libp2p 对等节点
//!
//! 使用 libp2p 0.54 实现：TCP + Noise 加密 + Yamux 多路复用 + Kademlia DHT + GossipSub

use futures::StreamExt;
use libp2p::{
    gossipsub::{
        self, ConfigBuilder as GossipsubConfigBuilder, IdentTopic, MessageAuthenticity,
    },
    identify::{self, Config as IdentifyConfig},
    identity,
    kad::{self, store::MemoryStore, Config as KadConfig, Quorum, Record, RecordKey},
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
}

#[derive(Debug)]
pub enum PeerEvent {
    Kademlia(kad::Event),
    Gossipsub(gossipsub::Event),
    Identify(identify::Event),
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

/// 真实 libp2p 对等节点
pub struct P2pPeer {
    pub peer_id: PeerId,
    pub swarm: Swarm<PeerBehaviour>,
}

impl P2pPeer {
    /// 创建新节点，自动生成 Ed25519 身份
    pub fn new() -> anyhow::Result<Self> {
        let local_key = identity::Keypair::generate_ed25519();
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
            .with_behaviour(|key| {
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
                    "/gsn/0.2.34".to_string(),
                    key.public(),
                ));

                PeerBehaviour {
                    kademlia,
                    gossipsub,
                    identify,
                }
            })?
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        Ok(Self { peer_id, swarm })
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
}
