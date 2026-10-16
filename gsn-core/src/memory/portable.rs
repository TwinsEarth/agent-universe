//! PortableMem：可移植记忆（v5.1.8）——按 roadmap「可移植记忆（token_budget+相关性分级）」。
//!
//! 设计要点：
//! - **token 预算（token_budget）**：`portable_pack` 在给定 `max_tokens` 预算内
//!   挑选检索结果装包；`estimate_tokens` 为**确定性估算**（每 4 字符 ≈ 1 token，
//!   明确"估算"口径，不宣称精确 tokenizer 等价）；
//! - **相关性分级**：`relevance_score` 确定性打分——要求标签每命中 +3、结论包含
//!   关键词 +5、argv 命中 +2、stdout_tail 命中 +1；分数 >=5 为 High、>=2 为
//!   Medium、其余为 Low；
//! - **贪心装包顺序**：先装 High 桶（桶内分数降序、同分新条目优先）→ Medium →
//!   Low；低于 `min_tier` 的条目整体不参与（语义为分级过滤，不计数）；预算装不
//!   下的条目显式列入 `omitted`（id + tier + 估算 token），**可移植性**：调用方
//!   明确知道哪些条目没装进去及原因；
//! - **fail-closed**：`max_tokens` 为 0 或超出上限、非法 tier 一律类型化拒绝；
//! - 纯函数、无 IO、无 panic：生产代码 0 `unwrap`/`expect`/`panic!`。

use crate::memory::persistent::{MemoryEntry, MemoryQuery};
use std::fmt;

/// token 预算上限：1_000_000（防手滑传天文数字导致长时间空转）。
pub const MAX_BUDGET: usize = 1_000_000;

/// 相关性分级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelevanceTier {
    Low,
    Medium,
    High,
}

impl RelevanceTier {
    pub fn label(&self) -> &'static str {
        match self {
            RelevanceTier::High => "high",
            RelevanceTier::Medium => "medium",
            RelevanceTier::Low => "low",
        }
    }

    pub fn cn_label(&self) -> &'static str {
        match self {
            RelevanceTier::High => "高",
            RelevanceTier::Medium => "中",
            RelevanceTier::Low => "低",
        }
    }

    /// 解析 CLI 输入（high/medium/low）；非法返回 None。
    pub fn parse(s: &str) -> Option<RelevanceTier> {
        match s.to_ascii_lowercase().as_str() {
            "high" => Some(RelevanceTier::High),
            "medium" | "mid" => Some(RelevanceTier::Medium),
            "low" => Some(RelevanceTier::Low),
            _ => None,
        }
    }

    /// 排序权重：High=3、Medium=2、Low=1。
    fn rank(&self) -> u8 {
        match self {
            RelevanceTier::High => 3,
            RelevanceTier::Medium => 2,
            RelevanceTier::Low => 1,
        }
    }
}

/// 已分级条目（装包结果里的单条）。
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredEntry {
    pub entry: MemoryEntry,
    /// 确定性相关性分数（整数）。
    pub score: i32,
    pub tier: RelevanceTier,
}

/// 预算外条目（明确记录：为什么没装进去）。
#[derive(Debug, Clone, PartialEq)]
pub struct OmittedEntry {
    /// 被排挤条目的 task_id（记忆条目标识）。
    pub entry_id: String,
    pub tier: RelevanceTier,
    /// 该条目的估算 token 数（>= 剩余预算才被排挤）。
    pub tokens: usize,
}

/// 分级计数（装包内）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TierCounts {
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

/// 可移植记忆查询：token 预算 + 最低分级门槛。
#[derive(Debug, Clone)]
pub struct PortableQuery {
    /// 检索过滤条件（复用 MemoryQuery 的过滤语义）。
    pub query: MemoryQuery,
    /// token 预算上限（必填，正整数，<= MAX_BUDGET）。
    pub max_tokens: usize,
    /// 最低分级：低于该级别的条目整体不参与装包。
    pub min_tier: RelevanceTier,
}

impl PortableQuery {
    pub fn new(query: MemoryQuery, max_tokens: usize) -> Self {
        Self {
            query,
            max_tokens,
            min_tier: RelevanceTier::Low,
        }
    }
}

