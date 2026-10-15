//! v5.1.6：记忆巩固三阶段（v5-memory-roadmap §二）。
//!
//! 对齐"短期 → 中期 → 长期"的记忆巩固：
//! - **短期（Short）**：带 TTL 的易失记忆，到期后触发巩固；
//! - **中期（Mid）**：短期到期后聚合为快照（保留来源 refs + 摘要内容）；
//! - **长期（Long）**：中期再巩固为分布式记忆，以 **Merkle-SHA256 根**锚定
//!   整个来源集合（每片叶子=一个源条目 id），篡改/缺叶 fail-closed 可检。
//!
//! 边界（有可验证的测试）：
//! - 巩固只产生新条目（`source_refs` 指向旧条目），**源条目不可变**
//!   （沿用 v5.1.4 remix / v5.1.5 envtrace 的"只产生新锚点"哲学）；
//! - 阶段只能前进：Short→Mid→Long，同级/降级拒绝（`StageNotAdvancing`）；
//! - Short 必须带 TTL，TTL 未到期且未 force 时拒绝巩固（`TtlNotExpired`）；
//! - Long 必须携带 Merkle-SHA256 根，根与来源集合重算不符拒绝（`MerkleMismatch`）；
//! - 空内容 / 空来源集合 / 空巩固者均为 fail-closed 拒绝。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::memory::fleet::now_iso;
use crate::memory::layered::sha256_hex;

/// 记忆巩固阶段（只允许向前推进）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// 短期：带 TTL 的易失记忆。
    Short,
    /// 中期：短期到期后形成的快照。
    Mid,
    /// 长期：分布式记忆，Merkle-SHA256 锚定来源集合。
    Long,
}

impl Stage {
    /// 阶段标签（持久化用）。
    pub fn label(&self) -> &'static str {
        match self {
            Stage::Short => "short",
            Stage::Mid => "mid",
            Stage::Long => "long",
        }
    }

    /// 从标签解析；未知标签返回 None（fail-closed）。
    pub fn parse(s: &str) -> Option<Stage> {
        match s {
            "short" => Some(Stage::Short),
            "mid" => Some(Stage::Mid),
            "long" => Some(Stage::Long),
            _ => None,
        }
    }

    /// 是否允许从 `other` 推进到当前阶段（只允许升阶）。
    fn advances_from(&self, other: Stage) -> bool {
        matches!(
            (other, self),
            (Stage::Short, Stage::Mid) | (Stage::Mid, Stage::Long)
        )
    }
}

/// 记忆巩固条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationEntry {
    /// 条目 id：SHA-256(stage|sources|content|by[|merkle])，确定性锚点。
    pub entry_id: String,
    /// 当前巩固阶段。
    pub stage: Stage,
    /// 被巩固的源条目 refs（保序；最短为源条目自身的 entry_id）。
    pub source_refs: Vec<String>,
    /// 巩固内容（原文或摘要）。
    pub content: String,
    /// 短期 TTL（秒）；中期/长期为 None（长期无 TTL）。
    pub ttl_secs: Option<u64>,
    /// 长期阶段的 Merkle-SHA256 根（仅 Long 有）。
    pub merkle_root: Option<String>,
    /// 巩固者（agent id）。
    pub consolidated_by: String,
    /// 创建时间（UTC Unix 秒字符串，与 fleet/svaf/remix 一致）。
    pub created_at: String,
}

impl ConsolidationEntry {
    /// 短期条目：必须携带 TTL。
    pub fn new_short(content: &str, by: &str, ttl_secs: u64) -> Result<Self, ConsolidationError> {
        if content.trim().is_empty() {
            return Err(ConsolidationError::EmptyContent);
        }
        if by.trim().is_empty() {
            return Err(ConsolidationError::EmptyConsolidator);
        }
        if ttl_secs == 0 {
            return Err(ConsolidationError::ZeroTtl);
        }
        let sources = vec![];
        let entry_id = entry_hash(Stage::Short, &sources, content, by, None);
        Ok(ConsolidationEntry {
            entry_id,
            stage: Stage::Short,
            source_refs: sources,
            content: content.to_string(),
            ttl_secs: Some(ttl_secs),
            merkle_root: None,
            consolidated_by: by.to_string(),
            created_at: now_iso(),
        })
    }

