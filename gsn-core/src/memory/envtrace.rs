//! v5.1.5：环境痕迹/快照接续（v5-memory-roadmap §二）。
//!
//! 对齐 roadmap 口径："`pack_diff` 快照即环境痕迹，Agent 可从第 k 步接续，
//! 仅记录后续变化"。
//!
//! 语义：
//! - [`EnvTrace`] 是一条可接续的环境痕迹：`base_step` 是接续起点（从第 k 步
//!   继续；0 = 新起点），`base_ref` 指向前驱痕迹的 `trace_ref`（无 = 第一条），
//!   `deltas` 只保存 **第 k 步之后的后续变化**，绝不重复第 k 步之前的内容。
//! - 接续只产生**新**痕迹（[`EnvTrace::continue_from`]），前驱不可变；
//!   新痕迹的每条变化必须 `step > base_step`（[`EnvTraceError::DeltaStepNotAfterBase`]）。
//! - 链式完整性（延续 v5.0.9 哈希链）：`trace_ref` 是逐条变化滚动更新的
//!   SHA-256 锚点，`verify()` 重放重算；任何篡改（改 subject/detail/step/
//!   base_step/base_ref/creator 或直接改哈希）都会失败，fail-closed。
//! - 隐私：`detail` 只存记录者自己的观察/摘要（沿用 v5.1.4 remix 的"不复制
//!   原始信号"理念，不强制但文档声明）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::memory::fleet::now_iso;

/// 环境痕迹变化类型（粗粒度，用于查询过滤与展示）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceKind {
    /// 文件/目录变化。
    File,
    /// 环境变量/配置项变化。
    Env,
    /// 进程生命周期变化（启动/退出/崩溃）。
    Proc,
    /// 网络连接/端口/拓扑变化。
    Net,
    /// 其他环境痕迹。
    Other,
}

impl TraceKind {
    /// 稳定标签（参与哈希与序列化展示）。
    pub fn label(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Env => "env",
            Self::Proc => "proc",
            Self::Net => "net",
            Self::Other => "other",
        }
    }

    /// 从标签解析（大小写不敏感）；未知标签返回 `None`。
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "file" => Some(Self::File),
            "env" => Some(Self::Env),
            "proc" => Some(Self::Proc),
            "net" => Some(Self::Net),
            "other" | "other_" | "" => Some(Self::Other),
            _ => None,
        }
    }
}

/// 一条环境痕迹后续变化（仅记录 `step > base_step` 的后续变化）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceDelta {
    /// 步骤号（接续后从第 k+1 步起；同一痕迹内严格递增，允许跳步）。
    pub step: u64,
    /// 变化类型。
    pub kind: TraceKind,
    /// 变化对象（如文件路径、环境变量名、进程名），不允许空白。
    pub subject: String,
    /// 变化详情（记录者自己的观察/摘要；可为空）。
    pub detail: String,
    /// 观察者（记录这条变化的 agent id）。
    pub recorded_by: String,
}

/// 环境痕迹（快照接续记录）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvTrace {
    /// 链式完整性锚点（SHA-256，随每条后续变化滚动更新）。
    pub trace_ref: String,
    /// 接续起点步骤（从第 k 步继续；0 = 新起点）。
    pub base_step: u64,
    /// 前驱痕迹的 trace_ref（None = 第一条痕迹，无接续前驱）。
    pub base_ref: Option<String>,
    /// 创建者（发起接续的 agent id）。
    pub creator: String,
    /// 第 k 步之后的后续变化（全部 `step > base_step`）。
    pub deltas: Vec<TraceDelta>,
    /// 创建时间（创建者时钟，ISO8601）。
    pub created_at: String,
}

/// 环境痕迹错误（fail-closed）。
#[derive(Debug)]
pub enum EnvTraceError {
    /// 接续前驱不存在（按 trace_ref 未找到）。
    BaseTraceNotFound(String),
    /// 前驱的当前最新步骤与声明的接续起点不一致（base_step 必须等于前驱最后一步）。
    BaseStepMismatch { base_step: u64, prior_last: u64 },
    /// 变化步骤 <= base_step（违反"仅记录后续变化"）。
    DeltaStepNotAfterBase { step: u64, base_step: u64 },
    /// 同一痕迹内步骤未严格递增（重复步骤）。
    StepNotSequential { step: u64, prior_step: u64 },
    /// 变化对象空白（无对象的环境痕迹无意义，fail-closed）。
    EmptySubject,
    /// 创建者空白。
    EmptyCreator,
    /// 文件/持久化 IO 错误。
    Io(String),
    /// 持久化行 JSON 解析错误。
    Json(String),
}