/// 可移植记忆装包结果。
#[derive(Debug, Clone, PartialEq)]
pub struct PortablePack {
    /// 已装包条目（按 High→Medium→Low，桶内分数降序）。
    pub entries: Vec<ScoredEntry>,
    /// 已装包条目的估算 token 合计（实测 sum，不预设）。
    pub total_tokens: usize,
    /// 本次预算上限。
    pub budget: usize,
    /// 各分级装包数。
    pub tier_counts: TierCounts,
    /// 预算排挤清单（id + 分级 + 估算 token）。
    pub omitted: Vec<OmittedEntry>,
}

/// fail-closed 类型化错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableError {
    /// 预算为 0 或超出 MAX_BUDGET。
    InvalidBudget(usize),
    /// 非法最低分级字符串。
    InvalidTier(String),
}

impl fmt::Display for PortableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortableError::InvalidBudget(b) => write!(
                f,
                "portable 预算非法: {b}（须为 1..={MAX_BUDGET} 的正整数）"
            ),
            PortableError::InvalidTier(s) => {
                write!(f, "portable 分级非法: {s}（支持 high/medium/low）")
            }
        }
    }
}

impl std::error::Error for PortableError {}

/// 估算文本的 token 数（确定性口径：每 4 个字符 ≈ 1 token，空串为 0）。
/// 明确为**估算**，不宣称与任何具体 tokenizer 等价。
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let chars = text.chars().count();
    chars.div_ceil(4)
}

/// 估算一条记忆条目的 token 合计（conclusion + stdout/stderr tail + argv + tags）。
pub fn estimate_entry_tokens(e: &MemoryEntry) -> usize {
    let mut total = 0usize;
    if let Some(c) = &e.conclusion {
        total += estimate_tokens(c);
    }
    if let Some(o) = &e.stdout_tail {
        total += estimate_tokens(o);
    }
    if let Some(o) = &e.stderr_tail {
        total += estimate_tokens(o);
    }
    for a in &e.argv {
        total += estimate_tokens(a);
    }
    for t in &e.tags {
        total += estimate_tokens(t);
    }
    total
}

/// 确定性相关性打分：要求标签每命中 +3、结论包含关键词 +5、
/// argv 命中 +2、stdout_tail 命中 +1（关键词取 `conclusion_contains`）。
pub fn relevance_score(e: &MemoryEntry, q: &MemoryQuery) -> i32 {
    let mut s = 0i32;
    for t in &q.tags {
        if e.tags.contains(t) {
            s += 3;
        }
    }
    if let Some(kw) = &q.conclusion_contains {
        if let Some(cc) = &e.conclusion {
            if cc.contains(kw) {
                s += 5;
            }
        }
        for a in &e.argv {
            if a.contains(kw) {
                s += 2;
            }
        }
        if let Some(tail) = &e.stdout_tail {
            if tail.contains(kw) {
                s += 1;
            }
        }
    }
    s
}

/// 分数 → 分级：>=5 High、>=2 Medium、其余 Low。
pub fn tier_of(score: i32) -> RelevanceTier {
    if score >= 5 {
        RelevanceTier::High
    } else if score >= 2 {
        RelevanceTier::Medium
    } else {
        RelevanceTier::Low
    }
}

