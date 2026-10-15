//! 集体记忆基座（v5.1.0，v5-memory-roadmap §二）
//!
//! `fleet-memory.jsonl`：append-only JSONL，每行一个 [`FleetEntry`]，与个体记忆
//! `memory.jsonl` 并列，属 L3 集体记忆：**facts（事实）/ decisions（决策）/
//! open problems（未决问题）** 三类策展，按角色化读写访问。
//!
//! 权限模型（fail-closed，对齐蓝图 §4.1 数值权威简版）：
//! | 角色        | 值 | 可执行操作 |
//! |------------|----|-----------|
//! | Observer   | 0  | 读（list/status） |
//! | Contributor| 1  | 读 + 写（post） |
//! | Captain    | 2  | 读 + 写 + 策展（curate 归档/淘汰） |
//! | Admin      | 3  | 读 + 写 + 策展 + 管理（purge 清空） |
//!
//! 边界说明：
//! - `claim_hash = SHA-256(kind|author|content)`：后续 v5.1.3 lineage（血缘）、
//!   v5.1.4 remix（再混合）以它作内容锚点；本版本只保证确定性可复算。
//! - 容量治理（轮转/巩固/遗忘）在 v5.1.6-v5.1.7，本版本 `status` 仅如实报行数与字节。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 角色（数值权威模型简版）。serde 用 snake_case 便于 CLI 直传。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetAccess {
    /// 只读：可查看集体记忆。
    Observer = 0,
    /// 可读写：可发布事实/决策/未决问题。
    Contributor = 1,
    /// 可策展：可归档/淘汰条目。
    Captain = 2,
    /// 可管理：可清空集体记忆。
    Admin = 3,
}

impl FleetAccess {
    /// 从 CLI 字符串解析（未知 → None，调用方 fail-closed 拒绝）。
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "observer" => Some(Self::Observer),
            "contributor" => Some(Self::Contributor),
            "captain" => Some(Self::Captain),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }

    /// 数值权威层级。
    pub fn level(self) -> u8 {
        self as u8
    }
}

/// 策展分类：事实 / 决策 / 未决问题。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetKind {
    /// 已被验证或权威确认的事实性结论。
    Fact,
    /// 已拍板/已执行的决策。
    Decision,
    /// 尚未解决、等待后续输入的问题。
    OpenProblem,
}

impl FleetKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "fact" => Some(Self::Fact),
            "decision" => Some(Self::Decision),
            "open_problem" | "open-problem" | "problem" => Some(Self::OpenProblem),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::OpenProblem => "open_problem",
        }
    }
}

/// 条目生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetStatus {
    /// 在用（默认）。
    Active,
    /// 已被 Captain+ 策展归档（不再出现在默认 list）。
    Archived,
}

/// 一条集体记忆。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetEntry {
    pub kind: FleetKind,
    /// 发布者 agent_id。
    pub author: String,
    /// 发布时的角色（写入时校验，回读仅作审计记录）。
    pub author_role: FleetAccess,
    pub content: String,
    /// ISO8601 时间戳。
    pub ts: String,
    pub status: FleetStatus,
    /// SHA-256(kind|author|content)，lineage/remix 的内容锚点。
    pub claim_hash: String,
}

impl FleetEntry {
    pub fn new(kind: FleetKind, author: &str, author_role: FleetAccess, content: &str) -> Self {
        let claim_hash = fleet_claim_hash(kind, author, content);
        Self {
            kind,
            author: author.to_string(),
            author_role,
            content: content.to_string(),
            ts: now_iso(),
            status: FleetStatus::Active,
            claim_hash,
        }
    }
}

