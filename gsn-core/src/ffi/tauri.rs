//! Tauri commands（桌面端）

use crate::net::GsnNode;
use std::sync::Arc;
use std::sync::RwLock;

pub struct AppState {
    pub node: Arc<RwLock<GsnNode>>,
}

impl AppState {
    pub fn new(peer_id: String, listen_addr: String) -> Self {
        Self {
            node: Arc::new(RwLock::new(GsnNode::new(peer_id, listen_addr))),
        }
    }
}
