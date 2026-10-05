//! 比特币/以太坊 Agent 经济体（v3.9.x）。
//!
//! # 定位
//!
//! v3.8.x 解决了「闲置资源如何记账、撮合、定价、结算守恒」，但其内部记账单位
//! Credits 不挂法币、不挂链。v3.9.x 把资源市场接到真实的数字货币基础设施上，
//! 逐步形成三层货币体系：
//!
//! - **价值锚定层**：比特币主链 RGB/Taproot/HTLC——大额/跨周期最终结算与质押；
//! - **可编程结算层**：以太坊 L2 x402 + 稳定币/ERC-4337——条件支付、身份信誉、DeFi；
//! - **即时支付层**：比特币闪电 L402/x402-LN——高频小额微支付。
//!
//! 与资源市场内核一致，本域**确定性内核以 T0 native 系统插件落地**（`u128` 守恒/
//! 整数阈值需 cargo 单测面），面向进程/插件的门面后续以 T1 官方插件承载。
//!
//! # 版本切分
//!
//! - **v3.9.0 结算路由（本版本）**：[`PaymentRouter`] 纯确定性选路——按金额（内部整数
//!   micro）、时延/周期（即时/常规/跨周期）、条件（智能合约/DeFi）与可用轨道，在闪电
//!   L402、EVM x402、BTC RGB/HTLC 三轨之间选唯一轨道并给理由。**只决策，不动钱**：
//!   不持私钥、不签名、不广播、不划转、不兑换、不连任何节点。
//! - v3.9.1 宿主签名服务与私钥隔离（本版本）：[`HostSignerGate`] 提供只读
//!   `wallet_sign_preview`（只规范化预览、不碰密钥）与宿主受限 `wallet_sign`（私钥/seed 绝不
//!   进沙盒，回执仅含公钥+签名；生产未配置密钥时 [`PaymentError::SignerNotConfigured`] 具名
//!   fail-closed，绝不伪造签名）。
//! - v3.9.2 EVM x402（USDC）、v3.9.3 闪电 L402 适配（仍决策/记账面，真网连接诚实标注）。
//! - v3.9.4 ERC-8004 身份/信誉/验证三注册表对接面，v3.9.5 ERC-4337 Paymaster 赞助面。
//! - v3.9.6 BTC HTLC/RGB 纯校验（哈希锁/脚本结构只校验不发链）。
//! - v3.9.7 链上锚定/跨链桥风控（fail-closed），v3.9.8 ZK 支付意图/OWS/合规接口占位。
//!
//! # 诚实边界
//!
//! 外部报道的各项数字（x402 交易量、BlackRock 模型储蓄占比、ERC-8004 采用量、
//! A402 性能等）均为**第三方报道口径，非本仓复测**；本内核不内置这些数字作为事实。

pub mod erc8004;
pub mod l402;
pub mod router;
pub mod signer;
pub mod x402;

pub use erc8004::{
    aggregate_feedback, hash_token_uri, validate_did, validate_pubkey_hex, Erc8004Error, Feedback,
    IdentityRecord, IdentityRegistry, ReputationAccumulator, ReputationDimension,
    ReputationRegistry, ReputationSnapshot, ValidationDecision, ValidationMethod, ValidationTally,
    ValidationVerdict, ValidationVote,
};
pub use l402::{
    parse_bolt11_amount_msat, verify_settlement as l402_verify_settlement, L402Challenge,
    L402Credential, L402Error, PaymentHash, Preimage, L402_SCHEME,
};
pub use router::{
    PaymentRouter, PaymentTrack, RouteDecision, RouteReason, RoutingInput, RoutingPolicy,
    SettlementUrgency, TrackAvailability,
};
pub use signer::{
    HostSignerGate, InMemoryBroker, SignIntent, SignPolicy, SignPreview, SignatureBroker,
    SignatureReceipt, TrackSignRule, UnconfiguredBroker, UnconfiguredSignerGate,
};
pub use x402::{
    keccak256, verify_settlement, EvmAddress, Nonce32, TransferAuthorization, X402Asset,
    X402Challenge, X402Domain, X402Error,
};

use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::runtime::native::NativeRuntime;
use serde::{Deserialize, Serialize};

/// 结算路由系统插件 id（T0，随内核进程内注册，不可热插拔）。
pub const PAYMENT_ROUTER_PLUGIN: &str = "com.twinsearth.sys.payment-router";

/// PMB 方法：结算路由只读状态（版本 + 轨道 + 诚实位）。
pub const METHOD_PAYMENT_ROUTER_STATUS: &str = "payment_router_status";

/// PMB 方法：宿主签名预览（只校验/规范化，不碰密钥、不产出签名）。
pub const METHOD_WALLET_SIGN_PREVIEW: &str = "wallet_sign_preview";

/// PMB 方法：宿主受限签发（私钥不进沙盒；生产未配置则具名 SignerNotConfigured）。
pub const METHOD_WALLET_SIGN: &str = "wallet_sign";

/// PMB 方法：x402 402 challenge 纯校验（now 显式传入，不读时钟）。
pub const METHOD_X402_CHALLENGE_VALIDATE: &str = "x402_challenge_validate";

/// PMB 方法：x402 EIP-3009 授权预览（构造 EIP-712 digest，不产出 secp256k1 签名）。
pub const METHOD_X402_AUTHORIZE_PREVIEW: &str = "x402_authorize_preview";

/// PMB 方法：x402 facilitator 结算回执精确守恒校验（实付须恰等于应付）。
pub const METHOD_X402_SETTLEMENT_VERIFY: &str = "x402_settlement_verify";

/// PMB 方法：x402 EVM 链上签发（本版无 secp256k1 后端，一律具名 fail-closed，不伪造）。
pub const METHOD_X402_SIGN: &str = "x402_sign";

/// PMB 方法：L402 闪电挑战解析（`WWW-Authenticate` 头 -> macaroon/invoice/金额 msat，只读）。
pub const METHOD_L402_PARSE_CHALLENGE: &str = "l402_parse_challenge";

/// PMB 方法：L402 凭证校验（macaroon 一致 + SHA256(preimage)==payment_hash + 精确金额）。
pub const METHOD_L402_VERIFY: &str = "l402_verify";

/// PMB 方法：L402 闪电支付/开票（本版不连闪电节点，一律具名 fail-closed，不伪造 HTLC）。
pub const METHOD_L402_PAY: &str = "l402_pay";

/// PMB 方法：ERC-8004 身份句柄绑定校验（tokenId/DID/Ed25519/tokenURI 承诺，纯只读）。
pub const METHOD_ERC8004_IDENTITY_CHECK: &str = "erc8004_identity_check";

