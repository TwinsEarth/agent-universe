//! 插件五级分类（信任级别）—— v3.0.0
//!
//! # 核心规则：分类仅由清单 `name` 前缀决定
//!
//! 不读取任何额外的「分类字段」——因为额外字段本身就是可伪造的输入。
//! 一个插件自称「官方」毫无意义，只有它的 `name` 命中官方命名前缀才是官方。
//!
//! | 级别 | 前缀 | 隔离 | 权限 |
//! |---|---|---|---|
//! | T0 系统（[`Tier::System`]） | `com.twinsearth.sys.*` | 进程内 | 全能力 |
//! | T1 官方（[`Tier::Official`]） | `com.twinsearth.official.*` | 独立进程 | 受限系统能力 |
//! | T2 认证（[`Tier::Certified`]） | `com.twinsearth.certified.*` | 独立进程 | 声明式 |
//! | T3 第三方（[`Tier::ThirdParty`]） | 其它任意域名 | 独立进程 · 最严配额 | 最小集 |
//! | 黑名单（[`Tier::Blacklist`]） | 精确匹配 | 禁止加载 | 无 |

use serde::{Deserialize, Serialize};

/// 系统插件命名前缀（T0）。
pub const SYS_PREFIX: &str = "com.twinsearth.sys.";
/// 官方插件命名前缀（T1）。
pub const OFFICIAL_PREFIX: &str = "com.twinsearth.official.";
/// 认证插件命名前缀（T2）。
pub const CERTIFIED_PREFIX: &str = "com.twinsearth.certified.";

/// 插件信任级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// T0 系统插件：随内核发布、进程内、全能力、不可热插拔（仅热配置）。
    System,
    /// T1 官方插件：官方密钥签名、独立进程、可热插拔。
    Official,
    /// T2 认证插件：开发者签名 + 官方副签、独立进程、可热插拔。
    Certified,
    /// T3 第三方插件：仅开发者签名、最严配额、默认零网络、需用户显式信任。
    ThirdParty,
    /// 黑名单：命中即禁止加载。
    Blacklist,
}

impl Tier {
    /// 仅依据插件 `name` 前缀判定级别。
    ///
    /// 黑名单不在这里判定（黑名单是按精确指纹/名称匹配，由黑名单模块在加载前检查）；
    /// 因此本函数对「保留前缀之外」的名字一律返回 [`Tier::ThirdParty`]。
    pub fn from_name(name: &str) -> Tier {
        if name.starts_with(SYS_PREFIX) {
            Tier::System
        } else if name.starts_with(OFFICIAL_PREFIX) {
            Tier::Official
        } else if name.starts_with(CERTIFIED_PREFIX) {
            Tier::Certified
        } else {
            Tier::ThirdParty
        }
    }

    /// 该级别是否可热插拔（v3.0.0）。
    ///
    /// T0 系统插件不可热插拔（仅热配置）；T1/T2/T3 均可。
    /// 黑名单不参与加载。
    pub fn hot_pluggable(self) -> bool {
        matches!(self, Tier::Official | Tier::Certified | Tier::ThirdParty)
    }

    /// 是否要求官方副签（counter-signature）。
    ///
    /// T1 官方、T2 认证需要官方副签；T3 无副签（因此永远不能获得 T1/T2 能力）。
    pub fn requires_counter_signature(self) -> bool {
        matches!(self, Tier::Official | Tier::Certified)
    }

    /// 稳定的级别标识（用于审计/序列化）。
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::System => "system",
            Tier::Official => "official",
            Tier::Certified => "certified",
            Tier::ThirdParty => "third_party",
            Tier::Blacklist => "blacklist",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_prefix_only() {
        assert_eq!(Tier::from_name("com.twinsearth.sys.identity"), Tier::System);
        assert_eq!(
            Tier::from_name("com.twinsearth.official.market"),
            Tier::Official
        );
        assert_eq!(
            Tier::from_name("com.twinsearth.certified.extra"),
            Tier::Certified
        );
        // 任何其它域名（含自称 official 但不在官方命名空间）都算第三方。
        assert_eq!(
            Tier::from_name("com.example.official.market"),
            Tier::ThirdParty
        );
        assert_eq!(Tier::from_name("org.foo.plugin"), Tier::ThirdParty);
    }

    #[test]
    fn prefix_must_be_at_start() {
        // 前缀出现在中间不算数（避免伪造如 evil-com.twinsearth.sys.x）。
        assert_eq!(
            Tier::from_name("evil.com.twinsearth.sys.x"),
            Tier::ThirdParty
        );
    }

    #[test]
    fn hot_pluggable_matrix() {
        assert!(!Tier::System.hot_pluggable());
        assert!(Tier::Official.hot_pluggable());
        assert!(Tier::Certified.hot_pluggable());
        assert!(Tier::ThirdParty.hot_pluggable());
    }

    #[test]
    fn counter_signature_matrix() {
        assert!(!Tier::System.requires_counter_signature());
        assert!(Tier::Official.requires_counter_signature());
        assert!(Tier::Certified.requires_counter_signature());
        assert!(!Tier::ThirdParty.requires_counter_signature());
    }
}
