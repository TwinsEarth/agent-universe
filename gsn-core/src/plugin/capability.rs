//! 能力模型 —— 能力令牌（Capability Token）与能力矩阵
//!
//! # 权限是令牌，不是约定
//!
//! 插件在清单里声明所需能力，宿主在加载时**签发能力令牌**；没有令牌的调用在总线入口
//! 就被拒绝，而不是靠插件自觉。
//!
//! 令牌（[`CapabilityToken`]）在加载时签发，随插件实例生命周期存在，绑定到具体清单
//! 字节（`manifest_digest`），换清单即失效。
//!
//! # 能力组
//!
//! - **基础能力**（所有级别）：`plugin:lifecycle:read`、`plugin:message:send`、
//!   `plugin:storage:own`；
//! - **网络 / 链能力**：T0 全有，T1/T2 声明式，T3 拒绝；
//! - **内核能力**：仅 T0（`kernel:plugin:manage` 等）。

use crate::plugin::tier::Tier;
use serde::{Deserialize, Serialize};

/// 插件能力（细粒度、命名空间化）。
///
/// 字符串形式为 `group:resource:action`，例如 `plugin:message:send`、`net:dht:read`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// 读取自身生命周期状态（基础，全级别）。
    #[serde(rename = "plugin:lifecycle:read")]
    LifecycleRead,
    /// 向总线发送消息（基础，全级别）。
    #[serde(rename = "plugin:message:send")]
    MessageSend,
    /// 读写自身沙盒目录（基础，全级别）。
    #[serde(rename = "plugin:storage:own")]
    StorageOwn,

    /// 读取 DHT。
    #[serde(rename = "net:dht:read")]
    DhtRead,
    /// 写入 DHT。
    #[serde(rename = "net:dht:write")]
    DhtWrite,
    /// 发布 GossipSub。
    #[serde(rename = "net:gossip:publish")]
    GossipPublish,
    /// 订阅 GossipSub。
    #[serde(rename = "net:gossip:subscribe")]
    GossipSubscribe,

    /// 读取链上状态。
    #[serde(rename = "chain:evm:read")]
    EvmRead,
    /// 写链上交易。
    #[serde(rename = "chain:evm:write")]
    EvmWrite,

    /// 经济结算。
    #[serde(rename = "economy:settle")]
    EconomySettle,
    /// 创建 AgentCard。
    #[serde(rename = "agent:card:create")]
    AgentCardCreate,
    /// 更新 AgentCard。
    #[serde(rename = "agent:card:update")]
    AgentCardUpdate,
    /// 参与群体智能共识。
    #[serde(rename = "swarm:consensus")]
    SwarmConsensus,

    /// 管理插件生命周期（内核，仅 T0）。
    #[serde(rename = "kernel:plugin:manage")]
    PluginManage,
    /// 写安全策略（内核，仅 T0）。
    #[serde(rename = "kernel:policy:write")]
    PolicyWrite,
    /// 配置隔离（内核，仅 T0）。
    #[serde(rename = "kernel:isolation:configure")]
    IsolationConfigure,

    // ── AUSec 弹性计算（v3.7.0 起），见 docs/ausec/AUSEC-DESIGN.md §4 ──
    // create/configure 分离：能创建沙盒 ≠ 能改隔离参数（后者是提权路径，仅 System）。
    /// 驱动沙盒生命周期。
    #[serde(rename = "sandbox:lifecycle")]
    SandboxLifecycle,
    /// 经 PMB 驱动沙盒动作。
    #[serde(rename = "sandbox:message")]
    SandboxMessage,
    /// 创建沙盒。
    #[serde(rename = "sandbox:create")]
    SandboxCreate,
    /// 生成 pack_diff 增量快照。
    #[serde(rename = "sandbox:snapshot")]
    SandboxSnapshot,
    /// 从快照恢复 / 轨迹分叉。
    #[serde(rename = "sandbox:restore")]
    SandboxRestore,
    /// 配置隔离参数（需额外审批，仅 T0）。
    #[serde(rename = "sandbox:configure")]
    SandboxConfigure,
    /// 下发 AppArmor/eBPF/安全策略（仅 T0）。
    #[serde(rename = "sandbox:policy:apply")]
    SandboxPolicyApply,
    /// 安全黑名单同步 / 紧急广播（仅 T0）。
    #[serde(rename = "sandbox:blacklist:sync")]
    SandboxBlacklistSync,

    // ── 资源市场（v3.8.0 起），见 releases/v3.8.0.md 与开发计划附录 B grant 矩阵 ──
    // 闲置算力/存储/网络/Agent 能力的挂单、求购、结算、质押与罚没。
    /// 挂出闲置资源供给（Offer）。OFF/CERT 声明式，3RD 拒绝。
    #[serde(rename = "market:resource:offer")]
    MarketResourceOffer,
    /// 发布资源求购（Ask）。开放入口：除黑名单外所有级别默认授予。
    #[serde(rename = "market:resource:ask")]
    MarketResourceAsk,
    /// 执行托管资金结算（守恒分账）。仅 T0 内核与 T1 官方结算插件，不可仅声明获得。
    #[serde(rename = "market:resource:settle")]
    MarketResourceSettle,
    /// 质押准入保证金（OFF/CERT 声明式，3RD 拒绝）。
    #[serde(rename = "market:stake")]
    MarketStake,
    /// 罚没质押（终局惩戒，仅 T0：Agent 审判/结算内核）。
    #[serde(rename = "market:slash")]
    MarketSlash,
}

