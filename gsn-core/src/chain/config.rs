//! 链上信任锚 · 网络配置（Base / Arbitrum 测试网 + 主网双就绪）。
//!
//! # 设计原则
//!
//! - **网络选择**：仅读环境变量 [`GSEN_NETWORK`]，取值
//!   `base-sepolia` / `arbitrum-sepolia` / `base` / `arbitrum`；缺省 `base-sepolia`。
//! - **RPC 端点**：内置各网络官方公开 RPC **占位**；可用 `GSEN_RPC_URL` 整体覆盖。
//!   公开 RPC 仅限冒烟，生产应自备私有节点/订阅端点。
//! - **合约地址**：`GSEN_CONTRACTS_FILE` 指向一个本地 JSON 文件，形如
//!   `{"usdc":"0x..","universalRouter":"0x.."}`；不把任何合约地址硬编码入库。
//! - **主网 fail-closed**：广播类操作必须同时满足
//!   (1) 已加载私钥；(2) `GSEN_CONFIRM_MAINNET=1` 显式确认主网。缺一则拒绝（403）。
//!   测试网同样需要私钥，但不要求主网确认开关。
//!
//! # 诚实边界
//!
//! 当前环境无真实 RPC / 私钥 / 资金。本模块只解析配置态；任何“已广播/已上链”的
//! 说法都不成立。真实交易一律标注为「未验证-待用户提供测试网凭据」。
//!
//! # 需外部审计
//!
//! 涉及主网资金路径的网络/合约配置加载策略，上线前需外部审计。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

/// 网络选择环境变量。
pub const ENV_NETWORK: &str = "GSEN_NETWORK";
/// RPC URL 覆盖环境变量。
pub const ENV_RPC_URL: &str = "GSEN_RPC_URL";
/// 合约地址 JSON 文件路径环境变量。
pub const ENV_CONTRACTS_FILE: &str = "GSEN_CONTRACTS_FILE";
/// 主网显式确认开关（必须等于字符串 `1`）。
pub const ENV_CONFIRM_MAINNET: &str = "GSEN_CONFIRM_MAINNET";

/// 受支持的 EVM 网络（Base / Arbitrum 的测试网与主网）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// Base Sepolia 测试网。
    BaseSepolia,
    /// Arbitrum Sepolia 测试网。
    ArbitrumSepolia,
    /// Base 主网。
    Base,
    /// Arbitrum 主网。
    Arbitrum,
}

impl Network {
    /// 从 `GSEN_NETWORK` 的字符串取值解析；未知取值报错（fail-closed，不静默兜底）。
    pub fn from_env_str(s: &str) -> Result<Self, ChainError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "base-sepolia" | "base_sepolia" | "baseSepolia" => Ok(Self::BaseSepolia),
            "arbitrum-sepolia" | "arbitrum_sepolia" | "arbitrumSepolia" => {
                Ok(Self::ArbitrumSepolia)
            }
            "base" => Ok(Self::Base),
            "arbitrum" => Ok(Self::Arbitrum),
            other => Err(ChainError::Config(format!(
                "未知 GSEN_NETWORK={other:?}（合法值: base-sepolia / arbitrum-sepolia / base / arbitrum）"
            ))),
        }
    }

    /// EIP-155 chain id。
    pub fn chain_id(self) -> u64 {
        match self {
            // https://docs.base.org/chain/base-sepolia
            Self::BaseSepolia => 84532,
            // https://docs.arbitrum.io/for-devs/concepts/network-and-rpc
            Self::ArbitrumSepolia => 421614,
            Self::Base => 8453,
            Self::Arbitrum => 42161,
        }
    }

    /// 人类可读网络名（状态端点展示用）。
    pub fn label(self) -> &'static str {
        match self {
            Self::BaseSepolia => "base-sepolia",
            Self::ArbitrumSepolia => "arbitrum-sepolia",
            Self::Base => "base",
            Self::Arbitrum => "arbitrum",
        }
    }

    /// 是否主网。
    pub fn is_mainnet(self) -> bool {
        matches!(self, Self::Base | Self::Arbitrum)
    }

    /// 内置官方公开 RPC 占位端点（生产应覆盖）。
    pub fn default_rpc_url(self) -> &'static str {
        match self {
            Self::BaseSepolia => "https://sepolia.base.org",
            Self::ArbitrumSepolia => "https://sepolia-rollup.arbitrum.io/rpc",
            Self::Base => "https://mainnet.base.org",
            Self::Arbitrum => "https://arb1.arbitrum.io/rpc",
        }
    }
}

