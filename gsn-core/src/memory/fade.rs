//! FadeMem：主动遗忘（v5.1.7）——按 roadmap「主动遗忘 | FadeMem 式自适应衰减/淘汰，
//! 降低存储、保持检索质量（存储收益数字实测，不预设）」。
//!
//! 设计要点：
//! - 每条记忆登记 `importance`（1-5）与 `half_life_secs`（半衰期），有效分
//!   `score = importance * 0.5^(age / half_life)` 随时间指数衰减；
//! - 主动遗忘由 `FadePolicy` 驱动：低于 `min_score` 淘汰、超出 `max_active`
//!   淘汰最低分、配置 `no_access_ttl` 时未访问超时直接淘汰；
//! - **存储收益数字实测，不预设**：`fade()` 只报告真实淘汰条目数与按 JSON
//!   序列化长度实测的释放字节，不宣称任何未测量的"收益百分比"；
//! - **Long 阶段保护**：来源阶段为 Long（分布式存储 + Merkle）的条目默认不淘汰，
//!   仅 `force` 可绕过（防止分布式内容被随意遗忘）；
//! - 淘汰为软标记（`faded_at` 置位），`restore()` 可恢复；`stats()` 输出活跃/
//!   已淘汰计数与实测释放字节。
//!
//! 所有时间均可注入（`now: u64` 秒），核心函数为纯函数，便于确定性单测。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// 记忆来源阶段（与 consolidation::Stage 对齐的衰减视角）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FadeStage {
    Short,
    Mid,
    Long,
}

impl FadeStage {
    pub fn label(&self) -> &'static str {
        match self {
            FadeStage::Short => "短期",
            FadeStage::Mid => "中期",
            FadeStage::Long => "长期",
        }
    }

    /// 解析 CLI 输入（short/mid/long）。
    pub fn parse(s: &str) -> Option<FadeStage> {
        match s.to_ascii_lowercase().as_str() {
            "short" => Some(FadeStage::Short),
            "mid" => Some(FadeStage::Mid),
            "long" => Some(FadeStage::Long),
            _ => None,
        }
    }
}

/// 主动遗忘策略。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FadePolicy {
    /// 有效分低于此值可淘汰（默认 0.05）。
    pub min_score: f64,
    /// 活跃条目上限；超出时按分数升序淘汰至达标（0 = 不限制）。
    pub max_active: usize,
    /// 未访问超过 N 秒直接淘汰（None = 不启用）。
    pub no_access_ttl: Option<u64>,
    /// 是否保护 Long 阶段条目（默认 true，force 可绕过）。
    pub protect_long: bool,
}

impl Default for FadePolicy {
    fn default() -> Self {
        FadePolicy {
            min_score: 0.05,
            max_active: 0,
            no_access_ttl: None,
            protect_long: true,
        }
    }
}

/// 主动遗忘条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FadeEntry {
    /// 被衰减记忆的条目 id（如 consolidation entry_id 前 16 位）。
    pub entry_id: String,
    /// 来源阶段。
    pub stage: FadeStage,
    /// 重要度 1-5。
    pub importance: u8,
    /// 半衰期（秒）。
    pub half_life_secs: u64,
    /// 登记时间（epoch 秒）。
    pub registered_at: u64,
    /// 最后访问时间（epoch 秒）。
    pub last_access_at: u64,
    /// 累计访问次数。
    pub access_count: u64,
    /// 淘汰时间（None = 活跃，Some = 已淘汰）。
    pub faded_at: Option<u64>,
}

impl FadeEntry {
    /// 计算当前有效分（纯函数）：score = importance * 0.5^(age/half_life)。
    /// 半衰期 0 视为永不衰减（score 恒等于 importance）。
    pub fn score(&self, now: u64) -> f64 {
        if self.half_life_secs == 0 {
            return f64::from(self.importance);
        }
        let age = now.saturating_sub(self.last_access_at);
        let halvings = age as f64 / self.half_life_secs as f64;
        f64::from(self.importance) * 0.5_f64.powf(halvings)
    }

    /// 距上次访问的秒数。
    pub fn idle_secs(&self, now: u64) -> u64 {
        now.saturating_sub(self.last_access_at)
    }
}

/// 单次淘汰的实测结果。
#[derive(Debug, Clone)]
pub struct FadeOutcome {
    pub entry_id: String,
    pub stage: FadeStage,
    pub score_at_fade: f64,
    /// 按 JSON 序列化长度实测的释放字节（存储收益数字实测，不预设）。
    pub freed_bytes: usize,
}