/// PMB 方法：ERC-8004 信誉反馈无状态聚合（整数守恒 + 重放/自评拦截，纯只读）。
pub const METHOD_ERC8004_REPUTATION_AGGREGATE: &str = "erc8004_reputation_aggregate";

/// PMB 方法：ERC-8004 独立验证 BFT-lite 裁决（n≥3f+1，纯只读，不写链）。
pub const METHOD_ERC8004_VALIDATION_DECIDE: &str = "erc8004_validation_decide";

/// 结算/支付域领域错误（类型化拒绝，不静默降级、不动钱）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentError {
    /// 结算金额必须为正（micro）。
    NonPositiveAmount,
    /// 选路阈值非法：阈值须为正且即时上限严格小于最终结算下限。
    RoutingPolicyInvalid {
        instant_cap_micro: u128,
        large_floor_micro: u128,
    },
    /// 本笔没有任何可用支付轨道。
    NoAvailableTrack,
    /// 决策所需轨道在本节点不可用——拒绝而非静默降级（不改变费用/最终性假设）。
    RouteTrackUnavailable { required: PaymentTrack },
    /// 声明需要 DeFi 组合却不声明需要智能合约，条件自相矛盾。
    DefiWithoutSmartContract,
    /// 宿主未配置签名密钥后端——fail-closed 具名拒签，绝不返回伪造签名。
    SignerNotConfigured,
    /// 签名域为空或仅空白。
    EmptySigningDomain,
    /// 待签摘要长度非法（须恰为 32 字节）。
    InvalidDigestLength { expected: usize, got: usize },
    /// 非纯认证签名却给出 0 支付上限。
    NonPositiveSpendCap,
    /// 声明纯认证(auth_only)却又带非零支付上限，自相矛盾。
    AuthOnlyWithSpendCap,
    /// 该支付轨道在签名策略中被禁用。
    SigningTrackDisabled { required: PaymentTrack },
    /// 本笔授权支付上限超出该轨道策略上限。
    SpendCapExceeded {
        requested_micro: u128,
        allowed_micro: u128,
    },
    /// 签名策略非法：启用轨道却给出零额度。
    SignPolicyInvalid { track: PaymentTrack },
}

impl std::fmt::Display for PaymentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentError::NonPositiveAmount => {
                write!(f, "PAYMENT_NON_POSITIVE_AMOUNT: amount_micro must be > 0")
            }
            PaymentError::RoutingPolicyInvalid {
                instant_cap_micro,
                large_floor_micro,
            } => write!(
                f,
                "PAYMENT_ROUTING_POLICY_INVALID: thresholds must be >0 and instant_cap({instant_cap_micro}) < large_floor({large_floor_micro})"
            ),
            PaymentError::NoAvailableTrack => {
                write!(f, "PAYMENT_NO_AVAILABLE_TRACK: no settlement track is enabled")
            }
            PaymentError::RouteTrackUnavailable { required } => write!(
                f,
                "PAYMENT_ROUTE_TRACK_UNAVAILABLE: required track {} is not enabled (refusing to silently downgrade)",
                required.as_str()
            ),
            PaymentError::DefiWithoutSmartContract => write!(
                f,
                "PAYMENT_DEFI_WITHOUT_SMART_CONTRACT: DeFi composition implies smart-contract capability"
            ),
            PaymentError::SignerNotConfigured => write!(
                f,
                "PAYMENT_SIGNER_NOT_CONFIGURED: host signing key backend is not configured (refusing to sign, no fabricated signature)"
            ),
            PaymentError::EmptySigningDomain => {
                write!(f, "PAYMENT_EMPTY_SIGNING_DOMAIN: signing domain must be non-blank")
            }
            PaymentError::InvalidDigestLength { expected, got } => write!(
                f,
                "PAYMENT_INVALID_DIGEST_LENGTH: message_digest must be {expected} bytes, got {got}"
            ),
            PaymentError::NonPositiveSpendCap => write!(
                f,
                "PAYMENT_NON_POSITIVE_SPEND_CAP: spend_cap_micro must be >0 unless auth_only"
            ),
            PaymentError::AuthOnlyWithSpendCap => write!(
                f,
                "PAYMENT_AUTH_ONLY_WITH_SPEND_CAP: auth_only intent must carry spend_cap_micro=0"
            ),
            PaymentError::SigningTrackDisabled { required } => write!(
                f,
                "PAYMENT_SIGNING_TRACK_DISABLED: track {} is disabled by host sign policy",
                required.as_str()
            ),
            PaymentError::SpendCapExceeded {
                requested_micro,
                allowed_micro,
            } => write!(
                f,
                "PAYMENT_SPEND_CAP_EXCEEDED: requested {requested_micro} > allowed {allowed_micro} micro"
            ),
            PaymentError::SignPolicyInvalid { track } => write!(
                f,
                "PAYMENT_SIGN_POLICY_INVALID: enabled track {} must have a positive spend cap",
                track.as_str()
            ),
        }
    }
}

impl std::error::Error for PaymentError {}