/// 从合约文件读出的地址集合（全部为小写 hex 字符串，`0x` 前缀）。
///
/// 用 BTreeMap 保证序列化/展示顺序稳定，便于审计比对。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contracts {
    inner: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ContractsFile {
    #[serde(default)]
    usdc: Option<String>,
    #[serde(default, rename = "universalRouter")]
    universal_router: Option<String>,
    #[serde(default)]
    paymaster: Option<String>,
    #[serde(default, flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

impl Contracts {
    /// 读取 `path` 指向的 JSON 合约地址文件。
    ///
    /// 文件缺失 / JSON 非法 / 地址非合法 hex 均报错（fail-closed），绝不猜测地址。
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ChainError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .map_err(|e| ChainError::Config(format!("读取合约文件 {path:?} 失败: {e}")))?;
        let parsed: ContractsFile = serde_json::from_str(&raw)
            .map_err(|e| ChainError::Config(format!("合约文件 {path:?} 非合法 JSON: {e}")))?;
        let mut inner = BTreeMap::new();
        if let Some(u) = parsed.usdc {
            insert_addr(&mut inner, "usdc", u)?;
        }
        if let Some(u) = parsed.universal_router {
            insert_addr(&mut inner, "universalRouter", u)?;
        }
        if let Some(p) = parsed.paymaster {
            insert_addr(&mut inner, "paymaster", p)?;
        }
        for (k, v) in parsed.extra {
            if let serde_json::Value::String(s) = v {
                insert_addr(&mut inner, &k, s)?;
            }
        }
        Ok(Self { inner })
    }

    /// 取某个命名地址。
    pub fn get(&self, name: &str) -> Option<&str> {
        self.inner.get(name).map(|s| s.as_str())
    }

    /// 是否已配置任意合约地址。
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// 快照（按名排序的 Vec），用于状态展示。
    pub fn snapshot(&self) -> Vec<(String, String)> {
        self.inner
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

fn insert_addr(map: &mut BTreeMap<String, String>, name: &str, raw: String) -> Result<(), ChainError> {
    let trimmed = raw.trim();
    let hex_part = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    if hex_part.len() != 40 || !hex_part.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        return Err(ChainError::Config(format!(
            "合约地址 {name}={trimmed:?} 非法：应为 20 字节 hex（40 个 hex 字符）"
        )));
    }
    map.insert(name.to_string(), format!("0x{hex_part}").to_ascii_lowercase());
    Ok(())
}

/// 解析完成的链上配置（不可变快照）。
#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub network: Network,
    pub chain_id: u64,
    pub network_name: String,
    /// 实际使用的 RPC URL（`GSEN_RPC_URL` 覆盖默认）。
    pub rpc_url: String,
    /// 是否来自用户显式 RPC 覆盖（否则是内置公开占位）。
    pub rpc_is_override: bool,
    /// 合约地址集合（可能为空）。
    pub contracts: Contracts,
    /// 主网是否被 `GSEN_CONFIRM_MAINNET=1` 显式放行。
    pub mainnet_approved: bool,
}

impl ChainConfig {
    /// 从环境变量解析完整配置。任何一步非法都报错；缺省网络为 base-sepolia。
    pub fn from_env() -> Result<Self, ChainError> {
        let network = match std::env::var(ENV_NETWORK) {
            Ok(v) => Network::from_env_str(&v)?,
            Err(_) => Network::BaseSepolia,
        };
        let chain_id = network.chain_id();

        let (rpc_url, rpc_is_override) = match std::env::var(ENV_RPC_URL) {
            Ok(v) => {
                let t = v.trim().to_string();
                if t.is_empty() {
                    return Err(ChainError::Config(format!(
                        "{ENV_RPC_URL} 被设为空串（请删除该变量以使用默认，或填写有效 https URL）"
                    )));
                }
                if !t.starts_with("https://") {
                    return Err(ChainError::Config(format!(
                        "{ENV_RPC_URL} 必须是 https:// URL，拒绝: {t:?}"
                    )));
                }
                (t, true)
            }
            Err(_) => (network.default_rpc_url().to_string(), false),
        };

        let contracts = match std::env::var(ENV_CONTRACTS_FILE) {
            Ok(p) => Contracts::from_file(p)?,
            Err(_) => Contracts::default(),
        };

        let mainnet_approved = std::env::var(ENV_CONFIRM_MAINNET).as_deref() == Ok("1");

        Ok(Self {
            network,
            chain_id,
            network_name: network.label().to_string(),
            rpc_url,
            rpc_is_override,
            contracts,
            mainnet_approved,
        })
    }

    /// 是否允许任何“上链/广播”操作。
    ///
    /// 规则（fail-closed）：
    /// - 测试网：只要调用方已持有私钥即可（这里只判网络维度，私钥由 keys.rs 判）；
    /// - 主网：必须 `GSEN_CONFIRM_MAINNET=1`，否则拒绝。
    pub fn require_mainnet_approved(&self) -> Result<(), ChainError> {
        if self.network.is_mainnet() && !self.mainnet_approved {
            return Err(ChainError::Gated(format!(
                "主网 {} 已被 fail-closed 门禁拒绝：请同时设置 {ENV_CONFIRM_MAINNET}=1 并提供测试过的私钥/RPC。拒绝任何广播。",
                self.network.label()
            )));
        }
        Ok(())
    }

