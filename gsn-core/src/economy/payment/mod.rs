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

pub mod anchorguard;
pub mod erc8004;
pub mod htlc;
pub mod l402;
pub mod paymaster;
pub mod rgb;
pub mod router;
pub mod signer;
pub mod x402;
pub mod zkwallet;

pub use anchorguard::{
    decide_bridge_transfer as bridge_transfer_decide,
    relay_bridge_transfer as bridge_relay_fail_closed,
    verify_anchor_finality as anchor_finality_verify, AnchorError, AnchorEvidence, AnchorPolicy,
    AnchorReceipt, BridgeDecision, BridgeError, BridgePolicy, BridgeTransfer,
};
pub use erc8004::{
    aggregate_feedback, hash_token_uri, validate_did, validate_pubkey_hex, Erc8004Error, Feedback,
    IdentityRecord, IdentityRegistry, ReputationAccumulator, ReputationDimension,
    ReputationRegistry, ReputationSnapshot, ValidationDecision, ValidationMethod, ValidationTally,
    ValidationVerdict, ValidationVote,
};
pub use htlc::{
    finalize_htlc as htlc_finalize_fail_closed, p2wsh_program, parse_htlc_script,
    verify_amount_conservation as htlc_verify_amount_conservation,
    verify_hashlock as htlc_verify_hashlock, verify_timeout as htlc_verify_timeout, HtlcError,
    HtlcOffer, HtlcReceipt, HtlcScript, LocktimeUnits,
};
pub use l402::{
    parse_bolt11_amount_msat, verify_settlement as l402_verify_settlement, L402Challenge,
    L402Credential, L402Error, PaymentHash, Preimage, L402_SCHEME,
};
pub use paymaster::{
    parse_paymaster_and_data, preview_paymaster_signature, sign_paymaster_data, PaymasterError,
    PaymasterSignPreview, SponsorPolicy, SponsorshipApproval, UserOperation, UserOperationGas,
};
pub use rgb::{
    verify_opret_commitment, verify_tapret_placement, CommitmentClaim, CommitmentReceipt,
    CommitmentScheme, RgbError,
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
pub use zkwallet::{
    enforce_compliance_decision as compliance_enforce_fail_closed,
    execute_ows_payment as ows_execute_fail_closed, route_ows_wallet, screen_compliance,
    verify_zk_payment_intent, verify_zk_proof as zk_proof_fail_closed, ComplianceError,
    CompliancePolicy, ComplianceSubject, OwsError, OwsIntent, OwsPolicy, OwsRoute, OwsTrack,
    ScreeningDecision, ScreeningVerdict, WalletCapability, ZkError, ZkIntent, ZkPolicy, ZkReceipt,
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

/// PMB 方法：ERC-4337 UserOperation 气体字段与 paymaster(And)Data 纯校验 + gas 上估（不广播）。
pub const METHOD_PAYMASTER_USEROP_VALIDATE: &str = "paymaster_userop_validate";

/// PMB 方法：ERC-4337 Paymaster 赞助决策（白名单/单笔上限/累计预算/时间窗，fail-closed）。
pub const METHOD_PAYMASTER_DECIDE_SPONSORSHIP: &str = "paymaster_decide_sponsorship";

/// PMB 方法：ERC-4337 Paymaster 担保签发（本版无宿主签名后端，一律具名 fail-closed，不伪造）。
pub const METHOD_PAYMASTER_SIGN: &str = "paymaster_sign";

/// PMB 方法：BTC HTLC 链下纯校验（SHA256 hashlock + CLTV 超时 + sats 守恒 + P2WSH 脚本结构，不连节点）。
pub const METHOD_BTC_HTLC_VERIFY: &str = "btc_htlc_verify";

/// PMB 方法：BTC HTLC 结算/广播（本版不连比特币节点、不持钥、不签名，一律具名 fail-closed）。
pub const METHOD_BTC_HTLC_FINALIZE: &str = "btc_htlc_finalize";

/// PMB 方法：RGB 客户端验证承诺位置校验（opret/tapret-first 锚定形状，纯只读，不做状态转换）。
pub const METHOD_RGB_COMMITMENT_VERIFY: &str = "rgb_commitment_verify";

/// PMB 方法：链上锚定最终性校验（确认数/最终性/重组深度/承诺一致性，取证快照传入，纯只读）。
pub const METHOD_ANCHOR_FINALITY_CHECK: &str = "anchor_finality_check";

/// PMB 方法：跨链桥风控放行决策（开关/暂停/白名单/额度/速率/累计，纯决策 fail-closed）。
pub const METHOD_BRIDGE_TRANSFER_DECIDE: &str = "bridge_transfer_decide";

/// PMB 方法：真实跨链中继（锁仓/铸造/广播）。本版无桥后端，一律 BRIDGE_NOT_CONFIGURED。
pub const METHOD_BRIDGE_RELAY: &str = "bridge_relay";

/// PMB 方法：ZK 隐私支付意图结构/额度/空值符/披露标签校验（不做 SNARK 验证，proof 恒不验）。
pub const METHOD_ZK_PAYMENT_INTENT_CHECK: &str = "zk_payment_intent_check";

/// PMB 方法：OWS 统一钱包确定性选轨（只决策，缺轨/未配置 fail-closed）。
pub const METHOD_OWS_WALLET_ROUTE: &str = "ows_wallet_route";

/// PMB 方法：合规筛查（仅用调用方名单/阈值，allow/report/block；不联网不执行）。
pub const METHOD_COMPLIANCE_SCREEN: &str = "compliance_screen";

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
    let paymaster_status = serde_json::json!({
        "introduced_in": "v3.9.5",
        "standard": "ERC-4337 Account Abstraction Paymaster (v0.6 paymasterAndData / v0.7 paymaster+paymasterData)",
        "methods": [
            "paymaster_userop_validate",
            "paymaster_decide_sponsorship",
            "paymaster_sign"
        ],
        "gas_cost_formula": "estimated_max_gas_cost = (preVerificationGas + verificationGasLimit + callGasLimit + paymasterVerificationGasLimit + paymasterPostOpGasLimit) * maxFeePerGas",
        "amount_unit": "wei raw units (integer, no float / no ETH price)",
        "decision_inputs": ["enabled", "chain_id", "valid_after/valid_until", "sender whitelist", "per-op gas cost cap", "cumulative budget"],
        "integer_checked_arithmetic": true,
        "default_deny_when_unconfigured_or_over_limit": true,
        "whitelist_empty_denies_all_by_default": true,
        "produces_paymaster_signature": false,
        "paymaster_sign_fail_closed": true,
        "fabricated_signature_on_failure": false,
        "bundler_or_entrypoint_connection": false,
        "useroperation_broadcast": false,
        "live_gas_payment": false,
        "paymaster_stake_or_deposit": false,
        "onchain_nonce_or_balance_check": false,
        "clock_read_in_kernel": false,
        "note": "只做链下确定性赞助决策面：校验 UserOperation 气体/费用字段、解析并对齐 v0.6/v0.7 paymaster 字段、按整数上估 gas 成本并在显式策略（开关/链/时间窗/白名单/单笔上限/累计预算）下决定是否赞助；不连 bundler/EntryPoint RPC、不聚合不广播 UserOp、不持 Paymaster 私钥不产出担保签名（paymaster_sign 一律 PAYMASTER_SIGNER_NOT_CONFIGURED fail-closed）、不垫付 gas、不在链上质押/充值、不校验链上 nonce/存款（由调用方取证）。累计 spent 与 now 均由调用方显式传入。"
    });
    let htlc_status = serde_json::json!({
        "introduced_in": "v3.9.6",
        "track": "btc_rgb_htlc",
        "amount_unit": "satoshis (integer sats, no float)",
        "methods": [
            "btc_htlc_verify",
            "btc_htlc_finalize"
        ],
        "hashlock_rule": "SHA256(preimage) == payment_hash (standard sha2, not keccak)",
        "timeout_rule": "CLTV absolute locktime (BIP65 CHECKLOCKTIMEVERIFY); <500_000_000 block height, >=500_000_000 unix seconds; canonical minimal push",
        "p2wsh_rule": "witnessProgram == SHA256(witnessScript), standard IF/ELSE HTLC template (receiver hash branch / sender timeout branch)",
        "integer_sat_amount_conservation": true,
        "canonical_locktime_encoding_check": true,
        "htlc_finalize_fail_closed": true,
        "fabricated_preimage_or_tx_on_failure": false,
        "clock_read_in_kernel": false,
        "current_height_or_time_provided_by_caller": true,
        "btc_node_connection": false,
        "utxo_or_block_read": false,
        "transaction_construction_or_signing": false,
        "transaction_broadcast": false,
        "secp256k1_signature_verification": false,
        "full_bitcoin_script_interpreter": false,
        "note": "只做链下确定性校验面：SHA256 哈希锁、CLTV 超时（当前高度/时间由调用方传入，内核不读时钟）、整数 sats 金额守恒 offered+fee==total、P2WSH witnessProgram=SHA256(witnessScript) 与标准 HTLC witnessScript 模板逐字节校验；模板内 33 字节公钥只做长度/位置结构校验，不验曲线点。不连比特币节点、不读 UTXO/区块、不构造/签名/广播交易、不持私钥、不实现完整 Bitcoin Script 解释器、不结算 HTLC（btc_htlc_finalize 一律 HTLC_NODE_NOT_CONFIGURED fail-closed）。校验通过仅表示给定取证数据自洽，不代表链上打包或最终确认。"
    });
    let rgb_status = serde_json::json!({
        "introduced_in": "v3.9.6",
        "standard": "RGB client-side validation commitment anchoring (opret-first / tapret-first)",
        "methods": ["rgb_commitment_verify"],
        "schemes_supported": ["opret_first", "tapret_first"],
        "commitment_size_bytes": 32,
        "opret_rule": "first unique OP_RETURN output must be exactly OP_PUSH32 <32 commitment>",
        "tapret_rule": "anchored output must be segwit v1 OP_1 <32> and be the first taproot output",
        "tapret_tweak_derivation": false,
        "rgb_state_transition_validation": false,
        "rgb_genesis_or_seal_validation": false,
        "onchain_anchor_write": false,
        "btc_node_connection": false,
        "note": "只做 RGB 承诺锚定的链下位置/形状校验：opret-first 定位唯一的 OP_RETURN <32 commitment> 输出并逐字节比对；tapret-first 校验被锚定输出是 segwit v1 OP_1 <32> 程序且为交易中第一个 Taproot 输出（本版不做内部键 tweak 密码学推导，receipt.derivation_verified 诚实置 false，require_derivation=true 一律 RGB_TAPRET_DERIVATION_NOT_IMPLEMENTED）。不做 RGB 状态转换/genesis/seal/inventory 客户端验证，不解析 bech32m，不连节点、不读区块、不构造广播交易、不持私钥。"
    });
    let anchor_status = serde_json::json!({
        "introduced_in": "v3.9.7",
        "methods": ["anchor_finality_check"],
        "decision_inputs": ["required_confirmations", "require_finalized", "max_tolerated_reorg_depth", "confirmations (evidence)", "finalized (evidence)", "observed_reorg_depth (evidence)", "anchor/claimed 32B commitment"],
        "integer_confirmations": true,
        "safety_gate_required": true,
        "reorg_checked_before_confirmations": true,
        "commitment_exact_32byte_match_when_provided": true,
        "finality_fail_closed": true,
        "merkle_proof_or_light_client_verify": false,
        "signature_or_aggregate_signature_verify": false,
        "onchain_read_in_kernel": false,
        "clock_read_in_kernel": false,
        "evm_or_btc_rpc_connection": false,
        "anchor_safe_meaning": "仅表示调用方取证快照按策略自洽，不代表链上真实不可回滚",
        "note": "只做链下确定性最终性决策：重组深度超容忍先拒、require_finalized 未给 finalized 即拒（不以确认数代替最终性）、确认数严格不足即拒（恰好达门槛通过）、提供 32B 锚点承诺时须与声明承诺逐字节一致（不提供则诚实置 commitment_verified=false）；策略既不要求 finalized 又把确认数门槛设 0 视为无闸门拒绝。确认数/重组深度/最终性均由调用方取证传入，内核不读区块/不连 RPC/不读时钟/不做默克尔或轻客户端证明。"
    });
    let bridge_status = serde_json::json!({
        "introduced_in": "v3.9.7",
        "methods": ["bridge_transfer_decide", "bridge_relay"],
        "amount_unit": "internal integer micro (u128, no float)",
        "decision_chain": ["enabled", "paused", "policy valid", "direction whitelist (empty=deny all)", "asset whitelist (empty=deny all)", "destination allowlist (optional)", "amount>0", "min", "per-transfer cap", "rate window count", "cumulative cap (checked add)"],
        "integer_checked_arithmetic": true,
        "whitelist_empty_denies_all_by_default": true,
        "spend_to_zero_allowed": true,
        "cumulative_cap_zero_denies_first_transfer": true,
        "bridge_fail_closed_when_disabled_paused_or_over_limit": true,
        "bridge_relay_fail_closed": true,
        "fabricated_lock_or_mint_on_failure": false,
        "bridge_or_contract_connection": false,
        "crosschain_message_signing_or_broadcast": false,
        "asset_locking_minting_or_release": false,
        "clock_read_in_kernel": false,
        "note": "只做链下确定性跨链风控决策：总开关/紧急暂停/方向与资产白名单（空默认全拒）/可选目的白名单/最小额/单笔上限/速率窗口笔数/累计额度（u128 checked，恰好花完允许、cumulative_cap=0 首笔即拒）逐条短路判定；策略非法（单笔上限 0、窗口上限 0、最小额>单笔上限）具名 BRIDGE_BAD_POLICY。window_prior_count/cumulative_prior/now 均由调用方传入，内核不持久化、不连桥与锁仓/铸造合约、不构造签名广播跨链消息、不锁定/铸造/释放资产；bridge_relay 一律 BRIDGE_NOT_CONFIGURED fail-closed。"
    });
    let zk_status = serde_json::json!({
        "introduced_in": "v3.9.8",
        "methods": ["zk_payment_intent_check"],
        "checks": ["32B commitment/nullifier hex shape", "integer credit line (u128 checked)", "selective-disclosure tag whitelist", "host-attested nullifier double-spend flag", "proof-present gate (structural only)"],
        "credit_line_edge_inclusive": true,
        "nullifier_double_spend_enforced_when_policy_on": true,
        "proof_present_check_only": true,
        "proof_verified": false,
        "zk_proof_verification_implemented": false,
        "recursive_snark_or_zkml_verify": false,
        "privacy_ready_meaning": "仅表示结构/额度/空值符/披露标签自洽且按需随附证明，不代表零知识证明密码学成立",
        "key_or_proof_materialized_in_kernel": false,
        "external_verifier_connection": false,
        "clock_or_persistence_in_kernel": false,
        "note": "只做隐私支付意图的链下结构/额度校验：32B 承诺与空值符 hex 形状、u128 checked 信用额度（恰好花完允许）、选择性披露标签白名单、宿主取证的空值符双花标志、按策略要求证明是否随附（只看在不在，绝不验证）。verify_zk_proof 永远 ZK_PROOF_VERIFICATION_NOT_IMPLEMENTED fail-closed；双花查重、证明密码学验证均需有状态宿主/外部验证器，内核不联网、不持久化、不读时钟、不持钥。"
    });
    let ows_status = serde_json::json!({
        "introduced_in": "v3.9.8",
        "methods": ["ows_wallet_route"],
        "tracks_supported": ["btc_lightning", "evm_x402_stable", "btc_rgb_htlc"],
        "asset_families": ["btc", "stable"],
        "deterministic_precedence": "stable->evm_x402_stable; btc+instant->btc_lightning then btc_rgb_htlc; btc+cross_epoch->btc_rgb_htlc then btc_lightning",
        "capabilities_host_attested": true,
        "require_configured_gate": true,
        "privacy_and_crosschain_constraints_enforced": true,
        "allowed_tracks_empty_denies_all": true,
        "wallet_execute_executed": false,
        "key_held_in_sandbox": false,
        "wallet_or_node_connection": false,
        "note": "只做 OWS 统一钱包层的链下确定性选轨：按资产族/即时性给出优先序，再在宿主显式声明的轨道能力快照（configured/privacy/cross_chain）与白名单交集内取第一个满足隐私与跨链要求的轨道；白名单为空默认无轨道、强制已配置而未配置、或隐私/跨链要求无轨道满足时 OWS_NO_AVAILABLE_TRACK fail-closed，未知资产族/未知轨道名具名拒绝。内核不探测真实钱包、不连节点、不持私钥；execute_ows_payment 一律 OWS_WALLET_NOT_CONFIGURED fail-closed。"
    });
    let compliance_status = serde_json::json!({
        "introduced_in": "v3.9.8",
        "methods": ["compliance_screen"],
        "verdicts": ["allow", "report_required", "blocked"],
        "address_kinds": ["evm (0x+20B, lowercase hex normalized)", "other (trimmed exact)"],
        "decision_chain": ["enabled", "jurisdiction non-empty", "address normalized", "amount>0", "address blocklist", "blocked jurisdiction", "allowed jurisdiction (empty=deny when required)", "hard block threshold (inclusive)", "report threshold (inclusive)"],
        "lists_provided_by_caller": true,
        "builtin_sanctions_list": false,
        "allowed_jurisdiction_empty_denies_all_when_required": true,
        "threshold_zero_disables_check": true,
        "integer_only": true,
        "enforcement_executed": false,
        "external_list_or_regulator_connection": false,
        "screening_pass_is_not_legal_advice": true,
        "note": "只依据调用方显式传入的地址/辖区名单与整数阈值做链下确定性筛查：地址按 evm(去0x、40hex、小写) 或 other(去空白精确) 归一化，命中地址名单/阻断辖区/（要求时）不在允许辖区/达硬阻断阈值即 blocked，达上报阈值即 report_required，否则 allow；阈值 0 关闭对应档；要求允许辖区而名单为空默认全拒。内核不内置任何国家或地址名单、不联网拉取制裁库、不解释法律含义；enforce_compliance_decision 一律 COMPLIANCE_ENFORCEMENT_NOT_CONFIGURED，不冻结不上报。"
    });
    let enforceable = serde_json::json!({
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
        "paymaster_integer_gas_cost_checks": true,
        "paymaster_fail_closed_when_over_limit_or_unconfigured": true,
        "paymaster_default_deny_non_whitelisted_sender": true,
        "htlc_integer_sat_conservation": true,
        "htlc_canonical_locktime_check": true,
        "htlc_p2wsh_program_check": true,
        "htlc_fail_closed_when_node_unconfigured": true,
        "rgb_opret_commitment_exact_match": true,
        "rgb_tapret_first_output_check": true,
        "rgb_tapret_derivation_honestly_not_claimed": true,
        "anchor_integer_confirmation_gate": true,
        "anchor_reorg_depth_gate": true,
        "anchor_finality_fail_closed": true,
        "anchor_commitment_exact_match": true,
        "bridge_fail_closed_when_paused_or_over_limit": true,
        "bridge_default_deny_non_whitelisted_direction_asset": true,
        "bridge_integer_checked_cumulative": true,
        "bridge_relay_not_configured_fail_closed": true,
        "zk_integer_checked_credit_line": true,
        "zk_nullifier_double_spend_enforced": true,
        "zk_disclosure_tag_whitelist": true,
        "zk_snark_verification_honestly_not_claimed": true,
        "ows_deterministic_track_routing": true,
        "ows_fail_closed_when_no_configured_track": true,
        "ows_privacy_crosschain_constraints_enforced": true,
        "compliance_integer_thresholds": true,
        "compliance_lists_caller_provided_fail_closed": true,
        "compliance_allowed_jurisdiction_empty_denies": true,
        "compliance_enforcement_not_configured_fail_closed": true,
        "fund_movement": false,
        "key_holding_in_sandbox": false,
        "transaction_signing": false,
        "transaction_broadcast": false,
        "onchain_anchor_write": false,
        "currency_exchange": false
    });
    let provided = serde_json::json!({
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
        "paymaster_userop_validate": true,
        "paymaster_decide_sponsorship": true,
        "paymaster_sign_when_host_signer_configured": true,
        "btc_htlc_verify": true,
        "btc_htlc_finalize_when_btc_node_configured": true,
        "rgb_commitment_verify": true,
        "anchor_finality_check": true,
        "bridge_transfer_decide": true,
        "bridge_relay_when_bridge_configured": true,
        "zk_payment_intent_check": true,
        "ows_wallet_route": true,
        "compliance_screen": true,
        "zk_proof_verify": false,
        "ows_wallet_execute": false,
        "compliance_enforce": false,
        "router_persistence": false,
        "lightning_node_connection": false,
        "evm_rpc_connection": false,
        "btc_rgb_connection": false,
        "btc_node_connection": false,
        "rgb_state_transition_validation": false,
        "rgb_tapret_tweak_derivation": false,
        "erc8004_onchain_registry_write": false,
        "erc4337_bundler_relay": false,
        "erc4337_paymaster_stake_write": false,
        "anchor_onchain_read_in_kernel": false,
        "anchor_merkle_or_lightclient_proof": false,
        "bridge_or_contract_connection": false,
        "crosschain_message_relay": false,
        "asset_locking_minting_or_release": false,
        "zk_external_verifier_connection": false,
        "zk_recursive_snark_verify": false,
        "ows_wallet_or_node_connection": false,
        "compliance_external_list_connection": false
    });
    serde_json::json!({
        "plugin": PAYMENT_ROUTER_PLUGIN,
        "version": env!("CARGO_PKG_VERSION"),
        "domain": "agent-payment-economy",
        "introduced_in": "v3.9.0",
        "signing_introduced_in": "v3.9.1",
        "x402_introduced_in": "v3.9.2",
        "l402_introduced_in": "v3.9.3",
        "erc8004_introduced_in": "v3.9.4",
        "paymaster_introduced_in": "v3.9.5",
        "btc_htlc_introduced_in": "v3.9.6",
        "rgb_introduced_in": "v3.9.6",
        "anchor_finality_introduced_in": "v3.9.7",
        "bridge_risk_introduced_in": "v3.9.7",
        "zk_payment_introduced_in": "v3.9.8",
        "ows_wallet_introduced_in": "v3.9.8",
        "compliance_introduced_in": "v3.9.8",
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
        // v3.9.1 只读选路 + 宿主受限签名；v3.9.2 增 x402 协议只读构造/校验；v3.9.3 增闪电 L402 只读协议；v3.9.4 增 ERC-8004 三注册表；v3.9.5 增 ERC-4337 Paymaster 只读赞助决策。执行/链上写仍在后续版本。
        "capabilities_declared": [
            "pay:route:read",
            "wallet:sign:host-restricted",
            "x402:protocol:read",
            "l402:protocol:read",
            "erc8004:registry:read",
            "erc4337:paymaster:decide",
            "btc:htlc:read",
            "rgb:commitment:read",
            "chain:anchor:read",
            "bridge:risk:decide",
            "zk:payment-intent:decide",
            "ows:wallet:route",
            "compliance:screen:decide"
        ],
        "capabilities_reserved_later": [
            "pay:execute",
            "chain:anchor:write",
            "bridge:relay:execute",
            "erc8004:registry:write",
            "erc4337:bundler:relay",
            "erc4337:paymaster:stake:write",
            "btc:htlc:execute",
            "btc:node:connect",
            "rgb:state-transition:validate",
            "rgb:tapret:derive",
            "zk:proof:verify",
            "ows:wallet:execute",
            "compliance:list:fetch",
            "compliance:enforce"
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
        "paymaster": paymaster_status,
        "btc_htlc": htlc_status,
        "rgb": rgb_status,
        "anchor_finality": anchor_status,
        "bridge_risk": bridge_status,
        "zk_payment": zk_status,
        "ows_wallet": ows_status,
        "compliance": compliance_status,
        "enforceable": enforceable,
        "provided": provided,
        "note": "v3.9.0：结算路由 PaymentRouter 纯确定性选路（只决策不动钱，缺轨 fail-closed）。v3.9.1：宿主签名闸门 HostSignerGate——wallet_sign_preview 只校验/规范化待签载荷不碰密钥，wallet_sign 仅宿主经 SignatureBroker 签发，私钥/seed 不进沙盒、回执只含公钥+签名；生产默认 UnconfiguredBroker，wallet_sign 一律 SignerNotConfigured 具名拒签、绝不伪造。v3.9.2：EVM x402(USDC) 纯协议内核——校验 402 challenge、用无依赖 keccak256 构造 EIP-3009 transferWithAuthorization 的 EIP-712 待签 digest（只预览不签）、按精确金额守恒校验 facilitator 回执；x402_sign 因本版无 secp256k1 宿主后端一律 X402_EVM_SIGNER_NOT_CONFIGURED fail-closed，不连 RPC、不广播、不划转、不兑换、不持久化、内核不读时钟。v3.9.3：闪电 L402 纯协议内核——解析 402 挑战头与 BOLT11 整数金额前缀（msat）、校验 SHA256(preimage)==payment_hash 与精确金额守恒；l402_pay 因本版不连闪电节点一律具名 fail-closed，不解码 bech32 数据/节点签名、不持私钥、不创建或结算 HTLC。v3.9.4：ERC-8004 三注册表纯协议面——身份句柄（tokenId↔DID↔Ed25519↔tokenURI SHA-256 承诺）绑定校验、四维整数信誉聚合（守恒/自评拦截/nonce 重放拦截）、独立验证者 BFT-lite（n≥3f+1）裁决；不铸造 ERC-721、不连 RPC、不读写链上注册表、不持私钥。v3.9.5：ERC-4337 Paymaster 纯决策面——校验 UserOperation 气体/费用字段、解析并对齐 v0.6 paymasterAndData 与 v0.7 paymaster/paymasterData、按五类气体上限之和×maxFeePerGas 整数 checked 上估 gas 成本，在显式赞助策略（开关/链/时间窗/白名单/单笔上限/累计预算）下决定是否赞助；不连 bundler/EntryPoint、不广播 UserOp、不持 Paymaster 私钥（paymaster_sign 一律 PAYMASTER_SIGNER_NOT_CONFIGURED fail-closed）、不垫付 gas、不链上质押。v3.9.6：BTC HTLC + RGB 纯校验面——HTLC 做 SHA256 哈希锁、BIP65 CLTV 超时（高度/秒由阈值区分，规范最小编码，当前值调用方传入）、整数 sats 守恒（offered+fee==total）、P2WSH witnessProgram=SHA256(witnessScript) 与标准 IF/ELSE 模板逐字节校验；RGB 做 opret-first（唯一 OP_RETURN<32 commitment>）与 tapret-first（第一个 OP_1<32> Taproot 输出）锚定位置/形状校验，tapret tweak 推导诚实标注未实现；不连比特币节点、不读 UTXO/区块、不构造/签名/广播交易、不持私钥、不做 RGB 状态转换、不结算 HTLC（btc_htlc_finalize 一律 HTLC_NODE_NOT_CONFIGURED fail-closed）。v3.9.7：锚定最终性 + 跨链桥风控纯决策面——anchor_finality_check 按重组深度（超容忍先拒）、require_finalized（未给 finalized 即拒，不以确认数代替最终性）、确认数严格门槛（恰好达门槛通过）、32B 锚点/声明承诺逐字节一致判定可否据以行动，策略无任何安全闸门（既不要求 finalized 又 required_confirmations=0）直接拒；bridge_transfer_decide 按总开关/紧急暂停/方向与资产白名单（空默认全拒）/可选目的白名单/最小额/单笔上限/速率窗口笔数/累计额度（u128 checked，恰好花完允许、cumulative_cap=0 首笔即拒）逐条短路放行；确认数/重组深度/窗口计数/累计已用均由调用方取证传入，内核不读区块与时钟、不连 RPC/桥/锁仓或铸造合约、不做默克尔或轻客户端证明、不构造签名广播跨链消息、不锁定/铸造/释放资产，bridge_relay 一律 BRIDGE_NOT_CONFIGURED fail-closed；anchor_safe/approved 仅表取证快照按策略自洽，不代表链上真实不可回滚或桥已执行。v3.9.8：ZK 隐私支付意图 + OWS 统一钱包选轨 + 合规筛查三段纯链下决策面——zk_payment_intent_check 只校验 32B 承诺/空值符 hex 形状、u128 checked 信用额度（恰好花完允许）、选择性披露标签白名单、宿主取证的空值符双花标志与按策略要求的证明是否随附（只看在不在，绝不验证，proof_verified/zk_proof_verification_implemented 恒 false，verify_zk_proof 永远 ZK_PROOF_VERIFICATION_NOT_IMPLEMENTED）；ows_wallet_route 按 stable→evm_x402_stable、btc 即时→btc_lightning 再 btc_rgb_htlc、btc 非即时→btc_rgb_htlc 再 btc_lightning 的确定性优先序，在宿主显式声明的轨道能力（configured/privacy/cross_chain）与白名单交集内取首个满足隐私与跨链要求的轨道，白名单空默认全拒、未配置/隐私/跨链不满足即 OWS_NO_AVAILABLE_TRACK，execute_ows_payment 一律 OWS_WALLET_NOT_CONFIGURED；compliance_screen 只用调用方传入的 evm/other 归一化地址名单、阻断辖区、允许辖区（要求时名单空默认全拒）、整数上报/硬阻断阈值（含边界，0 关闭）产出 allow/report_required/blocked，不内置名单不联网不解释法律，enforce_compliance_decision 一律 COMPLIANCE_ENFORCEMENT_NOT_CONFIGURED 不冻结不上报。外部协议采用量与性能数字均为第三方报道口径、非本仓复测。",
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

/// 字节桥：`paymaster_userop_validate`（校验 UserOperation 气体字段 + 解析对齐 paymaster + 整数上估 gas，纯只读）。
fn handle_paymaster_userop_validate(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let uo: UserOperation = parse_json("paymaster_userop_validate", payload)?;
    uo.gas
        .validate()
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    let total_gas = uo
        .gas
        .total_gas()
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    let estimated = uo
        .gas
        .estimated_max_gas_cost()
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    let (effective_paymaster, effective_data) = uo
        .effective_paymaster()
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&serde_json::json!({
        "valid": true,
        "sender": uo.sender.to_hex(),
        "nonce": uo.nonce.to_string(),
        "chain_id": uo.chain_id,
        "total_gas": total_gas.to_string(),
        "estimated_max_gas_cost_wei": estimated.to_string(),
        "effective_paymaster": effective_paymaster.to_hex(),
        "effective_paymaster_data_hex": format!("0x{}", hex::encode(&effective_data))
    }))
    .map_err(|e| PluginError::Runtime(format!("paymaster_userop_validate 序列化失败: {e}")))
}

/// 字节桥：`paymaster_decide_sponsorship`（显式策略下纯确定性赞助判定，fail-closed，不广播）。
fn handle_paymaster_decide_sponsorship(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct PolicyReq {
        paymaster: EvmAddress,
        chain_id: u64,
        enabled: bool,
        #[serde(default)]
        allow_any_sender: bool,
        #[serde(default)]
        whitelist: Vec<String>,
        per_op_max_gas_cost_wei: u128,
        total_budget_wei: u128,
        valid_after: u64,
        valid_until: u64,
    }
    #[derive(serde::Deserialize)]
    struct Req {
        uo: UserOperation,
        policy: PolicyReq,
        #[serde(default)]
        spent_before_wei: u128,
        now_unix: u64,
    }
    let req: Req = parse_json("paymaster_decide_sponsorship", payload)?;

    let mut whitelist = std::collections::BTreeSet::new();
    for h in &req.policy.whitelist {
        let a = EvmAddress::from_hex(h)
            .ok_or_else(|| PluginError::Runtime(format!("PAYMASTER_BAD_WHITELIST_ADDRESS: {h}")))?;
        whitelist.insert(a.0);
    }
    let policy = SponsorPolicy {
        paymaster: req.policy.paymaster,
        chain_id: req.policy.chain_id,
        enabled: req.policy.enabled,
        allow_any_sender: req.policy.allow_any_sender,
        whitelist,
        per_op_max_gas_cost_wei: req.policy.per_op_max_gas_cost_wei,
        total_budget_wei: req.policy.total_budget_wei,
        valid_after: req.policy.valid_after,
        valid_until: req.policy.valid_until,
    };
    let approval = policy
        .decide_sponsorship(&req.uo, req.spent_before_wei, req.now_unix)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&approval)
        .map_err(|e| PluginError::Runtime(format!("paymaster_decide_sponsorship 序列化失败: {e}")))
}

/// 字节桥：`paymaster_sign`（ERC-4337 担保签发）。
///
/// 本版**没有** Paymaster 宿主签名后端：对任何请求一律具名
/// [`PaymasterError::PaymasterSignerNotConfigured`] fail-closed，绝不伪造担保签名，
/// 也不连接 bundler 中继 UserOperation。
fn handle_paymaster_sign(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    Err(PluginError::Runtime(
        PaymasterError::PaymasterSignerNotConfigured.to_string(),
    ))
}

/// 字节桥：`btc_htlc_verify`（哈希锁 + CLTV 超时 + sats 守恒 + P2WSH 脚本结构，链下纯校验）。
///
/// 金额/脚本/当前高度时间全部由调用方取证后显式传入；内核不连节点、不读时钟、不读区块。
fn handle_btc_htlc_verify(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let offer: HtlcOffer = parse_json("btc_htlc_verify", payload)?;
    let receipt = offer
        .validate()
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&serde_json::json!({
        "verified": true,
        "payment_hash_hex": receipt.payment_hash_hex,
        "receiver_pubkey_hex": receipt.receiver_pubkey_hex,
        "sender_pubkey_hex": receipt.sender_pubkey_hex,
        "locktime": receipt.locktime,
        "locktime_units": receipt.units,
        "offered_sats": receipt.offered_sats.to_string(),
        "fee_sats": receipt.fee_sats.to_string(),
        "total_input_sats": receipt.total_input_sats.to_string(),
        "witness_program_matches": receipt.witness_program_matches,
        "onchain_packed_or_confirmed": false
    }))
    .map_err(|e| PluginError::Runtime(format!("btc_htlc_verify 序列化失败: {e}")))
}

/// 字节桥：`btc_htlc_finalize`（真实结算/广播）。
///
/// 本版不连比特币节点、不持私钥、不签名/广播 HTLC：对任何请求一律具名
/// `HTLC_NODE_NOT_CONFIGURED` fail-closed，绝不伪造交易或原像。
fn handle_btc_htlc_finalize(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    Err(PluginError::Runtime(
        HtlcError::HtlcNodeNotConfigured.to_string(),
    ))
}

/// 字节桥：`rgb_commitment_verify`（opret/tapret-first 承诺锚定位置校验，链下纯只读）。
fn handle_rgb_commitment_verify(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let claim: CommitmentClaim = parse_json("rgb_commitment_verify", payload)?;
    let receipt = claim
        .verify()
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&serde_json::json!({
        "verified": true,
        "scheme": receipt.scheme,
        "commitment_hex": receipt.commitment_hex,
        "anchor_output_index": receipt.anchor_output_index,
        "derivation_verified": receipt.derivation_verified,
        "rgb_state_transition_validated": false
    }))
    .map_err(|e| PluginError::Runtime(format!("rgb_commitment_verify 序列化失败: {e}")))
}

/// 字节桥：`anchor_finality_check`（锚定最终性链下纯决策，取证快照传入）。
fn handle_anchor_finality_check(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let req: AnchorFinalityRequest = parse_json("anchor_finality_check", payload)?;
    let receipt = anchor_finality_verify(&req.policy, &req.evidence)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&serde_json::json!({
        "anchor_safe": receipt.anchor_safe,
        "source_chain": receipt.source_chain,
        "confirmations": receipt.confirmations,
        "required_confirmations": receipt.required_confirmations,
        "finalized": receipt.finalized,
        "require_finalized": receipt.require_finalized,
        "observed_reorg_depth": receipt.observed_reorg_depth,
        "max_tolerated_reorg_depth": receipt.max_tolerated_reorg_depth,
        "commitment_verified": receipt.commitment_verified,
        "onchain_read_in_kernel": false
    }))
    .map_err(|e| PluginError::Runtime(format!("anchor_finality_check 序列化失败: {e}")))
}