impl Capability {
    /// 全部已知能力（用于清单能力名校验）。
    pub const ALL: &[Capability] = &[
        Capability::LifecycleRead,
        Capability::MessageSend,
        Capability::StorageOwn,
        Capability::DhtRead,
        Capability::DhtWrite,
        Capability::GossipPublish,
        Capability::GossipSubscribe,
        Capability::EvmRead,
        Capability::EvmWrite,
        Capability::EconomySettle,
        Capability::AgentCardCreate,
        Capability::AgentCardUpdate,
        Capability::SwarmConsensus,
        Capability::PluginManage,
        Capability::PolicyWrite,
        Capability::IsolationConfigure,
        Capability::SandboxLifecycle,
        Capability::SandboxMessage,
        Capability::SandboxCreate,
        Capability::SandboxSnapshot,
        Capability::SandboxRestore,
        Capability::SandboxConfigure,
        Capability::SandboxPolicyApply,
        Capability::SandboxBlacklistSync,
        Capability::MarketResourceOffer,
        Capability::MarketResourceAsk,
        Capability::MarketResourceSettle,
        Capability::MarketStake,
        Capability::MarketSlash,
    ];

    /// 能力的规范字符串形式。
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::LifecycleRead => "plugin:lifecycle:read",
            Capability::MessageSend => "plugin:message:send",
            Capability::StorageOwn => "plugin:storage:own",
            Capability::DhtRead => "net:dht:read",
            Capability::DhtWrite => "net:dht:write",
            Capability::GossipPublish => "net:gossip:publish",
            Capability::GossipSubscribe => "net:gossip:subscribe",
            Capability::EvmRead => "chain:evm:read",
            Capability::EvmWrite => "chain:evm:write",
            Capability::EconomySettle => "economy:settle",
            Capability::AgentCardCreate => "agent:card:create",
            Capability::AgentCardUpdate => "agent:card:update",
            Capability::SwarmConsensus => "swarm:consensus",
            Capability::PluginManage => "kernel:plugin:manage",
            Capability::PolicyWrite => "kernel:policy:write",
            Capability::IsolationConfigure => "kernel:isolation:configure",
            Capability::SandboxLifecycle => "sandbox:lifecycle",
            Capability::SandboxMessage => "sandbox:message",
            Capability::SandboxCreate => "sandbox:create",
            Capability::SandboxSnapshot => "sandbox:snapshot",
            Capability::SandboxRestore => "sandbox:restore",
            Capability::SandboxConfigure => "sandbox:configure",
            Capability::SandboxPolicyApply => "sandbox:policy:apply",
            Capability::SandboxBlacklistSync => "sandbox:blacklist:sync",
            Capability::MarketResourceOffer => "market:resource:offer",
            Capability::MarketResourceAsk => "market:resource:ask",
            Capability::MarketResourceSettle => "market:resource:settle",
            Capability::MarketStake => "market:stake",
            Capability::MarketSlash => "market:slash",
        }
    }

    /// 从字符串解析能力（清单能力名）。
    pub fn parse(s: &str) -> Option<Capability> {
        Capability::ALL.iter().copied().find(|c| c.as_str() == s)
    }

    /// 是否为内核能力（仅 T0 可拥有）。
    pub fn is_kernel(self) -> bool {
        matches!(
            self,
            Capability::PluginManage | Capability::PolicyWrite | Capability::IsolationConfigure
        )
    }

    /// 是否为基础能力（所有级别默认拥有）。
    pub fn is_basic(self) -> bool {
        matches!(
            self,
            Capability::LifecycleRead | Capability::MessageSend | Capability::StorageOwn
        )
    }

    /// 是否为 AUSec **治理类**能力（配置隔离 / 下发策略 / 黑名单同步）：
    /// 仅 T0 系统插件可拥有，其它级别一律拒绝（即使声明也不行）。
    /// 见 docs/ausec/AUSEC-DESIGN.md §4。
    pub fn is_sandbox_governance(self) -> bool {
        matches!(
            self,
            Capability::SandboxConfigure
                | Capability::SandboxPolicyApply
                | Capability::SandboxBlacklistSync
        )
    }

    /// 是否为 AUSec **可委托类**能力（生命周期/消息/创建/快照/恢复）：
    /// T0 默认授予，T1/T2 可声明并经审批，T3 拒绝。
    /// `sandbox:snapshot/restore` 即官方插件 agent-council（v3.8.0）所需。
    pub fn is_sandbox_delegable(self) -> bool {
        matches!(
            self,
            Capability::SandboxLifecycle
                | Capability::SandboxMessage
                | Capability::SandboxCreate
                | Capability::SandboxSnapshot
                | Capability::SandboxRestore
        )
    }

    /// 是否为资源市场**开放入口**能力（v3.8.0）：求购 Ask 对除黑名单外的所有
    /// 级别默认授予（消费者不设准入门槛；黑名单由分级本身拒绝）。
    pub fn is_market_open(self) -> bool {
        matches!(self, Capability::MarketResourceAsk)
    }

    /// 是否为资源市场**结算**能力（v3.8.0）：仅 T0 内核与 T1 官方结算插件
    /// （`com.twinsearth.official.market-settle`）。第三方/认证即使声明也拒绝，
    /// 因为结算持有托管资金的守恒分账权。
    pub fn is_market_settle(self) -> bool {
        matches!(self, Capability::MarketResourceSettle)
    }

    /// 是否为资源市场**终局罚没**能力（v3.8.0）：仅 T0（Agent 审判/结算内核）。
    pub fn is_market_slash(self) -> bool {
        matches!(self, Capability::MarketSlash)
    }
}