/// 内容锚点：SHA-256(kind|author|content) 的十六进制串。
/// 确定性可复算：同 kind+author+content 必得同 hash（v5.1.3 lineage 依赖此性质）。
pub fn fleet_claim_hash(kind: FleetKind, author: &str, content: &str) -> String {
    let mut h = Sha256::new();
    h.update(kind.as_str().as_bytes());
    h.update(b"|");
    h.update(author.as_bytes());
    h.update(b"|");
    h.update(content.as_bytes());
    hex_lower(&h.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// 集体记忆错误（fail-closed：权限不足即拒绝，不写盘）。
#[derive(Debug)]
pub enum FleetError {
    /// 权限不足：执行者角色 < 要求角色。
    Unauthorized {
        actor: FleetAccess,
        required: FleetAccess,
    },
    /// I/O 失败。
    Io(String),
    /// JSON 行解析失败（回读损坏行）。
    Json(String),
    /// 按 claim_hash 找不到 Active 条目。
    NotFound(String),
    /// 前缀匹配到多个 Active 条目（需完整 claim_hash 消歧）。
    Ambiguous(String),
    /// CAT7 块完整性校验失败（claim_hash 与内容不符，拒绝导入）。
    InvalidBlock(String),
}

impl fmt::Display for FleetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized { actor, required } => write!(
                f,
                "权限不足：角色 {actor:?} < 要求 {required:?}（fail-closed 拒绝）"
            ),
            Self::Io(e) => write!(f, "I/O 错误: {e}"),
            Self::Json(e) => write!(f, "JSON 解析错误: {e}"),
            Self::NotFound(h) => write!(f, "未找到活跃条目: {h}"),
            Self::Ambiguous(h) => write!(f, "前缀匹配到多个活跃条目（请用完整 claim_hash）: {h}"),
            Self::InvalidBlock(h) => {
                write!(f, "CAT7 块完整性校验失败（claim_hash 与内容不符）: {h}")
            }
        }
    }
}

impl std::error::Error for FleetError {}

impl From<std::io::Error> for FleetError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// 状态报告（status 用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetStatusReport {
    pub file: String,
    pub entries: usize,
    pub active: usize,
    pub archived: usize,
    pub facts: usize,
    pub decisions: usize,
    pub open_problems: usize,
    pub file_bytes: u64,
    /// 回读时跳过/计数的损坏行。
    pub corrupt_lines: usize,
}

/// 集体记忆存储：内存镜像 + append-only JSONL。
#[derive(Debug, Clone)]
pub struct FleetMemory {
    path: PathBuf,
    entries: Vec<FleetEntry>,
    corrupt_lines: usize,
}