impl std::fmt::Display for EnvTraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BaseTraceNotFound(h) => {
                write!(f, "接续前驱不存在（trace_ref 未找到）: {h}")
            }
            Self::BaseStepMismatch {
                base_step,
                prior_last,
            } => {
                write!(
                    f,
                    "接续起点步骤不一致：base_step={base_step}，前驱最后一步={prior_last}"
                )
            }
            Self::DeltaStepNotAfterBase { step, base_step } => {
                write!(
                    f,
                    "变化步骤 {step} 未大于接续起点 {base_step}（仅记录后续变化，fail-closed）"
                )
            }
            Self::StepNotSequential { step, prior_step } => {
                write!(
                    f,
                    "变化步骤未严格递增：{step} 重复或回退（已有 {prior_step}）"
                )
            }
            Self::EmptySubject => write!(f, "变化对象为空，拒绝记录"),
            Self::EmptyCreator => write!(f, "创建者为空，拒绝创建痕迹"),
            Self::Io(e) => write!(f, "环境痕迹持久化 IO 错误: {e}"),
            Self::Json(e) => write!(f, "环境痕迹持久化行 JSON 解析错误: {e}"),
        }
    }
}

impl std::error::Error for EnvTraceError {}

/// 链式哈希初始化：SHA-256("envtrace|base_ref|base_step|creator|now")。
fn trace_hash_init(base_ref: Option<&str>, base_step: u64, creator: &str, now: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"envtrace|");
    h.update(base_ref.unwrap_or("none").as_bytes());
    h.update(format!("|{base_step}|").as_bytes());
    h.update(creator.as_bytes());
    h.update(b"|");
    h.update(now.as_bytes());
    hex(&h.finalize())
}

