//! 持久化个体记忆（v5.0.0，FLEET-MEMORY-DESIGN §3.1）
//!
//! `memory.jsonl`：append-only JSONL，每行一个 [`MemoryEntry`]，位于节点状态目录、
//! 与 `agent-state.json` 并列。**构造即有界**：字节上限 + 滚动文件 + tail 截断，
//! 磁盘占用有硬上限（`max_bytes × (keep_rotated + 1)`），杜绝无界日志重演
//! （VPS 曾被 14.7 GB/天 的日志写满 30 GB 根分区）。
//!
//! 设计边界（对齐 FLEET §4）：
//! - 只记机器事实与结论（argv / exit code / tails / conclusion）；不存 prompt、secret。
//! - 本层是纯本地追加，不涉及 relay 协议；relay 仍只存哈希。

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// 一条持久化记忆，字段对齐 FLEET §3.1。
///
/// 可选字段在为空时不序列化，使旧版本（缺字段）写出的 JSON 也能被
/// 新版本反序列化（`#[serde(default)]`），保证向前兼容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryEntry {
    /// RFC3339 UTC 时间戳（`chrono::Utc::now().to_rfc3339()`）。
    pub at: String,
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gate: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cwd_rel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stdout_tail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stderr_tail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub conclusion: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
}

impl MemoryEntry {
    /// 以当前 UTC 时间创建一条最小记忆。
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            at: chrono::Utc::now().to_rfc3339(),
            task_id: task_id.into(),
            gate: None,
            argv: Vec::new(),
            cwd_rel: None,
            exit_code: None,
            ms: None,
            stdout_tail: None,
            stderr_tail: None,
            conclusion: None,
            tags: Vec::new(),
        }
    }

    /// 覆盖时间戳（测试或外部时钟时使用）。
    pub fn at(mut self, at: impl Into<String>) -> Self {
        self.at = at.into();
        self
    }
    pub fn gate(mut self, g: impl Into<String>) -> Self {
        self.gate = Some(g.into());
        self
    }
    pub fn argv(mut self, a: Vec<String>) -> Self {
        self.argv = a;
        self
    }
    pub fn cwd_rel(mut self, c: impl Into<String>) -> Self {
        self.cwd_rel = Some(c.into());
        self
    }
    pub fn exit_code(mut self, c: i32) -> Self {
        self.exit_code = Some(c);
        self
    }
    pub fn ms(mut self, m: u64) -> Self {
        self.ms = Some(m);
        self
    }
    pub fn stdout_tail(mut self, s: impl Into<String>) -> Self {
        self.stdout_tail = Some(s.into());
        self
    }
    pub fn stderr_tail(mut self, s: impl Into<String>) -> Self {
        self.stderr_tail = Some(s.into());
        self
    }
    pub fn conclusion(mut self, c: impl Into<String>) -> Self {
        self.conclusion = Some(c.into());
        self
    }
    pub fn tags(mut self, t: Vec<String>) -> Self {
        self.tags = t;
        self
    }
    pub fn add_tag(mut self, t: impl Into<String>) -> Self {
        self.tags.push(t.into());
        self
    }

    /// 按字符数截断两个 tail 字段（防止单行过大）。`max_chars=0` 表示不截断。
    fn truncate_tails(&mut self, max_chars: usize) {
        if max_chars == 0 {
            return;
        }
        truncate_field(&mut self.stdout_tail, max_chars);
        truncate_field(&mut self.stderr_tail, max_chars);
    }
}

fn truncate_field(field: &mut Option<String>, max_chars: usize) {
    if let Some(s) = field {
        if s.chars().count() > max_chars {
            let kept: String = s.chars().take(max_chars).collect();
            *s = format!("{kept}…[truncated]");
        }
    }
}

/// 记忆读写错误。
#[derive(Debug)]
pub enum MemoryError {
    Io(String),
    Json(String),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::Io(s) => write!(f, "memory io: {s}"),
            MemoryError::Json(s) => write!(f, "memory json: {s}"),
        }
    }
}

impl Error for MemoryError {}

