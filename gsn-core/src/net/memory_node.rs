//! 进程内节点替身（in-memory，测试/演示用）
//!
//! **这不是真实 libp2p 节点**：`publish_card` / `discover_by_capability`
//! 只操作本地 `HashMap`，不产生任何网络流量，也不写入真实 DHT。
//!
//! 真实网络组件见：
//! - [`crate::net::peer::P2pPeer`]：基于 libp2p 的真实传输
//!   （TCP/QUIC/WS + Noise + Yamux + Kademlia + GossipSub + Relay/DCUtR）
//! - [`crate::node`]：节点运行时（中继预约、DCUtR 直连、组网编排）

use crate::agent::AgentCard;
use std::collections::HashMap;

/// 进程内节点替身：仅在本地保存/检索 Agent 卡片，不联网。
pub struct InMemoryNode {
    pub peer_id: String,
    pub listen_addr: String,
    cards: HashMap<String, AgentCard>,
}

impl InMemoryNode {
    pub fn new(peer_id: String, listen_addr: String) -> Self {
        Self {
            peer_id,
            listen_addr,
            cards: HashMap::new(),
        }
    }

    /// 仅写入本地内存；保留 card 语义，但不会发布到真实 DHT。
    pub fn publish_card(&mut self, card: AgentCard) {
        self.cards.insert(card.did.clone(), card);
    }

    pub fn get_card(&self, did: &str) -> Option<&AgentCard> {
        self.cards.get(did)
    }

    pub fn list_cards(&self) -> Vec<&AgentCard> {
        self.cards.values().collect()
    }

    /// 仅在本地卡片中按能力过滤，不进行真实网络发现。
    pub fn discover_by_capability(&self, capability: &str) -> Vec<&AgentCard> {
        self.cards
            .values()
            .filter(|c| c.capabilities.iter().any(|cap| cap == capability))
            .collect()
    }
}