    /// 中期条目：由巩固函数构造（sources 已合并、非空）。
    fn new_mid(
        source_refs: Vec<String>,
        content: &str,
        by: &str,
    ) -> Result<Self, ConsolidationError> {
        if source_refs.is_empty() {
            return Err(ConsolidationError::EmptySourceRefs);
        }
        if content.trim().is_empty() {
            return Err(ConsolidationError::EmptyContent);
        }
        if by.trim().is_empty() {
            return Err(ConsolidationError::EmptyConsolidator);
        }
        let entry_id = entry_hash(Stage::Mid, &source_refs, content, by, None);
        Ok(ConsolidationEntry {
            entry_id,
            stage: Stage::Mid,
            source_refs,
            content: content.to_string(),
            ttl_secs: None,
            merkle_root: None,
            consolidated_by: by.to_string(),
            created_at: now_iso(),
        })
    }

    /// 长期条目：必须携带与来源集合重算一致的 Merkle-SHA256 根。
    fn new_long(
        source_refs: Vec<String>,
        content: &str,
        by: &str,
    ) -> Result<Self, ConsolidationError> {
        if source_refs.is_empty() {
            return Err(ConsolidationError::EmptySourceRefs);
        }
        if content.trim().is_empty() {
            return Err(ConsolidationError::EmptyContent);
        }
        if by.trim().is_empty() {
            return Err(ConsolidationError::EmptyConsolidator);
        }
        let merkle = merkle_sha256(&source_refs)?;
        let entry_id = entry_hash(Stage::Long, &source_refs, content, by, Some(&merkle));
        Ok(ConsolidationEntry {
            entry_id,
            stage: Stage::Long,
            source_refs,
            content: content.to_string(),
            ttl_secs: None,
            merkle_root: Some(merkle),
            consolidated_by: by.to_string(),
            created_at: now_iso(),
        })
    }

    /// 长期条目完整性校验：重算 Merkle-SHA256 根并比对（fail-closed）。
    pub fn verify_merkle(&self) -> bool {
        if self.stage != Stage::Long {
            return true;
        }
        let Some(claimed) = &self.merkle_root else {
            return false;
        };
        match merkle_sha256(&self.source_refs) {
            Ok(recomputed) => &recomputed == claimed,
            Err(_) => false,
        }
    }

    /// 简写 id（前端展示用，前 16 字符）。
    pub fn short_id(&self) -> String {
        self.entry_id.chars().take(16).collect()
    }
}

/// Merkle-SHA256 根：叶子为源条目 id 字符串，逐层配对合并，
/// 奇数叶子复制最后一个（标准 Merkle 补齐），根为最终单值。
pub fn merkle_sha256(leaves: &[String]) -> Result<String, ConsolidationError> {
    if leaves.is_empty() {
        return Err(ConsolidationError::EmptySourceRefs);
    }
    let mut level: Vec<String> = leaves.iter().map(|leaf| sha256_hex(leaf)).collect();
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            // level.len() > 1 ⇒ 末尾索引必然存在，无需 expect。
            let last_idx = level.len() - 1;
            let last = level[last_idx].clone();
            level.push(last);
        }
        let mut next = Vec::with_capacity(level.len() / 2);
        let mut i = 0;
        while i < level.len() {
            let a = &level[i];
            let b = &level[i + 1];
            let mut combined = String::with_capacity(a.len() + b.len());
            combined.push_str(a);
            combined.push_str(b);
            next.push(sha256_hex(&combined));
            i += 2;
        }
        level = next;
    }
    level.pop().ok_or(ConsolidationError::EmptySourceRefs)
}

