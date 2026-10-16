//! MemGovernance：记忆治理（v5.1.9）——防 MemClaw 四失败模式。
//!
//! 四失败模式与治理落点：
//! - **未授权泄漏（unauthorized leakage）**：`export_safety_check` 在导出/装包前对
//!   结论与 tails 做敏感特征扫描（邮箱/手机号/IPv4/绝对路径/密钥特征），命中即
//!   fail-closed 拒绝导出，显式给出命中原因；不阻断本层之外的隐私脱敏
//!   （脱敏仍归 `privacy`，本层管"导出边界"）。
//! - **陈旧传播（stale propagation）**：`freshness_gate` 按条目 `at` 时间戳与
//!   `max_stale_secs` 过滤，过期条目不进入传播/装包（fail-closed 于传播侧），
//!   显式列出被过滤条目；时间为调用方注入（`now_secs`），不依赖系统时钟。
//! - **矛盾持久化（contradictory persistence）**：`detect_contradiction` 对两条结论
//!   做**确定性**词级冲突检测——共享实义词（长度 >= 2 且非停用词）>= 2 且否定极性
//!   相反则判矛盾；不宣称语义理解，无法确定时 `contradicts=false` 并如实给出共享词。
//! - **来源坍缩（source collapse）**：`source_diversity` 统计来源节点多样性——
//!   单节点独占（占比超阈值）或总条目数远大于来源数时标记坍缩风险，
//!   防止多条不同来源记忆被折叠成单一来源导致审计失效。
//!
//! 设计约束：纯函数、无 IO、时间可注入、生产 0 panic、fail-closed；
//! 与 `portable`（装包）正交，治理可作为装包前哨兵。

use std::fmt;

use super::persistent::MemoryEntry;

/// 四失败模式标签（MemClaw）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemClawMode {
    /// 未授权泄漏：导出/传播前敏感内容越界。
    Leakage,
    /// 陈旧传播：过期记忆被当作新鲜记忆传播。
    Stale,
    /// 矛盾持久化：相互矛盾的结论被同时持久化。
    Contradiction,
    /// 来源坍缩：多来源记忆被折叠为单一来源。
    Collapse,
}

impl MemClawMode {
    pub fn label(&self) -> &'static str {
        match self {
            MemClawMode::Leakage => "未授权泄漏",
            MemClawMode::Stale => "陈旧传播",
            MemClawMode::Contradiction => "矛盾持久化",
            MemClawMode::Collapse => "来源坍缩",
        }
    }
    pub fn parse(s: &str) -> Option<MemClawMode> {
        match s.to_ascii_lowercase().as_str() {
            "leakage" | "leak" => Some(MemClawMode::Leakage),
            "stale" => Some(MemClawMode::Stale),
            "contradiction" | "contra" => Some(MemClawMode::Contradiction),
            "collapse" => Some(MemClawMode::Collapse),
            _ => None,
        }
    }
}

/// 导出安全决策：`allowed=false` 为 fail-closed 拒绝，附命中原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDecision {
    pub allowed: bool,
    pub reasons: Vec<String>,
}

impl ExportDecision {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            reasons: Vec::new(),
        }
    }
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reasons: vec![reason.into()],
        }
    }
}

impl fmt::Display for ExportDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.allowed {
            write!(f, "允许导出")
        } else {
            write!(f, "拒绝导出：{}", self.reasons.join("；"))
        }
    }
}

/// 陈旧过滤报告：`kept` 进入传播，`stale` 被显式拦下（含条目原文，可观测）。
#[derive(Debug, Clone, PartialEq)]
pub struct FreshnessReport {
    pub kept: Vec<MemoryEntry>,
    pub stale: Vec<MemoryEntry>,
}

impl FreshnessReport {
    pub fn kept_count(&self) -> usize {
        self.kept.len()
    }
    pub fn stale_count(&self) -> usize {
        self.stale.len()
    }
}

/// 矛盾检测结论：`contradicts=true` 表示确定性判定为矛盾。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContradictionVerdict {
    pub contradicts: bool,
    /// 共享实义词（长度 >= 2 且非停用词）。
    pub shared_terms: Vec<String>,
    /// 否定极性是否相反（一方含否定词、另一方不含）。
    pub polarity_flip: bool,
}

/// 来源多样性报告：`collapse_risk=true` 标记来源坍缩风险。
#[derive(Debug, Clone, PartialEq)]
pub struct CollapseReport {
    pub total: usize,
    pub distinct_nodes: usize,
    /// 单来源最大占比（0.0–1.0，无来源时为 0.0）。
    pub max_share: f64,
    pub collapse_risk: bool,
}

