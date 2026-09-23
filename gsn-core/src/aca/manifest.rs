//! Agent Manifest（智能体磁力链）
//!
//! 增强版 AgentCard：能力声明、硬件画像、验证模式、质押、信誉引用

use serde::{Deserialize, Serialize};

/// 硬件画像
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_free_gb: u64,
    pub gpu: Option<String>,
    pub bandwidth_up_mbps: u32,
    pub bandwidth_down_mbps: u32,
    /// 平台：desktop / mobile / server / browser
    pub platform: String,
}

impl Default for HardwareProfile {
    fn default() -> Self {
        Self {
            cpu_cores: 0,
            memory_mb: 0,
            disk_free_gb: 0,
            gpu: None,
            bandwidth_up_mbps: 0,
            bandwidth_down_mbps: 0,
            platform: "unknown".to_string(),
        }
    }
}

/// 验证模式声明
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationMode {
    /// L0：自报 + 抽样
    SelfReported,
    /// L1：冗余 2-of-3
    Redundant,
    /// L2：TEE
    TEE,
    /// L3：zkML
    ZkML,
    /// L4：委员会仲裁
    CommitteeArbitration,
}

/// Agent Manifest（智能体磁力链）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub did: String,
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub endpoints: Vec<String>,
    /// 硬件画像
    pub hardware: HardwareProfile,
    /// 支持的验证模式
    pub verification_modes: Vec<VerificationMode>,
    /// 质押量（GSP）
    pub stake: u64,
    /// 信誉引用（链上哈希）
    pub reputation_ref: Option<String>,
    /// 模型哈希
    pub model_hash: Option<String>,
    /// MCP 兼容：暴露的工具数
    pub mcp_tool_count: u32,
    /// 签名
    pub signature: String,
    /// 时间戳
    pub timestamp: u64,
}

impl AgentManifest {
    pub fn new(did: String, name: String) -> Self {
        Self {
            did,
            name,
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: Vec::new(),
            endpoints: Vec::new(),
            hardware: HardwareProfile::default(),
            verification_modes: vec![VerificationMode::SelfReported],
            stake: 0,
            reputation_ref: None,
            model_hash: None,
            mcp_tool_count: 0,
            signature: String::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn with_capability(mut self, cap: String) -> Self {
        self.capabilities.push(cap);
        self
    }

    pub fn with_hardware(mut self, hardware: HardwareProfile) -> Self {
        self.hardware = hardware;
        self
    }

    pub fn with_verification_mode(mut self, mode: VerificationMode) -> Self {
        self.verification_modes.push(mode);
        self
    }

    pub fn with_stake(mut self, stake: u64) -> Self {
        self.stake = stake;
        self
    }

    pub fn with_mcp_tools(mut self, count: u32) -> Self {
        self.mcp_tool_count = count;
        self
    }

    /// 兼容评分：硬件 × 质押 × 验证能力
    pub fn compatibility_score(&self) -> f64 {
        let hw_score = (self.hardware.cpu_cores as f64 * 2.0
            + self.hardware.memory_mb as f64 / 1024.0)
            .min(100.0);
        let stake_score = (self.stake as f64 / 10000.0).min(50.0);
        let verify_score = self.verification_modes.len() as f64 * 10.0;
        hw_score + stake_score + verify_score
    }
}