/// 条目 id：SHA-256(stage|sources|content|by|merkle)。
fn entry_hash(
    stage: Stage,
    sources: &[String],
    content: &str,
    by: &str,
    merkle: Option<&str>,
) -> String {
    let mut acc = String::new();
    acc.push_str(stage.label());
    acc.push('|');
    acc.push_str(&sources.join(","));
    acc.push('|');
    acc.push_str(content);
    acc.push('|');
    acc.push_str(by);
    acc.push('|');
    if let Some(m) = merkle {
        acc.push_str(m);
    }
    sha256_hex(&acc)
}

/// 合并来源 refs（保序去重；源条目自身 id 追加在现有 refs 之后）。
fn merge_sources(existing: &[String], own_id: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(existing.len() + 1);
    for r in existing {
        if !out.contains(r) {
            out.push(r.clone());
        }
    }
    if !out.iter().any(|r| r == own_id) {
        out.push(own_id.to_string());
    }
    out
}

/// 巩固错误（fail-closed）。
#[derive(Debug)]
pub enum ConsolidationError {
    /// 内容为空（防漏传）。
    EmptyContent,
    /// 来源集合为空（巩固必须有来源）。
    EmptySourceRefs,
    /// 巩固者为空。
    EmptyConsolidator,
    /// 短期条目必须带 TTL；TTL 为 0 无意义。
    ZeroTtl,
    /// 短期条目缺少 TTL（无法到期巩固）。
    ShortRequiresTtl,
    /// TTL 未到期（非 force 拒绝巩固）。
    TtlNotExpired {
        /// 条目 TTL（秒）。
        ttl_secs: u64,
        /// 条目创建时间（秒）。
        created_at: u64,
        /// 当前时间（秒）。
        now: u64,
    },
    /// 阶段未推进（同级/降级拒绝）。
    StageNotAdvancing { from: Stage, to: Stage },
    /// 提供的 Merkle 根与来源集合重算不符。
    MerkleMismatch { claimed: String, recomputed: String },
    /// 创建时间无法解析为秒。
    InvalidCreatedAt(String),
    /// 文件/持久化 IO 错误。
    Io(String),
    /// 持久化行 JSON 解析错误。
    Json(String),
}

impl std::fmt::Display for ConsolidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsolidationError::EmptyContent => write!(f, "内容为空（fail-closed）"),
            ConsolidationError::EmptySourceRefs => write!(f, "来源集合为空（fail-closed）"),
            ConsolidationError::EmptyConsolidator => write!(f, "巩固者为空（fail-closed）"),
            ConsolidationError::ZeroTtl => write!(f, "短期 TTL 必须大于 0"),
            ConsolidationError::ShortRequiresTtl => write!(f, "短期条目必须携带 TTL"),
            ConsolidationError::TtlNotExpired {
                ttl_secs,
                created_at,
                now,
            } => write!(
                f,
                "TTL 未到期（ttl={ttl_secs}s, created={created_at}, now={now}，非 force 拒绝）"
            ),
            ConsolidationError::StageNotAdvancing { from, to } => {
                write!(
                    f,
                    "巩固阶段未推进（{from:?}→{to:?}，只允许 Short→Mid→Long）"
                )
            }
            ConsolidationError::MerkleMismatch {
                claimed,
                recomputed,
            } => {
                write!(
                    f,
                    "Merkle 根不符（claimed={claimed}, recomputed={recomputed}）"
                )
            }
            ConsolidationError::InvalidCreatedAt(s) => write!(f, "创建时间无法解析: {s}"),
            ConsolidationError::Io(e) => write!(f, "IO 错误: {e}"),
            ConsolidationError::Json(e) => write!(f, "JSON 解析错误: {e}"),
        }
    }
}