/// 结算路由只读状态负载（诚实标注：只决策，不动钱）。
pub fn status_payload() -> serde_json::Value {
    serde_json::json!({
        "plugin": PAYMENT_ROUTER_PLUGIN,
        "version": env!("CARGO_PKG_VERSION"),
        "domain": "agent-payment-economy",
        "introduced_in": "v3.9.0",
        "signing_introduced_in": "v3.9.1",
        "x402_introduced_in": "v3.9.2",
        "l402_introduced_in": "v3.9.3",
        "erc8004_introduced_in": "v3.9.4",
        "amount_unit": {
            "name": "credits",
            "micro_units_per_credit": 1_000_000,
            "integer_only": true,
            "fiat_pegged": false,
            "note": "金额为内部整数 micro；阈值是宿主策略常量，内核不宣称法币/币价含义。"
        },
        "tracks": PaymentTrack::ALL.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        "urgencies": ["instant", "normal", "cross_epoch"],
        "routing_priority": [
            "defi_combo -> evm_x402",
            "smart_contract -> evm_x402",
            "amount>=large_floor or cross_epoch -> btc_rgb_htlc",
            "instant and amount<=instant_cap -> lightning_l402",
            "otherwise -> evm_x402(stablecoin default)"
        ],
        "default_thresholds_micro": {
            "instant_cap": RoutingPolicy::default().instant_cap_micro,
            "large_floor": RoutingPolicy::default().large_floor_micro
        },
        // v3.9.1 只读选路 + 宿主受限签名；v3.9.2 增 x402 协议只读构造/校验；v3.9.3 增闪电 L402 只读协议。执行/链上写仍在后续版本。
        "capabilities_declared": [
            "pay:route:read",
            "wallet:sign:host-restricted",
            "x402:protocol:read",
            "l402:protocol:read",
            "erc8004:registry:read"
        ],
        "capabilities_reserved_later": [
            "pay:execute",
            "chain:anchor:write",
            "erc8004:registry:write"
        ],
        "signing": {
            "introduced_in": "v3.9.1",
            "methods": ["wallet_sign_preview", "wallet_sign"],
            "private_key_enters_sandbox": false,
            "seed_enters_sandbox": false,
            "receipt_contains": ["track", "public_key_hex", "signature_hex", "signed_normalized_payload_hex"],
            "receipt_never_contains": ["private_key", "seed", "secret"],
            "sign_normalized_full_context": true,
            "host_signing_gate": "HostSignerGate",
            // 生产默认装配 UnconfiguredBroker：configured=false，wallet_sign 一律具名
            // SignerNotConfigured fail-closed；仅 wallet_sign_preview 可用（不需密钥）。
            "production_default_host_key_configured": false,
            "fail_closed_when_unconfigured": true,
            "fabricated_signature_on_failure": false,
            "replay_protection_note": "nonce 纳入被签规范化载荷；链下重放双花拦截需有状态宿主，本版本不声称已完成。"
        },
        "x402": {
            "introduced_in": "v3.9.2",
            "track": "evm_x402",
            "stablecoin": "USDC (raw units, 6 decimals on mainnets)",
            "methods": [
                "x402_challenge_validate",
                "x402_authorize_preview",
                "x402_settlement_verify",
                "x402_sign"
            ],
            "scheme_supported": ["exact"],
            "authorization_standard": "EIP-3009 transferWithAuthorization",
            "digest_standard": "EIP-712 typed-data (keccak256, dependency-free)",
            "produces_onchain_signature": false,
            "evm_signer_configured_by_default": false,
            "x402_sign_fail_closed": true,
            "fabricated_signature_on_failure": false,
            "exact_settlement_conservation": true,
            "clock_read_in_kernel": false,
            "evm_rpc_connection": false,
            "live_settlement": false,
            "note": "只校验 402 challenge、构造 EIP-3009 授权与 EIP-712 待签 digest、按精确金额守恒校验 facilitator 回执；不持 secp256k1 私钥、不产出链上签名、不连 RPC、不广播不划转。过期判定 now 由调用方传入。"
        },
        "l402": {
            "introduced_in": "v3.9.3",
            "track": "lightning_l402",
            "amount_unit": "msat (integer satoshis*1000)",
            "methods": [
                "l402_parse_challenge",
                "l402_verify",
                "l402_pay"
            ],
            "invoice_networks_supported": ["lnbc", "lntb", "lnbcrt"],
            "invoice_amount_multipliers": ["", "m", "u", "n", "p"],
            "payment_hash_algorithm": "BOLT: payment_hash = SHA256(preimage) (standard sha2, not keccak)",
            "bolt11_bech32_data_decoded": false,
            "macaroon_signature_verified": false,
            "l402_pay_fail_closed": true,
            "exact_settlement_conservation": true,
            "fabricated_preimage_on_failure": false,
            "clock_read_in_kernel": false,
            "lightning_node_connection": false,
            "htlc_creation_or_settlement": false,
            "live_settlement": false,
            "note": "只解析 402 挑战头与 BOLT11 人类可读金额前缀（整数 msat）、校验凭证 macaroon 一致性与 SHA256(preimage)==payment_hash、按精确金额守恒；不解码 bech32 数据段/节点签名、不校验 macaroon 签名（需服务端 root key）、不连闪电节点、不创建或结算 HTLC、不持私钥。payment_hash 须由受信任发票解码服务取得后传入；本地哈希关系通过不代表 HTLC 路由层最终确认。"
        },
        "erc8004": {
            "introduced_in": "v3.9.4",
            "standard": "ERC-8004 Trustless Agents (Identity / Reputation / Validation registries)",
            "methods": [
                "erc8004_identity_check",
                "erc8004_reputation_aggregate",
                "erc8004_validation_decide"
            ],
            "identity_handle": "ERC-721 tokenId <-> DID <-> Ed25519 pubkey <-> tokenURI SHA-256 commitment",
            "reputation_dimensions": ["quality", "speed", "honesty", "availability"],
            "reputation_score_range": "0..=1000 integer, neutral=500",
            "validation_methods": ["stake_rerun", "tee_attestation", "zkml"],
            "validation_rule": "BFT-lite n>=3f+1: a direction needs weight*3 >= total*2 and strictly more than the other",
            "integer_only": true,
            "self_feedback_blocked": true,
            "feedback_nonce_replay_guard": true,
            "onchain_mint_or_write": false,
            "evm_rpc_connection": false,
            "live_registry_read": false,
            "fail_closed_on_bad_identity_or_quorum": true,
            "note": "只做链下可验证决策面：身份绑定/tokenURI 承诺校验、带质押权重的整数信誉聚合（守恒+重放/自评拦截）、独立验证者 BFT-lite 多数裁决；不铸造 ERC-721、不连 RPC、不读取/写入链上注册表（链上状态由调用方取证后以快照传入）、不持私钥不广播。链上锚定写入在后续版本经宿主签名闸门单独授权。"
        },
        "enforceable": {
            "deterministic_route_decision": true,
            "integer_thresholds": true,
            "fail_closed_when_track_unavailable": true,
            "fail_closed_when_signer_unconfigured": true,
            "fail_closed_when_evm_signer_unconfigured": true,
            "host_only_signing_private_key_isolation": true,
            "sandbox_direct_transaction_signing": false,
            "x402_exact_amount_conservation": true,
            "l402_exact_amount_conservation": true,
            "fail_closed_when_lightning_not_configured": true,
            "erc8004_integer_reputation_conservation": true,
            "erc8004_bft_lite_two_thirds_quorum": true,
            "erc8004_fail_closed_on_bad_binding_or_quorum": true,
            "fund_movement": false,
            "key_holding_in_sandbox": false,
            "transaction_signing": false,
            "transaction_broadcast": false,
            "onchain_anchor_write": false,
            "currency_exchange": false
        },
        "provided": {
            "payment_router_status": true,
            "wallet_sign_preview": true,
            "wallet_sign_when_host_configured": true,
            "x402_challenge_validate": true,
            "x402_authorize_preview": true,
            "x402_settlement_verify": true,
            "x402_sign_when_evm_signer_configured": true,
            "l402_parse_challenge": true,
            "l402_verify": true,
            "l402_pay_when_lightning_configured": true,
            "erc8004_identity_check": true,
            "erc8004_reputation_aggregate": true,
            "erc8004_validation_decide": true,
            "router_persistence": false,
            "lightning_node_connection": false,
            "evm_rpc_connection": false,
            "btc_rgb_connection": false,
            "erc8004_onchain_registry_write": false
        },
        "note": "v3.9.0：结算路由 PaymentRouter 纯确定性选路（只决策不动钱，缺轨 fail-closed）。v3.9.1：宿主签名闸门 HostSignerGate——wallet_sign_preview 只校验/规范化待签载荷不碰密钥，wallet_sign 仅宿主经 SignatureBroker 签发，私钥/seed 不进沙盒、回执只含公钥+签名；生产默认 UnconfiguredBroker，wallet_sign 一律 SignerNotConfigured 具名拒签、绝不伪造。v3.9.2：EVM x402(USDC) 纯协议内核——校验 402 challenge、用无依赖 keccak256 构造 EIP-3009 transferWithAuthorization 的 EIP-712 待签 digest（只预览不签）、按精确金额守恒校验 facilitator 回执；x402_sign 因本版无 secp256k1 宿主后端一律 X402_EVM_SIGNER_NOT_CONFIGURED fail-closed，不连 RPC、不广播、不划转、不兑换、不持久化、内核不读时钟。v3.9.3：闪电 L402 纯协议内核——解析 402 挑战头与 BOLT11 整数金额前缀（msat）、校验 SHA256(preimage)==payment_hash 与精确金额守恒；l402_pay 因本版不连闪电节点一律具名 fail-closed，不解码 bech32 数据/节点签名、不持私钥、不创建或结算 HTLC。v3.9.4：ERC-8004 三注册表纯协议面——身份句柄（tokenId↔DID↔Ed25519↔tokenURI SHA-256 承诺）绑定校验、四维整数信誉聚合（守恒/自评拦截/nonce 重放拦截）、独立验证者 BFT-lite（n≥3f+1）裁决；不铸造 ERC-721、不连 RPC、不读写链上注册表、不持私钥。Paymaster v3.9.5、BTC HTLC/RGB v3.9.6、锚定/桥风控 v3.9.7、ZK/OWS/合规 v3.9.8。外部协议采用量与性能数字均为第三方报道口径、非本仓复测。"
    })
}

