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
/// v3.9.18: relay 掉线/失败后的 failover 冷却窗口（秒）。
/// 半死 relay 在冷却期内不会被反复重拨，避免连接抖动。
pub const DEFAULT_FAILOVER_COOLDOWN_SECS: u64 = 60;

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

    /// 选择优先级（数值越小优先级越高）：专用 > 自有 > 第三方 > 通用
    pub fn priority(&self) -> u8 {
        match self {
            RelayClass::Dedicated => 0,
            RelayClass::SelfHosted => 1,
            RelayClass::ThirdParty => 2,
            RelayClass::General => 3,
        }
    }

    /// 从线上字符串映射分类（未知值 → General；不失败）。
    /// 与 `FromStr` 等价但不返回 `Result`，避免 `Infallible` 的 unwrap。
    pub fn from_wire(s: &str) -> RelayClass {
        match s.to_lowercase().as_str() {
            "dedicated" => RelayClass::Dedicated,
            "self_hosted" | "selfhosted" | "own" => RelayClass::SelfHosted,
            "third_party" | "thirdparty" | "community" => RelayClass::ThirdParty,
            _ => RelayClass::General,
        }
    }
}

impl std::str::FromStr for RelayClass {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RelayClass::from_wire(s))
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
pub fn capacity_snapshot(
    elapsed_days: i64,
    manual_bonus: i64,
    current_size: i64,
) -> CapacitySnapshot {
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
/// 为 v3.9.18 的薄包装：不启用冷却（空冷却表 + now=0），保持旧调用兼容。
pub fn select_replacement(relays: &[StoredRelay], in_use: &HashSet<String>) -> Option<StoredRelay> {
    select_replacement_cooled(relays, in_use, &std::collections::HashMap::new(), 0)
}

/// v3.9.18: 带 failover 冷却窗口的替换选择。
///
/// 规则（在原有「健康 + 不在 in_use + 分类优先级」基础上）：
///   1. 冷却中的候选（`cooldown_until[id] > now_secs`）不参与本轮选择；
///   2. 若剔除冷却后仍有候选，则只在非冷却候选中择优；
///   3. 若全部候选都在冷却中，回退为在全部候选中择优（宁选劣化通道也不无中继）。
pub fn select_replacement_cooled(
    relays: &[StoredRelay],
    in_use: &HashSet<String>,
    cooldown_until: &std::collections::HashMap<String, u64>,
    now_secs: u64,
) -> Option<StoredRelay> {
    fn pick<'a>(it: impl Iterator<Item = &'a StoredRelay>) -> Option<&'a StoredRelay> {
        let mut best: Option<&StoredRelay> = None;
        for r in it {
            let cls: RelayClass = RelayClass::from_wire(&r.class);
            let better = match &best {
                None => true,
                Some(b) => {
                    let bcls: RelayClass = RelayClass::from_wire(&b.class);
                    cls.priority() < bcls.priority()
                        || (cls.priority() == bcls.priority() && r.fail_count < b.fail_count)
                }
            };
            if better {
                best = Some(r);
            }
        }
        best
    }

    let eligible = relays
        .iter()
        .filter(|r| r.healthy && !in_use.contains(&r.relay_id));
    let not_cooled: Vec<&StoredRelay> = eligible
        .clone()
        .filter(|r| cooldown_until.get(&r.relay_id).copied().unwrap_or(0) <= now_secs)
        .collect();
    if !not_cooled.is_empty() {
        return pick(not_cooled.into_iter()).cloned();
    }
    // 全部候选都在冷却中 → 回退全部候选
    pick(eligible).cloned()
}

/// 从健康 relay 中选出至多 `n` 个用于多通道同时在线（方案 4），
/// 按分类优先级排序、尽量跨分类以保证通道独立性。
pub fn select_parallel(relays: &[StoredRelay], n: usize) -> Vec<StoredRelay> {
    let mut healthy: Vec<StoredRelay> = relays.iter().filter(|r| r.healthy).cloned().collect();
    healthy.sort_by(|a, b| {
        let ca = RelayClass::from_wire(&a.class).priority();
        let cb = RelayClass::from_wire(&b.class).priority();
        ca.cmp(&cb).then(a.fail_count.cmp(&b.fail_count))
    });
    healthy.truncate(n);
    healthy
}

