//! libp2p 节点初始化（跨平台轻量版）

use std::collections::HashMap;
use crate::agent::AgentCard;

pub struct GsnNode {
    pub peer_id: String,
    pub listen_addr: String,
    cards: HashMap<String, AgentCard>,
}

impl GsnNode {
    pub fn new(peer_id: String, listen_addr: String) -> Self {
        Self {
            peer_id,
            listen_addr,
            cards: HashMap::new(),
        }
    }

    pub fn publish_card(&mut self, card: AgentCard) {
        self.cards.insert(card.did.clone(), card);
    }

    pub fn get_card(&self, did: &str) -> Option<&AgentCard> {
        self.cards.get(did)
    }

    pub fn list_cards(&self) -> Vec<&AgentCard> {
        self.cards.values().collect()
    }

    pub fn discover_by_capability(&self, capability: &str) -> Vec<&AgentCard> {
        self.cards.values()
            .filter(|c| c.capabilities.iter().any(|cap| cap == capability))
            .collect()
    }
}
