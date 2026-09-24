//! UniFFI 导出（移动端）

use std::sync::Arc;
use std::sync::RwLock;
use crate::net::GsnNode;

pub struct GsnClient {
    node: Arc<RwLock<GsnNode>>,
}

impl GsnClient {
    pub fn new(peer_id: String, listen_addr: String) -> Self {
        Self {
            node: Arc::new(RwLock::new(GsnNode::new(peer_id, listen_addr))),
        }
    }

    pub fn peer_id(&self) -> String {
        self.node.read().unwrap().peer_id.clone()
    }
}