/// 短期 → 中期巩固：TTL 到期（或 force）后，将短期条目聚合为中期快照。
///
/// 源条目不可变：新条目 `source_refs` = 源条目既有 refs + 源条目自身 id。
pub fn consolidate_short_to_mid(
    short: &ConsolidationEntry,
    content: &str,
    by: &str,
    now_secs: u64,
    force: bool,
) -> Result<ConsolidationEntry, ConsolidationError> {
    if !Stage::Mid.advances_from(short.stage) {
        return Err(ConsolidationError::StageNotAdvancing {
            from: short.stage,
            to: Stage::Mid,
        });
    }
    let ttl_secs = short.ttl_secs.ok_or(ConsolidationError::ShortRequiresTtl)?;
    let created_at = short
        .created_at
        .parse::<u64>()
        .map_err(|_| ConsolidationError::InvalidCreatedAt(short.created_at.clone()))?;
    if !force && now_secs < created_at + ttl_secs {
        return Err(ConsolidationError::TtlNotExpired {
            ttl_secs,
            created_at,
            now: now_secs,
        });
    }
    let sources = merge_sources(&short.source_refs, &short.entry_id);
    ConsolidationEntry::new_mid(sources, content, by)
}

/// 中期 → 长期巩固：为来源集合生成 Merkle-SHA256 根，形成分布式长期记忆。
pub fn consolidate_mid_to_long(
    mid: &ConsolidationEntry,
    by: &str,
) -> Result<ConsolidationEntry, ConsolidationError> {
    if !Stage::Long.advances_from(mid.stage) {
        return Err(ConsolidationError::StageNotAdvancing {
            from: mid.stage,
            to: Stage::Long,
        });
    }
    let sources = merge_sources(&mid.source_refs, &mid.entry_id);
    ConsolidationEntry::new_long(sources, &mid.content, by)
}

/// 记忆巩固存储（JSONL，逐行重放，按 entry_id 前缀去重）。
#[derive(Debug, Clone)]
pub struct ConsolidationStore {
    path: PathBuf,
    entries: Vec<ConsolidationEntry>,
}

