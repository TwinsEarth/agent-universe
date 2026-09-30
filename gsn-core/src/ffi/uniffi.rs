//! UniFFI 导出（移动端）

use std::sync::Arc;
use std::sync::RwLock;
use crate::net::InMemoryNode;

pub struct GsnClient {
    node: Arc<RwLock<InMemoryNode>>,
}

impl GsnClient {
    pub fn new(peer_id: String, listen_addr: String) -> Self {
        Self {
            node: Arc::new(RwLock::new(InMemoryNode::new(peer_id, listen_addr))),
        }
    }

    pub fn peer_id(&self) -> String {
        self.node
            .read()
            .unwrap_or_else(|e| {
                eprintln!("⚠️ uniffi: 节点锁曾毒化，恢复后继续（请人工核查）");
                e.into_inner()
            })
            .peer_id
            .clone()
    }
}
