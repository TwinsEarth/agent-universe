//! 本地存储（跨平台轻量版）

use std::collections::HashMap;

pub struct LocalStorage {
    data: HashMap<String, Vec<u8>>,
}

impl LocalStorage {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn put(&mut self, key: String, value: Vec<u8>) {
        self.data.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.data.get(key)
    }

    pub fn remove(&mut self, key: &str) {
        self.data.remove(key);
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}