impl FleetMemory {
    /// 打开（或创建）集体记忆库；回读既有行，损坏行计数不阻断。
    pub fn open(path: &Path) -> Result<Self, FleetError> {
        let mut entries = Vec::new();
        let mut corrupt_lines = 0usize;
        if path.exists() {
            let text = fs::read_to_string(path)?;
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<FleetEntry>(line) {
                    Ok(e) => entries.push(e),
                    Err(_) => corrupt_lines += 1,
                }
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            entries,
            corrupt_lines,
        })
    }

    /// 发布集体记忆。权限：Contributor+。写盘前做全部校验（fail-closed）。
    pub fn post(
        &mut self,
        actor: FleetAccess,
        kind: FleetKind,
        author: &str,
        content: &str,
    ) -> Result<FleetEntry, FleetError> {
        if actor < FleetAccess::Contributor {
            return Err(FleetError::Unauthorized {
                actor,
                required: FleetAccess::Contributor,
            });
        }
        let entry = FleetEntry::new(kind, author, actor, content);
        let line = serde_json::to_string(&entry).map_err(|e| FleetError::Json(e.to_string()))?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{line}")?;
        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// 策展归档：Captain+ 可将 Active 条目标记 Archived（不删除，保留审计）。
    pub fn curate(
        &mut self,
        actor: FleetAccess,
        claim_hash: &str,
    ) -> Result<FleetEntry, FleetError> {
        if actor < FleetAccess::Captain {
            return Err(FleetError::Unauthorized {
                actor,
                required: FleetAccess::Captain,
            });
        }
        // claim_hash 支持前缀匹配（list 只展示前 16 位）；多命中必须完整 hash 消歧。
        let matches: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.status == FleetStatus::Active
                    && (e.claim_hash == claim_hash || e.claim_hash.starts_with(claim_hash))
            })
            .map(|(i, _)| i)
            .collect();
        let idx = match matches.len() {
            0 => return Err(FleetError::NotFound(claim_hash.to_string())),
            1 => matches[0],
            _ => return Err(FleetError::Ambiguous(claim_hash.to_string())),
        };
        let mut updated = self.entries[idx].clone();
        updated.status = FleetStatus::Archived;
        self.entries[idx] = updated.clone();
        // 归档为状态变更：全量重写文件（内存镜像即文件内容），
        // 避免同一 claim 双行（Active + Archived）导致跨进程重读出现幻影。
        self.flush_all()?;
        Ok(updated)
    }

    /// 导出 CAT7 交换块（v5.1.1）：按文件内顺序，`source` 取前一条 Active 的
    /// claim_hash（顺序血缘近似；语义血缘由 v5.1.3 lineage 提供）。
    pub fn export_cat7(&self) -> Vec<crate::memory::cat7::Cat7Block> {
        let mut blocks = Vec::new();
        let mut prev: Option<String> = None;
        for e in &self.entries {
            if e.status == FleetStatus::Active {
                blocks.push(crate::memory::cat7::Cat7Block::from_fleet_entry(
                    e,
                    prev.as_deref(),
                ));
                prev = Some(e.claim_hash.clone());
            }
        }
        blocks
    }

    /// 导入 CAT7 交换块（v5.1.1）：原子导入（全有或全无）——
    /// 先对所有块做完整性校验（任一失败立即拒绝，此时不写任何条），
    /// 全部通过后再按 claim_hash 去重（已存在跳过）并一次性落盘，
    /// 保留块内 created_at 与作者。返回 (新增, 跳过)。
    pub fn import_cat7(
        &mut self,
        blocks: &[crate::memory::cat7::Cat7Block],
    ) -> Result<(usize, usize), FleetError> {
        // 第一遍：全量校验（失败即返回，无任何写入；保证全有/全无）。
        for b in blocks {
            if !b.verify() {
                return Err(FleetError::InvalidBlock(b.claim_hash.clone()));
            }
        }
        // 第二遍：去重 + 收集（内存改动，尚未落盘）。
        let mut added = 0usize;
        let mut skipped = 0usize;
        for b in blocks {
            if self.entries.iter().any(|e| e.claim_hash == b.claim_hash) {
                skipped += 1;
                continue;
            }
            self.entries.push(FleetEntry {
                kind: b.kind,
                author: b.author.clone(),
                author_role: FleetAccess::Contributor,
                content: b.content.clone(),
                ts: b.created_at.clone(),
                status: FleetStatus::Active,
                claim_hash: b.claim_hash.clone(),
            });
            added += 1;
        }
        if added > 0 {
            self.flush_all()?;
        }
        Ok((added, skipped))
    }

    /// 全量重写文件（entries 为权威镜像，损坏行随重写清除）。
    fn flush_all(&mut self) -> Result<(), FleetError> {
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&self.path)?;
        for e in &self.entries {
            let line = serde_json::to_string(e).map_err(|err| FleetError::Json(err.to_string()))?;
            writeln!(f, "{line}")?;
        }
        self.corrupt_lines = 0;
        Ok(())
    }

    /// 清空集体记忆：Admin only（物理删除文件，并清内存镜像）。
    pub fn purge(&mut self, actor: FleetAccess) -> Result<(), FleetError> {
        if actor != FleetAccess::Admin {
            return Err(FleetError::Unauthorized {
                actor,
                required: FleetAccess::Admin,
            });
        }
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        self.entries.clear();
        self.corrupt_lines = 0;
        Ok(())
    }

    /// 读取集体记忆（Observer+）：默认排除 Archived，可显式包含。
    pub fn list(
        &self,
        actor: FleetAccess,
        kind: Option<FleetKind>,
        include_archived: bool,
    ) -> Result<Vec<FleetEntry>, FleetError> {
        if actor < FleetAccess::Observer {
            return Err(FleetError::Unauthorized {
                actor,
                required: FleetAccess::Observer,
            });
        }
        Ok(self
            .entries
            .iter()
            .filter(|e| {
                let kind_ok = kind.map(|k| e.kind == k).unwrap_or(true);
                let status_ok = include_archived || e.status == FleetStatus::Active;
                kind_ok && status_ok
            })
            .cloned()
            .collect())
    }

    /// 状态报告（Observer+）。
    pub fn status(&self, actor: FleetAccess) -> Result<FleetStatusReport, FleetError> {
        if actor < FleetAccess::Observer {
            return Err(FleetError::Unauthorized {
                actor,
                required: FleetAccess::Observer,
            });
        }
        let file_bytes = fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        Ok(FleetStatusReport {
            file: self.path.display().to_string(),
            entries: self.entries.len(),
            active: self
                .entries
                .iter()
                .filter(|e| e.status == FleetStatus::Active)
                .count(),
            archived: self
                .entries
                .iter()
                .filter(|e| e.status == FleetStatus::Archived)
                .count(),
            facts: self
                .entries
                .iter()
                .filter(|e| e.kind == FleetKind::Fact)
                .count(),
            decisions: self
                .entries
                .iter()
                .filter(|e| e.kind == FleetKind::Decision)
                .count(),
            open_problems: self
                .entries
                .iter()
                .filter(|e| e.kind == FleetKind::OpenProblem)
                .count(),
            file_bytes,
            corrupt_lines: self.corrupt_lines,
        })
    }
}

