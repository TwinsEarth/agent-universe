//! 面向 Agent 的资源市场（v3.8.0 基座）。
//!
//! # 定位
//!
//! 未来每个人都会有多个 Agent；它们闲置时，本机/跨网络的**算力、存储、网络、
//! Agent 能力**都是被浪费的资源。资源市场把这些闲置资源变成可挂单、可撮合、
//! 可结算、可治理的供给，逐步沉淀为全球智能体网络与市场。
//!
//! v3.8.0 只交付**确定性领域内核**（纯决策 / 记账面，零浮点、零 syscall、零 unsafe）：
//!
//! - 领域类型：[`ResourceKind`] / [`MeterUnit`] / [`ResourceOffer`] / [`ResourceAsk`]
//!   / [`ResourceOrder`] / [`OrderState`]；
//! - 资源单状态机：[`OrderState::transition`]，单一合法转移赋值点，越态具名拒绝；
//! - 内部记账单位 [`Credits`]：`u128` 微单位（1 credit = 10^6 micro），**不挂法币、
//!   不挂链**，链上结算（BTC/ETH/稳定币）是 v3.9.x 的事，本版本只做守恒记账；
//! - 系统插件 [`RESOURCE_MARKET_PLUGIN`]：经 PMB 暴露只读 `resource_market_status`。
//! - **v3.8.1 注册容量**：[`CapacityRegistry`] 内存账本（register/hold/release/
//!   deregister）+ 挂单容量闸门 [`CapacityRegistry::check_offer`]，fail-closed，
//!   守恒不变量 `held <= capacity`；只登记准入质押，不冻结/罚没。
//! - **v3.8.2 计量账本**：[`MeteringLedger`] 按 `(订单,形态,维度)` 对真实用量做
//!   append-only 正计量（[`MeteringLedger::record`]），累计 `consumed` 不得超过撮合
//!   预留 `allocated`，并整数算出实耗/待退结算视图（[`MeterSettlement`]），恒有
//!   `reserved = consumed_cost + refund`；只记账、不动资金、不连链。
//! - **v3.8.3 撮合编排**：[`MatchingEngine`] 把订单状态机与容量/计量两账按状态边接线——
//!   `Matched → CapacityHeld` 按成交量 hold，执行/计量期开计量线并正计量，结算/判罚/
//!   持有期取消 release；一次 hold 恰好一次 release，跨账守恒
//!   `capacity.held == Σ 已 hold 未终态订单成交量`，副作用失败不改写（fail-closed）。
//!   仍是内存确定性编排，不动资金、不连链、不持久化、不新增对外写能力。
//! - **v3.8.4 准入质押**：[`StakeLedger`] 独立资金账本，把容量注册里仅登记的
//!   `stake_micro` 申报额落成可记账的四桶资金（available/frozen/slashed/withdrawn），
//!   恒有 `deposited == available+frozen+slashed+withdrawn`；deposit/freeze/unfreeze/
//!   slash/withdraw 全 checked、越界 fail-closed 不改写。**罚没只能来自己冻结保证金**
//!   （决策/资金分离，账本不判违规），且**不改动**容量账本语义、不与订单/链接线。
//!
//! # 为什么内核是系统插件（T0）而不是官方插件（T1）
//!
//! 资源市场的托管、守恒分账、罚没持有资金守恒权，与 AUSec 沙盒基础设施同级——
//! 属于「内核能力」而非「可替换功能」。T1 官方插件经 process/JS 承载，无法承载
//! `u128` 守恒的 cargo 单测面；故确定性记账内核以 T0 native 落地（与 `sys.ausec`
//! 同构）。面向插件/进程的市场门面（挂单受理、撮合入口）在后续小版本以 T1
//! `com.twinsearth.official.resource-market` 提供，其结算仍回调本内核。
//!
//! 后续小版本：3.8.1 注册容量、3.8.2 计量账本、3.8.3 撮合、3.8.4 质押罚没、
//! 3.8.5 托管结算守恒、3.8.6 快照商品化版税、3.8.7 四维信誉、3.8.8 BFT-lite QA、
//! 3.8.9 动态定价/聚合/冷启动开关。

pub mod capacity;
pub mod catalog;
pub mod matching;
pub mod metering;
pub mod stake;