/// 在 token 预算内按相关性分级装包。
///
/// 排序：分数降序，同分按 `at` 字典序降序（新条目优先）。
/// 装包：High → Medium → Low 依次贪心（桶内按上述顺序）；
/// 低于 `min_tier` 的桶整体不参与装包也不计 omitted（分级过滤语义）；
/// 单条估算 token 超过剩余预算的条目列入 `omitted`（预算排挤）。
pub fn portable_pack(
    entries: &[MemoryEntry],
    q: &PortableQuery,
) -> Result<PortablePack, PortableError> {
    if q.max_tokens == 0 || q.max_tokens > MAX_BUDGET {
        return Err(PortableError::InvalidBudget(q.max_tokens));
    }
    // 打分 + 排序：分数降序，同分 at 降序（新优先）。
    let mut scored: Vec<(MemoryEntry, i32)> = entries
        .iter()
        .map(|e| (e.clone(), relevance_score(e, &q.query)))
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.at.cmp(&a.0.at)));

    let mut packed: Vec<ScoredEntry> = Vec::new();
    let mut omitted: Vec<OmittedEntry> = Vec::new();
    let mut remaining = q.max_tokens;

    for tier in [
        RelevanceTier::High,
        RelevanceTier::Medium,
        RelevanceTier::Low,
    ] {
        if tier.rank() < q.min_tier.rank() {
            // 低于最低分级门槛：整体不参与（分级过滤）。
            continue;
        }
        for (e, s) in scored.iter() {
            if tier_of(*s) != tier {
                continue;
            }
            let t = estimate_entry_tokens(e);
            if t <= remaining {
                packed.push(ScoredEntry {
                    entry: e.clone(),
                    score: *s,
                    tier,
                });
                remaining -= t;
            } else {
                omitted.push(OmittedEntry {
                    entry_id: e.task_id.clone(),
                    tier,
                    tokens: t,
                });
            }
        }
    }

    let mut tier_counts = TierCounts::default();
    for se in &packed {
        match se.tier {
            RelevanceTier::High => tier_counts.high += 1,
            RelevanceTier::Medium => tier_counts.medium += 1,
            RelevanceTier::Low => tier_counts.low += 1,
        }
    }
    let total_tokens: usize = packed.iter().map(|s| estimate_entry_tokens(&s.entry)).sum();

    Ok(PortablePack {
        entries: packed,
        total_tokens,
        budget: q.max_tokens,
        tier_counts,
        omitted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, at: &str) -> MemoryEntry {
        MemoryEntry::new(id)
            .at(at)
            .gate("ci")
            .argv(vec!["node".into(), "test.mjs".into()])
            .exit_code(0)
            .ms(120)
            .stdout_tail("all ok")
            .conclusion("passes")
            .add_tag("ci")
    }

    // ---- estimate_tokens ----
    #[test]
    fn tokens_empty_is_zero() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn tokens_four_chars_per_token() {
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("a".repeat(9).as_str()), 3); // ceil(9/4)
        assert_eq!(estimate_tokens("a".repeat(12).as_str()), 3);
        assert_eq!(estimate_tokens("a".repeat(13).as_str()), 4);
    }

    #[test]
    fn tokens_counts_unicode_chars() {
        // 中文字符按字符数计（确定性估算口径）。
        assert_eq!(estimate_tokens("记忆"), 1); // ceil(2/4)
        assert_eq!(estimate_tokens("记忆系统测试用例"), 2); // ceil(8/4)
    }

    #[test]
    fn tokens_whitespace_only_is_one() {
        assert_eq!(estimate_tokens("   "), 1); // 3 chars → ceil(3/4)
    }

    // ---- estimate_entry_tokens ----
    #[test]
    fn entry_tokens_minimal_is_zero() {
        let e = MemoryEntry::new("t0").at("2026-10-16T00:00:00Z");
        assert_eq!(estimate_entry_tokens(&e), 0);
    }

    #[test]
    fn entry_tokens_sums_all_fields() {
        let e = entry("t1", "2026-10-16T00:00:00Z");
        let expected = estimate_tokens("passes")
            + estimate_tokens("all ok")
            + estimate_tokens("node")
            + estimate_tokens("test.mjs")
            + estimate_tokens("ci");
        assert_eq!(estimate_entry_tokens(&e), expected);
    }

    #[test]
    fn entry_tokens_ignores_exit_code_and_ms() {
        let mut e = entry("t2", "2026-10-16T00:00:00Z");
        e.exit_code = Some(0);
        e.ms = Some(120);
        let base = estimate_entry_tokens(&e);
        e.exit_code = Some(1);
        e.ms = Some(999999);
        assert_eq!(estimate_entry_tokens(&e), base);
    }

    // ---- relevance_score ----
    #[test]
    fn relevance_zero_when_no_keyword() {
        let e = entry("t3", "2026-10-16T00:00:00Z");
        let q = MemoryQuery::new();
        assert_eq!(relevance_score(&e, &q), 0);
        assert_eq!(tier_of(0), RelevanceTier::Low);
    }

    #[test]
    fn relevance_tag_hit_adds_three() {
        let e = entry("t4", "2026-10-16T00:00:00Z"); // tag: ci
        let q = MemoryQuery::new().tag("ci");
        assert_eq!(relevance_score(&e, &q), 3);
        assert_eq!(tier_of(3), RelevanceTier::Medium);
    }

    #[test]
    fn relevance_conclusion_hit_is_high() {
        let e = entry("t5", "2026-10-16T00:00:00Z"); // conclusion: passes
        let q = MemoryQuery::new().conclusion_contains("passes");
        assert_eq!(relevance_score(&e, &q), 5);
        assert_eq!(tier_of(5), RelevanceTier::High);
    }

    #[test]
    fn relevance_argv_hit_adds_two() {
        let e = entry("t6", "2026-10-16T00:00:00Z"); // argv: node test.mjs
        let q = MemoryQuery::new().conclusion_contains("test.mjs");
        assert_eq!(relevance_score(&e, &q), 2);
        assert_eq!(tier_of(2), RelevanceTier::Medium);
    }

    #[test]
    fn relevance_stdout_hit_adds_one() {
        let e = entry("t7", "2026-10-16T00:00:00Z"); // stdout_tail: all ok
        let q = MemoryQuery::new().conclusion_contains("all ok");
        assert_eq!(relevance_score(&e, &q), 1);
        assert_eq!(tier_of(1), RelevanceTier::Low);
    }

    #[test]
    fn relevance_multi_hits_accumulate() {
        let e = entry("t8", "2026-10-16T00:00:00Z"); // tag ci + conclusion passes
        let q = MemoryQuery::new().tag("ci").conclusion_contains("passes");
        assert_eq!(relevance_score(&e, &q), 3 + 5);
        assert_eq!(tier_of(8), RelevanceTier::High);
    }

    #[test]
    fn tier_threshold_boundaries() {
        assert_eq!(tier_of(5), RelevanceTier::High);
        assert_eq!(tier_of(4), RelevanceTier::Medium);
        assert_eq!(tier_of(2), RelevanceTier::Medium);
        assert_eq!(tier_of(1), RelevanceTier::Low);
        assert_eq!(tier_of(0), RelevanceTier::Low);
    }

    // ---- portable_pack ----
    #[test]
    fn pack_fail_closed_zero_budget() {
        let e = entry("t9", "2026-10-16T00:00:00Z");
        let q = PortableQuery::new(MemoryQuery::new(), 0);
        assert_eq!(
            portable_pack(&[e], &q),
            Err(PortableError::InvalidBudget(0))
        );
    }

    #[test]
    fn pack_fail_closed_over_budget_cap() {
        let e = entry("t10", "2026-10-16T00:00:00Z");
        let q = PortableQuery::new(MemoryQuery::new(), MAX_BUDGET + 1);
        assert_eq!(
            portable_pack(&[e], &q),
            Err(PortableError::InvalidBudget(MAX_BUDGET + 1))
        );
    }

    #[test]
    fn pack_empty_input_gives_empty_pack() {
        let q = PortableQuery::new(MemoryQuery::new(), 100);
        let p = portable_pack(&[], &q).unwrap();
        assert!(p.entries.is_empty());
        assert_eq!(p.total_tokens, 0);
        assert!(p.omitted.is_empty());
        assert_eq!(p.tier_counts, TierCounts::default());
    }

    #[test]
    fn pack_roomy_budget_packs_all() {
        let es = vec![
            entry("t11", "2026-10-16T00:00:00Z"),
            entry("t12", "2026-10-16T00:01:00Z"),
        ];
        let q = PortableQuery::new(MemoryQuery::new(), 1000);
        let p = portable_pack(&es, &q).unwrap();
        assert_eq!(p.entries.len(), 2);
        assert_eq!(p.budget, 1000);
        assert!(p.omitted.is_empty());
        let sum: usize = es.iter().map(estimate_entry_tokens).sum();
        assert_eq!(p.total_tokens, sum);
        assert_eq!(
            p.tier_counts.high + p.tier_counts.medium + p.tier_counts.low,
            2
        );
    }

    #[test]
    fn pack_orders_by_relevance_desc() {
        // 两个条目：一个结论命中（5 分 High），一个无命中（0 分 Low）。
        let hi = entry("hi", "2026-10-16T00:00:00Z")
            .conclusion("passes")
            .add_tag("extra");
        let mut lo = entry("lo", "2026-10-16T00:00:00Z");
        lo.conclusion = None;
        lo.tags = Vec::new();
        let q = PortableQuery::new(MemoryQuery::new().conclusion_contains("passes"), 1000);
        let p = portable_pack(&[lo, hi], &q).unwrap();
        assert_eq!(p.entries.len(), 2);
        assert_eq!(p.entries[0].entry.task_id, "hi");
        assert_eq!(p.entries[0].tier, RelevanceTier::High);
        assert_eq!(p.entries[1].tier, RelevanceTier::Low);
    }

    #[test]
    fn pack_tie_breaks_by_newer_at() {
        let old = entry("old", "2026-10-15T00:00:00Z");
        let new = entry("new", "2026-10-16T00:00:00Z");
        // 同分（均无关键词，0 分）→ at 降序：new 在前。
        let q = PortableQuery::new(MemoryQuery::new(), 1000);
        let p = portable_pack(&[old, new], &q).unwrap();
        assert_eq!(p.entries[0].entry.task_id, "new");
        assert_eq!(p.entries[1].entry.task_id, "old");
    }

    #[test]
    fn pack_oversized_high_omitted_but_low_fits_remaining() {
        // 预算 < High 单条 token：High 被预算排挤进 omitted（其大小不是分级依据），
        // 剩余预算继续装得下的更小条目——贪心语义：装不下即排挤，不阻塞后续。
        let hi = entry("hi", "2026-10-16T00:00:00Z").conclusion("passes");
        let mut lo = entry("lo", "2026-10-16T00:00:00Z");
        lo.conclusion = None;
        lo.tags = Vec::new();
        let lo_tokens = estimate_entry_tokens(&lo);
        let q = PortableQuery::new(
            MemoryQuery::new().conclusion_contains("passes"),
            lo_tokens + 1, // 只装得下 Low + 1 token；High 超预算
        );
        let p = portable_pack(&[lo, hi], &q).unwrap();
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0].entry.task_id, "lo");
        assert_eq!(p.omitted.len(), 1);
        assert_eq!(p.omitted[0].entry_id, "hi");
        assert_eq!(p.omitted[0].tier, RelevanceTier::High);
    }

    #[test]
    fn pack_high_precedes_low_even_if_smaller() {
        // High 条目（大）必须优先于 Low 条目（小）装包。
        let big_hi = entry("big-hi", "2026-10-16T00:00:00Z")
            .conclusion("passes ".repeat(20)) // 大体积 High
            .add_tag("ci");
        let mut small_lo = entry("small-lo", "2026-10-16T00:00:00Z");
        small_lo.conclusion = None;
        small_lo.tags = Vec::new();
        let budget = estimate_entry_tokens(&big_hi) + estimate_entry_tokens(&small_lo) - 1;
        let q = PortableQuery::new(MemoryQuery::new().conclusion_contains("passes"), budget);
        let p = portable_pack(&[small_lo, big_hi], &q).unwrap();
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0].entry.task_id, "big-hi");
        assert_eq!(p.entries[0].tier, RelevanceTier::High);
        // small-lo 被预算排挤进 omitted。
        assert_eq!(p.omitted.len(), 1);
        assert_eq!(p.omitted[0].entry_id, "small-lo");
        assert_eq!(p.omitted[0].tier, RelevanceTier::Low);
    }

    #[test]
    fn pack_omitted_records_id_tier_tokens() {
        let e = entry("big", "2026-10-16T00:00:00Z");
        let t = estimate_entry_tokens(&e);
        let q = PortableQuery::new(MemoryQuery::new(), t - 1); // 预算 < 单条 token
        let p = portable_pack(&[e], &q).unwrap();
        assert!(p.entries.is_empty());
        assert_eq!(p.omitted.len(), 1);
        assert_eq!(p.omitted[0].entry_id, "big");
        assert_eq!(p.omitted[0].tier, RelevanceTier::Low);
        assert_eq!(p.omitted[0].tokens, t);
    }

    #[test]
    fn pack_total_tokens_is_measured_sum() {
        let a = entry("a", "2026-10-16T00:00:00Z");
        let b = entry("b", "2026-10-16T00:01:00Z");
        let sum = estimate_entry_tokens(&a) + estimate_entry_tokens(&b);
        let q = PortableQuery::new(MemoryQuery::new(), 1000);
        let p = portable_pack(&[a, b], &q).unwrap();
        assert_eq!(p.total_tokens, sum);
    }

    #[test]
    fn pack_min_tier_filters_lower_buckets() {
        let hi = entry("hi", "2026-10-16T00:00:00Z").conclusion("passes");
        let mut lo = entry("lo", "2026-10-16T00:00:00Z");
        lo.conclusion = None;
        lo.tags = Vec::new();
        let mut pq = PortableQuery::new(MemoryQuery::new().conclusion_contains("passes"), 1000);
        pq.min_tier = RelevanceTier::High;
        let p = portable_pack(&[lo, hi], &pq).unwrap();
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0].entry.task_id, "hi");
        // 低于 High 的 Low 条目整体不参与：既不装包也不计 omitted。
        assert!(p.omitted.is_empty());
    }

    #[test]
    fn pack_min_tier_medium_allows_high_and_medium() {
        let hi = plain("hi", "2026-10-16T00:00:00Z").conclusion("passes"); // 5 分 High
        let med = plain("med", "2026-10-16T00:00:00Z").add_tag("ci"); // 3 分 Medium
        let lo = plain("lo", "2026-10-16T00:00:00Z"); // 0 分 Low
        let mut pq = PortableQuery::new(
            MemoryQuery::new().conclusion_contains("passes").tag("ci"),
            1000,
        );
        pq.min_tier = RelevanceTier::Medium;
        let p = portable_pack(&[lo, med, hi], &pq).unwrap();
        assert_eq!(p.entries.len(), 2);
        assert_eq!(p.entries[0].entry.task_id, "hi"); // High 先
        assert_eq!(p.entries[1].entry.task_id, "med"); // Medium 后
    }

    #[test]
    fn pack_budget_zero_when_entries_empty() {
        // 空输入 + 最小预算：空包且 total=0。
        let q = PortableQuery::new(MemoryQuery::new(), 1);
        let p = portable_pack(&[], &q).unwrap();
        assert_eq!(p.total_tokens, 0);
        assert!(p.entries.is_empty());
    }

    #[test]
    fn pack_tier_counts_reflect_packed_buckets() {
        let hi = plain("hi", "2026-10-16T00:00:00Z").conclusion("passes"); // 5 分 High
        let med = plain("med", "2026-10-16T00:00:00Z").add_tag("ci"); // 3 分 Medium
        let lo = plain("lo", "2026-10-16T00:00:00Z"); // 0 分 Low
        let q = PortableQuery::new(
            MemoryQuery::new().conclusion_contains("passes").tag("ci"),
            1000,
        );
        let p = portable_pack(&[lo, med, hi], &q).unwrap();
        assert_eq!(p.tier_counts.high, 1);
        assert_eq!(p.tier_counts.medium, 1);
        assert_eq!(p.tier_counts.low, 1);
        assert_eq!(p.entries.len(), 3);
    }

    #[test]
    fn pack_multiple_same_tier_sorted_by_score_desc() {
        // 两个 High：一个结论命中（5 分），一个结论+标签（8 分）→ 8 分在前。
        let s5 = plain("s5", "2026-10-16T00:00:00Z").conclusion("passes"); // 5 分
        let s8 = plain("s8", "2026-10-16T00:00:00Z")
            .conclusion("passes")
            .add_tag("ci"); // 5 + 3 = 8
        let q = PortableQuery::new(
            MemoryQuery::new().conclusion_contains("passes").tag("ci"),
            1000,
        );
        let p = portable_pack(&[s5, s8], &q).unwrap();
        assert_eq!(p.entries[0].entry.task_id, "s8");
        assert_eq!(p.entries[0].score, 8);
        assert_eq!(p.entries[1].score, 5);
    }

    #[test]
    fn pack_large_budget_omits_nothing_but_small_budget_omits_tail() {
        // 3 个 Low（0 分）：预算只够 1 条 → 装 1 条（at 新优先），其余 2 条 omitted。
        let e1 = plain("l1", "2026-10-16T00:00:00Z");
        let e2 = plain("l2", "2026-10-16T00:00:01Z");
        let e3 = plain("l3", "2026-10-16T00:00:02Z");
        let one_tokens = estimate_entry_tokens(&e1);
        let q = PortableQuery::new(MemoryQuery::new(), one_tokens);
        let p = portable_pack(&[e1, e2, e3], &q).unwrap();
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0].entry.task_id, "l3"); // 新优先
        assert_eq!(p.omitted.len(), 2);
    }

    fn plain(id: &str, at: &str) -> MemoryEntry {
        // 带最小内容（stdout_tail），保证估算 token > 0（空条目 token=0 无法测装包）。
        MemoryEntry::new(id).at(at).stdout_tail("ok")
    }
}
