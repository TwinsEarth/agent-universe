//! 节点模式管理

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeMode {
    Archive,
    Full,
    Light,
    Edge,
    Browser,
    Relay,
}

impl NodeMode {
    pub fn description(&self) -> &'static str {
        match self {
            NodeMode::Archive => "全量归档节点",
            NodeMode::Full => "全功能节点",
            NodeMode::Light => "轻量节点",
            NodeMode::Edge => "边缘节点",
            NodeMode::Browser => "浏览器节点",
            NodeMode::Relay => "纯中继节点（Circuit Relay + DHT，不生产应用数据）",
        }
    }

    pub fn requires_dht_server(&self) -> bool {
        // v3.9.24: Relay 为公网可达节点，需以 Kademlia Server 模式对外提供
        // DHT 查询/转发，否则 DHT 无法利用该中继。
        matches!(self, NodeMode::Archive | NodeMode::Full | NodeMode::Relay)
    }

    pub fn max_storage_gb(&self) -> u64 {
        match self {
            NodeMode::Archive => 100,
            NodeMode::Full => 50,
            NodeMode::Light => 10,
            NodeMode::Edge => 1,
            NodeMode::Browser => 0,
            NodeMode::Relay => 5,
        }
    }
}

pub struct ModeController {
    current: NodeMode,
}

impl ModeController {
    pub fn new(mode: NodeMode) -> Self {
        Self { current: mode }
    }

    pub fn current(&self) -> NodeMode {
        self.current
    }

    pub fn switch_to(&mut self, new_mode: NodeMode) -> Result<(), String> {
        self.current = new_mode;
        Ok(())
    }
}