pub use capacity::{CapacityRegistration, CapacityRegistry};
pub use catalog::{catalog_entries, default_units};
pub use matching::MatchingEngine;
pub use metering::{MeterSettlement, MeteringLedger, UsageLine};
pub use stake::{StakeAccount, StakeLedger};

use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::runtime::native::NativeRuntime;
use serde::{Deserialize, Serialize};

/// 资源市场内核系统插件 id（T0，随内核进程内注册，不可热插拔）。
pub const RESOURCE_MARKET_PLUGIN: &str = "com.twinsearth.sys.resource-market";

/// PMB 方法：资源市场基座只读状态（版本 + 记账单位 + 状态机口径 + 诚实位）。
pub const METHOD_RESOURCE_MARKET_STATUS: &str = "resource_market_status";

/// 1 个内部 credit 对应的微单位数（10^6）。所有金额在内部以整数 micro 记账，
/// 杜绝浮点误差；法币/加密货币换算不在本版本范围。
pub const MICRO_UNITS_PER_CREDIT: u128 = 1_000_000;

/// 闲置资源的四种形态（对应开发计划「闲置资源的四种形态」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    /// 算力：CPU 空闲周期、GPU 显存/算力、NPU 推理单元。
    Compute,
    /// 存储：本地磁盘、NAS、对象存储冷数据（UDOS 切片）。
    Storage,
    /// 网络：上行带宽、公网 IP、P2P 中继/DHT 路由。
    Network,
    /// Agent 能力：专业 Agent、技能插件、pack_diff 环境快照。
    AgentCapability,
}

impl ResourceKind {
    /// 规范字符串（蛇形，与 serde 表示一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            ResourceKind::Compute => "compute",
            ResourceKind::Storage => "storage",
            ResourceKind::Network => "network",
            ResourceKind::AgentCapability => "agent_capability",
        }
    }

    /// 全部已知资源形态。
    pub const ALL: &[ResourceKind] = &[
        ResourceKind::Compute,
        ResourceKind::Storage,
        ResourceKind::Network,
        ResourceKind::AgentCapability,
    ];

    /// 从字符串解析资源形态。
    pub fn parse(s: &str) -> Option<ResourceKind> {
        Self::ALL.iter().copied().find(|k| k.as_str() == s)
    }
}

/// 计量维度（对应多维资源计量：算力/内存/存储/网络/调用/快照恢复）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeterUnit {
    /// CPU 毫秒。
    CpuMillis,
    /// GPU 毫秒。
    GpuMillis,
    /// 内存 MB·秒（共享页缓存按折算计）。
    MemoryMbSec,
    /// 存储 GB·秒（按需加载镜像块）。
    StorageGbSec,
    /// 网络入/出字节。
    NetworkBytes,
    /// Agent/技能按次调用。
    Invocation,
    /// pack_diff 快照恢复次数。
    SnapshotRestore,
}

impl MeterUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            MeterUnit::CpuMillis => "cpu_millis",
            MeterUnit::GpuMillis => "gpu_millis",
            MeterUnit::MemoryMbSec => "memory_mb_sec",
            MeterUnit::StorageGbSec => "storage_gb_sec",
            MeterUnit::NetworkBytes => "network_bytes",
            MeterUnit::Invocation => "invocation",
            MeterUnit::SnapshotRestore => "snapshot_restore",
        }
    }
}

