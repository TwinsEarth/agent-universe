//! 进程内 GossipSub 替身（in-memory，测试/演示用）
//!
//! **这不是真实 GossipSub**：`subscribe` / `publish` 只操作本地集合，
//! 消息不会扩散到任何 peer。
//!
//! 真实主题广播见 [`crate::net::peer::P2pPeer`]（libp2p GossipSub）与 [`crate::node`]。

use std::collections::{HashMap, HashSet};

/// 进程内 pubsub 替身：仅本地保存主题消息，不联网。
pub struct InMemoryGossip {
    topics: HashSet<String>,
    messages: HashMap<String, Vec<Vec<u8>>>,
}

impl Default for InMemoryGossip {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryGossip {
    pub fn new() -> Self {
        Self {
            topics: HashSet::new(),
            messages: HashMap::new(),
        }
    }

    pub fn subscribe(&mut self, topic: String) {
        self.topics.insert(topic);
    }

    pub fn unsubscribe(&mut self, topic: &str) {
        self.topics.remove(topic);
    }

    pub fn publish(&mut self, topic: &str, message: Vec<u8>) {
        self.messages
            .entry(topic.to_string())
            .or_default()
            .push(message);
    }

    pub fn get_messages(&self, topic: &str) -> &[Vec<u8>] {
        self.messages
            .get(topic)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
