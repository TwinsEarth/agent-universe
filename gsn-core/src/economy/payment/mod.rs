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

pub mod router;
pub mod signer;

pub use router::{
    PaymentRouter, PaymentTrack, RouteDecision, RouteReason, RoutingInput, RoutingPolicy,
    SettlementUrgency, TrackAvailability,
};
pub use signer::{
    HostSignerGate, InMemoryBroker, SignIntent, SignPolicy, SignPreview, SignatureBroker,
    SignatureReceipt, TrackSignRule, UnconfiguredBroker, UnconfiguredSignerGate,
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
        // v3.9.1 起声明只读选路 + 宿主受限签名（私钥不进沙盒）；执行/链上写仍在后续版本。
        "capabilities_declared": ["pay:route:read", "wallet:sign:host-restricted"],
        "capabilities_reserved_later": [
            "pay:execute",
            "chain:anchor:write"
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
        "enforceable": {
            "deterministic_route_decision": true,
            "integer_thresholds": true,
            "fail_closed_when_track_unavailable": true,
            "fail_closed_when_signer_unconfigured": true,
            "host_only_signing_private_key_isolation": true,
            "sandbox_direct_transaction_signing": false,
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
            "router_persistence": false,
            "lightning_node_connection": false,
            "evm_rpc_connection": false,
            "btc_rgb_connection": false
        },
        "note": "v3.9.0：结算路由 PaymentRouter 纯确定性选路（只决策不动钱，缺轨 fail-closed）。v3.9.1：宿主签名闸门 HostSignerGate——wallet_sign_preview 只校验/规范化待签载荷（domain|track|cap|nonce|auth_only|digest）不碰密钥，wallet_sign 仅宿主经 SignatureBroker 签发，私钥/seed 不进沙盒、回执只含公钥+签名；生产默认 UnconfiguredBroker 不持密钥，wallet_sign 一律 SignerNotConfigured 具名拒签、绝不伪造，InMemoryBroker 为宿主侧参考后端。仍不连任何链/节点、不广播、不划转、不兑换、不持久化。x402/L402/ERC-8004/Paymaster/HTLC/RGB/ZK 对接在 v3.9.2-v3.9.8。外部协议采用量与性能数字均为第三方报道口径、非本仓复测。"
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

/// 把结算/签名内核处理器注册到 T0 native 运行时（随系统插件装配调用）。
///
/// v3.9.0 注册只读状态查询；v3.9.1 追加 `wallet_sign_preview`（只读预览）与宿主受限
/// `wallet_sign`（生产默认未配置密钥，具名 fail-closed）。选路执行/链上写在后续小版本
/// 接线，未接线方法明确不注册（NotFound），不伪造可用。
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
        assert_eq!(s["provided"]["lightning_node_connection"], false);
        assert_eq!(s["tracks"].as_array().unwrap().len(), 3);
        // v3.9.1 起声明只读选路 + 宿主受限签名。
        assert_eq!(s["capabilities_declared"].as_array().unwrap().len(), 2);
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
}