    /// 状态视图（无凭据，可安全暴露给未授权调用方；不含任何秘密）。
    pub fn public_status(&self) -> serde_json::Value {
        serde_json::json!({
            "network": self.network_name,
            "chainId": self.chain_id,
            "isMainnet": self.network.is_mainnet(),
            "rpcUrlConfigured": true,
            "rpcIsUserOverride": self.rpc_is_override,
            "mainnetApproved": self.mainnet_approved,
            "gated": self.network.is_mainnet() && !self.mainnet_approved,
            "contractsConfigured": !self.contracts.is_empty(),
            "note": "无凭据时仅展示配置态；广播/发送交易需私钥与主网确认开关，未配置时 fail-closed。",
        })
    }
}

/// 链上模块统一错误类型。
///
/// **绝不**在任何变体文本里嵌入私钥 / mnemonic / 完整签名材料。
#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("链上配置错误: {0}")]
    Config(String),
    /// 被安全门禁拒绝（主网未确认 / 无凭据）——对应 HTTP 403。
    #[error("链上操作被拒绝: {0}")]
    Gated(String),
    #[error("密钥错误: {0}")]
    Key(String),
    #[error("RLP 编码/解码错误: {0}")]
    Rlp(String),
    #[error("交易构造/签名错误: {0}")]
    Tx(String),
    #[error("EIP-712/EIP-3009 错误: {0}")]
    Eip712(String),
    #[error("JSON-RPC 错误: {0}")]
    Rpc(String),
    #[error("密码学错误: {0}")]
    Crypto(String),
}

impl From<k256::ecdsa::Error> for ChainError {
    fn from(e: k256::ecdsa::Error) -> Self {
        // 不携带任何上下文秘密；只给一个稳定分类。
        ChainError::Crypto(format!("secp256k1 运算失败: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_networks() {
        assert_eq!(Network::from_env_str("base-sepolia").unwrap(), Network::BaseSepolia);
        assert_eq!(Network::from_env_str("BASE").unwrap(), Network::Base);
        assert_eq!(Network::Base.chain_id(), 8453);
        assert_eq!(Network::BaseSepolia.chain_id(), 84532);
        assert_eq!(Network::Arbitrum.chain_id(), 42161);
        assert_eq!(Network::ArbitrumSepolia.chain_id(), 421614);
        assert!(Network::Base.is_mainnet());
        assert!(!Network::BaseSepolia.is_mainnet());
        assert!(Network::from_env_str("bitcoin").is_err());
    }

    #[test]
    fn mainnet_requires_explicit_approval() {
        let mut cfg = ChainConfig::from_env().unwrap();
        // 直接构造一个主网配置对象用于门禁测试，避免污染进程环境。
        cfg.network = Network::Base;
        cfg.chain_id = Network::Base.chain_id();
        cfg.mainnet_approved = false;
        assert!(cfg.require_mainnet_approved().is_err());
        cfg.mainnet_approved = true;
        assert!(cfg.require_mainnet_approved().is_ok());
        // 测试网不需要主网开关。
        cfg.network = Network::BaseSepolia;
        cfg.mainnet_approved = false;
        assert!(cfg.require_mainnet_approved().is_ok());
    }

    #[test]
    fn rejects_bad_contracts_file() {
        let dir = std::env::temp_dir();
        let f = dir.join("gsen_contracts_test.json");
        // 合法合约文件解析路径
        std::fs::write(&f, r#"{"usdc":"0x0000000000000000000000000000000000000001"}"#).unwrap();
        let c = Contracts::from_file(&f).unwrap();
        assert_eq!(c.get("usdc"), Some("0x0000000000000000000000000000000000000001"));
        // 非法长度地址被拒
        std::fs::write(&f, r#"{"usdc":"0x1234"}"#).unwrap();
        assert!(Contracts::from_file(&f).is_err());
        // 非法 JSON 被拒
        std::fs::write(&f, r#"{not json"#).unwrap();
        assert!(Contracts::from_file(&f).is_err());
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn status_json_is_safe_and_credential_free() {
        let cfg = ChainConfig {
            network: Network::BaseSepolia,
            chain_id: 84532,
            network_name: "base-sepolia".into(),
            rpc_url: "https://sepolia.base.org".into(),
            rpc_is_override: false,
            contracts: Contracts::default(),
            mainnet_approved: false,
        };
        let s = cfg.public_status();
        assert_eq!(s["network"], "base-sepolia");
        assert_eq!(s["chainId"], 84532);
        assert_eq!(s["gated"], false);
        // 主网未确认时 gated=true
        let mut s2 = cfg.clone();
        s2.network = Network::Base;
        s2.chain_id = 8453;
        let v = s2.public_status();
        assert_eq!(v["gated"], true);
    }
}