/// 一轮 `fade()` 的结果：淘汰列表 + 实测释放字节合计。
#[derive(Debug, Clone, Default)]
pub struct FadeReport {
    pub faded: Vec<FadeOutcome>,
    pub total_freed_bytes: usize,
    pub skipped_long: usize,
}

impl FadeReport {
    pub fn freed_count(&self) -> usize {
        self.faded.len()
    }
}

/// fail-closed 类型化错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FadeError {
    EmptyId,
    InvalidImportance(u8),
    InvalidHalfLife(u64),
    EntryNotFound(String),
    AlreadyFaded(String),
    NotFaded(String),
    InvalidStage(String),
    StoreIo(String),
    StoreJson(String),
}

impl std::fmt::Display for FadeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FadeError::EmptyId => write!(f, "条目 id 不能为空"),
            FadeError::InvalidImportance(v) => {
                write!(f, "重要度必须为 1-5（收到 {v}）")
            }
            FadeError::InvalidHalfLife(v) => {
                write!(f, "半衰期必须为正数（收到 {v}）")
            }
            FadeError::EntryNotFound(id) => write!(f, "衰减条目不存在（{id}）"),
            FadeError::AlreadyFaded(id) => write!(f, "条目已淘汰（{id}）"),
            FadeError::NotFaded(id) => write!(f, "条目尚未淘汰（{id}）"),
            FadeError::InvalidStage(s) => write!(f, "未知来源阶段（{s}）"),
            FadeError::StoreIo(m) => write!(f, "存储 IO 失败: {m}"),
            FadeError::StoreJson(m) => write!(f, "存储 JSON 失败: {m}"),
        }
    }
}

impl std::error::Error for FadeError {}

/// FadeMem 存储：`fade.jsonl` 逐行 JSONL，按 entry_id 去重，坏行跳过。
#[derive(Debug, Default)]
pub struct FadeStore {
    entries: BTreeMap<String, FadeEntry>,
    path: Option<String>,
}

impl FadeStore {
    pub fn open(path: &Path) -> Result<FadeStore, FadeError> {
        let mut store = FadeStore {
            entries: BTreeMap::new(),
            path: Some(path.to_string_lossy().to_string()),
        };
        if path.exists() {
            let file =
                File::open(path).map_err(|e| FadeError::StoreIo(format!("打开 {path:?}: {e}")))?;
            let reader = BufReader::new(file);
            for (idx, line) in reader.lines().enumerate() {
                let line = match line {
                    Ok(l) => l,
                    Err(e) => {
                        // 坏行跳过：不因单行损坏阻断整个库重放。
                        eprintln!("跳过 fade.jsonl 第 {} 行（读取失败: {e}）", idx + 1);
                        continue;
                    }
                };
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<FadeEntry>(&line) {
                    Ok(entry) => {
                        store.entries.insert(entry.entry_id.clone(), entry);
                    }
                    Err(e) => {
                        eprintln!("跳过 fade.jsonl 第 {} 行（JSON 解析失败: {e}）", idx + 1);
                    }
                }
            }
        }
        Ok(store)
    }

    pub fn entries(&self) -> Vec<&FadeEntry> {
        self.entries.values().collect()
    }

    pub fn get(&self, entry_id: &str) -> Option<&FadeEntry> {
        self.entries.get(entry_id)
    }

    /// 登记一条可衰减记忆（fail-closed 校验）。
    pub fn register(
        &mut self,
        entry_id: &str,
        stage: FadeStage,
        importance: u8,
        half_life_secs: u64,
        now: u64,
    ) -> Result<FadeEntry, FadeError> {
        if entry_id.trim().is_empty() {
            return Err(FadeError::EmptyId);
        }
        if !(1..=5).contains(&importance) {
            return Err(FadeError::InvalidImportance(importance));
        }
        if half_life_secs == 0 {
            return Err(FadeError::InvalidHalfLife(half_life_secs));
        }
        let entry = FadeEntry {
            entry_id: entry_id.trim().to_string(),
            stage,
            importance,
            half_life_secs,
            registered_at: now,
            last_access_at: now,
            access_count: 0,
            faded_at: None,
        };
        self.entries.insert(entry.entry_id.clone(), entry.clone());
        Ok(entry)
    }