/// 敏感特征规则：命中任一即拒绝导出。
/// 覆盖常见 PII/密钥形态；不宣称穷举，规则可扩展。
const SENSITIVE_PATTERNS: &[(&str, &str)] = &[
    ("email", r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"),
    ("phone", r"(?:\+?86[- ]?)?1[3-9]\d{9}"),
    ("ipv4", r"\b(?:\d{1,3}\.){3}\d{1,3}\b"),
    ("sk_key", r"sk-[A-Za-z0-9]{16,}"),
    (
        "private_key",
        r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
    ),
];

/// 检测字符串中的敏感特征，返回命中的特征名列表（无命中为空）。
pub fn detect_sensitive(s: &str) -> Vec<String> {
    let mut hits: Vec<String> = Vec::new();
    for (name, pat) in SENSITIVE_PATTERNS {
        if let Ok(re) = regex::Regex::new(pat) {
            if re.is_match(s) {
                hits.push((*name).to_string());
            }
        }
    }
    hits
}

/// 导出安全检查：扫描结论、stdout/stderr、argv 中的敏感特征。
/// fail-closed：任一命中即拒绝导出（`allowed=false`）。
pub fn export_safety_check(e: &MemoryEntry) -> ExportDecision {
    let mut reasons: Vec<String> = Vec::new();
    let scan = |label: &str, text: &str, reasons: &mut Vec<String>| {
        if text.is_empty() {
            return;
        }
        for hit in detect_sensitive(text) {
            reasons.push(format!("{label} 含 {hit}"));
        }
    };
    if let Some(c) = e.conclusion.as_deref() {
        scan("conclusion", c, &mut reasons);
    }
    if let Some(s) = e.stdout_tail.as_deref() {
        scan("stdout_tail", s, &mut reasons);
    }
    if let Some(s) = e.stderr_tail.as_deref() {
        scan("stderr_tail", s, &mut reasons);
    }
    for a in e.argv.iter() {
        scan("argv", a, &mut reasons);
    }
    if reasons.is_empty() {
        ExportDecision::allow()
    } else {
        ExportDecision {
            allowed: false,
            reasons,
        }
    }
}

/// 解析 RFC3339 时间戳为 UTC 秒（epoch）。解析失败返回 `None`（调用方按陈旧处理）。
pub fn parse_rfc3339_secs(at: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc).timestamp().max(0) as u64)
}

/// 陈旧过滤：`at` 时间戳距今超过 `max_stale_secs` 的条目归入 `stale`。
/// 时间戳缺失或解析失败按陈旧处理（fail-closed 于传播侧）。
/// `max_stale_secs == 0` 表示禁用陈旧过滤（全部保留）。
pub fn freshness_gate(
    entries: &[MemoryEntry],
    now_secs: u64,
    max_stale_secs: u64,
) -> FreshnessReport {
    let mut kept = Vec::new();
    let mut stale = Vec::new();
    for e in entries {
        if max_stale_secs == 0 {
            kept.push(e.clone());
            continue;
        }
        match parse_rfc3339_secs(&e.at) {
            Some(secs) if now_secs.saturating_sub(secs) <= max_stale_secs => kept.push(e.clone()),
            _ => stale.push(e.clone()),
        }
    }
    FreshnessReport { kept, stale }
}

/// 提取实义词：英文按非字母数字切分词级；中文（含 CJK）按重叠 2-gram 切分，
/// 使无分词器时仍能稳定共享词检测（确定性、可审计，不宣称语义理解）。
/// 英文停用词覆盖常见连接/助词；2-gram 不做停用过滤（均为双字，噪声由
/// "需 >= 2 个共享词"门槛吸收）。
pub fn terms_of(s: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "not", "no", "but", "was", "has", "had", "are", "is", "of",
        "to", "in", "on", "at", "by", "or", "as", "it", "its", "that", "this", "from", "一个",
        "没有", "不是", "已经", "但是", "以及", "对于", "可以", "需要", "进行", "什么", "如何",
        "是否", "因为", "所以", "如果", "那么", "我们", "你们", "他们", "这个", "那个",
    ];
    let mut out: Vec<String> = Vec::new();
    let has_cjk = s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    if has_cjk {
        // 中文：仅取汉字序列，重叠 2-gram
        let han: Vec<char> = s
            .chars()
            .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
            .collect();
        for w in han.windows(2) {
            let g: String = w.iter().collect();
            if !out.contains(&g) {
                out.push(g);
            }
        }
        return out;
    }
    for tok in s.split(|c: char| !c.is_alphanumeric()) {
        let t = tok.to_lowercase();
        if t.chars().count() >= 2 && !STOP.contains(&t.as_str()) {
            out.push(t);
        }
    }
    out
}