/// 字节桥：`payment_router_status`（无参或空负载，只读）。
fn handle_payment_router_status(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    serde_json::to_vec(&status_payload())
        .map_err(|e| PluginError::Runtime(format!("payment_router_status 序列化失败: {e}")))
}

/// 解析签名意图（坏 JSON 归类为运行时拒绝；语义错误由闸门返回类型化 PaymentError）。
fn parse_sign_intent(payload: &[u8]) -> PluginResult<SignIntent> {
    if payload.is_empty() {
        return Err(PluginError::Runtime(
            "wallet_sign: empty payload (expected SignIntent JSON)".to_string(),
        ));
    }
    serde_json::from_slice::<SignIntent>(payload)
        .map_err(|e| PluginError::Runtime(format!("wallet_sign: invalid SignIntent payload: {e}")))
}

/// 字节桥：`wallet_sign_preview`（只校验/规范化，不碰密钥、不产出签名）。
///
/// 生产默认闸门（未配置密钥）下仍可用——预览本就不需要密钥。
fn handle_wallet_sign_preview(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let intent = parse_sign_intent(payload)?;
    let gate = signer::HostSignerGate::unconfigured();
    let preview = gate
        .preview(&intent)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&preview)
        .map_err(|e| PluginError::Runtime(format!("wallet_sign_preview 序列化失败: {e}")))
}

/// 字节桥：`wallet_sign`（宿主受限）。
///
/// 生产默认装配 [`signer::UnconfiguredBroker`]：没有任何密钥，任何意图一律
/// [`PaymentError::SignerNotConfigured`] 具名拒绝，返回类型化错误而非伪造签名。真实平台
/// 密钥后端由宿主在装配处注入（独立能力闸门），不经过沙盒 handler。
fn handle_wallet_sign(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let intent = parse_sign_intent(payload)?;
    let gate = signer::HostSignerGate::unconfigured();
    let receipt = gate
        .request_sign(&intent)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&receipt)
        .map_err(|e| PluginError::Runtime(format!("wallet_sign 序列化失败: {e}")))
}

/// 字节桥：`x402_challenge_validate`（只读校验 402 challenge；now 显式传入）。
///
/// 负载：`{"challenge": <X402Challenge>, "now_unix": <u64>}`。坏 JSON/缺字段归类运行时
/// 拒绝，语义错误（过期/金额/合约等）由内核返回类型化 [`X402Error`]。
fn handle_x402_challenge_validate(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct Req {
        challenge: X402Challenge,
        now_unix: u64,
    }
    let req: Req = parse_json("x402_challenge_validate", payload)?;
    req.challenge
        .validate(req.now_unix)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&serde_json::json!({
        "valid": true,
        "scheme": req.challenge.scheme,
        "network": req.challenge.network,
        "amount_raw": req.challenge.max_amount_required_raw,
    }))
    .map_err(|e| PluginError::Runtime(format!("x402_challenge_validate 序列化失败: {e}")))
}

/// 字节桥：`x402_authorize_preview`（构造 EIP-3009 授权与 EIP-712 digest，不签名）。
fn handle_x402_authorize_preview(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct Req {
        challenge: X402Challenge,
        now_unix: u64,
        domain: X402Domain,
        from: EvmAddress,
        valid_after: u64,
        valid_before: u64,
        nonce: Nonce32,
    }
    let req: Req = parse_json("x402_authorize_preview", payload)?;
    let auth = req
        .challenge
        .build_authorization(
            req.now_unix,
            &req.domain,
            req.from,
            req.valid_after,
            req.valid_before,
            req.nonce,
        )
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&auth)
        .map_err(|e| PluginError::Runtime(format!("x402_authorize_preview 序列化失败: {e}")))
}

/// 字节桥：`x402_settlement_verify`（facilitator 回执精确金额守恒校验）。
fn handle_x402_settlement_verify(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct Req {
        required_raw: u128,
        paid_raw: u128,
    }
    let req: Req = parse_json("x402_settlement_verify", payload)?;
    verify_settlement(req.required_raw, req.paid_raw)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&serde_json::json!({
        "conserved": true,
        "required_raw": req.required_raw,
        "paid_raw": req.paid_raw,
    }))
    .map_err(|e| PluginError::Runtime(format!("x402_settlement_verify 序列化失败: {e}")))
}

/// 字节桥：`x402_sign`（EVM 链上签发）。
///
/// 本版**没有** secp256k1 宿主签名后端：对任何请求（含空/坏负载）一律具名
/// [`X402Error::EvmSignerNotConfigured`] fail-closed，绝不伪造链上签名，也不解析私钥。
fn handle_x402_sign(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    Err(PluginError::Runtime(
        X402Error::EvmSignerNotConfigured.to_string(),
    ))
}

