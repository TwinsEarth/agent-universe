//! Tauri commands（桌面端）

use crate::net::InMemoryNode;
use std::sync::Arc;
use std::sync::RwLock;

pub struct AppState {
    pub node: Arc<RwLock<InMemoryNode>>,
}

impl AppState {
    pub fn new(peer_id: String, listen_addr: String) -> Self {
        Self {
            node: Arc::new(RwLock::new(InMemoryNode::new(peer_id, listen_addr))),
        }
    }
}