/// 资源市场领域错误（类型化拒绝，不静默降级）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceError {
    /// 非法状态转移。
    InvalidTransition { from: OrderState, to: OrderState },
    /// 数量必须为正。
    NonPositiveQuantity,
    /// 单价必须为正（micro/单位）。
    NonPositiveUnitPrice,
    /// 挂单与求购的资源形态不匹配。
    KindMismatch {
        offer: ResourceKind,
        ask: ResourceKind,
    },
    /// 挂单与求购的计量单位不匹配。
    UnitMismatch { offer: MeterUnit, ask: MeterUnit },
    /// 求购最高限价低于供给单价，无法成交。
    AskPriceBelowOffer {
        ask_max_micro: u128,
        unit_price_micro: u128,
    },
    /// 求购数量超过供给可售数量。
    AskExceedsOfferQuantity { ask: u128, available: u128 },
    /// 供给方 DID 为空（注册/挂单必须有可追责主体）。
    EmptyProviderDid,
    /// 注册的计量维度不属于该资源形态的目录（v3.8.1，见 [`crate::economy::resource::catalog::default_units`]）。
    UnitNotInCatalog { kind: ResourceKind, unit: MeterUnit },
    /// 同一 (供给方, 资源形态, 计量单位) 容量已注册，不可重复注册（需先注销再改）。
    DuplicateRegistration {
        provider_did: String,
        kind: ResourceKind,
        unit: MeterUnit,
    },
    /// 容量注册不存在（未注册容量不得挂单/预留/释放）。
    RegistrationNotFound,
    /// 预留超出当前可售容量（held + 本次 > capacity）。
    CapacityExceeded {
        hold_requested: u128,
        available: u128,
    },
    /// 释放量超过已预留量（违反 held 守恒，疑似重复释放/记账篡改）。
    OverRelease { attempted: u128, held: u128 },
    /// 仍有预留容量时禁止注销（防止带着未结订单卷走供给）。
    CapacityStillHeld { held: u128 },
    /// 挂单数量超过该供给方注册的当前可售容量。
    OfferExceedsRegisteredCapacity {
        offer_quantity: u128,
        available: u128,
    },
    /// 计量订单 id 为空（v3.8.2，开计量线必须有可追责订单）。
    EmptyOrderId,
    /// 同一 (订单, 资源形态, 计量单位) 计量线已开，不可重复开。
    DuplicateUsageLine {
        order_id: String,
        kind: ResourceKind,
        unit: MeterUnit,
    },
    /// 计量线不存在（未开线不得上报用量）。
    UsageLineNotFound,
    /// 累计实耗超过撮合预留上限（违反 `consumed <= allocated`，fail-closed）。
    UsageExceedsAllocation {
        consumed_after: u128,
        allocated: u128,
    },
    /// 撮合订单不存在（v3.8.3，对未知订单 id 的任何编排操作具名拒绝）。
    OrderNotFound,
    /// 订单 id 已存在（v3.8.3，同一 id 不可重复提交成交）。
    DuplicateOrder { order_id: String },
    /// 质押账户不存在（v3.8.4，未开户主体不得冻结/解冻/罚没/提取）。
    StakeAccountNotFound,
    /// 可用保证金不足（v3.8.4，冻结/提取超过 available，fail-closed）。
    InsufficientFreeStake { requested: u128, available: u128 },
    /// 解冻超过冻结额（v3.8.4，违反 frozen 守恒，疑似记账篡改）。
    UnfreezeExceedsFrozen { attempted: u128, frozen: u128 },
    /// 罚没超过冻结额（v3.8.4；罚没只能来自己冻结保证金，不能动可用余额）。
    SlashExceedsFrozen { attempted: u128, frozen: u128 },
    /// 记账溢出（u128）。
    ArithmeticOverflow,
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceError::InvalidTransition { from, to } => write!(
                f,
                "RESOURCE_INVALID_TRANSITION: 非法状态转移: {} → {}",
                from.as_str(),
                to.as_str()
            ),
            ResourceError::NonPositiveQuantity => {
                write!(f, "RESOURCE_NON_POSITIVE_QUANTITY: 数量必须为正")
            }
            ResourceError::NonPositiveUnitPrice => {
                write!(f, "RESOURCE_NON_POSITIVE_PRICE: 单价必须为正(micro/单位)")
            }
            ResourceError::KindMismatch { offer, ask } => write!(
                f,
                "RESOURCE_KIND_MISMATCH: 资源形态不匹配: offer={} ask={}",
                offer.as_str(),
                ask.as_str()
            ),
            ResourceError::UnitMismatch { offer, ask } => write!(
                f,
                "RESOURCE_UNIT_MISMATCH: 计量单位不匹配: offer={} ask={}",
                offer.as_str(),
                ask.as_str()
            ),
            ResourceError::AskPriceBelowOffer {
                ask_max_micro,
                unit_price_micro,
            } => write!(
                f,
                "RESOURCE_PRICE_BELOW_OFFER: 求购限价 {ask_max_micro} micro 低于供给单价 {unit_price_micro} micro/单位"
            ),
            ResourceError::AskExceedsOfferQuantity { ask, available } => write!(
                f,
                "RESOURCE_QTY_EXCEEDS: 求购数量 {ask} 超过可售 {available}"
            ),
            ResourceError::EmptyProviderDid => {
                write!(f, "RESOURCE_EMPTY_PROVIDER_DID: 供给方 DID 不能为空")
            }
            ResourceError::UnitNotInCatalog { kind, unit } => write!(
                f,
                "RESOURCE_UNIT_NOT_IN_CATALOG: 计量维度 {} 不属于资源形态 {} 的目录",
                unit.as_str(),
                kind.as_str()
            ),
            ResourceError::DuplicateRegistration {
                provider_did,
                kind,
                unit,
            } => write!(
                f,
                "RESOURCE_DUPLICATE_REGISTRATION: 容量已注册: provider={provider_did} kind={} unit={}",
                kind.as_str(),
                unit.as_str()
            ),
            ResourceError::RegistrationNotFound => {
                write!(f, "RESOURCE_REGISTRATION_NOT_FOUND: 容量注册不存在")
            }
            ResourceError::CapacityExceeded {
                hold_requested,
                available,
            } => write!(
                f,
                "RESOURCE_CAPACITY_EXCEEDED: 预留 {hold_requested} 超过当前可售容量 {available}"
            ),
            ResourceError::OverRelease { attempted, held } => write!(
                f,
                "RESOURCE_OVER_RELEASE: 释放 {attempted} 超过已预留 {held}（违反 held 守恒）"
            ),
            ResourceError::CapacityStillHeld { held } => write!(
                f,
                "RESOURCE_CAPACITY_STILL_HELD: 仍有 {held} 预留容量，禁止注销"
            ),
            ResourceError::OfferExceedsRegisteredCapacity {
                offer_quantity,
                available,
            } => write!(
                f,
                "RESOURCE_OFFER_EXCEEDS_CAPACITY: 挂单数量 {offer_quantity} 超过注册可售 {available}"
            ),
            ResourceError::EmptyOrderId => {
                write!(f, "RESOURCE_EMPTY_ORDER_ID: 计量订单 id 不能为空")
            }
            ResourceError::DuplicateUsageLine {
                order_id,
                kind,
                unit,
            } => write!(
                f,
                "RESOURCE_DUPLICATE_USAGE_LINE: 计量线已开: order={order_id} kind={} unit={}",
                kind.as_str(),
                unit.as_str()
            ),
            ResourceError::UsageLineNotFound => {
                write!(f, "RESOURCE_USAGE_LINE_NOT_FOUND: 计量线不存在（未开线不得计量）")
            }
            ResourceError::UsageExceedsAllocation {
                consumed_after,
                allocated,
            } => write!(
                f,
                "RESOURCE_USAGE_EXCEEDS_ALLOCATION: 累计实耗 {consumed_after} 超过预留上限 {allocated}"
            ),
            ResourceError::OrderNotFound => {
                write!(f, "RESOURCE_ORDER_NOT_FOUND: 撮合订单不存在")
            }
            ResourceError::DuplicateOrder { order_id } => {
                write!(f, "RESOURCE_DUPLICATE_ORDER: 订单 id 已存在: {order_id}")
            }
            ResourceError::StakeAccountNotFound => {
                write!(f, "RESOURCE_STAKE_ACCOUNT_NOT_FOUND: 质押账户不存在（未开户不得冻结/解冻/罚没/提取）")
            }
            ResourceError::InsufficientFreeStake { requested, available } => write!(
                f,
                "RESOURCE_INSUFFICIENT_FREE_STAKE: 请求 {requested} 超过可用保证金 {available}"
            ),
            ResourceError::UnfreezeExceedsFrozen { attempted, frozen } => write!(
                f,
                "RESOURCE_UNFREEZE_EXCEEDS_FROZEN: 解冻 {attempted} 超过冻结额 {frozen}（违反 frozen 守恒）"
            ),
            ResourceError::SlashExceedsFrozen { attempted, frozen } => write!(
                f,
                "RESOURCE_SLASH_EXCEEDS_FROZEN: 罚没 {attempted} 超过冻结额 {frozen}（只能罚没已冻结保证金）"
            ),
            ResourceError::ArithmeticOverflow => {
                write!(f, "RESOURCE_ARITHMETIC_OVERFLOW: u128 记账溢出")
            }
        }
    }
}