/// 字节桥：`l402_parse_challenge`（只读解析 402 挑战头 + BOLT11 金额，不连节点）。
fn handle_l402_parse_challenge(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct Req {
        www_authenticate: String,
    }
    let req: Req = parse_json("l402_parse_challenge", payload)?;
    let challenge = L402Challenge::parse_www_authenticate(&req.www_authenticate)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    let amount_msat = challenge
        .invoice_amount_msat()
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&serde_json::json!({
        "scheme": L402_SCHEME,
        "macaroon": challenge.macaroon,
        "invoice": challenge.invoice,
        "amount_msat": amount_msat,
    }))
    .map_err(|e| PluginError::Runtime(format!("l402_parse_challenge 序列化失败: {e}")))
}

/// 字节桥：`l402_verify`（凭证 macaroon 一致 + SHA256(preimage)==payment_hash + 精确金额）。
///
/// `payment_hash_hex` 由调用方从**受信任**发票解码服务取得后传入；内核不自行 bech32 解码。
fn handle_l402_verify(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct Req {
        www_authenticate: String,
        payment_hash_hex: String,
        preimage_hex: String,
        paid_msat: u64,
    }
    let req: Req = parse_json("l402_verify", payload)?;
    let challenge = L402Challenge::parse_www_authenticate(&req.www_authenticate)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    let expected = PaymentHash::from_hex(&req.payment_hash_hex)
        .ok_or_else(|| PluginError::Runtime("L402_INVALID_PAYMENT_HASH".to_string()))?;
    let cred = L402Credential::new(&challenge, &req.preimage_hex)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    cred.verify(&challenge, &expected, req.paid_msat)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    let required_msat = challenge
        .invoice_amount_msat()
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&serde_json::json!({
        "settled": true,
        "required_msat": required_msat,
        "paid_msat": req.paid_msat,
        "payment_hash_hex": expected.to_hex(),
        "authorization_header": cred.to_authorization_header(),
    }))
    .map_err(|e| PluginError::Runtime(format!("l402_verify 序列化失败: {e}")))
}

/// 字节桥：`l402_pay`（闪电真实支付/开票）。
///
/// 本版**不连接任何闪电节点**、不持私钥、不创建或结算 HTLC：对任何请求一律具名
/// [`L402Error`] 风格 fail-closed，绝不伪造发票或原像。
fn handle_l402_pay(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    Err(PluginError::Runtime(
        "L402_LIGHTNING_NOT_CONFIGURED".to_string(),
    ))
}

/// 字节桥：`erc8004_identity_check`（链下身份绑定校验，纯只读）。
///
/// 调用方把从受信任索引/RPC 取证的注册表快照（身份条目数组）与待核四元组一并传入；
/// 内核不连 RPC、不铸造、不写链。
fn handle_erc8004_identity_check(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct IdentityEntry {
        token_id: u64,
        did: String,
        pubkey_hex: String,
        uri_hash_hex: String,
    }
    #[derive(serde::Deserialize)]
    struct Req {
        identities: Vec<IdentityEntry>,
        token_id: u64,
        did: String,
        pubkey_hex: String,
        uri_hash_hex: String,
    }
    let req: Req = parse_json("erc8004_identity_check", payload)?;
    let mut reg = erc8004::IdentityRegistry::new();
    for e in &req.identities {
        reg.mint_with_hash(e.token_id, &e.did, &e.pubkey_hex, &e.uri_hash_hex)
            .map_err(|err| PluginError::Runtime(err.to_string()))?;
    }
    let bound = reg.verify_binding(req.token_id, &req.did, &req.pubkey_hex, &req.uri_hash_hex);
    serde_json::to_vec(&serde_json::json!({
        "token_id": req.token_id,
        "did": req.did,
        "binding_matches": bound,
        "registry_entries": reg.len()
    }))
    .map_err(|e| PluginError::Runtime(format!("erc8004_identity_check 序列化失败: {e}")))
}

fn dim_from_str(s: &str) -> PluginResult<erc8004::ReputationDimension> {
    match s {
        "quality" => Ok(erc8004::ReputationDimension::Quality),
        "speed" => Ok(erc8004::ReputationDimension::Speed),
        "honesty" => Ok(erc8004::ReputationDimension::Honesty),
        "availability" => Ok(erc8004::ReputationDimension::Availability),
        other => Err(PluginError::Runtime(format!(
            "ERC8004_INVALID_DIMENSION: {other}"
        ))),
    }
}

/// 字节桥：`erc8004_reputation_aggregate`（无状态整数聚合，纯只读）。
fn handle_erc8004_reputation_aggregate(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct IdentityEntry {
        token_id: u64,
        did: String,
        pubkey_hex: String,
        uri_hash_hex: String,
    }
    #[derive(serde::Deserialize)]
    struct FeedbackEntry {
        reviewer: u64,
        subject: u64,
        dimension: String,
        score: i16,
        weight: u64,
        nonce: u64,
    }
    #[derive(serde::Deserialize)]
    struct Req {
        identities: Vec<IdentityEntry>,
        feedback: Vec<FeedbackEntry>,
    }
    let req: Req = parse_json("erc8004_reputation_aggregate", payload)?;
    let mut ids = erc8004::IdentityRegistry::new();
    for e in &req.identities {
        ids.mint_with_hash(e.token_id, &e.did, &e.pubkey_hex, &e.uri_hash_hex)
            .map_err(|err| PluginError::Runtime(err.to_string()))?;
    }
    let mut fbs: Vec<erc8004::Feedback> = Vec::with_capacity(req.feedback.len());
    for f in &req.feedback {
        fbs.push(erc8004::Feedback {
            reviewer: f.reviewer,
            subject: f.subject,
            dimension: dim_from_str(&f.dimension)?,
            score: f.score,
            weight: f.weight,
            nonce: f.nonce,
        });
    }
    let agg = erc8004::aggregate_feedback(&fbs, &ids)
        .map_err(|err| PluginError::Runtime(err.to_string()))?;
    let subjects: serde_json::Value = serde_json::Value::Array(
        agg.iter()
            .map(|(subject, snap)| {
                serde_json::json!({
                    "subject": subject,
                    "quality": snap.score_01k(erc8004::ReputationDimension::Quality).ok(),
                    "speed": snap.score_01k(erc8004::ReputationDimension::Speed).ok(),
                    "honesty": snap.score_01k(erc8004::ReputationDimension::Honesty).ok(),
                    "availability": snap.score_01k(erc8004::ReputationDimension::Availability).ok(),
                })
            })
            .collect(),
    );
    serde_json::to_vec(&serde_json::json!({ "subjects": subjects }))
        .map_err(|e| PluginError::Runtime(format!("erc8004_reputation_aggregate 序列化失败: {e}")))
}

