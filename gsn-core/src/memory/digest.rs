//! v5.0.8：会话摘要聚合。
//!
//! 单条记忆已有 `conclusion`（daemon 自动生成的结论摘要，见 v5.0.2）；
//! 本模块在其之上提供**会话级摘要**：按 `task_id` 分组聚合全部条目，
//! 输出每个会话的条目数、成功率、耗时统计、gate 分布与去重结论要点，
//! 供调配/审核 Agent 与 CLI（`gsn memory digest`）快速复盘一个任务会话，
//! 而不必逐条阅读原始记忆。
//!
//! 统计口径与 [`super::persistent::NodeProfile::aggregate_from_entries`] 保持一致：
//! - 成功率只统计有 `exit_code` 的条目（`0` 计成功）；
//! - 耗时只统计有 `ms` 的条目；
//! - gate 去重、字典序；结论去重、保留出现顺序（上限 [`DIGEST_CONCLUSION_MAX`]）。

use super::persistent::MemoryEntry;

/// 单个会话摘要最多保留的去重结论要点数。
pub const DIGEST_CONCLUSION_MAX: usize = 8;

/// 一个任务会话（task_id）的聚合摘要。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionDigest {
    pub task_id: String,
    /// 参与聚合的记忆条目数（含无 exit_code / ms 的条目）。
    pub entries: u64,
    /// 会话成功率（0.0–1.0）；仅有 exit_code 的条目计，无样本为 `None`。
    pub success_rate: Option<f64>,
    /// 平均耗时（毫秒）；仅有 ms 的条目计，无样本为 `None`。
    pub avg_ms: Option<u64>,
    /// 最大耗时（毫秒）；无样本为 `None`。
    pub max_ms: Option<u64>,
    /// gate 去重、字典序。
    pub gates: Vec<String>,
    /// 去重后的结论要点（保留出现顺序，上限 [`DIGEST_CONCLUSION_MAX`]）。
    pub conclusions: Vec<String>,
    /// 会话首条记忆时间（RFC3339 字符串比较，条目 `at` 原样保留）。
    pub first_at: Option<String>,
    /// 会话末条记忆时间。
    pub last_at: Option<String>,
}

impl SessionDigest {
    fn empty(task_id: &str) -> Self {
        Self {
            task_id: task_id.to_string(),
            entries: 0,
            success_rate: None,
            avg_ms: None,
            max_ms: None,
            gates: Vec::new(),
            conclusions: Vec::new(),
            first_at: None,
            last_at: None,
        }
    }

    fn absorb(&mut self, e: &MemoryEntry) {
        self.entries += 1;
        // 成功率 / 平均耗时由 digest_sessions 以 (ok, coded) / (ms_sum, ms_n) 统一计算，
        // absorb 只处理无需计数的字段。
        if let Some(m) = e.ms {
            self.max_ms = Some(match self.max_ms {
                Some(prev) => prev.max(m),
                None => m,
            });
        }
        if let Some(g) = e.gate.as_deref() {
            if !self.gates.iter().any(|x| x == g) {
                self.gates.push(g.to_string());
            }
        }
        if let Some(c) = e.conclusion.as_deref() {
            if !self.conclusions.iter().any(|x| x == c)
                && self.conclusions.len() < DIGEST_CONCLUSION_MAX
            {
                self.conclusions.push(c.to_string());
            }
        }
        if self.first_at.is_none() {
            self.first_at = Some(e.at.clone());
        }
        self.last_at = Some(e.at.clone());
    }
}