impl std::error::Error for ResourceError {}

/// 内部记账金额：`u128` 微单位（1 credit = [`MICRO_UNITS_PER_CREDIT`] micro）。
///
/// 只用整数运算；本版本不与法币/链币挂钩，故无汇率、无浮点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Credits {
    /// 微单位总额。
    pub micro: u128,
}

impl Credits {
    /// 从微单位构造。
    pub fn from_micro(micro: u128) -> Self {
        Self { micro }
    }

    /// 从整数 credit 构造（×10^6）。
    pub fn from_credits(credits: u128) -> Result<Self, ResourceError> {
        credits
            .checked_mul(MICRO_UNITS_PER_CREDIT)
            .map(|micro| Self { micro })
            .ok_or(ResourceError::ArithmeticOverflow)
    }

    /// 微单位原值。
    pub fn as_micro(self) -> u128 {
        self.micro
    }

    /// 整数 credit 部分（向下取整）。
    pub fn credits(self) -> u128 {
        self.micro / MICRO_UNITS_PER_CREDIT
    }

    /// 零金额。
    pub fn zero() -> Self {
        Self { micro: 0 }
    }

    /// 饱和加法（记账溢出具名报错由调用方按需选择；这里提供 checked 版本）。
    pub fn checked_add(self, other: Credits) -> Result<Credits, ResourceError> {
        self.micro
            .checked_add(other.micro)
            .map(|micro| Self { micro })
            .ok_or(ResourceError::ArithmeticOverflow)
    }
}