/// 字节桥：`bridge_transfer_decide`（跨链风控链下纯决策，fail-closed）。
fn handle_bridge_transfer_decide(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let req: BridgeTransferRequest = parse_json("bridge_transfer_decide", payload)?;
    let decision = bridge_transfer_decide(&req.policy, &req.transfer)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&decision)
        .map_err(|e| PluginError::Runtime(format!("bridge_transfer_decide 序列化失败: {e}")))
}

/// 字节桥：`bridge_relay`（真实跨链中继）。本版无桥后端，一律 BRIDGE_NOT_CONFIGURED。
fn handle_bridge_relay(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    Err(PluginError::Runtime(
        BridgeError::BridgeNotConfigured.to_string(),
    ))
}

/// v3.9.8 字节桥：`zk_payment_intent_check`（ZK 隐私意图链下结构/额度校验，不验 SNARK）。
fn handle_zk_payment_intent_check(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let req: ZkIntentRequest = parse_json("zk_payment_intent_check", payload)?;
    let receipt = verify_zk_payment_intent(&req.policy, &req.intent)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&receipt)
        .map_err(|e| PluginError::Runtime(format!("zk_payment_intent_check 序列化失败: {e}")))
}

/// v3.9.8 字节桥：`ows_wallet_route`（OWS 统一钱包确定性选轨，fail-closed）。
fn handle_ows_wallet_route(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let req: OwsRouteRequest = parse_json("ows_wallet_route", payload)?;
    let route = route_ows_wallet(&req.policy, &req.capabilities, &req.intent)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&route)
        .map_err(|e| PluginError::Runtime(format!("ows_wallet_route 序列化失败: {e}")))
}