    /// 访问：刷新 last_access_at 并累计访问次数。
    pub fn touch(&mut self, entry_id: &str, now: u64) -> Result<(), FadeError> {
        let entry = self
            .entries
            .get_mut(entry_id)
            .ok_or_else(|| FadeError::EntryNotFound(entry_id.to_string()))?;
        if entry.faded_at.is_some() {
            return Err(FadeError::AlreadyFaded(entry_id.to_string()));
        }
        entry.last_access_at = now;
        entry.access_count += 1;
        Ok(())
    }

    /// 恢复已淘汰条目。
    pub fn restore(&mut self, entry_id: &str) -> Result<(), FadeError> {
        let entry = self
            .entries
            .get_mut(entry_id)
            .ok_or_else(|| FadeError::EntryNotFound(entry_id.to_string()))?;
        if entry.faded_at.is_none() {
            return Err(FadeError::NotFaded(entry_id.to_string()));
        }
        entry.faded_at = None;
        Ok(())
    }

    /// 按策略执行一轮主动遗忘。返回实测淘汰结果（不预设收益）。
    pub fn fade(&mut self, now: u64, policy: &FadePolicy) -> FadeReport {
        let mut report = FadeReport::default();
        // 第一遍：收集命中淘汰的 id（按分数升序，淘汰最低分优先）。
        let mut candidates: Vec<(String, f64)> = Vec::new();
        for entry in self.entries.values() {
            if entry.faded_at.is_some() {
                continue;
            }
            // Long 阶段保护：除非 force，否则跳过。
            if entry.stage == FadeStage::Long && policy.protect_long {
                report.skipped_long += 1;
                continue;
            }
            let score = entry.score(now);
            let mut evict = score < policy.min_score;
            if !evict {
                if let Some(ttl) = policy.no_access_ttl {
                    if entry.idle_secs(now) > ttl {
                        evict = true;
                    }
                }
            }
            if evict {
                candidates.push((entry.entry_id.clone(), score));
            }
        }
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // 第二遍：执行淘汰，统计实测释放字节。
        for (id, score) in &candidates {
            let removed = self.entries.remove(id);
            if let Some(entry) = removed {
                let mut faded_entry = entry.clone();
                faded_entry.faded_at = Some(now);
                let json_len = match serde_json::to_string(&faded_entry) {
                    Ok(s) => s.len(),
                    Err(e) => {
                        eprintln!("序列化淘汰条目失败（{id}: {e}），按 0 计");
                        0
                    }
                };
                report.faded.push(FadeOutcome {
                    entry_id: id.clone(),
                    stage: faded_entry.stage,
                    score_at_fade: *score,
                    freed_bytes: json_len,
                });
                report.total_freed_bytes += json_len;
                // 淘汰后仍保留在库中（软标记），便于 restore 与 stats。
                self.entries.insert(id.clone(), faded_entry);
            }
        }

        // 第三遍：超出活跃上限时，按分数升序继续淘汰至达标。
        if policy.max_active > 0 {
            let mut active: Vec<(String, f64)> = self
                .entries
                .values()
                .filter(|e| e.faded_at.is_none())
                .filter(|e| !(e.stage == FadeStage::Long && policy.protect_long))
                .map(|e| (e.entry_id.clone(), e.score(now)))
                .collect();
            active.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            while active.len() > policy.max_active {
                let Some((id, score)) = active.pop() else {
                    break;
                };
                let removed = self.entries.remove(&id);
                if let Some(entry) = removed {
                    let mut faded_entry = entry.clone();
                    faded_entry.faded_at = Some(now);
                    let json_len = match serde_json::to_string(&faded_entry) {
                        Ok(s) => s.len(),
                        Err(e) => {
                            eprintln!("序列化淘汰条目失败（{id}: {e}），按 0 计");
                            0
                        }
                    };
                    report.faded.push(FadeOutcome {
                        entry_id: id.clone(),
                        stage: faded_entry.stage,
                        score_at_fade: score,
                        freed_bytes: json_len,
                    });
                    report.total_freed_bytes += json_len;
                    self.entries.insert(id.clone(), faded_entry);
                }
            }
        }
        report
    }

    /// 统计（活跃/已淘汰/实测释放字节）。
    pub fn stats(&self) -> FadeStats {
        let mut stats = FadeStats::default();
        for entry in self.entries.values() {
            match entry.faded_at {
                Some(_) => stats.faded_count += 1,
                None => stats.active_count += 1,
            }
        }
        for entry in self.entries.values() {
            if entry.faded_at.is_some() {
                let json_len = serde_json::to_string(entry).map(|s| s.len()).unwrap_or(0);
                stats.freed_bytes += json_len;
            }
        }
        stats
    }

