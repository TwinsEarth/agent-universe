use serde::{Serialize, Deserialize};

/// Agent 身份卡
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub did: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub endpoints: Vec<String>,
    pub reputation_bps: u16,
    pub version: String,
}

impl AgentCard {
    pub fn new(did: String, name: String) -> Self {
        Self {
            did,
            name,
            capabilities: Vec::new(),
            endpoints: Vec::new(),
            reputation_bps: 5000,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn with_capability(mut self, cap: String) -> Self {
        self.capabilities.push(cap);
        self
    }

    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoints.push(endpoint);
        self
    }
}
