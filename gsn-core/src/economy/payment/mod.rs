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
//! - v3.9.1 宿主签名服务与私钥隔离（沙盒内只见公钥/具名拒签）。
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

pub use router::{
    PaymentRouter, PaymentTrack, RouteDecision, RouteReason, RoutingInput, RoutingPolicy,
    SettlementUrgency, TrackAvailability,
};

use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::runtime::native::NativeRuntime;
use serde::{Deserialize, Serialize};

/// 结算路由系统插件 id（T0，随内核进程内注册，不可热插拔）。
pub const PAYMENT_ROUTER_PLUGIN: &str = "com.twinsearth.sys.payment-router";

/// PMB 方法：结算路由只读状态（版本 + 轨道 + 诚实位）。
pub const METHOD_PAYMENT_ROUTER_STATUS: &str = "payment_router_status";

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
        // 本版本只授予只读选路决策；执行/签名/链上写均在后续版本且需独立能力令牌。
        "capabilities_declared": ["pay:route:read"],
        "capabilities_reserved_later": [
            "pay:execute",
            "wallet:sign",
            "chain:anchor:write"
        ],
        "enforceable": {
            "deterministic_route_decision": true,
            "integer_thresholds": true,
            "fail_closed_when_track_unavailable": true,
            "fund_movement": false,
            "key_holding": false,
            "transaction_signing": false,
            "transaction_broadcast": false,
            "onchain_anchor_write": false,
            "currency_exchange": false
        },
        "provided": {
            "payment_router_status": true,
            "router_persistence": false,
            "lightning_node_connection": false,
            "evm_rpc_connection": false,
            "btc_rgb_connection": false
        },
        "note": "v3.9.0：结算路由 PaymentRouter 纯确定性选路——按整数金额 micro、时延/周期（即时/常规/跨周期）、条件（智能合约/DeFi）与可用轨道，在闪电 L402、EVM x402、BTC RGB/HTLC 间选唯一轨道并给可审计理由；DeFi/智能合约需要可编程性时 EVM 优先（即便大额/跨周期），大额达下限或跨周期且无合约要求走 BTC 主链最终结算，即时小额走闪电，常规区间走稳定币 x402 默认。只决策不动钱：不持私钥、不签名、不广播、不划转、不兑换、不连任何节点、不持久化、不经 PMB 受理外部写；所需轨道不可用具名 RouteTrackUnavailable 拒绝而非静默降级（不改变费用/最终性/信任假设）。宿主签名服务与私钥隔离在 v3.9.1，x402/L402/ERC-8004/Paymaster/HTLC/RGB/ZK 对接在 v3.9.2-v3.9.8。外部协议采用量与性能数字均为第三方报道口径、非本仓复测。"
    })
}

/// 字节桥：`payment_router_status`（无参或空负载，只读）。
fn handle_payment_router_status(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    serde_json::to_vec(&status_payload())
        .map_err(|e| PluginError::Runtime(format!("payment_router_status 序列化失败: {e}")))
}

/// 把结算路由内核只读处理器注册到 T0 native 运行时（随系统插件装配调用）。
///
/// v3.9.0 只注册只读状态查询；选路执行/签名/链上写在后续小版本接线，未接线方法明确
/// 不注册（NotFound），不伪造可用。
pub fn register(rt: &mut NativeRuntime) {
    rt.register_handler(
        PAYMENT_ROUTER_PLUGIN,
        METHOD_PAYMENT_ROUTER_STATUS,
        handle_payment_router_status,
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
        assert_eq!(s["enforceable"]["key_holding"], false);
        assert_eq!(s["enforceable"]["transaction_signing"], false);
        assert_eq!(s["enforceable"]["onchain_anchor_write"], false);
        assert_eq!(s["provided"]["payment_router_status"], true);
        assert_eq!(s["provided"]["lightning_node_connection"], false);
        assert_eq!(s["tracks"].as_array().unwrap().len(), 3);
        // 本版本只声明只读选路能力，执行/签名/链上写仅登记为后续。
        assert_eq!(s["capabilities_declared"].as_array().unwrap().len(), 1);
    }
}