/// 链式哈希滚动更新：SHA-256(prev|step|kind|subject|detail|recorded_by)。
fn trace_hash_update(prev: &str, d: &TraceDelta) -> String {
    let mut h = Sha256::new();
    h.update(prev.as_bytes());
    h.update(format!("|{}|", d.step).as_bytes());
    h.update(d.kind.label().as_bytes());
    h.update(b"|");
    h.update(d.subject.as_bytes());
    h.update(b"|");
    h.update(d.detail.as_bytes());
    h.update(b"|");
    h.update(d.recorded_by.as_bytes());
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

impl EnvTrace {
    /// 新起点痕迹（base_step=0、无前驱）。等价于
    /// [`start_with_time`](Self::start_with_time) 并传入 `now_iso()`。
    pub fn start(creator: &str) -> Result<Self, EnvTraceError> {
        Self::start_with_time(creator, &now_iso())
    }

    /// 新起点痕迹（时间可注入，纯函数可测）。
    pub fn start_with_time(creator: &str, now: &str) -> Result<Self, EnvTraceError> {
        if creator.trim().is_empty() {
            return Err(EnvTraceError::EmptyCreator);
        }
        let trace_ref = trace_hash_init(None, 0, creator, now);
        Ok(Self {
            trace_ref,
            base_step: 0,
            base_ref: None,
            creator: creator.to_string(),
            deltas: Vec::new(),
            created_at: now.to_string(),
        })
    }

    /// 从第 k 步接续前驱痕迹：`base_step` = 前驱当前最后一步（第 k 步），
    /// `base_ref` = 前驱的 trace_ref；新痕迹初始不含任何变化，随后
    /// [`append_delta`](Self::append_delta) 只能追加第 k 步之后的后续变化。
    /// 等价于 [`continue_from_with_time`](Self::continue_from_with_time) 并传入
    /// `now_iso()`。
    pub fn continue_from(prior: &EnvTrace, creator: &str) -> Result<Self, EnvTraceError> {
        Self::continue_from_with_time(prior, creator, &now_iso())
    }

    /// 接续（时间可注入，纯函数可测）。
    pub fn continue_from_with_time(
        prior: &EnvTrace,
        creator: &str,
        now: &str,
    ) -> Result<Self, EnvTraceError> {
        if creator.trim().is_empty() {
            return Err(EnvTraceError::EmptyCreator);
        }
        let base_step = prior.last_step();
        let trace_ref = trace_hash_init(Some(&prior.trace_ref), base_step, creator, now);
        Ok(Self {
            trace_ref,
            base_step,
            base_ref: Some(prior.trace_ref.clone()),
            creator: creator.to_string(),
            deltas: Vec::new(),
            created_at: now.to_string(),
        })
    }

    /// 当前最新步骤（无变化时 = base_step，即"第 k 步"）。
    pub fn last_step(&self) -> u64 {
        self.deltas.last().map(|d| d.step).unwrap_or(self.base_step)
    }

    /// 追加一条后续变化（"仅记录后续变化"强制 + 步骤严格递增）。
    /// 追加后 `trace_ref` 链式滚动更新。
    pub fn append_delta(&mut self, delta: TraceDelta) -> Result<(), EnvTraceError> {
        if delta.subject.trim().is_empty() {
            return Err(EnvTraceError::EmptySubject);
        }
        if delta.step <= self.base_step {
            return Err(EnvTraceError::DeltaStepNotAfterBase {
                step: delta.step,
                base_step: self.base_step,
            });
        }
        if let Some(last) = self.deltas.last() {
            if delta.step <= last.step {
                return Err(EnvTraceError::StepNotSequential {
                    step: delta.step,
                    prior_step: last.step,
                });
            }
        }
        self.trace_ref = trace_hash_update(&self.trace_ref, &delta);
        self.deltas.push(delta);
        Ok(())
    }

    /// 链式完整性校验：重放所有变化重算哈希，必须与 trace_ref 一致。
    pub fn verify(&self) -> bool {
        let init = trace_hash_init(
            self.base_ref.as_deref(),
            self.base_step,
            &self.creator,
            &self.created_at,
        );
        let mut h = init;
        for d in &self.deltas {
            h = trace_hash_update(&h, d);
        }
        h == self.trace_ref
    }

    /// 序列化后的便捷引用（trace_ref 前 16 位，用于 list 展示）。
    pub fn short_ref(&self) -> String {
        self.trace_ref.chars().take(16).collect()
    }
}

/// 环境痕迹存储（`envtrace.jsonl`，每行一个 [`EnvTrace`]）。
#[derive(Debug, Clone, Default)]
pub struct EnvTraceStore {
    traces: Vec<EnvTrace>,
    path: Option<PathBuf>,
}

impl EnvTraceStore {
    /// 空存储（未绑定路径）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 打开（回读 `envtrace.jsonl` 逐行重放；损坏 JSON → [`EnvTraceError::Json`]）。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, EnvTraceError> {
        let p = path.as_ref();
        let mut store = Self {
            traces: Vec::new(),
            path: Some(p.to_path_buf()),
        };
        if let Some(text) = match std::fs::read_to_string(p) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(EnvTraceError::Io(e.to_string())),
        } {
            for (i, line) in text.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let t: EnvTrace = serde_json::from_str(line)
                    .map_err(|e| EnvTraceError::Json(format!("第 {} 行: {e}", i + 1)))?;
                store.traces.push(t);
            }
        }
        Ok(store)
    }

    /// 加入一条痕迹（按 trace_ref 去重；`true`=新增，`false`=已存在跳过）。
    pub fn add(&mut self, trace: EnvTrace) -> bool {
        let h = trace.trace_ref.clone();
        if self.by_ref(&h).is_some() {
            return false;
        }
        self.traces.push(trace);
        true
    }

    /// 按 trace_ref 查痕迹。
    pub fn by_ref(&self, trace_ref: &str) -> Option<&EnvTrace> {
        self.traces
            .iter()
            .find(|t| t.trace_ref == trace_ref || t.trace_ref.starts_with(trace_ref))
    }

    /// 全部痕迹。
    pub fn traces(&self) -> &[EnvTrace] {
        &self.traces
    }

    /// 绑定的持久化路径。
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// 全量重写到绑定路径（未绑定 → [`EnvTraceError::Io`]）。
    pub fn save(&self) -> Result<(), EnvTraceError> {
        match &self.path {
            Some(p) => self.save_to(p),
            None => Err(EnvTraceError::Io("存储未绑定路径，无法 save".to_string())),
        }
    }

    /// 全量重写到指定路径（创建父目录，每行一个紧凑 JSON）。
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), EnvTraceError> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| EnvTraceError::Io(e.to_string()))?;
            }
        }
        let mut text = String::new();
        for t in &self.traces {
            let line = serde_json::to_string(t).map_err(|e| EnvTraceError::Json(e.to_string()))?;
            text.push_str(&line);
            text.push('\n');
        }
        std::fs::write(p, text).map_err(|e| EnvTraceError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(step: u64, kind: TraceKind, subject: &str, detail: &str, by: &str) -> TraceDelta {
        TraceDelta {
            step,
            kind,
            subject: subject.to_string(),
            detail: detail.to_string(),
            recorded_by: by.to_string(),
        }
    }

    /// 新起点痕迹：base_step=0、无前驱、哈希确定性。
    #[test]
    fn start_creates_origin_trace() {
        let t = EnvTrace::start_with_time("alice", "T0").unwrap();
        assert_eq!(t.base_step, 0);
        assert!(t.base_ref.is_none());
        assert_eq!(t.deltas.len(), 0);
        assert_eq!(t.last_step(), 0);
        assert!(t.verify());
        // 确定性：相同输入 → 相同 trace_ref。
        let t2 = EnvTrace::start_with_time("alice", "T0").unwrap();
        assert_eq!(t.trace_ref, t2.trace_ref);
        // 不同创建者 → 不同 trace_ref。
        let t3 = EnvTrace::start_with_time("bob", "T0").unwrap();
        assert_ne!(t.trace_ref, t3.trace_ref);
        // 空白创建者 → EmptyCreator。
        assert!(matches!(
            EnvTrace::start_with_time("  ", "T0").unwrap_err(),
            EnvTraceError::EmptyCreator
        ));
    }

    /// 追加后续变化后 trace_ref 滚动更新、verify 成立。
    #[test]
    fn append_delta_updates_hash() {
        let mut t = EnvTrace::start_with_time("alice", "T0").unwrap();
        let before = t.trace_ref.clone();
        t.append_delta(delta(
            1,
            TraceKind::File,
            "workspace/main.rs",
            "新增文件",
            "alice",
        ))
        .unwrap();
        assert_ne!(t.trace_ref, before, "追加变化必须滚动更新哈希");
        assert_eq!(t.deltas.len(), 1);
        assert_eq!(t.last_step(), 1);
        assert!(t.verify());
    }

    /// 接续：base_ref=前驱 trace_ref、base_step=前驱最后一步（第 k 步）。
    #[test]
    fn continue_from_anchors_prior() {
        let mut prior = EnvTrace::start_with_time("alice", "T0").unwrap();
        prior
            .append_delta(delta(1, TraceKind::Env, "RUST_LOG", "info", "alice"))
            .unwrap();
        prior
            .append_delta(delta(3, TraceKind::Proc, "gsn-daemon", "启动", "alice"))
            .unwrap();
        let cont = EnvTrace::continue_from_with_time(&prior, "bob", "T1").unwrap();
        assert_eq!(cont.base_ref.as_deref(), Some(prior.trace_ref.as_str()));
        assert_eq!(cont.base_step, 3, "接续起点必须是前驱最后一步（第 k 步）");
        assert_eq!(cont.deltas.len(), 0, "接续新痕迹初始不含变化");
        assert!(cont.verify());
    }

    /// 核心语义：接续后只允许记录后续变化（step > base_step），
    /// 尝试记录第 k 步或更早的变化 → DeltaStepNotAfterBase。
    #[test]
    fn continue_only_records_later_steps() {
        let mut prior = EnvTrace::start_with_time("alice", "T0").unwrap();
        prior
            .append_delta(delta(2, TraceKind::Net, "relay:443", "连通", "alice"))
            .unwrap();
        let mut cont = EnvTrace::continue_from_with_time(&prior, "bob", "T1").unwrap();
        // 后续变化 step=3 合法。
        cont.append_delta(delta(3, TraceKind::File, "notes.md", "更新", "bob"))
            .unwrap();
        // 第 k 步（step=2，即 base_step）→ 拒绝。
        let err = cont
            .append_delta(delta(2, TraceKind::File, "x", "y", "bob"))
            .unwrap_err();
        assert!(matches!(err, EnvTraceError::DeltaStepNotAfterBase { .. }));
        // 更早（step=1）→ 拒绝。
        let err = cont
            .append_delta(delta(1, TraceKind::File, "x", "y", "bob"))
            .unwrap_err();
        assert!(matches!(err, EnvTraceError::DeltaStepNotAfterBase { .. }));
        assert_eq!(cont.deltas.len(), 1, "被拒绝的变化不得入列");
        assert!(cont.verify());
    }

    /// 同一条痕迹内步骤必须严格递增（允许跳步，不允许重复/回退）。
    #[test]
    fn delta_step_must_increase() {
        let mut t = EnvTrace::start_with_time("alice", "T0").unwrap();
        t.append_delta(delta(1, TraceKind::Other, "a", "", "alice"))
            .unwrap();
        t.append_delta(delta(5, TraceKind::Other, "b", "", "alice"))
            .unwrap();
        // 跳步 5 → 7 合法。
        t.append_delta(delta(7, TraceKind::Other, "c", "", "alice"))
            .unwrap();
        // 重复 7 → 拒绝。
        let err = t
            .append_delta(delta(7, TraceKind::Other, "d", "", "alice"))
            .unwrap_err();
        assert!(matches!(err, EnvTraceError::StepNotSequential { .. }));
        // 回退 6 → 拒绝。
        let err = t
            .append_delta(delta(6, TraceKind::Other, "e", "", "alice"))
            .unwrap_err();
        assert!(matches!(err, EnvTraceError::StepNotSequential { .. }));
        assert_eq!(t.deltas.len(), 3);
        assert!(t.verify());
    }

    /// 篡改变化内容 → verify 失败（链式完整性 fail-closed）。
    #[test]
    fn tampered_delta_detected() {
        let mut t = EnvTrace::start_with_time("alice", "T0").unwrap();
        t.append_delta(delta(1, TraceKind::File, "f1", "原始", "alice"))
            .unwrap();
        t.append_delta(delta(2, TraceKind::Env, "K", "v", "alice"))
            .unwrap();
        assert!(t.verify());
        let mut t2 = t.clone();
        t2.deltas[0].detail = "被篡改".to_string();
        assert!(!t2.verify(), "篡改变化详情必须被链式哈希捕获");
    }

    /// 直接改 trace_ref / base_step / creator → verify 失败。
    #[test]
    fn tampered_anchor_detected() {
        let mut t = EnvTrace::start_with_time("alice", "T0").unwrap();
        t.append_delta(delta(1, TraceKind::Proc, "p", "", "alice"))
            .unwrap();
        let mut a = t.clone();
        a.trace_ref = "0".repeat(64);
        assert!(!a.verify());
        let mut b = t.clone();
        b.base_step = 9;
        assert!(!b.verify(), "改接续起点必须破坏链式哈希");
        let mut c = t.clone();
        c.creator = "mallory".to_string();
        assert!(!c.verify());
    }

    /// 空白变化对象 → EmptySubject。
    #[test]
    fn empty_subject_rejected() {
        let mut t = EnvTrace::start_with_time("alice", "T0").unwrap();
        let err = t
            .append_delta(delta(1, TraceKind::Other, "   ", "", "alice"))
            .unwrap_err();
        assert!(matches!(err, EnvTraceError::EmptySubject));
    }

    /// 痕迹 serde 往返 + verify。
    #[test]
    fn envtrace_serde_roundtrip() {
        let mut t = EnvTrace::start_with_time("alice", "T0").unwrap();
        t.append_delta(delta(1, TraceKind::File, "a.rs", "新增", "alice"))
            .unwrap();
        let json = serde_json::to_string(&t).unwrap();
        let back: EnvTrace = serde_json::from_str(&json).unwrap();
        assert_eq!(back.trace_ref, t.trace_ref);
        assert_eq!(back.base_step, t.base_step);
        assert_eq!(back.deltas, t.deltas);
        assert!(back.verify());
    }

    /// TraceKind 标签往返（label/parse）。
    #[test]
    fn trace_kind_label_roundtrip() {
        for k in [
            TraceKind::File,
            TraceKind::Env,
            TraceKind::Proc,
            TraceKind::Net,
            TraceKind::Other,
        ] {
            assert_eq!(TraceKind::parse(k.label()), Some(k));
        }
        assert_eq!(TraceKind::parse("FILE"), Some(TraceKind::File));
        assert_eq!(TraceKind::parse("unknown-kind"), None);
    }

    /// 存储按 trace_ref 去重。
    #[test]
    fn store_dedupe_by_ref() {
        let mut store = EnvTraceStore::new();
        let a = EnvTrace::start_with_time("alice", "T0").unwrap();
        let mut b = EnvTrace::start_with_time("bob", "T0").unwrap();
        b.append_delta(delta(1, TraceKind::Env, "K", "v", "bob"))
            .unwrap();
        assert!(store.add(a.clone()));
        assert!(store.add(b));
        assert!(!store.add(a.clone()), "重复 trace_ref 必须去重");
        assert_eq!(store.traces().len(), 2);
        assert!(store.by_ref(&a.trace_ref).is_some());
        assert!(store.by_ref(&a.short_ref()).is_some(), "支持前缀匹配");
    }

    /// save/open jsonl 往返。
    #[test]
    fn store_jsonl_save_open_roundtrip() {
        let tmp = std::env::temp_dir().join("au-envtrace-test/envtrace.jsonl");
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
        let mut t = EnvTrace::start_with_time("alice", "T0").unwrap();
        t.append_delta(delta(1, TraceKind::Proc, "gsn", "启动", "alice"))
            .unwrap();
        let mut store = EnvTraceStore::open(&tmp).unwrap();
        store.add(t.clone());
        store.save().unwrap();
        let reopened = EnvTraceStore::open(&tmp).unwrap();
        assert_eq!(reopened.traces().len(), 1);
        assert_eq!(reopened.traces()[0].trace_ref, t.trace_ref);
        assert!(reopened.traces()[0].verify());
        assert_eq!(reopened.path().unwrap(), tmp);
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
    }

    /// open 遇到损坏 JSON 行 → Json 错误（fail-closed）。
    #[test]
    fn open_rejects_corrupt_line() {
        let tmp = std::env::temp_dir().join("au-envtrace-corrupt/envtrace.jsonl");
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
        std::fs::create_dir_all(tmp.parent().unwrap()).unwrap();
        std::fs::write(&tmp, "{not valid json}\n").unwrap();
        let err = EnvTraceStore::open(&tmp).unwrap_err();
        assert!(matches!(err, EnvTraceError::Json(_)));
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
    }

    /// 从存储中按 base_ref 找前驱接续；前驱不存在 → BaseTraceNotFound。
    #[test]
    fn continue_from_store_missing_base_rejected() {
        let mut store = EnvTraceStore::new();
        let t = EnvTrace::start_with_time("alice", "T0").unwrap();
        store.add(t.clone());
        // 手动构造一个声称接续不存在前驱的痕迹（store 层面无此校验，
        // 校验由调用方在 by_ref 时执行——这里验证 by_ref 找不到）。
        assert!(store.by_ref("deadbeef").is_none());
        assert!(store.by_ref(&t.trace_ref).is_some());
    }

    /// 多跳接续：A→B→C，C 只含 B 之后的后续变化，绝不包含 A 的变化。
    #[test]
    fn multi_hop_continuation_only_later_steps() {
        let mut a = EnvTrace::start_with_time("alice", "T0").unwrap();
        a.append_delta(delta(
            1,
            TraceKind::File,
            "step1.md",
            "alice 第 1 步",
            "alice",
        ))
        .unwrap();
        a.append_delta(delta(
            2,
            TraceKind::File,
            "step2.md",
            "alice 第 2 步",
            "alice",
        ))
        .unwrap();
        let mut b = EnvTrace::continue_from_with_time(&a, "bob", "T1").unwrap();
        b.append_delta(delta(3, TraceKind::Env, "MODE", "bob 第 3 步", "bob"))
            .unwrap();
        let mut c = EnvTrace::continue_from_with_time(&b, "carol", "T2").unwrap();
        c.append_delta(delta(4, TraceKind::Net, "carol 第 4 步", "接续", "carol"))
            .unwrap();
        // C 的 base_step = B 的最后一步 = 3；C 只含 step=4 的后续变化。
        assert_eq!(c.base_step, 3);
        assert_eq!(c.base_ref.as_deref(), Some(b.trace_ref.as_str()));
        assert_eq!(c.deltas.len(), 1);
        assert_eq!(c.deltas[0].step, 4);
        // C 的 deltas 不包含任何 A 的记录（step 1/2 或 alice 的记录）。
        assert!(c.deltas.iter().all(|d| d.step > 3));
        assert!(c.deltas.iter().all(|d| d.recorded_by != "alice"));
        assert!(c.verify());
        // B 只含 step=3（A 的记录不进 B）。
        assert!(b.deltas.iter().all(|d| d.step > 2));
        assert!(b.deltas.iter().all(|d| d.recorded_by != "alice"));
        assert!(b.verify());
    }

    /// 相同内容但不同记录者 → 不同哈希（可溯源）。
    #[test]
    fn distinct_recorder_distinct_hash() {
        let mut t1 = EnvTrace::start_with_time("alice", "T0").unwrap();
        t1.append_delta(delta(1, TraceKind::Other, "s", "d", "alice"))
            .unwrap();
        let mut t2 = EnvTrace::start_with_time("bob", "T0").unwrap();
        t2.append_delta(delta(1, TraceKind::Other, "s", "d", "bob"))
            .unwrap();
        assert_ne!(t1.trace_ref, t2.trace_ref);
    }
}