impl From<io::Error> for MemoryError {
    fn from(e: io::Error) -> Self {
        MemoryError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for MemoryError {
    fn from(e: serde_json::Error) -> Self {
        MemoryError::Json(e.to_string())
    }
}

/// 记忆检索条件；所有字段为「与」关系，空字段不过滤。
#[derive(Debug, Default, Clone)]
pub struct MemoryQuery {
    pub task_id: Option<String>,
    pub gate: Option<String>,
    /// 条目需同时包含全部给定标签（AND）；空则不过滤。
    pub tags: Vec<String>,
    pub conclusion_contains: Option<String>,
    /// 精确匹配退出码；仅对携带 `exit_code` 的条目生效。
    pub exit_code: Option<i32>,
    /// 便捷成功/失败过滤：`Some(true)` 仅 exit=0，`Some(false)` 仅退出码非零。
    pub ok: Option<bool>,
    /// 仅返回 `at >= since` 的条目；RFC3339 UTC 字符串（`…Z`）字典序等价于时间序。
    pub since: Option<String>,
    /// 仅返回 `at < until` 的条目（与 `since` 组成半开区间）。
    pub until: Option<String>,
    pub limit: Option<usize>,
}

impl MemoryQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn task_id(mut self, v: impl Into<String>) -> Self {
        self.task_id = Some(v.into());
        self
    }
    pub fn gate(mut self, v: impl Into<String>) -> Self {
        self.gate = Some(v.into());
        self
    }
    /// 追加一个要求标签；可链式重复调用，多个标签为 AND 关系。
    pub fn tag(mut self, v: impl Into<String>) -> Self {
        self.tags.push(v.into());
        self
    }
    /// 一次性设置要求标签集合（需全部包含）。
    pub fn tags(mut self, v: Vec<String>) -> Self {
        self.tags = v;
        self
    }
    pub fn conclusion_contains(mut self, v: impl Into<String>) -> Self {
        self.conclusion_contains = Some(v.into());
        self
    }
    pub fn exit_code(mut self, v: i32) -> Self {
        self.exit_code = Some(v);
        self
    }
    /// 便捷成功/失败过滤：`true` 仅 exit=0，`false` 仅退出码非零。
    pub fn ok(mut self, v: bool) -> Self {
        self.ok = Some(v);
        self
    }
    pub fn since(mut self, v: impl Into<String>) -> Self {
        self.since = Some(v.into());
        self
    }
    pub fn until(mut self, v: impl Into<String>) -> Self {
        self.until = Some(v.into());
        self
    }
    pub fn limit(mut self, v: usize) -> Self {
        self.limit = Some(v);
        self
    }
}

/// 默认单文件软上限：1 MB。
pub const DEFAULT_MAX_BYTES: u64 = 1_000_000;
/// 默认保留滚动文件数。
pub const DEFAULT_KEEP_ROTATED: usize = 2;
/// 默认 tail 字段最大字符数。
pub const DEFAULT_TAIL_MAX_CHARS: usize = 2000;

/// 一个滚动归档文件（memory.<index>.jsonl）的容量信息。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RotatedFileStatus {
    pub index: usize,
    pub bytes: u64,
}

/// 记忆存储的容量状态快照（当前文件 + 全部滚动文件）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryStorageStatus {
    pub path: String,
    /// 单文件软上限（字节）；0 表示不限。
    pub max_bytes: u64,
    pub keep_rotated: usize,
    pub tail_max_chars: usize,
    pub current_bytes: u64,
    pub current_entries: usize,
    pub rotated: Vec<RotatedFileStatus>,
    /// 当前文件 + 全部滚动文件合计字节。
    pub total_bytes: u64,
    /// 磁盘硬上限；`None` 表示不限。
    pub disk_bound_bytes: Option<u64>,
}

/// 有界 append-only JSONL 记忆存储。
///
/// 仅持有路径与配置（无内部可变状态），因此读写方法都用 `&self`。
#[derive(Debug, Clone)]
pub struct PersistentMemoryStore {
    path: PathBuf,
    max_bytes: u64,
    keep_rotated: usize,
    tail_max_chars: usize,
}