/// 字节桥：`erc8004_validation_decide`（独立验证者 BFT-lite 裁决，纯只读）。
fn handle_erc8004_validation_decide(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct IdentityEntry {
        token_id: u64,
        did: String,
        pubkey_hex: String,
        uri_hash_hex: String,
    }
    #[derive(serde::Deserialize)]
    struct VoteEntry {
        validator: u64,
        method: String,
        verdict: String,
        weight: u64,
        evidence_hash_hex: String,
    }
    #[derive(serde::Deserialize)]
    struct Req {
        identities: Vec<IdentityEntry>,
        votes: Vec<VoteEntry>,
    }
    let req: Req = parse_json("erc8004_validation_decide", payload)?;
    let mut ids = erc8004::IdentityRegistry::new();
    for e in &req.identities {
        ids.mint_with_hash(e.token_id, &e.did, &e.pubkey_hex, &e.uri_hash_hex)
            .map_err(|err| PluginError::Runtime(err.to_string()))?;
    }
    let mut tally = erc8004::ValidationTally::new();
    for v in &req.votes {
        let method = match v.method.as_str() {
            "stake_rerun" => erc8004::ValidationMethod::StakeRerun,
            "tee_attestation" => erc8004::ValidationMethod::TeeAttestation,
            "zkml" => erc8004::ValidationMethod::Zkml,
            other => {
                return Err(PluginError::Runtime(format!(
                    "ERC8004_INVALID_METHOD: {other}"
                )))
            }
        };
        let verdict = match v.verdict.as_str() {
            "valid" => erc8004::ValidationVerdict::Valid,
            "invalid" => erc8004::ValidationVerdict::Invalid,
            other => {
                return Err(PluginError::Runtime(format!(
                    "ERC8004_INVALID_VERDICT: {other}"
                )))
            }
        };
        tally
            .record_vote_checked(
                erc8004::ValidationVote {
                    validator: v.validator,
                    method,
                    verdict,
                    weight: v.weight,
                },
                &ids,
                &v.evidence_hash_hex,
            )
            .map_err(|err| PluginError::Runtime(err.to_string()))?;
    }
    let decision = tally
        .decide()
        .map_err(|err| PluginError::Runtime(err.to_string()))?;
    let d = match decision {
        erc8004::ValidationDecision::ConfirmedValid => "confirmed_valid",
        erc8004::ValidationDecision::ConfirmedInvalid => "confirmed_invalid",
        erc8004::ValidationDecision::Inconclusive => "inconclusive",
    };
    serde_json::to_vec(&serde_json::json!({
        "decision": d,
        "valid_weight": tally.valid_weight.to_string(),
        "invalid_weight": tally.invalid_weight.to_string(),
        "total_weight": tally.total_weight.to_string()
    }))
    .map_err(|e| PluginError::Runtime(format!("erc8004_validation_decide 序列化失败: {e}")))
}

fn parse_json<'a, T: serde::Deserialize<'a>>(method: &str, payload: &'a [u8]) -> PluginResult<T> {
    if payload.is_empty() {
        return Err(PluginError::Runtime(format!(
            "{method}: empty payload (expected JSON)"
        )));
    }
    serde_json::from_slice::<T>(payload)
        .map_err(|e| PluginError::Runtime(format!("{method}: invalid payload: {e}")))
}