/// 资源单（订单）生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderState {
    /// 草稿：尚未发布。
    Drafted,
    /// 已发布：挂单/求购在场，等待撮合。
    Published,
    /// 已撮合：offer 与 ask 匹配成功。
    Matched,
    /// 容量已预留：供给方资源额度被持有。
    CapacityHeld,
    /// 执行中：任务在沙盒里运行。
    Executing,
    /// 计量中：按多维单位累计用量。
    Metering,
    /// 待验证：结果交 BFT-lite QA / 抽样验证。
    QaPending,
    /// 已结算：分账完成（终态）。
    Settled,
    /// 争议中：验证失败进入仲裁。
    Disputed,
    /// 已罚没：违规裁决后扣质押（终态）。
    Slashed,
    /// 已取消：撮合前/容量持有阶段释放（终态）。
    Cancelled,
}

impl OrderState {
    /// 规范字符串（蛇形，与 serde 一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            OrderState::Drafted => "drafted",
            OrderState::Published => "published",
            OrderState::Matched => "matched",
            OrderState::CapacityHeld => "capacity_held",
            OrderState::Executing => "executing",
            OrderState::Metering => "metering",
            OrderState::QaPending => "qa_pending",
            OrderState::Settled => "settled",
            OrderState::Disputed => "disputed",
            OrderState::Slashed => "slashed",
            OrderState::Cancelled => "cancelled",
        }
    }

    /// 是否为终态（不可再转移）。
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            OrderState::Settled | OrderState::Slashed | OrderState::Cancelled
        )
    }

    /// 状态机唯一合法转移判定（单一赋值点）。
    ///
    /// 合法前向边：
    /// ```text
    /// Drafted      -> Published | Cancelled
    /// Published    -> Matched   | Cancelled
    /// Matched      -> CapacityHeld | Cancelled
    /// CapacityHeld -> Executing | Cancelled
    /// Executing    -> Metering
    /// Metering     -> QaPending
    /// QaPending    -> Settled | Disputed
    /// Disputed     -> Settled | Slashed
    /// Settled/Slashed/Cancelled 为终态
    /// ```
    pub fn can_transition(self, to: OrderState) -> bool {
        use OrderState::*;
        matches!(
            (self, to),
            (Drafted, Published | Cancelled)
                | (Published, Matched | Cancelled)
                | (Matched, CapacityHeld | Cancelled)
                | (CapacityHeld, Executing | Cancelled)
                | (Executing, Metering)
                | (Metering, QaPending)
                | (QaPending, Settled | Disputed)
                | (Disputed, Settled | Slashed)
        )
    }

    /// 执行一次状态转移：越态 / 从终态转出一律具名拒绝。
    pub fn transition(self, to: OrderState) -> Result<OrderState, ResourceError> {
        if self == to {
            return Err(ResourceError::InvalidTransition { from: self, to });
        }
        if !self.can_transition(to) {
            return Err(ResourceError::InvalidTransition { from: self, to });
        }
        Ok(to)
    }
}