impl PersistentMemoryStore {
    /// 以默认参数（1 MB / 保留 2 滚动 / tail 2000）打开 `path`。
    /// 文件与目录在首次 [`append`](Self::append) 时才创建。
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            max_bytes: DEFAULT_MAX_BYTES,
            keep_rotated: DEFAULT_KEEP_ROTATED,
            tail_max_chars: DEFAULT_TAIL_MAX_CHARS,
        }
    }

    /// 设置单文件软上限（字节）。磁盘硬上限 = `max_bytes × (keep_rotated + 1)`。
    /// 设为 0 表示不限制大小（不推荐，仅测试用）。
    pub fn max_bytes(mut self, bytes: u64) -> Self {
        self.max_bytes = bytes;
        self
    }

    /// 设置保留的滚动文件数（`memory.1.jsonl` … `memory.N.jsonl`）。
    pub fn keep_rotated(mut self, n: usize) -> Self {
        self.keep_rotated = n;
        self
    }

    /// 设置每个 tail 字段最大字符数。0 表示不截断。
    pub fn tail_max_chars(mut self, n: usize) -> Self {
        self.tail_max_chars = n;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 单文件软上限（字节）。
    pub fn max_bytes_limit(&self) -> u64 {
        self.max_bytes
    }

    /// 保留的滚动文件数。
    pub fn keep_rotated_count(&self) -> usize {
        self.keep_rotated
    }

    /// 每个 tail 字段最大字符数。
    pub fn tail_max_chars_limit(&self) -> usize {
        self.tail_max_chars
    }

    /// 磁盘占用硬上限（字节）：当前文件 + 全部滚动文件。
    pub fn disk_bound_bytes(&self) -> Option<u64> {
        if self.max_bytes == 0 {
            None
        } else {
            Some(self.max_bytes * (self.keep_rotated as u64 + 1))
        }
    }

    /// 采集当前文件与全部滚动文件的容量状态（不修改任何内容）。
    pub fn status(&self) -> Result<MemoryStorageStatus, MemoryError> {
        let current_bytes = fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        let current_entries = self.load_current()?.len();
        let mut rotated: Vec<RotatedFileStatus> = Vec::new();
        for i in 1..=self.keep_rotated {
            let p = self.rotated_path(i);
            if p.exists() {
                rotated.push(RotatedFileStatus {
                    index: i,
                    bytes: fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
                });
            }
        }
        let rotated_bytes = rotated.iter().map(|r| r.bytes).sum::<u64>();
        Ok(MemoryStorageStatus {
            path: self.path.display().to_string(),
            max_bytes: self.max_bytes,
            keep_rotated: self.keep_rotated,
            tail_max_chars: self.tail_max_chars,
            current_bytes,
            current_entries,
            rotated,
            total_bytes: current_bytes + rotated_bytes,
            disk_bound_bytes: self.disk_bound_bytes(),
        })
    }

    fn rotated_path(&self, i: usize) -> PathBuf {
        let stem = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("memory");
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        parent.join(format!("{stem}.{i}.jsonl"))
    }

    /// 轮转：当前文件归档为 `memory.1.jsonl`，旧滚动逐级后移，删除最老的。
    /// 当前文件不存在时为空操作。
    fn rotate(&self) -> Result<(), MemoryError> {
        if !self.path.exists() {
            return Ok(());
        }
        if self.keep_rotated == 0 {
            // 不保留历史：直接删除当前文件，新条目写入空文件。
            fs::remove_file(&self.path)?;
            return Ok(());
        }
        // 删除最老的，给逐级后移腾位。
        let oldest = self.rotated_path(self.keep_rotated);
        if oldest.exists() {
            fs::remove_file(&oldest)?;
        }
        for i in (1..self.keep_rotated).rev() {
            let src = self.rotated_path(i);
            if src.exists() {
                fs::rename(&src, self.rotated_path(i + 1))?;
            }
        }
        fs::rename(&self.path, self.rotated_path(1))?;
        Ok(())
    }

    /// 追加一条记忆（原子追加 + 超限前轮转 + tail 截断 + fsync）。
    pub fn append(&self, mut entry: MemoryEntry) -> Result<(), MemoryError> {
        entry.truncate_tails(self.tail_max_chars);
        let line = serde_json::to_vec(&entry)?;
        let line_len = line.len() as u64 + 1; // + 换行

        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let current = fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        if self.max_bytes > 0 && current + line_len > self.max_bytes {
            self.rotate()?;
        }

        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        f.write_all(&line)?;
        f.write_all(b"\n")?;
        f.sync_all()?;
        Ok(())
    }

    /// 读取单个 JSONL 文件；文件不存在返回空；损坏行跳过（不中断其余行）。
    fn read_file(&self, p: &Path) -> Result<Vec<MemoryEntry>, MemoryError> {
        let content = match fs::read_to_string(p) {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(e) = serde_json::from_str::<MemoryEntry>(line) {
                out.push(e);
            }
            // 损坏行：跳过，不影响其它行（append 路径保证正常情况下不会产生）。
        }
        Ok(out)
    }

    /// 仅读取当前文件（最新一段）的记忆。
    pub fn load_current(&self) -> Result<Vec<MemoryEntry>, MemoryError> {
        self.read_file(&self.path)
    }

    /// 读取当前文件 + 全部滚动文件，按时间从老到新排序。
    pub fn load_all(&self) -> Result<Vec<MemoryEntry>, MemoryError> {
        let mut out = Vec::new();
        for i in (1..=self.keep_rotated).rev() {
            out.extend(self.read_file(&self.rotated_path(i))?);
        }
        out.extend(self.read_file(&self.path)?);
        Ok(out)
    }

    /// 按 [`MemoryQuery`] 检索（基于当前 + 滚动文件）。
    pub fn query(&self, q: &MemoryQuery) -> Result<Vec<MemoryEntry>, MemoryError> {
        let all = self.load_all()?;
        let mut res: Vec<MemoryEntry> = all
            .into_iter()
            .filter(|e| {
                if let Some(t) = &q.task_id {
                    if e.task_id != *t {
                        return false;
                    }
                }
                if let Some(g) = &q.gate {
                    if e.gate.as_deref() != Some(g.as_str()) {
                        return false;
                    }
                }
                // 多标签 AND：条目须包含全部要求标签。
                if !q.tags.iter().all(|t| e.tags.contains(t)) {
                    return false;
                }
                if let Some(c) = &q.conclusion_contains {
                    match &e.conclusion {
                        Some(cc) if cc.contains(c) => {}
                        _ => return false,
                    }
                }
                if let Some(want) = q.exit_code {
                    if e.exit_code != Some(want) {
                        return false;
                    }
                }
                if let Some(want_ok) = q.ok {
                    // 无退出码的条目无法判定成败，一律排除。
                    match e.exit_code {
                        Some(0) if want_ok => {}
                        Some(c) if !want_ok && c != 0 => {}
                        _ => return false,
                    }
                }
                if let Some(since) = &q.since {
                    if e.at.as_str() < since.as_str() {
                        return false;
                    }
                }
                if let Some(until) = &q.until {
                    if e.at.as_str() >= until.as_str() {
                        return false;
                    }
                }
                true
            })
            .collect();
        if let Some(lim) = q.limit {
            res.truncate(lim);
        }
        Ok(res)
    }

    /// 当前文件已记录条数。
    pub fn current_len(&self) -> Result<usize, MemoryError> {
        Ok(self.load_current()?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_path(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("au-mem-{tag}-{nanos}.jsonl"))
    }

    fn sample(id: &str, t: &str) -> MemoryEntry {
        MemoryEntry::new(id)
            .at(t)
            .gate("ci")
            .argv(vec!["node".into(), "test.mjs".into()])
            .exit_code(0)
            .ms(120)
            .stdout_tail("all ok")
            .conclusion("passes")
            .add_tag("ci")
    }

    #[test]
    fn append_and_load_current() {
        let p = tmp_path("basic");
        let store = PersistentMemoryStore::open(&p);
        store.append(sample("t1", "2026-10-14T01:00:00Z")).unwrap();
        store.append(sample("t2", "2026-10-14T02:00:00Z")).unwrap();

        let cur = store.load_current().unwrap();
        assert_eq!(cur.len(), 2);
        assert_eq!(cur[0].task_id, "t1");
        assert_eq!(cur[1].task_id, "t2");
        assert_eq!(cur[0].exit_code, Some(0));
        assert_eq!(cur[0].tags, vec!["ci".to_string()]);
        fs::remove_file(&p).ok();
    }

    #[test]
    fn query_filters() {
        let p = tmp_path("query");
        let store = PersistentMemoryStore::open(&p);
        store.append(sample("t1", "2026-10-14T01:00:00Z")).unwrap();
        store
            .append(
                MemoryEntry::new("t2")
                    .at("2026-10-14T03:00:00Z")
                    .add_tag("network")
                    .conclusion("relay reachable"),
            )
            .unwrap();
        // 失败样本：exit=2，带 network + sandbox 标签。
        store
            .append(
                MemoryEntry::new("t3")
                    .at("2026-10-14T05:00:00Z")
                    .gate("sandbox.exec")
                    .exit_code(2)
                    .add_tag("network")
                    .add_tag("sandbox")
                    .conclusion("exit=2 (python)"),
            )
            .unwrap();

        assert_eq!(
            store
                .query(&MemoryQuery::new().tag("network"))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            store
                .query(&MemoryQuery::new().task_id("t1"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .query(&MemoryQuery::new().conclusion_contains("reachable"))
                .unwrap()
                .len(),
            1
        );
        // since 过滤：只返回 02:00 之后
        assert_eq!(
            store
                .query(&MemoryQuery::new().since("2026-10-14T02:00:00Z"))
                .unwrap()
                .len(),
            2
        );
        // 多标签 AND：network + sandbox 仅 t3
        assert_eq!(
            store
                .query(&MemoryQuery::new().tag("network").tag("sandbox"))
                .unwrap()
                .len(),
            1
        );
        // 退出码精确匹配
        assert_eq!(
            store.query(&MemoryQuery::new().exit_code(2)).unwrap().len(),
            1
        );
        // 成功/失败便捷过滤
        assert_eq!(
            store.query(&MemoryQuery::new().ok(true)).unwrap().len(),
            1,
            "仅 t1 exit=0"
        );
        assert_eq!(
            store.query(&MemoryQuery::new().ok(false)).unwrap().len(),
            1,
            "仅 t3 exit=2；t2 无退出码被排除"
        );
        // until 过滤：只返回 03:00 之前（不含边界）
        assert_eq!(
            store
                .query(&MemoryQuery::new().until("2026-10-14T03:00:00Z"))
                .unwrap()
                .len(),
            1
        );
        // since + until 半开区间
        assert_eq!(
            store
                .query(
                    &MemoryQuery::new()
                        .since("2026-10-14T03:00:00Z")
                        .until("2026-10-14T05:00:00Z")
                )
                .unwrap()
                .len(),
            1
        );
        assert_eq!(store.query(&MemoryQuery::new().limit(1)).unwrap().len(), 1);
        fs::remove_file(&p).ok();
    }

    #[test]
    fn rotation_keeps_bounded_and_loadable() {
        let p = tmp_path("rotate");
        // 极小上限：每写 1~2 条即轮转；保留 2 个滚动文件。
        let store = PersistentMemoryStore::open(&p)
            .max_bytes(220)
            .keep_rotated(2)
            .tail_max_chars(50);
        for i in 0..8 {
            store
                .append(
                    MemoryEntry::new(format!("task-{i}"))
                        .at(format!("2026-10-14T0{i}:00:00Z"))
                        .conclusion(format!("conclusion number {i}")),
                )
                .unwrap();
        }

        // 当前文件始终不超过软上限。
        let cur_size = fs::metadata(&p).unwrap().len();
        assert!(cur_size <= 220, "current size {cur_size} exceeds max");

        // 磁盘总占用不超过硬上限（当前 + 2 滚动）。
        let bound = store.disk_bound_bytes().unwrap();
        let mut total = cur_size;
        for i in 1..=2 {
            let rp = store.rotated_path(i);
            if rp.exists() {
                total += fs::metadata(&rp).unwrap().len();
            }
        }
        assert!(total <= bound, "total {total} exceeds bound {bound}");

        // load_all 能读到当前 + 滚动（条数受保留窗口限制，不丢当前段）。
        let all = store.load_all().unwrap();
        assert!(!all.is_empty());
        // 最新条目（task-7）一定在。
        assert!(all.iter().any(|e| e.task_id == "task-7"));
        for i in 1..=2 {
            fs::remove_file(store.rotated_path(i)).ok();
        }
        fs::remove_file(&p).ok();
    }

    #[test]
    fn status_reports_bytes_and_rotated_files() {
        let p = tmp_path("status");
        let store = PersistentMemoryStore::open(&p)
            .max_bytes(220)
            .keep_rotated(2)
            .tail_max_chars(50);
        for i in 0..6 {
            store
                .append(
                    MemoryEntry::new(format!("s-task-{i}"))
                        .at(format!("2026-10-13T0{i}:00:00Z"))
                        .exit_code(i)
                        .conclusion(format!("conclusion number {i}")),
                )
                .unwrap();
        }
        let s = store.status().unwrap();
        assert_eq!(s.max_bytes, 220);
        assert_eq!(s.keep_rotated, 2);
        assert!(s.current_entries > 0);
        assert!(s.current_bytes <= 220);
        // 已发生轮转：至少有一个滚动文件。
        assert!(!s.rotated.is_empty(), "应至少有一个滚动文件");
        // total_bytes 与各文件字节之和一致。
        let rotated_sum: u64 = s.rotated.iter().map(|r| r.bytes).sum();
        assert_eq!(s.total_bytes, s.current_bytes + rotated_sum);
        // 硬上限 = 220 × (2+1) = 660。
        assert_eq!(s.disk_bound_bytes, Some(660));
        // status 不影响 load_all：最新条目仍在。
        assert!(store
            .load_all()
            .unwrap()
            .iter()
            .any(|e| e.task_id == "s-task-5"));
        for i in 1..=2 {
            fs::remove_file(store.rotated_path(i)).ok();
        }
        fs::remove_file(&p).ok();
    }

    #[test]
    fn empty_status_is_zeroed() {
        let p = tmp_path("status-empty");
        let store = PersistentMemoryStore::open(&p);
        let s = store.status().unwrap();
        assert_eq!(s.current_bytes, 0);
        assert_eq!(s.current_entries, 0);
        assert!(s.rotated.is_empty());
        assert_eq!(s.total_bytes, 0);
        assert_eq!(s.disk_bound_bytes, Some(DEFAULT_MAX_BYTES * 3));
    }

    #[test]
    fn long_tail_is_truncated() {
        let p = tmp_path("tail");
        let store = PersistentMemoryStore::open(&p).tail_max_chars(10);
        let big = "x".repeat(500);
        store
            .append(MemoryEntry::new("t1").stdout_tail(big))
            .unwrap();
        let cur = store.load_current().unwrap();
        let tail = cur[0].stdout_tail.as_ref().unwrap();
        assert!(tail.chars().count() <= 30);
        assert!(tail.ends_with("[truncated]"));
        fs::remove_file(&p).ok();
    }

    #[test]
    fn legacy_json_without_optional_fields_parses() {
        // 只有两个必填字段的旧 JSON 应可反序列化。
        let raw = r#"{"at":"2026-10-14T01:00:00Z","task_id":"old"}"#;
        let e: MemoryEntry = serde_json::from_str(raw).unwrap();
        assert_eq!(e.task_id, "old");
        assert_eq!(e.exit_code, None);
        assert!(e.tags.is_empty());
    }

    #[test]
    fn keep_zero_drops_history() {
        let p = tmp_path("keep0");
        let store = PersistentMemoryStore::open(&p)
            .max_bytes(120)
            .keep_rotated(0);
        for i in 0..5 {
            store
                .append(MemoryEntry::new(format!("t{i}")).conclusion("x"))
                .unwrap();
        }
        // 不保留任何滚动文件，磁盘上只有当前文件且不超过上限。
        assert!(fs::metadata(&p).unwrap().len() <= 120);
        fs::remove_file(&p).ok();
    }
}