fn now_iso() -> String {
    // 轻量本地时间戳：UTC（RFC3339 简化）。避免额外依赖 chrono。
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 1970-01-01T00:00:00Z + secs 的近似 ISO（精确到秒，足够策展排序）。
    format!("{}", secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_path(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("au-fleet-{name}-{}.jsonl", std::process::id()));
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn observer_cannot_post_fail_closed() {
        let p = tmp_path("perm-post");
        let mut m = FleetMemory::open(&p).unwrap();
        let err = m
            .post(FleetAccess::Observer, FleetKind::Fact, "n1", "x")
            .unwrap_err();
        assert!(matches!(
            err,
            FleetError::Unauthorized {
                required: FleetAccess::Contributor,
                ..
            }
        ));
        // fail-closed：未写盘。
        assert!(!p.exists());
    }

    #[test]
    fn contributor_can_post_and_roundtrip() {
        let p = tmp_path("roundtrip");
        let mut m = FleetMemory::open(&p).unwrap();
        m.post(
            FleetAccess::Contributor,
            FleetKind::Decision,
            "n1",
            "采用自建中继作为默认跨网通道",
        )
        .unwrap();
        let reopened = FleetMemory::open(&p).unwrap();
        let rows = reopened.list(FleetAccess::Observer, None, false).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, FleetKind::Decision);
        assert_eq!(rows[0].author, "n1");
    }

    #[test]
    fn three_kinds_curated_separately() {
        let p = tmp_path("kinds");
        let mut m = FleetMemory::open(&p).unwrap();
        m.post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
            .unwrap();
        m.post(FleetAccess::Contributor, FleetKind::Decision, "n1", "d1")
            .unwrap();
        m.post(FleetAccess::Contributor, FleetKind::OpenProblem, "n1", "o1")
            .unwrap();
        let facts = m
            .list(FleetAccess::Observer, Some(FleetKind::Fact), false)
            .unwrap();
        let problems = m
            .list(FleetAccess::Observer, Some(FleetKind::OpenProblem), false)
            .unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].kind, FleetKind::OpenProblem);
    }

    #[test]
    fn contributor_cannot_curate() {
        let p = tmp_path("perm-curate");
        let mut m = FleetMemory::open(&p).unwrap();
        let e = m
            .post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
            .unwrap();
        let err = m
            .curate(FleetAccess::Contributor, &e.claim_hash)
            .unwrap_err();
        assert!(matches!(
            err,
            FleetError::Unauthorized {
                required: FleetAccess::Captain,
                ..
            }
        ));
    }

    #[test]
    fn captain_curates_archives_and_excludes_from_default_list() {
        let p = tmp_path("curate");
        let mut m = FleetMemory::open(&p).unwrap();
        let e = m
            .post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
            .unwrap();
        let archived = m.curate(FleetAccess::Captain, &e.claim_hash).unwrap();
        assert_eq!(archived.status, FleetStatus::Archived);
        assert!(m
            .list(FleetAccess::Observer, None, false)
            .unwrap()
            .is_empty());
        assert_eq!(m.list(FleetAccess::Observer, None, true).unwrap().len(), 1);
    }

    #[test]
    fn claim_hash_is_deterministic_and_content_sensitive() {
        let a = fleet_claim_hash(FleetKind::Fact, "n1", "same");
        let b = fleet_claim_hash(FleetKind::Fact, "n1", "same");
        let c = fleet_claim_hash(FleetKind::Fact, "n1", "other");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn curate_accepts_claim_prefix_and_rejects_missing() {
        let p = tmp_path("curate_prefix");
        let mut m = FleetMemory::open(&p).unwrap();
        let e = m
            .post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
            .unwrap();
        // 前缀（前 16 位）可消歧归档。
        let prefix = &e.claim_hash[..16];
        let archived = m.curate(FleetAccess::Captain, prefix).unwrap();
        assert_eq!(archived.status, FleetStatus::Archived);
        // 不存在的 claim 报 NotFound。
        let missing = m.curate(FleetAccess::Captain, "deadbeefdeadbeef");
        assert!(matches!(missing, Err(FleetError::NotFound(_))));
        // 全部已归档后再次前缀匹配同样 NotFound（仅 Active 可策展）。
        let gone = m.curate(FleetAccess::Captain, prefix);
        assert!(matches!(gone, Err(FleetError::NotFound(_))));
    }

    #[test]
    fn non_admin_cannot_purge() {
        let p = tmp_path("perm-purge");
        let mut m = FleetMemory::open(&p).unwrap();
        m.post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
            .unwrap();
        let err = m.purge(FleetAccess::Captain).unwrap_err();
        assert!(matches!(
            err,
            FleetError::Unauthorized {
                required: FleetAccess::Admin,
                ..
            }
        ));
        // 文件仍在。
        assert!(p.exists());
    }

    #[test]
    fn corrupt_line_counted_not_fatal() {
        let p = tmp_path("corrupt");
        let mut m = FleetMemory::open(&p).unwrap();
        m.post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
            .unwrap();
        let mut f = OpenOptions::new().append(true).open(&p).unwrap();
        writeln!(f, "{{not-json").unwrap();
        drop(f);
        let reopened = FleetMemory::open(&p).unwrap();
        let st = reopened.status(FleetAccess::Observer).unwrap();
        assert_eq!(st.corrupt_lines, 1);
        assert_eq!(st.entries, 1);
    }

    #[test]
    fn role_parse_roundtrip() {
        assert_eq!(FleetAccess::parse("CAPTAIN"), Some(FleetAccess::Captain));
        assert_eq!(FleetAccess::parse("admin"), Some(FleetAccess::Admin));
        assert_eq!(FleetAccess::parse("root"), None);
        assert_eq!(
            FleetKind::parse("open-problem"),
            Some(FleetKind::OpenProblem)
        );
        assert_eq!(FleetKind::parse("fact"), Some(FleetKind::Fact));
    }

    // ---- v5.1.1：CAT7 导出 / 导入 ----

    /// 导出：按文件内顺序，source 逐块指向前一条 Active 的 claim_hash（顺序血缘）。
    #[test]
    fn export_cat7_chains_source_in_order() {
        let p = tmp_path("export-cat7");
        let mut m = FleetMemory::open(&p).unwrap();
        m.post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
            .unwrap();
        m.post(FleetAccess::Contributor, FleetKind::Decision, "n2", "d1")
            .unwrap();
        m.post(FleetAccess::Contributor, FleetKind::OpenProblem, "n3", "o1")
            .unwrap();
        let blocks = m.export_cat7();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].source, None);
        assert_eq!(
            blocks[1].source.as_deref(),
            Some(blocks[0].claim_hash.as_str())
        );
        assert_eq!(
            blocks[2].source.as_deref(),
            Some(blocks[1].claim_hash.as_str())
        );
        // 每个块均可通过完整性校验（跨 Agent 接收方可独立验证）。
        assert!(blocks.iter().all(|b| b.verify()));
        // confidence 诚实为 None（未评估）。
        assert!(blocks.iter().all(|b| b.confidence.is_none()));
    }

    /// 导出 → 新实例导入：去重（已存在跳过）+ 保留 created_at/作者。
    #[test]
    fn import_cat7_dedups_and_roundtrips() {
        let p = tmp_path("import-cat7");
        let mut m = FleetMemory::open(&p).unwrap();
        m.post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
            .unwrap();
        let blocks = m.export_cat7();

        // 另一个节点：导入同样的块。
        let p2 = tmp_path("import-cat7-peer");
        let mut m2 = FleetMemory::open(&p2).unwrap();
        let (added, skipped) = m2.import_cat7(&blocks).unwrap();
        assert_eq!(added, 1);
        assert_eq!(skipped, 0);
        let (added2, skipped2) = m2.import_cat7(&blocks).unwrap();
        assert_eq!(added2, 0);
        assert_eq!(skipped2, 1, "重复导入必须被去重");
        // 跨实例内容一致。
        let rows = m2.list(FleetAccess::Observer, None, false).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].claim_hash, blocks[0].claim_hash);
        assert_eq!(rows[0].content, "f1");
    }

    /// 导入完整性校验 fail-closed：篡改 content 的块必须被整体拒绝（不落盘任何条）。
    #[test]
    fn import_cat7_rejects_tampered_block() {
        let p = tmp_path("import-tamper");
        let mut m = FleetMemory::open(&p).unwrap();
        let mut blocks = {
            let p_src = tmp_path("import-tamper-src");
            let mut mm = FleetMemory::open(&p_src).unwrap();
            mm.post(FleetAccess::Contributor, FleetKind::Fact, "n1", "f1")
                .unwrap();
            mm.post(FleetAccess::Contributor, FleetKind::Decision, "n2", "d1")
                .unwrap();
            mm.export_cat7()
        };
        // 篡改第二块内容（不改 claim_hash → verify 失败）。
        blocks[1].content = "d1（被篡改）".to_string();
        let err = m.import_cat7(&blocks).unwrap_err();
        assert!(matches!(err, FleetError::InvalidBlock(_)));
        // fail-closed：任何条都不落盘。
        let st = m.status(FleetAccess::Observer).unwrap();
        assert_eq!(st.entries, 0);
    }
}