/// 把结算/签名内核处理器注册到 T0 native 运行时（随系统插件装配调用）。
///
/// v3.9.0 注册只读状态查询；v3.9.1 追加 `wallet_sign_preview`（只读预览）与宿主受限
/// `wallet_sign`（生产默认未配置密钥，具名 fail-closed）。v3.9.2 追加 x402 四个方法：
/// challenge 校验、授权预览、结算守恒校验（只读）与 `x402_sign`（无 secp256k1 后端，
/// 具名 fail-closed）。选路执行/链上写在后续小版本接线，未接线方法不注册（NotFound）。
pub fn register(rt: &mut NativeRuntime) {
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_PAYMENT_ROUTER_STATUS,
        handle_payment_router_status,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_WALLET_SIGN_PREVIEW,
        handle_wallet_sign_preview,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_WALLET_SIGN,
        handle_wallet_sign,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_X402_CHALLENGE_VALIDATE,
        handle_x402_challenge_validate,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_X402_AUTHORIZE_PREVIEW,
        handle_x402_authorize_preview,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_X402_SETTLEMENT_VERIFY,
        handle_x402_settlement_verify,
    );
    rt.register_handler(PAYMENT_ROUTER_PLUGIN, METHOD_X402_SIGN, handle_x402_sign);
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_L402_PARSE_CHALLENGE,
        handle_l402_parse_challenge,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_L402_VERIFY,
        handle_l402_verify,
    );
    rt.register_handler(PAYMENT_ROUTER_PLUGIN, METHOD_L402_PAY, handle_l402_pay);
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_ERC8004_IDENTITY_CHECK,
        handle_erc8004_identity_check,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_ERC8004_REPUTATION_AGGREGATE,
        handle_erc8004_reputation_aggregate,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_ERC8004_VALIDATION_DECIDE,
        handle_erc8004_validation_decide,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payment_error_codes_are_stable() {
        assert_eq!(
            PaymentError::NonPositiveAmount.to_string(),
            "PAYMENT_NON_POSITIVE_AMOUNT: amount_micro must be > 0"
        );
        assert!(PaymentError::RouteTrackUnavailable {
            required: PaymentTrack::BtcRgbHtlc
        }
        .to_string()
        .contains("btc_rgb_htlc"));
        assert!(PaymentError::RoutingPolicyInvalid {
            instant_cap_micro: 10,
            large_floor_micro: 5
        }
        .to_string()
        .starts_with("PAYMENT_ROUTING_POLICY_INVALID"));
        assert!(PaymentError::DefiWithoutSmartContract
            .to_string()
            .starts_with("PAYMENT_DEFI_WITHOUT_SMART_CONTRACT"));
        assert!(PaymentError::NoAvailableTrack
            .to_string()
            .starts_with("PAYMENT_NO_AVAILABLE_TRACK"));
    }

    #[test]
    fn status_payload_is_honest_and_readonly() {
        let s = status_payload();
        assert_eq!(s["plugin"], PAYMENT_ROUTER_PLUGIN);
        assert_eq!(s["enforceable"]["deterministic_route_decision"], true);
        assert_eq!(s["enforceable"]["fund_movement"], false);
        assert_eq!(s["enforceable"]["key_holding_in_sandbox"], false);
        assert_eq!(
            s["enforceable"]["sandbox_direct_transaction_signing"],
            false
        );
        assert_eq!(
            s["enforceable"]["host_only_signing_private_key_isolation"],
            true
        );
        assert_eq!(
            s["enforceable"]["fail_closed_when_signer_unconfigured"],
            true
        );
        assert_eq!(s["enforceable"]["onchain_anchor_write"], false);
        assert_eq!(s["provided"]["payment_router_status"], true);
        assert_eq!(s["provided"]["wallet_sign_preview"], true);
        assert_eq!(s["provided"]["x402_challenge_validate"], true);
        assert_eq!(s["provided"]["x402_authorize_preview"], true);
        assert_eq!(s["provided"]["x402_settlement_verify"], true);
        assert_eq!(s["provided"]["l402_parse_challenge"], true);
        assert_eq!(s["provided"]["l402_verify"], true);
        assert_eq!(s["provided"]["l402_pay_when_lightning_configured"], true);
        assert_eq!(s["provided"]["erc8004_identity_check"], true);
        assert_eq!(s["provided"]["erc8004_reputation_aggregate"], true);
        assert_eq!(s["provided"]["erc8004_validation_decide"], true);
        assert_eq!(s["provided"]["erc8004_onchain_registry_write"], false);
        assert_eq!(s["provided"]["lightning_node_connection"], false);
        assert_eq!(s["tracks"].as_array().unwrap().len(), 3);
        // v3.9.1 两个 + v3.9.2 x402 + v3.9.3 l402 + v3.9.4 erc8004 只读协议能力。
        assert_eq!(s["capabilities_declared"].as_array().unwrap().len(), 5);
        assert_eq!(
            s["enforceable"]["fail_closed_when_evm_signer_unconfigured"],
            true
        );
        assert_eq!(s["enforceable"]["x402_exact_amount_conservation"], true);
        assert_eq!(s["enforceable"]["l402_exact_amount_conservation"], true);
        assert_eq!(
            s["enforceable"]["fail_closed_when_lightning_not_configured"],
            true
        );
        assert_eq!(s["x402"]["introduced_in"], "v3.9.2");
        assert_eq!(s["x402"]["produces_onchain_signature"], false);
        assert_eq!(s["x402"]["live_settlement"], false);
        assert_eq!(s["l402"]["introduced_in"], "v3.9.3");
        assert_eq!(s["l402"]["live_settlement"], false);
        assert_eq!(s["l402"]["lightning_node_connection"], false);
        assert_eq!(s["l402"]["l402_pay_fail_closed"], true);
        assert_eq!(s["erc8004"]["introduced_in"], "v3.9.4");
        assert_eq!(s["erc8004"]["onchain_mint_or_write"], false);
        assert_eq!(s["erc8004"]["evm_rpc_connection"], false);
        assert_eq!(s["erc8004"]["self_feedback_blocked"], true);
        assert_eq!(s["enforceable"]["erc8004_bft_lite_two_thirds_quorum"], true);
        assert_eq!(
            s["enforceable"]["erc8004_fail_closed_on_bad_binding_or_quorum"],
            true
        );
        assert_eq!(s["signing"]["private_key_enters_sandbox"], false);
        assert_eq!(s["signing"]["seed_enters_sandbox"], false);
        assert_eq!(
            s["signing"]["production_default_host_key_configured"],
            false
        );
        assert_eq!(s["signing"]["fail_closed_when_unconfigured"], true);
        assert_eq!(s["signing"]["fabricated_signature_on_failure"], false);
    }

    #[test]
    fn preview_handler_works_but_sign_handler_is_named_refused_by_default() {
        let digest = serde_json::Value::Array(vec![serde_json::Value::Number(7.into()); 32]);
        let intent = serde_json::json!({
            "track": "evm_x402",
            "domain": "x402.agent-universe.local",
            "message_digest": digest,
            "spend_cap_micro": 1000u64,
            "nonce": 1u64,
            "auth_only": false
        });
        let payload = serde_json::to_vec(&intent).unwrap();

        // 预览成功且不产出签名。
        let prev_bytes = handle_wallet_sign_preview(METHOD_WALLET_SIGN_PREVIEW, &payload).unwrap();
        let prev: serde_json::Value = serde_json::from_slice(&prev_bytes).unwrap();
        assert_eq!(prev["signing_requires_host_key"], true);
        assert_eq!(prev["host_key_configured"], false);
        assert!(prev.get("signature_hex").is_none());

        // 生产默认签发具名拒绝，错误串稳定且不伪造签名。
        let err = handle_wallet_sign(METHOD_WALLET_SIGN, &payload).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("PAYMENT_SIGNER_NOT_CONFIGURED"), "got {msg}");

        // 坏输入（0 上限）先被类型化策略拒绝。
        let mut bad = intent.clone();
        bad["spend_cap_micro"] = 0.into();
        let bad_payload = serde_json::to_vec(&bad).unwrap();
        let err2 = handle_wallet_sign_preview(METHOD_WALLET_SIGN_PREVIEW, &bad_payload)
            .unwrap_err()
            .to_string();
        assert!(
            err2.contains("PAYMENT_NON_POSITIVE_SPEND_CAP"),
            "got {err2}"
        );

        // 空负载拒绝。
        assert!(handle_wallet_sign(METHOD_WALLET_SIGN, &[]).is_err());
    }

    #[test]
    fn l402_handlers_parse_verify_and_pay_fail_closed() {
        use sha2::{Digest, Sha256};
        let pre = [9u8; 32];
        let hash_hex = hex::encode(Sha256::digest(pre));
        let pre_hex = hex::encode(pre);
        let header = r#"L402 macaroon="MAC9", invoice="lnbc100n1p""#; // 10_000 msat

        // parse_challenge。
        let p = serde_json::to_vec(&serde_json::json!({ "www_authenticate": header })).unwrap();
        let out: serde_json::Value = serde_json::from_slice(
            &handle_l402_parse_challenge(METHOD_L402_PARSE_CHALLENGE, &p).unwrap(),
        )
        .unwrap();
        assert_eq!(out["amount_msat"], 10_000);
        assert_eq!(out["macaroon"], "MAC9");

        // verify 成功：正确 hash + 精确金额。
        let v = serde_json::to_vec(&serde_json::json!({
            "www_authenticate": header,
            "payment_hash_hex": hash_hex,
            "preimage_hex": pre_hex,
            "paid_msat": 10_000u64,
        }))
        .unwrap();
        let ok: serde_json::Value =
            serde_json::from_slice(&handle_l402_verify(METHOD_L402_VERIFY, &v).unwrap()).unwrap();
        assert_eq!(ok["settled"], true);
        assert_eq!(ok["authorization_header"], format!("L402 MAC9:{pre_hex}"));

        // 金额不守恒拒绝。
        let mut bad = serde_json::from_slice::<serde_json::Value>(&v).unwrap();
        bad["paid_msat"] = 9_999.into();
        let badp = serde_json::to_vec(&bad).unwrap();
        assert!(handle_l402_verify(METHOD_L402_VERIFY, &badp).is_err());

        // 伪造原像（hash 不符）拒绝。
        let mut wrong_pre = [0u8; 32];
        wrong_pre[0] = 1;
        let mut bad2 = serde_json::from_slice::<serde_json::Value>(&v).unwrap();
        bad2["preimage_hex"] = hex::encode(wrong_pre).into();
        let bad2p = serde_json::to_vec(&bad2).unwrap();
        assert!(handle_l402_verify(METHOD_L402_VERIFY, &bad2p).is_err());

        // 非法 payment_hash 拒绝。
        let mut bad3 = serde_json::from_slice::<serde_json::Value>(&v).unwrap();
        bad3["payment_hash_hex"] = "0x12".into();
        let bad3p = serde_json::to_vec(&bad3).unwrap();
        assert!(handle_l402_verify(METHOD_L402_VERIFY, &bad3p).is_err());

        // pay：不连闪电节点，一律具名 fail-closed。
        let err = handle_l402_pay(METHOD_L402_PAY, &v)
            .unwrap_err()
            .to_string();
        assert!(err.contains("L402_LIGHTNING_NOT_CONFIGURED"), "got {err}");

        // 空负载拒绝。
        assert!(handle_l402_parse_challenge(METHOD_L402_PARSE_CHALLENGE, &[]).is_err());
        assert!(handle_l402_verify(METHOD_L402_VERIFY, &[]).is_err());
    }

    #[test]
    fn erc8004_handlers_identity_reputation_validation_roundtrip() {
        let pk1 = "11".repeat(32);
        let pk2 = "22".repeat(32);
        let pk3 = "33".repeat(32);
        let uh1 = erc8004::hash_token_uri("ipfs://card-alpha");
        let uh2 = erc8004::hash_token_uri("ipfs://card-beta");
        let uh3 = erc8004::hash_token_uri("ipfs://card-gamma");
        let identities = serde_json::json!([
            {"token_id": 1, "did": "did:tw:alpha", "pubkey_hex": pk1, "uri_hash_hex": uh1},
            {"token_id": 2, "did": "did:tw:beta", "pubkey_hex": pk2, "uri_hash_hex": uh2},
            {"token_id": 3, "did": "did:tw:gamma", "pubkey_hex": pk3, "uri_hash_hex": uh3}
        ]);

        // identity_check：匹配 true。
        let idp = serde_json::to_vec(&serde_json::json!({
            "identities": identities,
            "token_id": 1,
            "did": "did:tw:alpha",
            "pubkey_hex": pk1,
            "uri_hash_hex": uh1
        }))
        .unwrap();
        let idout: serde_json::Value = serde_json::from_slice(
            &handle_erc8004_identity_check(METHOD_ERC8004_IDENTITY_CHECK, &idp).unwrap(),
        )
        .unwrap();
        assert_eq!(idout["binding_matches"], true);
        assert_eq!(idout["registry_entries"], 3);

        // identity_check：公钥不符 -> false（非错误，正常校验结论）。
        let mut badid = serde_json::from_slice::<serde_json::Value>(&idp).unwrap();
        badid["pubkey_hex"] = serde_json::json!(pk3);
        let badidp = serde_json::to_vec(&badid).unwrap();
        let badidout: serde_json::Value = serde_json::from_slice(
            &handle_erc8004_identity_check(METHOD_ERC8004_IDENTITY_CHECK, &badidp).unwrap(),
        )
        .unwrap();
        assert_eq!(badidout["binding_matches"], false);

        // 非法身份快照（重复 DID）-> 具名 fail-closed。
        let dup = serde_json::json!({
            "identities": [
                {"token_id": 1, "did": "did:x", "pubkey_hex": pk1, "uri_hash_hex": uh1},
                {"token_id": 2, "did": "did:x", "pubkey_hex": pk2, "uri_hash_hex": uh2}
            ],
            "token_id": 1, "did": "did:x", "pubkey_hex": pk1, "uri_hash_hex": uh1
        });
        let dupp = serde_json::to_vec(&dup).unwrap();
        assert!(handle_erc8004_identity_check(METHOD_ERC8004_IDENTITY_CHECK, &dupp).is_err());

        // reputation_aggregate：两正一负加权 -> quality=750。
        let rp = serde_json::to_vec(&serde_json::json!({
            "identities": identities,
            "feedback": [
                {"reviewer": 1, "subject": 2, "dimension": "quality", "score": 100, "weight": 30, "nonce": 1},
                {"reviewer": 3, "subject": 2, "dimension": "quality", "score": -100, "weight": 10, "nonce": 1}
            ]
        }))
        .unwrap();
        let rout: serde_json::Value = serde_json::from_slice(
            &handle_erc8004_reputation_aggregate(METHOD_ERC8004_REPUTATION_AGGREGATE, &rp).unwrap(),
        )
        .unwrap();
        assert_eq!(rout["subjects"][0]["subject"], 2);
        assert_eq!(rout["subjects"][0]["quality"], 750);

        // 自评 -> 具名拒绝。
        let selfp = serde_json::to_vec(&serde_json::json!({
            "identities": identities,
            "feedback": [
                {"reviewer": 2, "subject": 2, "dimension": "quality", "score": 10, "weight": 1, "nonce": 1}
            ]
        }))
        .unwrap();
        assert!(
            handle_erc8004_reputation_aggregate(METHOD_ERC8004_REPUTATION_AGGREGATE, &selfp)
                .is_err()
        );

        // validation_decide：2 valid + 1 invalid（总权重 3）-> confirmed_valid。
        let ev = "aa".repeat(32);
        let vp = serde_json::to_vec(&serde_json::json!({
            "identities": identities,
            "votes": [
                {"validator": 1, "method": "stake_rerun", "verdict": "valid", "weight": 1, "evidence_hash_hex": ev},
                {"validator": 2, "method": "tee_attestation", "verdict": "valid", "weight": 1, "evidence_hash_hex": ev},
                {"validator": 3, "method": "zkml", "verdict": "invalid", "weight": 1, "evidence_hash_hex": ev}
            ]
        }))
        .unwrap();
        let vout: serde_json::Value = serde_json::from_slice(
            &handle_erc8004_validation_decide(METHOD_ERC8004_VALIDATION_DECIDE, &vp).unwrap(),
        )
        .unwrap();
        assert_eq!(vout["decision"], "confirmed_valid");

        // 空投票 -> 非法计账 fail-closed。
        let empty =
            serde_json::to_vec(&serde_json::json!({ "identities": identities, "votes": [] }))
                .unwrap();
        assert!(
            handle_erc8004_validation_decide(METHOD_ERC8004_VALIDATION_DECIDE, &empty).is_err()
        );
    }
}