    pub fn save(&self) -> Result<(), FadeError> {
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| FadeError::StoreIo("未绑定路径".to_string()))?;
        let tmp = format!("{path}.tmp");
        let mut out = File::create(&tmp)
            .map_err(|e| FadeError::StoreIo(format!("创建临时文件 {tmp}: {e}")))?;
        for entry in self.entries.values() {
            let line =
                serde_json::to_string(entry).map_err(|e| FadeError::StoreJson(e.to_string()))?;
            writeln!(out, "{line}").map_err(|e| FadeError::StoreIo(format!("写入 {tmp}: {e}")))?;
        }
        out.flush()
            .map_err(|e| FadeError::StoreIo(format!("flush {tmp}: {e}")))?;
        fs::rename(&tmp, path).map_err(|e| FadeError::StoreIo(format!("替换 {path}: {e}")))
    }

    pub fn save_to(&self, path: &Path) -> Result<(), FadeError> {
        let mut out =
            File::create(path).map_err(|e| FadeError::StoreIo(format!("创建 {path:?}: {e}")))?;
        for entry in self.entries.values() {
            let line =
                serde_json::to_string(entry).map_err(|e| FadeError::StoreJson(e.to_string()))?;
            writeln!(out, "{line}")
                .map_err(|e| FadeError::StoreIo(format!("写入 {path:?}: {e}")))?;
        }
        out.flush()
            .map_err(|e| FadeError::StoreIo(format!("flush {path:?}: {e}")))
    }
}

/// 统计输出。
#[derive(Debug, Clone, Default)]
pub struct FadeStats {
    pub active_count: usize,
    pub faded_count: usize,
    /// 已淘汰条目按 JSON 序列化长度实测的释放字节合计。
    pub freed_bytes: usize,
}