/// 依据级别，决定某项能力是否在「默认授权」范围内。
///
/// - 基础能力：全级别授予；
/// - 网络/链能力：T0 全有，T1/T2 需在清单显式声明并经审批（这里返回 `false` 表示
///   「不在默认范围，需声明」），T3 永远拒绝（即使声明也不行）；
/// - 内核能力：仅 T0。
///
/// 返回值：
/// - `Granted`：默认授予；
/// - `Declarable`：不在默认范围，但该级别可在清单声明并经审批；
/// - `Denied`：该级别永远不可拥有。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    Granted,
    Declarable,
    Denied,
}

/// 能力矩阵：给定级别与能力，返回授权类别。
pub fn grant_for(tier: Tier, cap: Capability) -> Grant {
    if cap.is_basic() {
        return Grant::Granted;
    }
    if cap.is_kernel() || cap.is_sandbox_governance() || cap.is_market_slash() {
        return if tier == Tier::System {
            Grant::Granted
        } else {
            Grant::Denied
        };
    }
    // 资源市场结算：T0 内核 / T1 官方结算插件直接授予；认证/第三方/黑名单即使
    // 声明也拒绝（持有托管资金守恒分账权，不能靠声明获得）。
    if cap.is_market_settle() {
        return match tier {
            Tier::System | Tier::Official => Grant::Granted,
            Tier::Certified | Tier::ThirdParty | Tier::Blacklist => Grant::Denied,
        };
    }
    // 资源市场开放求购入口：除黑名单外所有级别默认授予。
    if cap.is_market_open() {
        return if tier == Tier::Blacklist {
            Grant::Denied
        } else {
            Grant::Granted
        };
    }
    // 网络 / 链 / 经济 / 智能体能力，以及 AUSec 可委托类沙盒能力。
    // 资源市场的 offer / stake 也落入此档：T0 全有，T1/T2 声明式，T3/黑名单拒绝。
    match tier {
        Tier::System => Grant::Granted,
        Tier::Official | Tier::Certified => Grant::Declarable,
        Tier::ThirdParty | Tier::Blacklist => Grant::Denied,
    }
}