/// v3.9.8 字节桥：`compliance_screen`（仅用调用方名单/阈值，allow/report/block）。
fn handle_compliance_screen(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let req: ComplianceScreenRequest = parse_json("compliance_screen", payload)?;
    let decision = screen_compliance(&req.policy, &req.subject)
        .map_err(|e| PluginError::Runtime(e.to_string()))?;
    serde_json::to_vec(&decision)
        .map_err(|e| PluginError::Runtime(format!("compliance_screen 序列化失败: {e}")))
}

/// `anchor_finality_check` 请求体：策略 + 取证快照均由调用方传入。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct AnchorFinalityRequest {
    policy: AnchorPolicy,
    evidence: AnchorEvidence,
}

/// `bridge_transfer_decide` 请求体：策略 + 转移请求均由调用方传入。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct BridgeTransferRequest {
    policy: BridgePolicy,
    transfer: BridgeTransfer,
}

/// `zk_payment_intent_check` 请求体：策略 + 意图取证均由调用方传入。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct ZkIntentRequest {
    policy: ZkPolicy,
    intent: ZkIntent,
}

/// `ows_wallet_route` 请求体：策略 + 宿主能力快照 + 归一化意图均由调用方传入。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct OwsRouteRequest {
    policy: OwsPolicy,
    capabilities: Vec<WalletCapability>,
    intent: OwsIntent,
}