impl ConsolidationStore {
    /// 打开（或创建）巩固库：逐行重放已有条目，损坏行跳过。
    pub fn open(path: &Path) -> Result<Self, ConsolidationError> {
        let mut entries = Vec::new();
        if path.exists() {
            let data =
                std::fs::read_to_string(path).map_err(|e| ConsolidationError::Io(e.to_string()))?;
            for line in data.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<ConsolidationEntry>(line) {
                    entries.push(entry);
                }
            }
        }
        Ok(ConsolidationStore {
            path: path.to_path_buf(),
            entries,
        })
    }

    /// 当前所有条目。
    pub fn entries(&self) -> &[ConsolidationEntry] {
        &self.entries
    }

    /// 按完整 id 或前缀查找。
    pub fn by_id(&self, prefix: &str) -> Option<&ConsolidationEntry> {
        self.entries.iter().find(|e| e.entry_id.starts_with(prefix))
    }

    /// 添加条目（按 entry_id 去重）；已存在返回 false。
    pub fn add(&mut self, entry: ConsolidationEntry) -> bool {
        if self.by_id(&entry.entry_id).is_some() {
            return false;
        }
        self.entries.push(entry);
        true
    }

    /// 落盘（覆盖写入当前路径）。
    pub fn save(&self) -> Result<(), ConsolidationError> {
        self.save_to(&self.path)
    }

    /// 落盘到指定路径。
    pub fn save_to(&self, path: &Path) -> Result<(), ConsolidationError> {
        let mut out = String::new();
        for e in &self.entries {
            let line =
                serde_json::to_string(e).map_err(|e| ConsolidationError::Json(e.to_string()))?;
            out.push_str(&line);
            out.push('\n');
        }
        std::fs::write(path, out).map_err(|e| ConsolidationError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn short_entry(content: &str, by: &str, ttl: u64) -> ConsolidationEntry {
        ConsolidationEntry::new_short(content, by, ttl).expect("短期条目应可构造")
    }

    /// TTL 已到期后的当前秒（= created + ttl + 1），与真实时钟无关。
    fn expire_now(e: &ConsolidationEntry) -> u64 {
        let created: u64 = e.created_at.parse().expect("created_at 应为秒");
        created + e.ttl_secs.expect("短期应有 TTL") + 1
    }

    #[test]
    fn stage_label_roundtrip() {
        for s in [Stage::Short, Stage::Mid, Stage::Long] {
            assert_eq!(Stage::parse(s.label()), Some(s));
        }
        assert_eq!(Stage::parse("unknown"), None);
        assert_eq!(Stage::parse(""), None);
    }

    #[test]
    fn short_entry_requires_ttl_and_content() {
        let e = short_entry("短期记忆", "alice", 3600);
        assert_eq!(e.stage, Stage::Short);
        assert_eq!(e.ttl_secs, Some(3600));
        assert!(ConsolidationEntry::new_short("", "alice", 3600).is_err());
        assert!(ConsolidationEntry::new_short("   ", "alice", 3600).is_err());
        assert!(ConsolidationEntry::new_short("x", "", 3600).is_err());
        assert!(ConsolidationEntry::new_short("x", "alice", 0).is_err());
    }

    #[test]
    fn empty_source_refs_rejected() {
        assert!(ConsolidationEntry::new_mid(vec![], "x", "alice").is_err());
        assert!(ConsolidationEntry::new_long(vec![], "x", "alice").is_err());
        assert!(merkle_sha256(&[]).is_err());
    }

    #[test]
    fn consolidate_short_to_mid_advances_stage() {
        let s = short_entry("短期任务记录", "alice", 3600);
        let m = consolidate_short_to_mid(&s, "中期快照摘要", "alice", expire_now(&s), false)
            .expect("TTL 已到期应可巩固");
        assert_eq!(m.stage, Stage::Mid);
        assert_eq!(m.ttl_secs, None);
        assert!(m.source_refs.contains(&s.entry_id));
        // 源条目不可变：原条目仍是 Short
        assert_eq!(s.stage, Stage::Short);
    }

    #[test]
    fn ttl_not_expired_rejected_unless_force() {
        let s = short_entry("未到期记忆", "alice", 3600);
        let created: u64 = s.created_at.parse().expect("秒");
        let err = consolidate_short_to_mid(&s, "摘要", "alice", created + 100, false)
            .expect_err("TTL 未到期应被拒");
        assert!(matches!(err, ConsolidationError::TtlNotExpired { .. }));
        // force 绕过
        let m = consolidate_short_to_mid(&s, "摘要", "alice", created + 100, true)
            .expect("force 应绕过 TTL");
        assert_eq!(m.stage, Stage::Mid);
    }

    #[test]
    fn ttl_just_expired_allows() {
        let s = short_entry("刚好到期", "bob", 3600);
        let m = consolidate_short_to_mid(&s, "摘要", "bob", expire_now(&s) - 1, false)
            .expect("now == created+ttl 视为到期");
        assert_eq!(m.stage, Stage::Mid);
    }

    #[test]
    fn short_requires_ttl_field() {
        // 构造一个无 TTL 的短期条目（绕过构造器，模拟坏数据）
        let s = short_entry("x", "alice", 3600);
        let mut broken = s.clone();
        broken.ttl_secs = None;
        let err = consolidate_short_to_mid(&broken, "摘要", "alice", 0, false)
            .expect_err("无 TTL 短期条目应被拒");
        assert!(matches!(err, ConsolidationError::ShortRequiresTtl));
    }

    #[test]
    fn stage_not_advancing_rejected() {
        // Short→Short 不允许（应通过 Short→Mid 构造后不能再次 Short→Mid 之外）
        let s = short_entry("x", "alice", 3600);
        // 用 Short 条目直接做 Mid→Long 巩固：from=Short, to=Long 不允许
        let err = consolidate_mid_to_long(&s, "alice").expect_err("Short 不能直接到 Long");
        assert!(matches!(err, ConsolidationError::StageNotAdvancing { .. }));
        // Mid 条目不能再走 Short→Mid
        let m = consolidate_short_to_mid(&s, "摘要", "alice", expire_now(&s), false).unwrap();
        let err = consolidate_short_to_mid(&m, "再次摘要", "alice", 0, false)
            .expect_err("Mid 不能再 Short→Mid");
        assert!(matches!(err, ConsolidationError::StageNotAdvancing { .. }));
    }

    #[test]
    fn mid_to_long_builds_merkle() {
        let s = short_entry("短期", "alice", 3600);
        let m = consolidate_short_to_mid(&s, "中期摘要", "alice", expire_now(&s), false).unwrap();
        let l = consolidate_mid_to_long(&m, "alice").expect("Mid→Long 应成功");
        assert_eq!(l.stage, Stage::Long);
        assert!(l.merkle_root.is_some());
        assert!(l.verify_merkle());
        // 长期来源包含中期 id 与短期 id（链式）
        assert!(l.source_refs.contains(&m.entry_id));
        assert!(l.source_refs.contains(&s.entry_id));
    }

    #[test]
    fn merkle_deterministic_same_root() {
        let leaves = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let r1 = merkle_sha256(&leaves).unwrap();
        let r2 = merkle_sha256(&leaves).unwrap();
        assert_eq!(r1, r2);
        assert_eq!(r1.len(), 64);
    }

    #[test]
    fn merkle_detects_tampered_leaf() {
        let leaves = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let root = merkle_sha256(&leaves).unwrap();
        let tampered = vec!["a".to_string(), "b".to_string(), "X".to_string()];
        let root2 = merkle_sha256(&tampered).unwrap();
        assert_ne!(root, root2);
    }

    #[test]
    fn merkle_odd_leaf_duplicates_last() {
        // 3 片叶子：level=[h(a),h(b),h(c)] → 复制 h(c) → [h(a),h(b),h(c),h(c)]
        // → [h(h(a)||h(b)), h(h(c)||h(c))] → 根
        let leaves3 = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let r3 = merkle_sha256(&leaves3).unwrap();
        // 与手工构造的 4 片（复制 c）同根
        let leaves4 = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "c".to_string(),
        ];
        let r4 = merkle_sha256(&leaves4).unwrap();
        assert_eq!(r3, r4);
    }

    #[test]
    fn verify_merkle_fails_on_tampered_root() {
        let s = short_entry("短期", "alice", 3600);
        let m = consolidate_short_to_mid(&s, "中期", "alice", expire_now(&s), false).unwrap();
        let l = consolidate_mid_to_long(&m, "alice").unwrap();
        assert!(l.verify_merkle());
        // 篡改来源集合后根不再匹配
        let mut tampered = l.clone();
        tampered.source_refs.push("fake-ref".to_string());
        assert!(!tampered.verify_merkle());
        // 非 Long 条目 verify 恒真
        assert!(s.verify_merkle());
        assert!(m.verify_merkle());
    }

    #[test]
    fn long_content_kept_from_mid() {
        let s = short_entry("原始短期内容", "carol", 7200);
        let m = consolidate_short_to_mid(&s, "中期摘要", "carol", expire_now(&s), false).unwrap();
        let l = consolidate_mid_to_long(&m, "carol").unwrap();
        assert_eq!(l.content, "中期摘要");
        assert_eq!(l.consolidated_by, "carol");
    }

    #[test]
    fn entry_id_deterministic_same_input() {
        let s1 = short_entry("短期", "alice", 3600);
        let m1 = consolidate_short_to_mid(&s1, "摘要", "alice", expire_now(&s1), false).unwrap();
        let s2 = short_entry("短期", "alice", 3600);
        let m2 = consolidate_short_to_mid(&s2, "摘要", "alice", expire_now(&s2), false).unwrap();
        // 相同输入（同一来源 id + 同一内容 + 同一巩固者）→ 相同 entry_id
        assert_eq!(m1.entry_id, m2.entry_id);
        // 不同巩固者 → 不同 id（各自保留自己的巩固版本）
        let m3 = consolidate_short_to_mid(&s1, "摘要", "bob", expire_now(&s1), false).unwrap();
        assert_ne!(m1.entry_id, m3.entry_id);
    }

    #[test]
    fn multi_source_chain_immutable() {
        // 两条短期 → 各自中期 → 合并？当前语义：每条独立巩固成自己的链。
        // 验证同一条链上 source_refs 逐步累积（不可变来源哲学）。
        let s = short_entry("来源", "alice", 3600);
        let m = consolidate_short_to_mid(&s, "快照", "alice", expire_now(&s), false).unwrap();
        assert_eq!(m.source_refs, vec![s.entry_id.clone()]);
        let l = consolidate_mid_to_long(&m, "alice").unwrap();
        // long.source_refs = [s, m]
        assert_eq!(l.source_refs.len(), 2);
        assert_eq!(l.source_refs[0], s.entry_id);
        assert_eq!(l.source_refs[1], m.entry_id);
        // 原条目全部未被修改
        assert_eq!(s.stage, Stage::Short);
        assert_eq!(m.stage, Stage::Mid);
    }

    #[test]
    fn store_dedupe_by_id() {
        let dir = std::env::temp_dir().join("au-consolidation-dedupe-test");
        let path = dir.join("consolidation.jsonl");
        std::fs::create_dir_all(&dir).ok();
        let mut store = ConsolidationStore::open(&path).unwrap();
        let s = short_entry("短期", "alice", 3600);
        let m = consolidate_short_to_mid(&s, "摘要", "alice", expire_now(&s), false).unwrap();
        assert!(store.add(m.clone()));
        assert!(!store.add(m.clone()), "同 id 应去重");
        assert_eq!(store.entries().len(), 1);
        assert!(store.by_id(&m.short_id()).is_some());
        assert!(store.by_id("不存在的id").is_none());
    }

    #[test]
    fn store_jsonl_save_open_roundtrip() {
        let dir = std::env::temp_dir().join("au-consolidation-roundtrip-test");
        let path = dir.join("consolidation.jsonl");
        std::fs::create_dir_all(&dir).ok();
        let mut store = ConsolidationStore::open(&path).unwrap();
        let s = short_entry("短期", "alice", 3600);
        let m = consolidate_short_to_mid(&s, "摘要", "alice", expire_now(&s), false).unwrap();
        let l = consolidate_mid_to_long(&m, "alice").unwrap();
        store.add(s.clone());
        store.add(m.clone());
        store.add(l.clone());
        store.save().unwrap();

        let reopened = ConsolidationStore::open(&path).unwrap();
        assert_eq!(reopened.entries().len(), 3);
        let long = reopened.by_id(&l.short_id()).expect("长期条目应可找回");
        assert_eq!(long.stage, Stage::Long);
        assert!(long.verify_merkle());
        // 坏行跳过
        let mut data = std::fs::read_to_string(&path).unwrap();
        data.push_str("{\"broken\":true}\n");
        std::fs::write(&path, data).unwrap();
        let reopened2 = ConsolidationStore::open(&path).unwrap();
        assert_eq!(reopened2.entries().len(), 3, "坏行应被跳过");
        // 清理
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_corrupt_line_skipped() {
        let dir = std::env::temp_dir().join("au-consolidation-corrupt-test");
        let path = dir.join("consolidation.jsonl");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(&path, "not-json\n").unwrap();
        let store = ConsolidationStore::open(&path).unwrap();
        assert_eq!(store.entries().len(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
