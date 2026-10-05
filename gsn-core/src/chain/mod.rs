//! 链上信任锚（Base / Arbitrum 测试网+主网双就绪）。
//!
//! 子模块：
//! - [`config`]：网络/端点/合约配置 + 主网 fail-closed 门禁；
//! - [`rlp`]：手写 RLP 编解码；
//! - [`keys`]：secp256k1 私钥加载（仅环境变量/本地文件）+ EVM 地址派生；
//! - [`tx`]：EIP-1559 交易构造与签名；
//! - [`eip712`]：EIP-712 域 + EIP-3009 transferWithAuthorization 签名；
//! - [`paymaster`]：ERC-4337 / x402 载荷 schema；
//! - [`rpc`]：EVM JSON-RPC 适配器（rustls + 手写 HTTP/1.1 POST）；
//! - [`http`]：自包含异步 HTTP 处理函数（被 node.rs 调用，本模块不改 node.rs）。
//!
//! # 安全铁律
//!
//! 私钥/RPC 只来自环境变量或本地密钥文件；绝不入库/进日志/进错误信息。
//! 主网默认 gated：必须 `GSEN_CONFIRM_MAINNET=1` + 凭据才允许广播。
//!
//! # 需外部审计
//!
//! 所有密码学实现（secp256k1 签名、EIP-712、EIP-3009、地址派生、Paymaster）
//! 在对应模块文档注释中已显著标注「需外部审计」。

pub mod config;
pub mod eip712;
pub mod http;
pub mod keys;
pub mod paymaster;
pub mod pocv;
pub mod rlp;
pub mod rpc;
pub mod tx;

pub use config::{ChainConfig, ChainError, Network};
pub use http::handle_chain_api;
pub use keys::{
    evm_address_from_signing_key, load_signing_key, to_checksum_address, to_lower_address,
};
pub use paymaster::{PaymasterClient, PaymentRequired402, SponsorRequest, SponsorResponse};
pub use rpc::RpcClient;
pub use tx::{inspect_raw_tx, SignedTx, TxEip1559};

// 保留 pocv 现有导出（不要改 pocv.rs）。
pub use pocv::{PoCVVerifier, ProofOfComputation, SignedProofOfComputation};