/// 按 `task_id` 分组聚合全部条目，输出会话级摘要。
///
/// - 空输入返回空 Vec；
/// - 每个会话独立聚合；成功率只统计有 `exit_code` 的条目（`0` 计成功），
///   无 exit_code 样本时 `success_rate` 为 `None`；
/// - 平均耗时按会话内**全部**有 ms 条目的算术平均计，最大耗时取其最大值；
/// - 结果按 `last_at` 倒序（RFC3339 同为 UTC 定长前缀，字典序即时间序）；
///   相同 `last_at` 时按 `task_id` 字典序稳定排序。
pub fn digest_sessions(entries: &[MemoryEntry]) -> Vec<SessionDigest> {
    // 先按 task_id 分组（保持首次出现顺序），再逐会话聚合。
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&MemoryEntry>> =
        std::collections::HashMap::new();
    for e in entries {
        if !groups.contains_key(&e.task_id) {
            order.push(e.task_id.clone());
        }
        groups.entry(e.task_id.clone()).or_default().push(e);
    }
    let mut out: Vec<SessionDigest> = Vec::with_capacity(order.len());
    for tid in order {
        // order 与 groups 同步构造，取不到即为编程错误；用 if-let 保持无 panic 路径。
        let Some(group) = groups.get(&tid) else {
            continue;
        };
        let mut d = SessionDigest::empty(&tid);
        let mut ok = 0u64;
        let mut coded = 0u64;
        let mut ms_sum = 0u64;
        let mut ms_n = 0u64;
        for e in group {
            d.absorb(e);
            if let Some(code) = e.exit_code {
                coded += 1;
                if code == 0 {
                    ok += 1;
                }
            }
            if let Some(m) = e.ms {
                ms_sum += m;
                ms_n += 1;
            }
        }
        if coded > 0 {
            d.success_rate = Some(ok as f64 / coded as f64);
        }
        if ms_n > 0 {
            // ms_n > 0 已保证非零，用 checked_div 满足 Static Gates（无 panic 路径）。
            d.avg_ms = ms_sum.checked_div(ms_n);
        }
        d.gates.sort();
        out.push(d);
    }
    out.sort_by(|a, b| {
        let k = b.last_at.cmp(&a.last_at);
        if k == std::cmp::Ordering::Equal {
            a.task_id.cmp(&b.task_id)
        } else {
            k
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        tid: &str,
        at: &str,
        code: Option<i32>,
        ms: Option<u64>,
        gate: Option<&str>,
        concl: Option<&str>,
    ) -> MemoryEntry {
        let mut e = MemoryEntry::new(tid);
        e.at = at.to_string();
        if let Some(c) = code {
            e = e.exit_code(c);
        }
        if let Some(m) = ms {
            e = e.ms(m);
        }
        if let Some(g) = gate {
            e = e.gate(g);
        }
        if let Some(c) = concl {
            e = e.conclusion(c);
        }
        e
    }

    #[test]
    fn groups_by_task_and_computes_stats() {
        let entries = vec![
            entry(
                "a",
                "2026-10-15T00:00:01Z",
                Some(0),
                Some(10),
                Some("ci"),
                Some("exit=0 ok"),
            ),
            entry(
                "a",
                "2026-10-15T00:00:02Z",
                Some(1),
                Some(30),
                Some("ci"),
                Some("exit=1 (rust)"),
            ),
            entry(
                "a",
                "2026-10-15T00:00:03Z",
                Some(0),
                Some(50),
                Some("test"),
                Some("exit=0 ok"),
            ),
            entry(
                "b",
                "2026-10-15T00:00:02Z",
                Some(0),
                Some(5),
                Some("ci"),
                Some("exit=0 ok"),
            ),
        ];
        let d = digest_sessions(&entries);
        assert_eq!(d.len(), 2);
        // 按 last_at 倒序：a 的 last(00:00:03) > b 的 last(00:00:02)。
        assert_eq!(d[0].task_id, "a");
        assert_eq!(d[0].entries, 3);
        assert_eq!(d[0].success_rate, Some(2.0 / 3.0));
        assert_eq!(d[0].avg_ms, Some(30)); // (10+30+50)/3
        assert_eq!(d[0].max_ms, Some(50));
        assert_eq!(d[0].gates, vec!["ci", "test"]);
        // 结论去重、保序。
        assert_eq!(d[0].conclusions, vec!["exit=0 ok", "exit=1 (rust)"]);
        assert_eq!(d[0].first_at.as_deref(), Some("2026-10-15T00:00:01Z"));
        assert_eq!(d[0].last_at.as_deref(), Some("2026-10-15T00:00:03Z"));

        assert_eq!(d[1].task_id, "b");
        assert_eq!(d[1].entries, 1);
        assert_eq!(d[1].success_rate, Some(1.0));
    }

    #[test]
    fn no_exit_code_entries_leave_success_none() {
        let entries = vec![
            entry("a", "2026-10-15T00:00:01Z", None, Some(7), Some("ci"), None),
            entry("a", "2026-10-15T00:00:02Z", None, None, None, None),
        ];
        let d = digest_sessions(&entries);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].entries, 2);
        assert_eq!(d[0].success_rate, None);
        assert_eq!(d[0].avg_ms, Some(7)); // 仅 1 条有 ms
        assert_eq!(d[0].max_ms, Some(7));
        assert_eq!(d[0].gates, vec!["ci"]);
        assert!(d[0].conclusions.is_empty());
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(digest_sessions(&[]).is_empty());
    }

    #[test]
    fn conclusions_truncated_to_max() {
        let mut entries = Vec::new();
        for i in 0..(DIGEST_CONCLUSION_MAX + 3) {
            entries.push(entry(
                "a",
                &format!("2026-10-15T00:00:{i:02}Z"),
                Some(0),
                Some(1),
                None,
                Some(&format!("c{i}")),
            ));
        }
        let d = digest_sessions(&entries);
        assert_eq!(d[0].entries, (DIGEST_CONCLUSION_MAX + 3) as u64);
        assert_eq!(d[0].conclusions.len(), DIGEST_CONCLUSION_MAX);
        assert_eq!(d[0].conclusions.first().map(String::as_str), Some("c0"));
        assert_eq!(d[0].conclusions.last().map(String::as_str), Some("c7"));
    }

    #[test]
    fn ties_on_last_at_sort_by_task_id() {
        let entries = vec![
            entry("b", "2026-10-15T00:00:00Z", Some(0), Some(1), None, None),
            entry("a", "2026-10-15T00:00:00Z", Some(0), Some(1), None, None),
        ];
        let d = digest_sessions(&entries);
        assert_eq!(d[0].task_id, "a");
        assert_eq!(d[1].task_id, "b");
    }
}