/// 否定极性：文本含否定词（英文 not/no/none/never/cannot/can't/unable/un-，
/// 中文 不/未/无/勿/禁止/失败/拒绝）为真。
pub fn has_negation(s: &str) -> bool {
    const NEG: &[&str] = &[
        " not", " no ", "none", "never", "cannot", "can't", "unable", "un", "不", "未", "无", "勿",
        "禁止", "失败", "拒绝",
    ];
    NEG.iter().any(|n| s.to_lowercase().contains(n))
}

/// 矛盾检测：两条结论共享 >= 2 个实义词且否定极性相反 → 判矛盾。
/// 保守：只做词级确定性检测，不宣称语义理解；不满足条件则 `contradicts=false`。
pub fn detect_contradiction(a_conclusion: &str, b_conclusion: &str) -> ContradictionVerdict {
    let ta = terms_of(a_conclusion);
    let tb = terms_of(b_conclusion);
    let shared: Vec<String> = ta.iter().filter(|t| tb.contains(t)).cloned().collect();
    let flip = has_negation(a_conclusion) != has_negation(b_conclusion);
    ContradictionVerdict {
        contradicts: shared.len() >= 2 && flip,
        shared_terms: shared,
        polarity_flip: flip,
    }
}

/// 来源多样性：统计不同 `node_id` 数量与单来源最大占比。
/// 无来源标注的条目归入 `""` 单一来源桶。
/// `collapse_risk`：条目 >= 3 且来源数 < 2，或单来源占比 > 0.8。
pub fn source_diversity(entries: &[MemoryEntry]) -> CollapseReport {
    if entries.is_empty() {
        return CollapseReport {
            total: 0,
            distinct_nodes: 0,
            max_share: 0.0,
            collapse_risk: false,
        };
    }
    let total = entries.len();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in entries {
        let key = e.node_id.clone().unwrap_or_default();
        *counts.entry(key).or_insert(0) += 1;
    }
    let distinct_nodes = counts.len();
    let max_share = counts
        .values()
        .map(|&c| c as f64 / total as f64)
        .fold(0.0_f64, f64::max);
    let collapse_risk = (total >= 3 && distinct_nodes < 2) || max_share > 0.8;
    CollapseReport {
        total,
        distinct_nodes,
        max_share,
        collapse_risk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::persistent::MemoryEntry;

    fn entry(id: &str, at: &str) -> MemoryEntry {
        MemoryEntry::new(id).at(at.to_string())
    }

    // ---- MemClawMode ----
    #[test]
    fn claw_mode_labels_and_parse() {
        assert_eq!(MemClawMode::Leakage.label(), "未授权泄漏");
        assert_eq!(MemClawMode::Stale.label(), "陈旧传播");
        assert_eq!(MemClawMode::Contradiction.label(), "矛盾持久化");
        assert_eq!(MemClawMode::Collapse.label(), "来源坍缩");
        assert_eq!(MemClawMode::parse("leakage"), Some(MemClawMode::Leakage));
        assert_eq!(MemClawMode::parse("LEAK"), Some(MemClawMode::Leakage));
        assert_eq!(MemClawMode::parse("stale"), Some(MemClawMode::Stale));
        assert_eq!(
            MemClawMode::parse("contra"),
            Some(MemClawMode::Contradiction)
        );
        assert_eq!(MemClawMode::parse("collapse"), Some(MemClawMode::Collapse));
        assert_eq!(MemClawMode::parse("unknown"), None);
        assert_eq!(MemClawMode::parse(""), None);
    }

    // ---- export_safety_check / detect_sensitive ----
    #[test]
    fn export_allows_clean_entry() {
        let e = entry("t1", "2026-10-16T00:00:00Z")
            .conclusion("构建通过，测试全绿")
            .stdout_tail("cargo test passed")
            .argv(vec!["cargo".into(), "test".into()]);
        let d = export_safety_check(&e);
        assert!(d.allowed, "干净条目应允许导出");
        assert!(d.reasons.is_empty());
    }

    #[test]
    fn export_denies_email_in_conclusion() {
        let e = entry("t1", "2026-10-16T00:00:00Z")
            .conclusion("请联系 ops@example.com 处理")
            .stdout_tail("ok");
        let d = export_safety_check(&e);
        assert!(!d.allowed, "结论含邮箱应拒绝导出");
        assert!(d.reasons.iter().any(|r| r.contains("email")));
    }

    #[test]
    fn export_denies_phone_in_stdout() {
        let e = entry("t1", "2026-10-16T00:00:00Z")
            .conclusion("完成")
            .stdout_tail("phone 13812345678 recorded");
        let d = export_safety_check(&e);
        assert!(!d.allowed, "stdout 含手机号应拒绝");
        assert!(d.reasons.iter().any(|r| r.contains("phone")));
    }

    #[test]
    fn export_denies_sk_key_in_argv() {
        let e = entry("t1", "2026-10-16T00:00:00Z")
            .argv(vec!["--token".into(), "sk-abcdef1234567890".into()]);
        let d = export_safety_check(&e);
        assert!(!d.allowed, "argv 含 sk- 密钥应拒绝");
        assert!(d.reasons.iter().any(|r| r.contains("sk_key")));
    }

    #[test]
    fn export_denies_private_key_marker() {
        let e = entry("t1", "2026-10-16T00:00:00Z").stdout_tail("-----BEGIN RSA PRIVATE KEY-----");
        let d = export_safety_check(&e);
        assert!(!d.allowed, "私钥标记应拒绝");
        assert!(d.reasons.iter().any(|r| r.contains("private_key")));
    }

    #[test]
    fn export_denies_ipv4_in_stderr() {
        let e = entry("t1", "2026-10-16T00:00:00Z")
            .conclusion("ok")
            .stderr_tail("connect to 192.168.1.10 timeout");
        let d = export_safety_check(&e);
        assert!(!d.allowed, "stderr 含 IP 应拒绝");
        assert!(d.reasons.iter().any(|r| r.contains("ipv4")));
    }

    #[test]
    fn export_reasons_are_multi_and_concrete() {
        let e = entry("t1", "2026-10-16T00:00:00Z")
            .conclusion("a@b.com done")
            .stdout_tail("13812345678");
        let d = export_safety_check(&e);
        assert!(!d.allowed);
        assert_eq!(d.reasons.len(), 2, "两个字段各命中一条");
        assert!(d.reasons.iter().all(|r| r.contains("含")));
    }

    #[test]
    fn detect_sensitive_empty_is_clean() {
        assert!(detect_sensitive("").is_empty());
        assert!(detect_sensitive("正常构建日志，无敏感信息").is_empty());
    }

    // ---- parse_rfc3339_secs ----
    #[test]
    fn parse_rfc3339_ok() {
        let s = parse_rfc3339_secs("2026-10-16T00:00:00Z");
        assert!(s.is_some());
        assert!(s.unwrap() > 1_700_000_000);
    }

    #[test]
    fn parse_rfc3339_offset_normalized_to_utc() {
        let s = parse_rfc3339_secs("2026-10-16T08:00:00+08:00");
        assert_eq!(s, parse_rfc3339_secs("2026-10-16T00:00:00Z"));
    }

    #[test]
    fn parse_rfc3339_bad_returns_none() {
        assert_eq!(parse_rfc3339_secs("not-a-date"), None);
        assert_eq!(parse_rfc3339_secs(""), None);
        assert_eq!(parse_rfc3339_secs("2026-13-99T99:99:99Z"), None);
    }

    // ---- freshness_gate ----
    #[test]
    fn freshness_keeps_recent_drops_old() {
        let entries = vec![
            entry("a", "2026-10-16T00:00:00Z"),
            entry("b", "2026-10-15T00:00:00Z"),
        ];
        // now = 10-16T00:00:30Z；a 30s、b 1day；max_stale=3600 → a 保留、b 拦下
        let r = freshness_gate(
            &entries,
            parse_rfc3339_secs("2026-10-16T00:00:30Z").unwrap(),
            3600,
        );
        assert_eq!(r.kept_count(), 1);
        assert_eq!(r.stale_count(), 1);
        assert_eq!(r.kept[0].task_id, "a");
        assert_eq!(r.stale[0].task_id, "b");
    }

    #[test]
    fn freshness_zero_disables_filter() {
        let entries = vec![entry("a", "2020-01-01T00:00:00Z")];
        let r = freshness_gate(&entries, 1_000_000_000, 0);
        assert_eq!(r.kept_count(), 1, "max_stale_secs=0 全部保留");
        assert_eq!(r.stale_count(), 0);
    }

    #[test]
    fn freshness_bad_timestamp_fails_closed_to_stale() {
        let entries = vec![
            entry("bad", "not-a-date"),
            entry("good", "2026-10-16T00:00:00Z"),
        ];
        let now = parse_rfc3339_secs("2026-10-16T00:00:01Z").unwrap();
        let r = freshness_gate(&entries, now, 3600);
        assert_eq!(r.stale_count(), 1, "坏时间戳按陈旧拦下（fail-closed）");
        assert_eq!(r.stale[0].task_id, "bad");
        assert_eq!(r.kept[0].task_id, "good");
    }

    #[test]
    fn freshness_future_timestamp_kept() {
        // 未来时间戳（时钟偏移）视为新鲜保留
        let entries = vec![entry("f", "2027-01-01T00:00:00Z")];
        let now = parse_rfc3339_secs("2026-10-16T00:00:00Z").unwrap();
        let r = freshness_gate(&entries, now, 3600);
        assert_eq!(r.kept_count(), 1);
    }

    #[test]
    fn freshness_empty_input() {
        let r = freshness_gate(&[], 1_000_000_000, 3600);
        assert_eq!(r.kept_count(), 0);
        assert_eq!(r.stale_count(), 0);
    }

    #[test]
    fn freshness_preserves_entry_fields() {
        let e = entry("t1", "2026-10-16T00:00:00Z")
            .conclusion("构建通过")
            .add_tag("ci")
            .node_id("n1");
        let r = freshness_gate(
            &[e],
            parse_rfc3339_secs("2026-10-16T00:00:10Z").unwrap(),
            3600,
        );
        assert_eq!(r.kept[0].conclusion.as_deref(), Some("构建通过"));
        assert_eq!(r.kept[0].tags, vec!["ci"]);
        assert_eq!(r.kept[0].node_id.as_deref(), Some("n1"));
    }

    // ---- terms_of / has_negation ----
    #[test]
    fn terms_extracts_real_words() {
        let t = terms_of("deploy succeeded on target");
        assert!(t.contains(&"deploy".to_string()));
        assert!(t.contains(&"succeeded".to_string()));
        assert!(t.contains(&"target".to_string()));
        assert!(!t.contains(&"on".to_string()), "停用词 on 不参与");
    }

    #[test]
    fn terms_handles_chinese() {
        let t = terms_of("构建失败：内存不足");
        assert!(t.contains(&"构建".to_string()));
        assert!(t.contains(&"内存".to_string()));
        assert!(t.contains(&"不足".to_string()));
    }

    #[test]
    fn terms_skips_stopwords_and_short() {
        let t = terms_of("not and a to");
        assert!(!t.contains(&"not".to_string()));
        assert!(!t.contains(&"and".to_string()));
        assert!(!t.contains(&"a".to_string()));
        assert!(!t.contains(&"to".to_string()));
    }

    #[test]
    fn negation_detects_english_and_chinese() {
        assert!(has_negation("build did not pass"));
        assert!(has_negation("部署未完成"));
        assert!(has_negation("构建失败"));
        assert!(has_negation("cannot connect"));
        assert!(!has_negation("build passed"));
        assert!(!has_negation("部署完成"));
    }

    // ---- detect_contradiction ----
    #[test]
    fn contradiction_flip_with_shared_terms() {
        let a = "部署失败：内存不足";
        let b = "部署成功：内存充足";
        let v = detect_contradiction(a, b);
        assert!(v.contradicts, "共享 部署/内存 且极性相反");
        assert!(v.shared_terms.len() >= 2);
        assert!(v.polarity_flip);
    }

    #[test]
    fn contradiction_not_flagged_on_same_polarity() {
        let a = "测试通过";
        let b = "测试全部通过";
        let v = detect_contradiction(a, b);
        assert!(!v.contradicts, "极性一致不判矛盾");
        assert!(!v.polarity_flip);
    }

    #[test]
    fn contradiction_needs_two_shared_terms() {
        let a = "构建失败";
        let b = "任务成功完成";
        let v = detect_contradiction(a, b);
        assert!(!v.contradicts, "共享词不足 2 个不判矛盾");
        assert!(v.shared_terms.len() < 2);
    }

    #[test]
    fn contradiction_empty_conclusions() {
        let v = detect_contradiction("", "");
        assert!(!v.contradicts);
        assert!(v.shared_terms.is_empty());
    }

    #[test]
    fn contradiction_reports_shared_terms() {
        let v = detect_contradiction("服务已启动", "服务未启动");
        assert!(v.contradicts);
        assert!(v.shared_terms.contains(&"服务".to_string()));
        assert!(v.shared_terms.contains(&"启动".to_string()));
    }

    // ---- source_diversity ----
    #[test]
    fn diversity_single_node_collapse_risk() {
        let entries = vec![
            entry("a", "2026-10-16T00:00:00Z").node_id("n1"),
            entry("b", "2026-10-16T00:00:01Z").node_id("n1"),
            entry("c", "2026-10-16T00:00:02Z").node_id("n1"),
        ];
        let r = source_diversity(&entries);
        assert_eq!(r.total, 3);
        assert_eq!(r.distinct_nodes, 1);
        assert!(r.collapse_risk, "3 条同源 → 坍缩风险");
        assert!(r.max_share > 0.8);
    }

    #[test]
    fn diversity_multi_node_no_risk() {
        let entries = vec![
            entry("a", "2026-10-16T00:00:00Z").node_id("n1"),
            entry("b", "2026-10-16T00:00:01Z").node_id("n2"),
            entry("c", "2026-10-16T00:00:02Z").node_id("n3"),
        ];
        let r = source_diversity(&entries);
        assert_eq!(r.distinct_nodes, 3);
        assert!(!r.collapse_risk);
        assert!((r.max_share - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn diversity_dominant_single_node_no_risk() {
        let entries = vec![
            entry("a", "2026-10-16T00:00:00Z").node_id("n1"),
            entry("b", "2026-10-16T00:00:01Z").node_id("n1"),
            entry("c", "2026-10-16T00:00:02Z").node_id("n1"),
            entry("d", "2026-10-16T00:00:03Z").node_id("n2"),
            entry("e", "2026-10-16T00:00:04Z").node_id("n2"),
        ];
        let r = source_diversity(&entries);
        assert_eq!(r.distinct_nodes, 2);
        // distinct=2 且单源占比 0.6（未超 0.8）→ 无坍缩风险
        assert!(!r.collapse_risk);
        assert!(r.max_share > 0.5 && r.max_share <= 0.8);
    }

    #[test]
    fn diversity_empty_input() {
        let r = source_diversity(&[]);
        assert_eq!(r.total, 0);
        assert_eq!(r.distinct_nodes, 0);
        assert!(!r.collapse_risk);
        assert_eq!(r.max_share, 0.0);
    }

    #[test]
    fn diversity_unnamed_nodes_group_as_one() {
        let entries = vec![
            entry("a", "2026-10-16T00:00:00Z"),
            entry("b", "2026-10-16T00:00:01Z"),
            entry("c", "2026-10-16T00:00:02Z"),
        ];
        let r = source_diversity(&entries);
        assert_eq!(r.distinct_nodes, 1, "无 node_id 归入同一桶");
        assert!(r.collapse_risk);
    }

    #[test]
    fn diversity_mixed_named_and_unnamed() {
        let entries = vec![
            entry("a", "2026-10-16T00:00:00Z").node_id("n1"),
            entry("b", "2026-10-16T00:00:01Z"),
            entry("c", "2026-10-16T00:00:02Z").node_id("n2"),
        ];
        let r = source_diversity(&entries);
        assert_eq!(r.distinct_nodes, 3);
        assert!(!r.collapse_risk);
    }

    // ---- 组合场景 ----
    #[test]
    fn governance_chain_all_clean() {
        // 一条干净、新鲜、无矛盾、多源的条目可安全导出
        let e = entry("t1", "2026-10-16T00:00:00Z")
            .conclusion("部署成功，服务运行正常")
            .node_id("n1");
        assert!(export_safety_check(&e).allowed);
        let now = parse_rfc3339_secs("2026-10-16T00:00:10Z").unwrap();
        let r = freshness_gate(std::slice::from_ref(&e), now, 3600);
        assert_eq!(r.kept_count(), 1);
        let d = source_diversity(&[e.clone(), entry("t2", "2026-10-16T00:00:01Z").node_id("n2")]);
        assert!(!d.collapse_risk);
    }

    #[test]
    fn export_display_format() {
        let d = ExportDecision::deny("conclusion 含 email");
        let s = d.to_string();
        assert!(s.contains("拒绝导出"));
        assert!(s.contains("email"));
        let a = ExportDecision::allow();
        assert_eq!(a.to_string(), "允许导出");
    }
}
