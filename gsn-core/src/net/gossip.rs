//! GossipSub 消息广播（跨平台轻量版）

use std::collections::{HashMap, HashSet};

pub struct GossipSub {
    topics: HashSet<String>,
    messages: HashMap<String, Vec<Vec<u8>>>,
}

impl GossipSub {
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
        self.messages.entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(message);
    }

    pub fn get_messages(&self, topic: &str) -> &[Vec<u8>] {
        self.messages.get(topic).map(|v| v.as_slice()).unwrap_or(&[])
    }
}