/// 资源供给挂单（Offer）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceOffer {
    /// 供给方 DID。
    pub provider_did: String,
    /// 资源形态。
    pub kind: ResourceKind,
    /// 计量单位。
    pub unit: MeterUnit,
    /// 可售数量（按 `unit`，必须为正）。
    pub quantity: u128,
    /// 单价（micro/单位，必须为正）。
    pub unit_price_micro: u128,
    /// 关联 pack_diff 环境快照引用（仅 AgentCapability/快照类使用，可空；本版本不解析）。
    pub snapshot_ref: Option<String>,
}

impl ResourceOffer {
    /// 构造并做最基本的正数校验。
    pub fn new(
        provider_did: impl Into<String>,
        kind: ResourceKind,
        unit: MeterUnit,
        quantity: u128,
        unit_price_micro: u128,
        snapshot_ref: Option<String>,
    ) -> Result<Self, ResourceError> {
        if quantity == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        if unit_price_micro == 0 {
            return Err(ResourceError::NonPositiveUnitPrice);
        }
        Ok(Self {
            provider_did: provider_did.into(),
            kind,
            unit,
            quantity,
            unit_price_micro,
            snapshot_ref,
        })
    }
}

/// 资源求购单（Ask）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceAsk {
    /// 消费方 DID。
    pub buyer_did: String,
    /// 资源形态。
    pub kind: ResourceKind,
    /// 计量单位。
    pub unit: MeterUnit,
    /// 求购数量（按 `unit`，必须为正）。
    pub quantity: u128,
    /// 最高可接受单价（micro/单位，必须为正）。
    pub max_unit_price_micro: u128,
    /// 是否时延敏感（影响后续撮合/调度优先级，本版本仅记录）。
    pub latency_sensitive: bool,
}

impl ResourceAsk {
    pub fn new(
        buyer_did: impl Into<String>,
        kind: ResourceKind,
        unit: MeterUnit,
        quantity: u128,
        max_unit_price_micro: u128,
        latency_sensitive: bool,
    ) -> Result<Self, ResourceError> {
        if quantity == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        if max_unit_price_micro == 0 {
            return Err(ResourceError::NonPositiveUnitPrice);
        }
        Ok(Self {
            buyer_did: buyer_did.into(),
            kind,
            unit,
            quantity,
            max_unit_price_micro,
            latency_sensitive,
        })
    }
}

/// 撮合后的资源订单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceOrder {
    /// 订单 id（调用方/后续撮合器分配；本版本不生成 id）。
    pub order_id: String,
    /// 成交供给。
    pub offer: ResourceOffer,
    /// 成交求购。
    pub ask: ResourceAsk,
    /// 成交数量（≤ offer.quantity，≤ ask.quantity）。
    pub filled_quantity: u128,
    /// 成交单价（取供给单价；不得高于 ask 限价）。
    pub fill_unit_price_micro: u128,
    /// 当前生命周期状态。
    pub state: OrderState,
}

impl ResourceOrder {
    /// 以给定 offer/ask 尝试成交：校验形态/单位/价格/数量，全部满足才建单，
    /// 初始状态 [`OrderState::Matched`]。这是后续 3.8.3 撮合器的内核入场函数。
    pub fn match_offer_ask(
        order_id: impl Into<String>,
        offer: &ResourceOffer,
        ask: &ResourceAsk,
    ) -> Result<Self, ResourceError> {
        if offer.kind != ask.kind {
            return Err(ResourceError::KindMismatch {
                offer: offer.kind,
                ask: ask.kind,
            });
        }
        if offer.unit != ask.unit {
            return Err(ResourceError::UnitMismatch {
                offer: offer.unit,
                ask: ask.unit,
            });
        }
        if ask.max_unit_price_micro < offer.unit_price_micro {
            return Err(ResourceError::AskPriceBelowOffer {
                ask_max_micro: ask.max_unit_price_micro,
                unit_price_micro: offer.unit_price_micro,
            });
        }
        let filled = ask.quantity.min(offer.quantity);
        if filled == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        Ok(Self {
            order_id: order_id.into(),
            offer: offer.clone(),
            ask: ask.clone(),
            filled_quantity: filled,
            fill_unit_price_micro: offer.unit_price_micro,
            state: OrderState::Matched,
        })
    }

    /// 订单名义成交额（micro）= 成交数量 × 成交单价，checked 防溢出。
    /// 守恒分账（3.8.5）以此为待托管总额。
    pub fn notional_micro(&self) -> Result<u128, ResourceError> {
        self.filled_quantity
            .checked_mul(self.fill_unit_price_micro)
            .ok_or(ResourceError::ArithmeticOverflow)
    }