/// 能力令牌：加载时签发，绑定到具体清单字节与插件实例。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// 令牌所属插件 id。
    pub plugin_id: String,
    /// 已授予的能力。
    pub granted: Vec<Capability>,
    /// 签发时间（Unix 秒）。
    pub issued_at: u64,
    /// 绑定的清单摘要（换清单即失效）。
    pub manifest_digest: String,
}

impl CapabilityToken {
    /// 令牌是否包含某项能力（总线入口检查）。
    pub fn has(&self, cap: Capability) -> bool {
        self.granted.contains(&cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_capabilities_all_tiers() {
        for tier in [
            Tier::System,
            Tier::Official,
            Tier::Certified,
            Tier::ThirdParty,
        ] {
            assert_eq!(grant_for(tier, Capability::MessageSend), Grant::Granted);
            assert_eq!(grant_for(tier, Capability::StorageOwn), Grant::Granted);
        }
    }

    #[test]
    fn kernel_capabilities_system_only() {
        assert_eq!(
            grant_for(Tier::System, Capability::PluginManage),
            Grant::Granted
        );
        assert_eq!(
            grant_for(Tier::Official, Capability::PluginManage),
            Grant::Denied
        );
        assert_eq!(
            grant_for(Tier::ThirdParty, Capability::PolicyWrite),
            Grant::Denied
        );
    }

    #[test]
    fn third_party_denied_network_even_if_declared() {
        // 关键安全属性：T3 即使在清单里声明网络能力，也永远被拒绝。
        assert_eq!(
            grant_for(Tier::ThirdParty, Capability::DhtWrite),
            Grant::Denied
        );
        assert_eq!(
            grant_for(Tier::ThirdParty, Capability::EvmWrite),
            Grant::Denied
        );
        assert_eq!(
            grant_for(Tier::ThirdParty, Capability::EconomySettle),
            Grant::Denied
        );
    }

    #[test]
    fn official_certified_declarable() {
        for tier in [Tier::Official, Tier::Certified] {
            assert_eq!(grant_for(tier, Capability::DhtWrite), Grant::Declarable);
            assert_eq!(
                grant_for(tier, Capability::EconomySettle),
                Grant::Declarable
            );
        }
    }

    #[test]
    fn roundtrip_parse_str() {
        for cap in Capability::ALL {
            assert_eq!(Capability::parse(cap.as_str()), Some(*cap));
        }
        assert_eq!(Capability::parse("not:a:cap"), None);
    }

    // ── AUSec（v3.7.0）能力矩阵，见 docs/ausec/AUSEC-DESIGN.md §4 ──
    #[test]
    fn ausec_governance_is_system_only() {
        // configure / policy:apply / blacklist:sync 是提权敏感能力：仅 T0。
        for gov in [
            Capability::SandboxConfigure,
            Capability::SandboxPolicyApply,
            Capability::SandboxBlacklistSync,
        ] {
            assert!(gov.is_sandbox_governance());
            assert_eq!(grant_for(Tier::System, gov), Grant::Granted);
            for t in [Tier::Official, Tier::Certified, Tier::ThirdParty] {
                assert_eq!(grant_for(t, gov), Grant::Denied, "{gov:?} for {t:?}");
            }
        }
    }

    #[test]
    fn ausec_delegable_offcert_declarable_thirdparty_denied() {
        for cap in [
            Capability::SandboxLifecycle,
            Capability::SandboxMessage,
            Capability::SandboxCreate,
            Capability::SandboxSnapshot,
            Capability::SandboxRestore,
        ] {
            assert!(cap.is_sandbox_delegable());
            assert_eq!(grant_for(Tier::System, cap), Grant::Granted);
            assert_eq!(grant_for(Tier::Official, cap), Grant::Declarable);
            assert_eq!(grant_for(Tier::Certified, cap), Grant::Declarable);
            assert_eq!(grant_for(Tier::ThirdParty, cap), Grant::Denied);
        }
    }

    // ── 资源市场（v3.8.0）能力矩阵，见开发计划附录 B ──
    #[test]
    fn market_offer_and_stake_offcert_declarable_thirdparty_denied() {
        for cap in [Capability::MarketResourceOffer, Capability::MarketStake] {
            assert_eq!(grant_for(Tier::System, cap), Grant::Granted);
            assert_eq!(grant_for(Tier::Official, cap), Grant::Declarable);
            assert_eq!(grant_for(Tier::Certified, cap), Grant::Declarable);
            assert_eq!(grant_for(Tier::ThirdParty, cap), Grant::Denied);
            assert_eq!(grant_for(Tier::Blacklist, cap), Grant::Denied);
        }
    }

    #[test]
    fn market_ask_open_to_all_except_blacklist() {
        for tier in [
            Tier::System,
            Tier::Official,
            Tier::Certified,
            Tier::ThirdParty,
        ] {
            assert_eq!(
                grant_for(tier, Capability::MarketResourceAsk),
                Grant::Granted,
                "ask 应对 {tier:?} 默认授予"
            );
        }
        assert_eq!(
            grant_for(Tier::Blacklist, Capability::MarketResourceAsk),
            Grant::Denied
        );
    }

    #[test]
    fn market_settle_system_official_only() {
        assert_eq!(
            grant_for(Tier::System, Capability::MarketResourceSettle),
            Grant::Granted
        );
        assert_eq!(
            grant_for(Tier::Official, Capability::MarketResourceSettle),
            Grant::Granted
        );
        for tier in [Tier::Certified, Tier::ThirdParty, Tier::Blacklist] {
            assert_eq!(
                grant_for(tier, Capability::MarketResourceSettle),
                Grant::Denied,
                "settle 必须对 {tier:?} 拒绝"
            );
        }
    }

    #[test]
    fn market_slash_system_only() {
        assert_eq!(
            grant_for(Tier::System, Capability::MarketSlash),
            Grant::Granted
        );
        for tier in [
            Tier::Official,
            Tier::Certified,
            Tier::ThirdParty,
            Tier::Blacklist,
        ] {
            assert_eq!(grant_for(tier, Capability::MarketSlash), Grant::Denied);
        }
    }

    #[test]
    fn market_capabilities_roundtrip_parse_str() {
        for cap in [
            Capability::MarketResourceOffer,
            Capability::MarketResourceAsk,
            Capability::MarketResourceSettle,
            Capability::MarketStake,
            Capability::MarketSlash,
        ] {
            assert_eq!(Capability::parse(cap.as_str()), Some(cap));
        }
    }
}
