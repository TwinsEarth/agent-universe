//! v2.5.5: Circuit Relay 节点池管理
//!
//! 两大职责：
//! 1. **容量策略**：初始上限 1 万，每月自动 +1 万，硬上限 = 运行年限 × 10 万
//!    （10 年 100 万 / 100 年 1000 万 / 1000 年 1 亿），支持手动扩容。
//! 2. **relay 分类与选择**：专用 / 自有 / 第三方 / 通用四类，
//!    失效时按 DHT 候选与分类优先级自动重新分配。
//!
//! 本模块只放纯数据模型、容量计算与选择逻辑（可单测）；
//! SQLite 访问见 `storage::persist`，巡检/发现/切换的运行时编排见 `node`。

use crate::storage::StoredRelay;
use std::collections::HashSet;

// ───────────────────────── 容量常量 ─────────────────────────

/// 初始列表上限
pub const BASE_CAP: i64 = 10_000;
/// 每月自动扩容额度
pub const MONTHLY_GROWTH: i64 = 10_000;
/// 每运行一年的硬上限基数
pub const YEARLY_HARD: i64 = 100_000;
/// 每月天数（整月口径）
pub const DAYS_PER_MONTH: f64 = 30.0;
/// 每年天数（年限口径）
pub const DAYS_PER_YEAR: f64 = 365.25;

/// 默认同时在线的 relay 通道数（方案 4：多通道）
pub const DEFAULT_PARALLEL_RELAYS: usize = 3;

// ───────────────────────── relay 分类 ─────────────────────────

/// Relay 分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayClass {
    /// 专用（自建/专属基础设施，优先级最高）
    Dedicated,
    /// 自有（自己拥有、可控的节点）
    SelfHosted,
    /// 第三方（社区/他人提供）
    ThirdParty,
    /// 通用（公共池，兜底）
    General,
}

impl RelayClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelayClass::Dedicated => "dedicated",
            RelayClass::SelfHosted => "self_hosted",
            RelayClass::ThirdParty => "third_party",
            RelayClass::General => "general",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "dedicated" => RelayClass::Dedicated,
            "self_hosted" | "selfhosted" | "own" => RelayClass::SelfHosted,
            "third_party" | "thirdparty" | "community" => RelayClass::ThirdParty,
            _ => RelayClass::General,
        }
    }

    /// 选择优先级（数值越小优先级越高）：专用 > 自有 > 第三方 > 通用
    pub fn priority(&self) -> u8 {
        match self {
            RelayClass::Dedicated => 0,
            RelayClass::SelfHosted => 1,
            RelayClass::ThirdParty => 2,
            RelayClass::General => 3,
        }
    }
}

// ───────────────────────── 容量快照 ─────────────────────────

/// 某一时刻的容量快照（全部字段可由入参复算）
#[derive(Debug, Clone, serde::Serialize)]
pub struct CapacitySnapshot {
    pub elapsed_days: i64,
    pub elapsed_months: i64,
    pub elapsed_years: i64,
    /// 月度增长口径：10_000 + months × 10_000
    pub monthly_cap: i64,
    /// 年限硬上限：years × 100_000
    pub hard_cap: i64,
    /// 自动有效上限 = min(monthly_cap, hard_cap)
    pub auto_cap: i64,
    /// 手动扩容额度
    pub manual_bonus: i64,
    /// 最终有效上限 = auto_cap + manual_bonus
    pub effective_cap: i64,
    pub current_size: i64,
    pub remaining: i64,
}

/// 纯函数：由已运行天数、手动扩容额度、当前池大小计算容量快照
pub fn capacity_snapshot(elapsed_days: i64, manual_bonus: i64, current_size: i64) -> CapacitySnapshot {
    let days = elapsed_days.max(0) as f64;
    let elapsed_months = (days / DAYS_PER_MONTH).floor() as i64;
    // 运行年限：不满 1 年按 1 年计（保证初始阶段硬上限为 10 万，给月度增长留空间）
    let raw_years = (days / DAYS_PER_YEAR).ceil() as i64;
    let elapsed_years = raw_years.max(1);

    let monthly_cap = BASE_CAP + elapsed_months.max(0) * MONTHLY_GROWTH;
    let hard_cap = elapsed_years * YEARLY_HARD;
    let auto_cap = monthly_cap.min(hard_cap);
    let effective_cap = auto_cap + manual_bonus.max(0);
    let remaining = effective_cap - current_size;

    CapacitySnapshot {
        elapsed_days: elapsed_days.max(0),
        elapsed_months: elapsed_months.max(0),
        elapsed_years,
        monthly_cap,
        hard_cap,
        auto_cap,
        manual_bonus: manual_bonus.max(0),
        effective_cap,
        current_size,
        remaining,
    }
}

// ───────────────────────── relay 选择 ─────────────────────────

