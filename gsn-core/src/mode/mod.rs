//! 节点模式管理

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeMode {
    Archive,
    Full,
    Light,
    Edge,
    Browser,
}

impl NodeMode {
    pub fn description(&self) -> &'static str {
        match self {
            NodeMode::Archive => "全量归档节点",
            NodeMode::Full => "全功能节点",
            NodeMode::Light => "轻量节点",
            NodeMode::Edge => "边缘节点",
            NodeMode::Browser => "浏览器节点",
        }
    }

    pub fn requires_dht_server(&self) -> bool {
        matches!(self, NodeMode::Archive | NodeMode::Full)
    }

    pub fn max_storage_gb(&self) -> u64 {
        match self {
            NodeMode::Archive => 100,
            NodeMode::Full => 50,
            NodeMode::Light => 10,
            NodeMode::Edge => 1,
            NodeMode::Browser => 0,
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