impl FadeStats {
    /// 总数。
    pub fn total(&self) -> usize {
        self.active_count + self.faded_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn entry(id: &str, importance: u8, half_life: u64, at: u64) -> FadeEntry {
        FadeEntry {
            entry_id: id.to_string(),
            stage: FadeStage::Short,
            importance,
            half_life_secs: half_life,
            registered_at: at,
            last_access_at: at,
            access_count: 0,
            faded_at: None,
        }
    }

    #[test]
    fn register_requires_valid_importance() {
        let mut s = FadeStore::default();
        assert!(matches!(
            s.register("e1", FadeStage::Short, 0, 100, 1000),
            Err(FadeError::InvalidImportance(0))
        ));
        assert!(matches!(
            s.register("e2", FadeStage::Short, 6, 100, 1000),
            Err(FadeError::InvalidImportance(6))
        ));
    }

    #[test]
    fn register_requires_positive_half_life() {
        let mut s = FadeStore::default();
        assert!(matches!(
            s.register("e1", FadeStage::Mid, 3, 0, 1000),
            Err(FadeError::InvalidHalfLife(0))
        ));
    }

    #[test]
    fn register_rejects_empty_id() {
        let mut s = FadeStore::default();
        assert!(matches!(
            s.register("  ", FadeStage::Short, 3, 100, 1000),
            Err(FadeError::EmptyId)
        ));
    }

    #[test]
    fn score_decays_over_time() {
        let e = entry("e1", 4, 100, 1000);
        let s0 = e.score(1000);
        let s100 = e.score(1100);
        let s200 = e.score(1200);
        assert!((s0 - 4.0).abs() < 1e-9);
        assert!(s100 < s0);
        assert!(s200 < s100);
    }

    #[test]
    fn score_halves_after_half_life() {
        let e = entry("e1", 4, 100, 1000);
        let s0 = e.score(1000);
        let s100 = e.score(1100);
        assert!((s100 - s0 / 2.0).abs() < 1e-9);
    }

    #[test]
    fn touch_refreshes_score() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 3, 10, 1000).unwrap();
        // 1000 → 1050：半个半衰期，分下降。
        let before = s.get("e1").unwrap().score(1050);
        s.touch("e1", 1050).unwrap();
        // touch 后 last_access=1050，同一时刻分回到满值。
        let after = s.get("e1").unwrap().score(1050);
        assert!(after > before);
        assert!((after - 3.0).abs() < 1e-9);
    }

    #[test]
    fn touch_increments_access_count() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 3, 100, 1000).unwrap();
        s.touch("e1", 1100).unwrap();
        s.touch("e1", 1200).unwrap();
        assert_eq!(s.get("e1").unwrap().access_count, 2);
    }

    #[test]
    fn touch_rejects_faded_entry() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 1, 100, 1000).unwrap();
        let policy = FadePolicy {
            min_score: 5.0,
            max_active: 0,
            no_access_ttl: None,
            protect_long: true,
        };
        let report = s.fade(1000, &policy);
        assert_eq!(report.freed_count(), 1);
        assert!(matches!(
            s.touch("e1", 1001),
            Err(FadeError::AlreadyFaded(_))
        ));
    }

    #[test]
    fn fade_below_min_score() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 1, 100, 1000).unwrap();
        // 200 秒后分 ≈ 1 * 0.5^2 = 0.25，仍高于默认 0.05 → 不淘汰。
        let policy = FadePolicy::default();
        let report = s.fade(1200, &policy);
        assert_eq!(report.freed_count(), 0);
        // 800 秒后分 ≈ 1 * 0.5^8 = 0.0039 < 0.05 → 淘汰。
        let report2 = s.fade(1800, &policy);
        assert_eq!(report2.freed_count(), 1);
        assert_eq!(report2.total_freed_bytes, report2.faded[0].freed_bytes);
    }

    #[test]
    fn fade_keeps_above_min_score() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 5, 1000, 1000).unwrap();
        let policy = FadePolicy::default();
        // 100 秒后分 ≈ 5 * 0.5^0.1 ≈ 4.66 > 0.05。
        let report = s.fade(1100, &policy);
        assert_eq!(report.freed_count(), 0);
    }

    #[test]
    fn no_access_ttl_evicts_stale() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 5, 10000, 1000).unwrap();
        let policy = FadePolicy {
            min_score: 0.0,
            max_active: 0,
            no_access_ttl: Some(60),
            protect_long: true,
        };
        // 30 秒未访问：不淘汰。
        let r1 = s.fade(1030, &policy);
        assert_eq!(r1.freed_count(), 0);
        // 61 秒未访问：淘汰。
        let r2 = s.fade(1061, &policy);
        assert_eq!(r2.freed_count(), 1);
    }

    #[test]
    fn max_active_evicts_lowest_score() {
        let mut s = FadeStore::default();
        s.register("hot", FadeStage::Short, 5, 10000, 1000).unwrap();
        s.register("cold", FadeStage::Short, 1, 100, 1000).unwrap();
        s.register("tepid", FadeStage::Short, 3, 100, 1000).unwrap();
        let policy = FadePolicy {
            min_score: 0.0,
            max_active: 2,
            no_access_ttl: None,
            protect_long: true,
        };
        let report = s.fade(1500, &policy);
        // 淘汰 1 条（1500 时 cold 分最低）。
        assert_eq!(report.freed_count(), 1);
        assert_eq!(report.faded[0].entry_id, "cold");
        assert_eq!(s.stats().active_count, 2);
    }

    #[test]
    fn long_stage_protected_unless_force() {
        let mut s = FadeStore::default();
        s.register("long1", FadeStage::Long, 1, 10, 1000).unwrap();
        // 默认 protect_long=true：即便分数极低也不淘汰。
        let policy = FadePolicy {
            min_score: 100.0,
            max_active: 0,
            no_access_ttl: None,
            protect_long: true,
        };
        let report = s.fade(2000, &policy);
        assert_eq!(report.freed_count(), 0);
        assert_eq!(report.skipped_long, 1);
        // force（protect_long=false）：可淘汰。
        let policy2 = FadePolicy {
            min_score: 100.0,
            max_active: 0,
            no_access_ttl: None,
            protect_long: false,
        };
        let report2 = s.fade(2000, &policy2);
        assert_eq!(report2.freed_count(), 1);
    }

    #[test]
    fn restore_reactivates() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 1, 100, 1000).unwrap();
        let policy = FadePolicy {
            min_score: 5.0,
            max_active: 0,
            no_access_ttl: None,
            protect_long: true,
        };
        s.fade(1000, &policy);
        assert_eq!(s.stats().faded_count, 1);
        s.restore("e1").unwrap();
        assert_eq!(s.stats().faded_count, 0);
        assert_eq!(s.stats().active_count, 1);
    }

    #[test]
    fn restore_rejects_active_entry() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 3, 100, 1000).unwrap();
        assert!(matches!(s.restore("e1"), Err(FadeError::NotFaded(_))));
    }

    #[test]
    fn stats_measures_real_bytes() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 1, 100, 1000).unwrap();
        s.register("e2", FadeStage::Mid, 1, 100, 1000).unwrap();
        let policy = FadePolicy {
            min_score: 5.0,
            max_active: 0,
            no_access_ttl: None,
            protect_long: true,
        };
        let report = s.fade(1000, &policy);
        assert_eq!(report.freed_count(), 2);
        let stats = s.stats();
        assert_eq!(stats.faded_count, 2);
        // 实测释放字节 = 两个淘汰条目 JSON 长度之和。
        let e1_len = serde_json::to_string(s.get("e1").unwrap()).unwrap().len();
        let e2_len = serde_json::to_string(s.get("e2").unwrap()).unwrap().len();
        assert_eq!(stats.freed_bytes, e1_len + e2_len);
        assert_eq!(stats.freed_bytes, report.total_freed_bytes);
    }

    #[test]
    fn stats_counts_active_and_faded() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 3, 100, 1000).unwrap();
        s.register("e2", FadeStage::Mid, 3, 100, 1000).unwrap();
        let policy = FadePolicy {
            min_score: 5.0,
            max_active: 0,
            no_access_ttl: None,
            protect_long: true,
        };
        s.fade(1000, &policy);
        let stats = s.stats();
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.faded_count, 2);
        assert_eq!(stats.active_count, 0);
    }

    #[test]
    fn store_dedupe_by_id() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 3, 100, 1000).unwrap();
        s.register("e1", FadeStage::Mid, 4, 200, 2000).unwrap();
        assert_eq!(s.entries().len(), 1);
        assert_eq!(s.get("e1").unwrap().importance, 4);
        assert_eq!(s.get("e1").unwrap().half_life_secs, 200);
    }

    #[test]
    fn store_jsonl_save_open_roundtrip() {
        let dir = std::env::temp_dir().join(format!("fade_roundtrip_{}", now()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fade.jsonl");
        {
            let mut s = FadeStore::open(&path).unwrap();
            s.register("e1", FadeStage::Short, 3, 100, 1000).unwrap();
            s.register("e2", FadeStage::Long, 5, 200, 2000).unwrap();
            s.save_to(&path).unwrap();
        }
        {
            let s = FadeStore::open(&path).unwrap();
            assert_eq!(s.entries().len(), 2);
            assert_eq!(s.get("e1").unwrap().importance, 3);
            assert_eq!(s.get("e2").unwrap().stage, FadeStage::Long);
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_corrupt_line_skipped() {
        let dir = std::env::temp_dir().join(format!("fade_corrupt_{}", now()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fade.jsonl");
        fs::write(&path, "not-json\n").unwrap();
        let s = FadeStore::open(&path).unwrap();
        assert_eq!(s.entries().len(), 0);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fade_report_zero_when_nothing_to_fade() {
        let mut s = FadeStore::default();
        s.register("e1", FadeStage::Short, 5, 100000, 1000).unwrap();
        let policy = FadePolicy::default();
        let report = s.fade(1001, &policy);
        assert_eq!(report.freed_count(), 0);
        assert_eq!(report.total_freed_bytes, 0);
        assert_eq!(report.skipped_long, 0);
    }

    #[test]
    fn importance_bounds_1_to_5() {
        let mut s = FadeStore::default();
        for imp in 1..=5 {
            let id = format!("e{imp}");
            s.register(&id, FadeStage::Short, imp, 100, 1000).unwrap();
            assert_eq!(s.get(&id).unwrap().importance, imp);
        }
        assert!(matches!(
            s.register("bad", FadeStage::Short, 0, 100, 1000),
            Err(FadeError::InvalidImportance(0))
        ));
        assert!(matches!(
            s.register("bad2", FadeStage::Short, 7, 100, 1000),
            Err(FadeError::InvalidImportance(7))
        ));
    }

    #[test]
    fn stage_label_and_parse() {
        assert_eq!(FadeStage::Short.label(), "短期");
        assert_eq!(FadeStage::Mid.label(), "中期");
        assert_eq!(FadeStage::Long.label(), "长期");
        assert_eq!(FadeStage::parse("short"), Some(FadeStage::Short));
        assert_eq!(FadeStage::parse("MID"), Some(FadeStage::Mid));
        assert_eq!(FadeStage::parse("Long"), Some(FadeStage::Long));
        assert_eq!(FadeStage::parse("archive"), None);
    }
}