/// 从候选中按「健康 + 分类优先级」选择一个替换 relay，排除正在使用的节点。
///
/// 优先级：专用 > 自有 > 第三方 > 通用；同类中优先 fail_count 低者。
pub fn select_replacement(
    relays: &[StoredRelay],
    in_use: &HashSet<String>,
) -> Option<StoredRelay> {
    let mut best: Option<&StoredRelay> = None;
    for r in relays {
        if !r.healthy || in_use.contains(&r.relay_id) {
            continue;
        }
        let cls = RelayClass::from_str(&r.class);
        let better = match &best {
            None => true,
            Some(b) => {
                let bcls = RelayClass::from_str(&b.class);
                cls.priority() < bcls.priority()
                    || (cls.priority() == bcls.priority() && r.fail_count < b.fail_count)
            }
        };
        if better {
            best = Some(r);
        }
    }
    best.cloned()
}

/// 从健康 relay 中选出至多 `n` 个用于多通道同时在线（方案 4），
/// 按分类优先级排序、尽量跨分类以保证通道独立性。
pub fn select_parallel(relays: &[StoredRelay], n: usize) -> Vec<StoredRelay> {
    let mut healthy: Vec<StoredRelay> = relays.iter().filter(|r| r.healthy).cloned().collect();
    healthy.sort_by(|a, b| {
        let ca = RelayClass::from_str(&a.class).priority();
        let cb = RelayClass::from_str(&b.class).priority();
        ca.cmp(&cb).then(a.fail_count.cmp(&b.fail_count))
    });
    healthy.truncate(n);
    healthy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_capacity() {
        // 初始（0 天）：自动上限 1 万
        let s = capacity_snapshot(0, 0, 0);
        assert_eq!(s.elapsed_years, 1);
        assert_eq!(s.auto_cap, 10_000);
        assert_eq!(s.effective_cap, 10_000);
        assert_eq!(s.remaining, 10_000);
    }

    #[test]
    fn test_monthly_growth() {
        // 6 个月 ≈ 180 天：monthly = 70_000，hard(1 年)=100_000 → auto 70_000
        let s = capacity_snapshot(180, 0, 0);
        assert_eq!(s.elapsed_months, 6);
        assert_eq!(s.monthly_cap, 70_000);
        assert_eq!(s.auto_cap, 70_000);
    }

    #[test]
    fn test_first_year_hard_cap() {
        // 12 个月 ≈ 365 天：monthly 130_000 > hard 100_000 → auto 100_000
        let s = capacity_snapshot(365, 0, 0);
        assert_eq!(s.elapsed_years, 1);
        assert_eq!(s.auto_cap, 100_000);
    }

    #[test]
    fn test_ten_years_capacity() {
        // 10 年 ≈ 3652 天：hard = 1_000_000
        let s = capacity_snapshot(3652, 0, 0);
        assert_eq!(s.elapsed_years, 10);
        assert_eq!(s.hard_cap, 1_000_000);
        assert_eq!(s.auto_cap, 1_000_000);
    }

    #[test]
    fn test_hundred_and_thousand_years() {
        // 100 年：1000 万；1000 年：1 亿
        assert_eq!(capacity_snapshot(36525, 0, 0).hard_cap, 10_000_000);
        assert_eq!(capacity_snapshot(365250, 0, 0).hard_cap, 100_000_000);
    }

    #[test]
    fn test_manual_expansion() {
        // 手动 +50_000：effective = auto + 50_000
        let s = capacity_snapshot(0, 50_000, 0);
        assert_eq!(s.effective_cap, 60_000);
        assert_eq!(s.remaining, 60_000);
    }

    #[test]
    fn test_class_priority_order() {
        assert!(RelayClass::Dedicated.priority() < RelayClass::SelfHosted.priority());
        assert!(RelayClass::SelfHosted.priority() < RelayClass::ThirdParty.priority());
        assert!(RelayClass::ThirdParty.priority() < RelayClass::General.priority());
    }

    fn mk_relay(id: &str, class: &str, healthy: bool, fail: i64) -> StoredRelay {
        StoredRelay {
            relay_id: id.to_string(),
            multiaddr: format!("/ip4/1.2.3.4/tcp/4001/p2p/{}", id),
            class: class.to_string(),
            status: if healthy { "healthy" } else { "dead" }.to_string(),
            healthy,
            fail_count: fail,
            limit_sec: 120,
            data_bytes: 131072,
            last_check: String::new(),
            created_at: String::new(),
        }
    }

    #[test]
    fn test_select_replacement_prefers_dedicated() {
        let relays = vec![
            mk_relay("third", "third_party", true, 0),
            mk_relay("gen", "general", true, 0),
            mk_relay("ded", "dedicated", true, 0),
        ];
        let pick = select_replacement(&relays, &HashSet::new()).unwrap();
        assert_eq!(pick.relay_id, "ded");
    }

    #[test]
    fn test_select_replacement_excludes_in_use() {
        let relays = vec![
            mk_relay("ded", "dedicated", true, 0),
            mk_relay("self", "self_hosted", true, 0),
        ];
        let mut in_use = HashSet::new();
        in_use.insert("ded".to_string());
        let pick = select_replacement(&relays, &in_use).unwrap();
        assert_eq!(pick.relay_id, "self");
    }

    #[test]
    fn test_select_parallel_caps_count() {
        let relays: Vec<StoredRelay> = (0..6)
            .map(|i| mk_relay(&format!("r{}", i), "general", true, 0))
            .collect();
        assert_eq!(select_parallel(&relays, 3).len(), 3);
    }
}
