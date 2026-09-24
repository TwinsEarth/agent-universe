//! 根种子节点模块

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeedMode {
    Root,
    Bootstrap,
    Normal,
}

#[derive(Debug, Clone)]
pub struct RootSeedConfig {
    pub mode: SeedMode,
    pub bootstrap_duration_secs: u64,
    pub enable_dht_server: bool,
    pub enable_relay: bool,
    pub upgrade_threshold: u32,
}

impl Default for RootSeedConfig {
    fn default() -> Self {
        Self {
            mode: SeedMode::Root,
            bootstrap_duration_secs: 3600,
            enable_dht_server: true,
            enable_relay: true,
            upgrade_threshold: 1000,
        }
    }
}

impl RootSeedConfig {
    pub fn new(mode: SeedMode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }

    pub fn should_downgrade(&self, active_peers: u32, dht_health: u32) -> bool {
        self.mode == SeedMode::Root 
            && active_peers > self.upgrade_threshold 
            && dht_health > (self.upgrade_threshold / 2)
    }

    pub fn downgrade(&mut self) {
        if self.mode == SeedMode::Root {
            self.mode = SeedMode::Bootstrap;
        }
    }
}