/// `compliance_screen` 请求体：名单/阈值策略 + 待筛查主体均由调用方传入。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct ComplianceScreenRequest {
    policy: CompliancePolicy,
    subject: ComplianceSubject,
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
    // v3.9.5 ERC-4337 Paymaster：只校验/决策，签发 fail-closed，不广播、不中继。
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_PAYMASTER_USEROP_VALIDATE,
        handle_paymaster_userop_validate,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_PAYMASTER_DECIDE_SPONSORSHIP,
        handle_paymaster_decide_sponsorship,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_PAYMASTER_SIGN,
        handle_paymaster_sign,
    );
    // v3.9.6 BTC HTLC + RGB：链下纯校验/位置校验；结算 fail-closed，不连节点、不广播、不转资产。
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_BTC_HTLC_VERIFY,
        handle_btc_htlc_verify,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_BTC_HTLC_FINALIZE,
        handle_btc_htlc_finalize,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_RGB_COMMITMENT_VERIFY,
        handle_rgb_commitment_verify,
    );
    // v3.9.7 锚定最终性 + 跨链桥风控：链下纯决策 fail-closed；中继无后端具名拒绝，不连桥/RPC、不广播、不转资产。
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_ANCHOR_FINALITY_CHECK,
        handle_anchor_finality_check,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_BRIDGE_TRANSFER_DECIDE,
        handle_bridge_transfer_decide,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_BRIDGE_RELAY,
        handle_bridge_relay,
    );
    // v3.9.8 ZK 隐私意图 / OWS 统一钱包选轨 / 合规筛查：三段链下纯决策 fail-closed。
    // SNARK 验证不注册（verify_zk_proof 恒 NOT_IMPLEMENTED）；钱包执行与合规外部执行不注册。
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_ZK_PAYMENT_INTENT_CHECK,
        handle_zk_payment_intent_check,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_OWS_WALLET_ROUTE,
        handle_ows_wallet_route,
    );
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_COMPLIANCE_SCREEN,
        handle_compliance_screen,
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
        assert_eq!(s["provided"]["paymaster_userop_validate"], true);
        assert_eq!(s["provided"]["paymaster_decide_sponsorship"], true);
        assert_eq!(
            s["provided"]["paymaster_sign_when_host_signer_configured"],
            true
        );
        assert_eq!(s["provided"]["erc4337_bundler_relay"], false);
        assert_eq!(s["provided"]["erc4337_paymaster_stake_write"], false);
        assert_eq!(s["provided"]["lightning_node_connection"], false);
        assert_eq!(s["tracks"].as_array().unwrap().len(), 3);
        // v3.9.1 两个 + v3.9.2 x402 + v3.9.3 l402 + v3.9.4 erc8004 + v3.9.5 paymaster 只读协议能力。
        // v3.9.6 再 + btc:htlc:read + rgb:commitment:read = 8。
        // v3.9.7 再 + chain:anchor:read + bridge:risk:decide = 10。
        // v3.9.8 再 + zk:payment-intent:decide + ows:wallet:route + compliance:screen:decide = 13。
        assert_eq!(s["capabilities_declared"].as_array().unwrap().len(), 13);
        assert_eq!(
            s["capabilities_reserved_later"].as_array().unwrap().len(),
            14
        );
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
        assert_eq!(s["paymaster"]["introduced_in"], "v3.9.5");
        assert_eq!(s["paymaster"]["produces_paymaster_signature"], false);
        assert_eq!(s["paymaster"]["bundler_or_entrypoint_connection"], false);
        assert_eq!(s["paymaster"]["useroperation_broadcast"], false);
        assert_eq!(s["paymaster"]["live_gas_payment"], false);
        assert_eq!(
            s["enforceable"]["paymaster_fail_closed_when_over_limit_or_unconfigured"],
            true
        );
        assert_eq!(
            s["enforceable"]["paymaster_default_deny_non_whitelisted_sender"],
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
        // v3.9.6 BTC HTLC / RGB 纯校验面。
        assert_eq!(s["btc_htlc"]["introduced_in"], "v3.9.6");
        assert_eq!(s["btc_htlc"]["btc_node_connection"], false);
        assert_eq!(s["btc_htlc"]["transaction_broadcast"], false);
        assert_eq!(s["btc_htlc"]["full_bitcoin_script_interpreter"], false);
        assert_eq!(s["btc_htlc"]["integer_sat_amount_conservation"], true);
        assert_eq!(s["btc_htlc"]["htlc_finalize_fail_closed"], true);
        assert_eq!(s["btc_htlc"]["clock_read_in_kernel"], false);
        assert_eq!(s["rgb"]["introduced_in"], "v3.9.6");
        assert_eq!(s["rgb"]["tapret_tweak_derivation"], false);
        assert_eq!(s["rgb"]["rgb_state_transition_validation"], false);
        assert_eq!(s["rgb"]["onchain_anchor_write"], false);
        assert_eq!(s["rgb"]["btc_node_connection"], false);
        assert_eq!(
            s["enforceable"]["htlc_fail_closed_when_node_unconfigured"],
            true
        );
        assert_eq!(s["enforceable"]["htlc_integer_sat_conservation"], true);
        assert_eq!(s["enforceable"]["rgb_opret_commitment_exact_match"], true);
        assert_eq!(
            s["enforceable"]["rgb_tapret_derivation_honestly_not_claimed"],
            true
        );
        assert_eq!(s["provided"]["btc_htlc_verify"], true);
        assert_eq!(s["provided"]["rgb_commitment_verify"], true);
        assert_eq!(s["provided"]["btc_node_connection"], false);
        assert_eq!(s["provided"]["rgb_state_transition_validation"], false);
        assert_eq!(s["btc_htlc_introduced_in"], "v3.9.6");
        assert_eq!(s["rgb_introduced_in"], "v3.9.6");
        // v3.9.7 锚定最终性 / 跨链桥风控纯决策面。
        assert_eq!(s["anchor_finality"]["introduced_in"], "v3.9.7");
        assert_eq!(s["anchor_finality"]["onchain_read_in_kernel"], false);
        assert_eq!(s["anchor_finality"]["evm_or_btc_rpc_connection"], false);
        assert_eq!(s["anchor_finality"]["finality_fail_closed"], true);
        assert_eq!(
            s["anchor_finality"]["reorg_checked_before_confirmations"],
            true
        );
        assert_eq!(s["bridge_risk"]["introduced_in"], "v3.9.7");
        assert_eq!(s["bridge_risk"]["bridge_or_contract_connection"], false);
        assert_eq!(
            s["bridge_risk"]["crosschain_message_signing_or_broadcast"],
            false
        );
        assert_eq!(s["bridge_risk"]["asset_locking_minting_or_release"], false);
        assert_eq!(s["bridge_risk"]["bridge_relay_fail_closed"], true);
        assert_eq!(
            s["bridge_risk"]["whitelist_empty_denies_all_by_default"],
            true
        );
        assert_eq!(s["enforceable"]["anchor_finality_fail_closed"], true);
        assert_eq!(
            s["enforceable"]["bridge_fail_closed_when_paused_or_over_limit"],
            true
        );
        assert_eq!(
            s["enforceable"]["bridge_relay_not_configured_fail_closed"],
            true
        );
        assert_eq!(s["provided"]["anchor_finality_check"], true);
        assert_eq!(s["provided"]["bridge_transfer_decide"], true);
        assert_eq!(s["provided"]["crosschain_message_relay"], false);
        assert_eq!(s["anchor_finality_introduced_in"], "v3.9.7");
        assert_eq!(s["bridge_risk_introduced_in"], "v3.9.7");
        // v3.9.8 ZK 隐私意图 / OWS 统一钱包 / 合规筛查纯链下决策面。
        assert_eq!(s["zk_payment"]["introduced_in"], "v3.9.8");
        assert_eq!(s["zk_payment"]["proof_verified"], false);
        assert_eq!(s["zk_payment"]["zk_proof_verification_implemented"], false);
        assert_eq!(s["zk_payment"]["recursive_snark_or_zkml_verify"], false);
        assert_eq!(s["zk_payment"]["external_verifier_connection"], false);
        assert_eq!(
            s["zk_payment"]["key_or_proof_materialized_in_kernel"],
            false
        );
        assert_eq!(s["zk_payment"]["credit_line_edge_inclusive"], true);
        assert_eq!(s["ows_wallet"]["introduced_in"], "v3.9.8");
        assert_eq!(s["ows_wallet"]["wallet_execute_executed"], false);
        assert_eq!(s["ows_wallet"]["key_held_in_sandbox"], false);
        assert_eq!(s["ows_wallet"]["wallet_or_node_connection"], false);
        assert_eq!(s["ows_wallet"]["allowed_tracks_empty_denies_all"], true);
        assert_eq!(s["compliance"]["introduced_in"], "v3.9.8");
        assert_eq!(s["compliance"]["builtin_sanctions_list"], false);
        assert_eq!(s["compliance"]["enforcement_executed"], false);
        assert_eq!(
            s["compliance"]["external_list_or_regulator_connection"],
            false
        );
        assert_eq!(s["compliance"]["lists_provided_by_caller"], true);
        assert_eq!(s["zk_payment_introduced_in"], "v3.9.8");
        assert_eq!(s["ows_wallet_introduced_in"], "v3.9.8");
        assert_eq!(s["compliance_introduced_in"], "v3.9.8");
        assert_eq!(s["provided"]["zk_payment_intent_check"], true);
        assert_eq!(s["provided"]["ows_wallet_route"], true);
        assert_eq!(s["provided"]["compliance_screen"], true);
        assert_eq!(s["provided"]["zk_proof_verify"], false);
        assert_eq!(s["provided"]["ows_wallet_execute"], false);
        assert_eq!(s["provided"]["compliance_enforce"], false);
        assert_eq!(
            s["enforceable"]["zk_snark_verification_honestly_not_claimed"],
            true
        );
        assert_eq!(
            s["enforceable"]["ows_fail_closed_when_no_configured_track"],
            true
        );
        assert_eq!(
            s["enforceable"]["compliance_enforcement_not_configured_fail_closed"],
            true
        );
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

    #[test]
    fn paymaster_handlers_validate_decide_and_sign_fail_closed() {
        let pm = format!("0x{}", "11".repeat(20));
        let sender = format!("0x{}", "22".repeat(20));
        // v0.6 拼接形态：20B paymaster 地址 + 4B data。
        let pad = format!("0x{}{}", "11".repeat(20), "deadbeef");
        let gas = serde_json::json!({
            "pre_verification_gas": 50000,
            "verification_gas_limit": 100000,
            "call_gas_limit": 200000,
            "paymaster_verification_gas_limit": 50000,
            "paymaster_post_op_gas_limit": 30000,
            "max_fee_per_gas": 1_000_000_000,
            "max_priority_fee_per_gas": 100_000_000
        });
        let uo = serde_json::json!({
            "sender": sender, "nonce": 7, "chain_id": 8453,
            "gas": gas, "paymaster_and_data_hex": pad
        });

        // validate：解析地址、整数上估 gas（430_000 * 1e9）。
        let vp = serde_json::to_vec(&uo).unwrap();
        let vout: serde_json::Value = serde_json::from_slice(
            &handle_paymaster_userop_validate(METHOD_PAYMASTER_USEROP_VALIDATE, &vp).unwrap(),
        )
        .unwrap();
        assert_eq!(vout["valid"], true);
        assert_eq!(vout["effective_paymaster"], pm);
        assert_eq!(vout["estimated_max_gas_cost_wei"], "430000000000000");
        assert_eq!(vout["total_gas"], "430000");

        // validate 坏气体（priority>max）fail-closed。
        let mut bad_gas = gas.clone();
        bad_gas["max_priority_fee_per_gas"] = serde_json::json!(2_000_000_000u64);
        let mut bad_uo = uo.clone();
        bad_uo["gas"] = bad_gas;
        let bp = serde_json::to_vec(&bad_uo).unwrap();
        assert!(handle_paymaster_userop_validate(METHOD_PAYMASTER_USEROP_VALIDATE, &bp).is_err());

        // decide：白名单命中 + 预算足够 → approved。
        let dp = serde_json::to_vec(&serde_json::json!({
            "uo": uo,
            "policy": {
                "paymaster": pm, "chain_id": 8453, "enabled": true,
                "whitelist": [sender],
                "per_op_max_gas_cost_wei": 1_000_000_000_000_000u64,
                "total_budget_wei": 10_000_000_000_000_000u64,
                "valid_after": 1000, "valid_until": 2_000_000_000u64
            },
            "spent_before_wei": 0, "now_unix": 1_000_000
        }))
        .unwrap();
        let dout: serde_json::Value = serde_json::from_slice(
            &handle_paymaster_decide_sponsorship(METHOD_PAYMASTER_DECIDE_SPONSORSHIP, &dp).unwrap(),
        )
        .unwrap();
        assert_eq!(dout["decision"], "approved");
        assert_eq!(dout["produces_paymaster_signature"], false);
        assert_eq!(dout["relays_user_operation"], false);

        // decide：白名单坏地址 fail-closed（不静默当任意地址）。
        let mut bad_policy = serde_json::json!({
            "paymaster": pm, "chain_id": 8453, "enabled": true,
            "whitelist": ["0xnothex"],
            "per_op_max_gas_cost_wei": 1,
            "total_budget_wei": 1,
            "valid_after": 1000, "valid_until": 2_000_000_000u64
        });
        bad_policy["uo"] = uo.clone();
        let bpp = serde_json::to_vec(&bad_policy).unwrap();
        assert!(
            handle_paymaster_decide_sponsorship(METHOD_PAYMASTER_DECIDE_SPONSORSHIP, &bpp).is_err()
        );

        // sign：无宿主签名后端，一律具名 fail-closed，不伪造。
        let err = handle_paymaster_sign(METHOD_PAYMASTER_SIGN, b"{}").unwrap_err();
        assert!(err.to_string().contains("PAYMASTER_SIGNER_NOT_CONFIGURED"));
    }

    fn sample_htlc_script(hash: &[u8; 32], recv: &[u8; 33], send: &[u8; 33]) -> Vec<u8> {
        let mut s = Vec::new();
        s.extend_from_slice(&[
            0x63, // OP_IF
            0xaa, // OP_HASH256
            0x20,
        ]);
        s.extend_from_slice(hash);
        s.extend_from_slice(&[
            0x88, // OP_EQUALVERIFY
            0x75, // OP_DROP
            0x21,
        ]);
        s.extend_from_slice(recv);
        s.extend_from_slice(&[
            0xac, // OP_CHECKSIG
            0x67, // OP_ELSE
            0x03, // 3-byte minimal LE locktime push
        ]);
        // 700_000 = 0x0AAE60 -> minimal LE [0x60, 0xAE, 0x0A]
        s.extend_from_slice(&[0x60, 0xae, 0x0a]);
        s.extend_from_slice(&[
            0xb1, // OP_CHECKLOCKTIMEVERIFY
            0x75, // OP_DROP
            0x21,
        ]);
        s.extend_from_slice(send);
        s.extend_from_slice(&[
            0xac, // OP_CHECKSIG
            0x68, // OP_ENDIF
        ]);
        s
    }

    #[test]
    fn btc_htlc_verify_handler_happy_path_and_gates() {
        use sha2::{Digest, Sha256};
        let preimage = [9u8; 32];
        let hash: [u8; 32] = Sha256::digest(preimage).into();
        let recv = [2u8; 33];
        let send = [3u8; 33];
        let script = sample_htlc_script(&hash, &recv, &send);
        let program = p2wsh_program(&script);

        let ok = serde_json::json!({
            "offered_sats": 99_000u64,
            "fee_sats": 1_000u64,
            "total_input_sats": 100_000u64,
            "preimage_hex": hex::encode(preimage),
            "payment_hash_hex": hex::encode(hash),
            "witness_program_hex": hex::encode(program),
            "witness_script_hex": hex::encode(&script),
            "current": 690_000u32,
            "min_remaining": 1_000u32
        });
        let out: serde_json::Value = serde_json::from_slice(
            &handle_btc_htlc_verify(METHOD_BTC_HTLC_VERIFY, &serde_json::to_vec(&ok).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out["verified"], true);
        assert_eq!(out["witness_program_matches"], true);
        assert_eq!(out["locktime"], 700_000);
        assert_eq!(out["locktime_units"], "block_height");
        assert_eq!(out["onchain_packed_or_confirmed"], false);

        // 金额不守恒 → 具名拒绝。
        let mut bad = ok.clone();
        bad["total_input_sats"] = serde_json::json!(100_001u64);
        let err =
            handle_btc_htlc_verify(METHOD_BTC_HTLC_VERIFY, &serde_json::to_vec(&bad).unwrap())
                .unwrap_err();
        assert!(err.to_string().contains("HTLC_SETTLEMENT_MISMATCH"));

        // 空/坏负载 → Runtime（不 panic）。
        assert!(handle_btc_htlc_verify(METHOD_BTC_HTLC_VERIFY, b"").is_err());

        // finalize 一律 fail-closed。
        let ferr = handle_btc_htlc_finalize(METHOD_BTC_HTLC_FINALIZE, b"{}").unwrap_err();
        assert!(ferr.to_string().contains("HTLC_NODE_NOT_CONFIGURED"));
    }

    #[test]
    fn rgb_commitment_handler_opret_and_tapret_boundaries() {
        let commitment = [0xabu8; 32];
        let mut opret = vec![0x6a, 0x20];
        opret.extend_from_slice(&commitment);
        let mut taproot = vec![0x51, 0x20];
        taproot.extend_from_slice(&[4u8; 32]);

        // opret：承诺在索引 1。
        let opret_claim = serde_json::json!({
            "scheme": "opret_first",
            "commitment_hex": hex::encode(commitment),
            "outputs_script_hex": [hex::encode(&taproot), hex::encode(&opret)],
            "taproot_output_index": 0,
            "require_derivation": false
        });
        let out: serde_json::Value = serde_json::from_slice(
            &handle_rgb_commitment_verify(
                METHOD_RGB_COMMITMENT_VERIFY,
                &serde_json::to_vec(&opret_claim).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(out["verified"], true);
        assert_eq!(out["anchor_output_index"], 1);
        assert_eq!(out["derivation_verified"], true);

        // opret：找不到承诺 → 具名拒绝。
        let missing = serde_json::json!({
            "scheme": "opret",
            "commitment_hex": hex::encode([0x11u8; 32]),
            "outputs_script_hex": [hex::encode(&taproot)],
            "taproot_output_index": 0,
            "require_derivation": false
        });
        let merr = handle_rgb_commitment_verify(
            METHOD_RGB_COMMITMENT_VERIFY,
            &serde_json::to_vec(&missing).unwrap(),
        )
        .unwrap_err();
        assert!(merr.to_string().contains("RGB_OPRET_COMMITMENT_NOT_FOUND"));

        // tapret：形状/位置通过，但 tweak 推导诚实未实现。
        let tapret_claim = serde_json::json!({
            "scheme": "tapret_first",
            "commitment_hex": hex::encode(commitment),
            "outputs_script_hex": [hex::encode(&taproot)],
            "taproot_output_index": 0,
            "require_derivation": true
        });
        let terr = handle_rgb_commitment_verify(
            METHOD_RGB_COMMITMENT_VERIFY,
            &serde_json::to_vec(&tapret_claim).unwrap(),
        )
        .unwrap_err();
        assert!(terr
            .to_string()
            .contains("RGB_TAPRET_DERIVATION_NOT_IMPLEMENTED"));

        // require_derivation=false：形状校验通过，derivation_verified 诚实为 false。
        let mut shape_ok = tapret_claim.clone();
        shape_ok["require_derivation"] = serde_json::json!(false);
        let sout: serde_json::Value = serde_json::from_slice(
            &handle_rgb_commitment_verify(
                METHOD_RGB_COMMITMENT_VERIFY,
                &serde_json::to_vec(&shape_ok).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(sout["verified"], true);
        assert_eq!(sout["derivation_verified"], false);

        // 空负载 → Runtime（不 panic）。
        assert!(handle_rgb_commitment_verify(METHOD_RGB_COMMITMENT_VERIFY, b"").is_err());
    }

    #[test]
    fn anchor_finality_handler_roundtrip_and_fail_closed() {
        let commitment = format!("0x{}", "ab".repeat(32));
        let ok = serde_json::json!({
            "policy": {"required_confirmations": 6, "require_finalized": false, "max_tolerated_reorg_depth": 1},
            "evidence": {
                "source_chain": "bitcoin",
                "confirmations": 7,
                "finalized": false,
                "observed_reorg_depth": 0,
                "anchor_commitment_hex": commitment,
                "claimed_commitment_hex": commitment
            }
        });
        let bytes = handle_anchor_finality_check(
            METHOD_ANCHOR_FINALITY_CHECK,
            serde_json::to_vec(&ok).unwrap().as_slice(),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["anchor_safe"], true);
        assert_eq!(v["commitment_verified"], true);
        assert_eq!(v["onchain_read_in_kernel"], false);

        // 确认数不足 → 具名 ANCHOR_* Runtime 拒绝。
        let mut bad = ok.clone();
        bad["evidence"]["confirmations"] = serde_json::json!(2);
        let err = handle_anchor_finality_check(
            METHOD_ANCHOR_FINALITY_CHECK,
            serde_json::to_vec(&bad).unwrap().as_slice(),
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("ANCHOR_INSUFFICIENT_CONFIRMATIONS"));
        assert!(handle_anchor_finality_check(METHOD_ANCHOR_FINALITY_CHECK, b"").is_err());
    }

    #[test]
    fn bridge_handler_decide_and_relay_named_refused() {
        let ok = serde_json::json!({
            "policy": {
                "enabled": true,
                "paused": false,
                "allowed_directions": ["btc->evm"],
                "allowed_assets": ["WBTC"],
                "require_allowlisted_destination": true,
                "min_transfer": 100,
                "per_transfer_cap": 1_000_000,
                "rate_window_max_count": 3,
                "cumulative_cap": 5_000_000
            },
            "transfer": {
                "direction": "btc->evm",
                "asset": "WBTC",
                "amount": 500_000,
                "destination_allowlisted": true,
                "window_prior_count": 1,
                "cumulative_prior": 1_000_000
            }
        });
        let bytes = handle_bridge_transfer_decide(
            METHOD_BRIDGE_TRANSFER_DECIDE,
            serde_json::to_vec(&ok).unwrap().as_slice(),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["approved"], true);
        assert_eq!(v["bridge_relay_executed"], false);

        // 暂停 → 具名 BRIDGE_PAUSED。
        let mut paused = ok.clone();
        paused["policy"]["paused"] = serde_json::json!(true);
        let err = handle_bridge_transfer_decide(
            METHOD_BRIDGE_TRANSFER_DECIDE,
            serde_json::to_vec(&paused).unwrap().as_slice(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("BRIDGE_PAUSED"));
        assert!(handle_bridge_transfer_decide(METHOD_BRIDGE_TRANSFER_DECIDE, b"").is_err());

        // 真实中继无后端 → BRIDGE_NOT_CONFIGURED。
        let relay_err = handle_bridge_relay(METHOD_BRIDGE_RELAY, b"{}").unwrap_err();
        assert!(relay_err.to_string().contains("BRIDGE_NOT_CONFIGURED"));
    }
}