/// v3.9.15: 判断 relay 池是否缺失「自建 Dedicated」种子（需要补写）。
///
/// 自建中继（Dedicated，优先级最高）是本网络的信任锚基础设施；即使池中已有
/// 其它社区/第三方 relay，只要没有任何 Dedicated 节点，也应补写，保证
/// ensure_channels 始终有最高优先级通道可选。
pub fn missing_dedicated_seed(relays: &[StoredRelay]) -> bool {
    !relays
        .iter()
        .any(|r| RelayClass::from_wire(&r.class) == RelayClass::Dedicated)
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

    #[test]
    fn test_missing_dedicated_seed_detects_absence() {
        // 只有第三方/通用 → 判定为缺失
        let relays = vec![
            mk_relay("third", "third_party", true, 0),
            mk_relay("gen", "general", true, 0),
        ];
        assert!(missing_dedicated_seed(&relays));
    }

    #[test]
    fn test_missing_dedicated_seed_false_when_present() {
        // 含自建 Dedicated → 不缺失
        let relays = vec![
            mk_relay("ded", "dedicated", true, 0),
            mk_relay("third", "third_party", true, 0),
        ];
        assert!(!missing_dedicated_seed(&relays));
    }

    #[test]
    fn test_missing_dedicated_seed_empty_pool() {
        assert!(missing_dedicated_seed(&[]));
    }

    // ─────── v3.9.18 failover 冷却窗口 ───────

    #[test]
    fn test_cooled_select_skips_cooldown_when_alternatives_exist() {
        let relays = vec![
            mk_relay("ded1", "dedicated", true, 0),
            mk_relay("com1", "community", true, 0),
        ];
        let in_use = HashSet::new();
        let mut cool = std::collections::HashMap::new();
        cool.insert("ded1".to_string(), 1000); // ded1 冷却中（now=500）
        let picked = select_replacement_cooled(&relays, &in_use, &cool, 500).unwrap();
        assert_eq!(
            picked.relay_id, "com1",
            "冷却中的专用 relay 应被跳过，改选社区 relay"
        );
    }

    #[test]
    fn test_cooled_select_falls_back_when_all_cooled() {
        let relays = vec![
            mk_relay("ded1", "dedicated", true, 0),
            mk_relay("com1", "community", true, 0),
        ];
        let in_use = HashSet::new();
        let mut cool = std::collections::HashMap::new();
        cool.insert("ded1".to_string(), 1000);
        cool.insert("com1".to_string(), 1000);
        // 全部冷却 → 回退选择（宁选劣化通道也不无中继），仍按优先级取 ded1
        let picked = select_replacement_cooled(&relays, &in_use, &cool, 500).unwrap();
        assert_eq!(picked.relay_id, "ded1");
    }

    #[test]
    fn test_cooled_select_expired_cooldown_is_selectable() {
        let relays = vec![
            mk_relay("ded1", "dedicated", true, 0),
            mk_relay("com1", "community", true, 0),
        ];
        let in_use = HashSet::new();
        let mut cool = std::collections::HashMap::new();
        cool.insert("ded1".to_string(), 499); // 冷却已过期（now=500）
        let picked = select_replacement_cooled(&relays, &in_use, &cool, 500).unwrap();
        assert_eq!(picked.relay_id, "ded1", "冷却过期后专用 relay 恢复优先");
    }

    #[test]
    fn test_cooled_select_respects_priority_and_fail_count() {
        // 同优先级下 fail_count 低的优先；dedicated 始终优先于 community
        let relays = vec![
            mk_relay("com_hi", "community", true, 9),
            mk_relay("com_lo", "community", true, 1),
            mk_relay("ded", "dedicated", true, 5),
        ];
        let in_use = HashSet::new();
        let picked =
            select_replacement_cooled(&relays, &in_use, &std::collections::HashMap::new(), 0)
                .unwrap();
        assert_eq!(picked.relay_id, "ded", "分类优先级优先于 fail_count");
    }

    #[test]
    fn test_cooled_select_in_use_excluded() {
        let relays = vec![
            mk_relay("ded1", "dedicated", true, 0),
            mk_relay("com1", "community", true, 0),
        ];
        let mut in_use = HashSet::new();
        in_use.insert("com1".to_string());
        let picked =
            select_replacement_cooled(&relays, &in_use, &std::collections::HashMap::new(), 0)
                .unwrap();
        assert_eq!(picked.relay_id, "ded1");
    }

    #[test]
    fn test_cooled_select_unhealthy_excluded() {
        let relays = vec![
            mk_relay("dead1", "dedicated", false, 0),
            mk_relay("com1", "community", true, 0),
        ];
        let picked = select_replacement_cooled(
            &relays,
            &HashSet::new(),
            &std::collections::HashMap::new(),
            0,
        )
        .unwrap();
        assert_eq!(picked.relay_id, "com1", "不健康候选被排除");
    }
}