    /// 推进订单状态（状态机唯一外部入口）。
    pub fn advance(&mut self, to: OrderState) -> Result<OrderState, ResourceError> {
        self.state = self.state.transition(to)?;
        Ok(self.state)
    }
}

/// 资源市场基座只读状态（PMB `resource_market_status`）。
pub fn status_payload() -> serde_json::Value {
    serde_json::json!({
        "plugin": RESOURCE_MARKET_PLUGIN,
        "version": env!("CARGO_PKG_VERSION"),
        "domain": "agent-resource-market",
        "introduced_in": "v3.8.0",
        "accounting_unit": {
            "name": "credits",
            "micro_units_per_credit": MICRO_UNITS_PER_CREDIT,
            "integer_only": true,
            "fiat_pegged": false,
            "chain_settled": false,
            "note": "内部记账单位；不挂法币、不挂链。BTC/ETH/稳定币结算在 v3.9.x。"
        },
        "resource_kinds": ResourceKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
        "order_states": [
            OrderState::Drafted.as_str(),
            OrderState::Published.as_str(),
            OrderState::Matched.as_str(),
            OrderState::CapacityHeld.as_str(),
            OrderState::Executing.as_str(),
            OrderState::Metering.as_str(),
            OrderState::QaPending.as_str(),
            OrderState::Settled.as_str(),
            OrderState::Disputed.as_str(),
            OrderState::Slashed.as_str(),
            OrderState::Cancelled.as_str(),
        ],
        "capabilities": [
            "market:resource:offer",
            "market:resource:ask",
            "market:resource:settle",
            "market:stake",
            "market:slash"
        ],
        // 诚实位：基座只提供确定性领域内核与只读查询；撮合/托管/质押/链上结算均未接线。
        "enforceable": {
            "state_machine": true,
            "matching_kernel": true,
            "capacity_registration": true,
            "metering_ledger": true,
            "matching_orchestration": true,
            "stake_ledger": true,
            "escrow_settlement": false,
            "stake_slash": false,
            "onchain_payment": false
        },
        "provided": {
            "resource_market_status": true,
            "order_lifecycle_advance": false,
            "offer_ask_submit": false,
            "capacity_registry_persistence": false,
            "metering_ledger_persistence": false,
            "matching_engine_persistence": false,
            "stake_ledger_persistence": false
        },
        "note": "v3.8.4：新增独立准入质押账本 StakeLedger——deposit/freeze/unfreeze/slash/withdraw 四桶资金（available/frozen/slashed/withdrawn）守恒 deposited=四桶之和，全 checked、越界 fail-closed 不改写；罚没只能来自己冻结保证金（决策/资金分离，账本不判违规），不改动容量账本、不与订单/PMB/链接线、不持久化。stake_ledger 内核已就绪可单测，但质押按订单自动冻结、QA/审判驱动罚没、托管分账与链上结算仍在后续小版本（stake_slash/escrow_settlement/onchain_payment 暂 false）。"
    })
}

/// 字节桥：`resource_market_status`（无参或空负载，只读）。
fn handle_resource_market_status(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    serde_json::to_vec(&status_payload())
        .map_err(|e| PluginError::Runtime(format!("resource_market_status 序列化失败: {e}")))
}

/// 把资源市场内核只读处理器注册到 T0 native 运行时（随系统插件装配调用）。
///
/// v3.8.0 只注册只读状态查询；状态推进/挂单受理在后续小版本随撮合与托管接线，
/// 未接线的方法明确不注册（NotFound），不伪造可用。
pub fn register(rt: &mut NativeRuntime) {
    rt.register_handler(
        RESOURCE_MARKET_PLUGIN,
        METHOD_RESOURCE_MARKET_STATUS,
        handle_resource_market_status,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_forward_path_reaches_settled() {
        let mut s = OrderState::Drafted;
        for next in [
            OrderState::Published,
            OrderState::Matched,
            OrderState::CapacityHeld,
            OrderState::Executing,
            OrderState::Metering,
            OrderState::QaPending,
            OrderState::Settled,
        ] {
            s = s.transition(next).unwrap();
        }
        assert_eq!(s, OrderState::Settled);
        assert!(s.is_terminal());
    }

    #[test]
    fn dispute_branch_can_settle_or_slash() {
        let s = OrderState::QaPending
            .transition(OrderState::Disputed)
            .unwrap();
        assert_eq!(
            s.transition(OrderState::Settled).unwrap(),
            OrderState::Settled
        );
        let s2 = OrderState::QaPending
            .transition(OrderState::Disputed)
            .unwrap();
        assert_eq!(
            s2.transition(OrderState::Slashed).unwrap(),
            OrderState::Slashed
        );
    }

    #[test]
    fn cancel_allowed_before_execution_only() {
        assert_eq!(
            OrderState::CapacityHeld
                .transition(OrderState::Cancelled)
                .unwrap(),
            OrderState::Cancelled
        );
        // 进入执行后不可取消。
        assert!(matches!(
            OrderState::Executing.transition(OrderState::Cancelled),
            Err(ResourceError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn illegal_skip_and_terminal_exit_rejected() {
        // 不能跳过撮合直接执行。
        assert!(matches!(
            OrderState::Published.transition(OrderState::Executing),
            Err(ResourceError::InvalidTransition { .. })
        ));
        // 终态不能转出。
        assert!(matches!(
            OrderState::Settled.transition(OrderState::Executing),
            Err(ResourceError::InvalidTransition { .. })
        ));
        // 自转非法。
        assert!(matches!(
            OrderState::Matched.transition(OrderState::Matched),
            Err(ResourceError::InvalidTransition { .. })
        ));
    }

    fn sample_offer() -> ResourceOffer {
        ResourceOffer::new(
            "did:nau:provider",
            ResourceKind::Compute,
            MeterUnit::CpuMillis,
            1000,
            2,
            None,
        )
        .unwrap()
    }

    #[test]
    fn match_rejects_kind_unit_price_and_validates_notional() {
        let offer = sample_offer();
        // 形态不匹配。
        let bad_kind = ResourceAsk::new(
            "did:nau:buyer",
            ResourceKind::Storage,
            MeterUnit::CpuMillis,
            100,
            5,
            true,
        )
        .unwrap();
        assert!(matches!(
            ResourceOrder::match_offer_ask("o1", &offer, &bad_kind),
            Err(ResourceError::KindMismatch { .. })
        ));
        // 限价低于单价。
        let cheap = ResourceAsk::new(
            "did:nau:buyer",
            ResourceKind::Compute,
            MeterUnit::CpuMillis,
            100,
            1,
            true,
        )
        .unwrap();
        assert!(matches!(
            ResourceOrder::match_offer_ask("o1", &offer, &cheap),
            Err(ResourceError::AskPriceBelowOffer { .. })
        ));
        // 成交：数量取较小者（ask=300 < offer=1000）。
        let ask = ResourceAsk::new(
            "did:nau:buyer",
            ResourceKind::Compute,
            MeterUnit::CpuMillis,
            300,
            5,
            true,
        )
        .unwrap();
        let order = ResourceOrder::match_offer_ask("o1", &offer, &ask).unwrap();
        assert_eq!(order.state, OrderState::Matched);
        assert_eq!(order.filled_quantity, 300);
        assert_eq!(order.fill_unit_price_micro, 2);
        // 名义额 300 × 2 = 600 micro。
        assert_eq!(order.notional_micro().unwrap(), 600);
    }

    #[test]
    fn credits_integer_unit_and_checked_arithmetic() {
        let c = Credits::from_credits(3).unwrap();
        assert_eq!(c.as_micro(), 3_000_000);
        assert_eq!(c.credits(), 3);
        let sum = c.checked_add(Credits::from_micro(500_000)).unwrap();
        assert_eq!(sum.as_micro(), 3_500_000);
        assert_eq!(sum.credits(), 3); // 向下取整
                                      // 溢出具名报错而非 panic。
        assert!(matches!(
            Credits::from_micro(u128::MAX).checked_add(Credits::from_micro(1)),
            Err(ResourceError::ArithmeticOverflow)
        ));
        // 非正构造拒绝。
        assert!(matches!(
            ResourceOffer::new(
                "d",
                ResourceKind::Storage,
                MeterUnit::StorageGbSec,
                0,
                1,
                None
            ),
            Err(ResourceError::NonPositiveQuantity)
        ));
    }

    #[test]
    fn resource_kind_parse_roundtrip() {
        for k in ResourceKind::ALL {
            assert_eq!(ResourceKind::parse(k.as_str()), Some(*k));
        }
        assert_eq!(ResourceKind::parse("nope"), None);
    }
}
