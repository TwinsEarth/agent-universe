//! v5.1.4：remix 再混合（v5-memory-roadmap §二）。
//!
//! 对齐 Mesh Memory Protocol 的 **remix 原语**：接收方对同伴 [`Cat7Block`]
//! 做字段级评估（[`SvafAssessment`]）后，**只持久化自己的角色评估与自己重写
//! 的理解**，**永不复制原始同伴信号**。
//!
//! 隐私边界（有可验证的测试）：
//! - 原始块仅以 `claim_hash` 指针（[`RemixRecord::origin_claim_hash`]）被引用，
//!   原始 `content` / `author` 不出现在记录中（记录序列化后不含原始内容字符串）。
//! - 接收方形成的新块（[`RemixRecord::remix_block`]）以 `assessor` 为作者、
//!   以自己的理解为内容，`source` 指回来源 claim_hash，是接收方自己的新声明。
//!
//! 采纳边界（fail-closed）：
//! - 原始块必须通过 `verify`（篡改块不能进入再混合）；
//! - 评估锚点必须与原始块匹配、评估方必须是接收方本人；
//! - 评估决策为 `Reject`（核心字段未被接受）时不产生再混合；
//! - 接收方自己的理解不允许为空白（防止漏传，fail-closed）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::memory::cat7::Cat7Block;
use crate::memory::fleet::{fleet_claim_hash, now_iso};
use crate::memory::svaf::{SvafAssessment, SvafDecision};

/// 再混合记录（只含接收方自己的信号 + 来源 claim_hash 指针）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemixRecord {
    /// 来源同伴块的 claim_hash（仅指针，不存原始 content/author）。
    pub origin_claim_hash: String,
    /// 接收方再混合后形成的新块（作者=assessor、内容=自己的理解、source 指回来源）。
    pub remix_block: Cat7Block,
    /// 接收方（再混合者）agent id。
    pub assessor: String,
    /// 字段级评估聚合决策（AcceptAll/PartialAccept；Reject 不产生 remix）。
    pub decision: SvafDecision,
    /// 再混合时间（接收方时钟，ISO8601）。
    pub created_at: String,
}

/// remix 错误（fail-closed）。
#[derive(Debug)]
pub enum RemixError {
    /// 原始块未通过完整性校验（篡改），不能进入再混合。
    BlockNotVerified(String),
    /// 评估锚点与原始块不匹配（评估的是另一块）。
    AnchorMismatch,
    /// 评估方与接收方（再混合者）不一致（不能用别人的评估做自己的再混合）。
    AssessorMismatch,
    /// 评估决策为 Reject（核心字段未被接受），不能再混合。
    RejectedNoRemix,
    /// 接收方自己的理解为空白（疑似漏传）。
    EmptyUnderstanding,
    /// 文件/持久化 IO 错误。
    Io(String),
    /// 持久化行 JSON 解析错误。
    Json(String),
}

impl std::fmt::Display for RemixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlockNotVerified(h) => {
                write!(f, "原始块未通过完整性校验（篡改），不能进入再混合: {h}")
            }
            Self::AnchorMismatch => write!(f, "评估锚点与原始块不匹配"),
            Self::AssessorMismatch => write!(f, "评估方与接收方（再混合者）不一致"),
            Self::RejectedNoRemix => {
                write!(f, "评估决策为 Reject（核心字段未被接受），不能再混合")
            }
            Self::EmptyUnderstanding => write!(f, "接收方自己的理解为空白，拒绝再混合"),
            Self::Io(e) => write!(f, "remix 持久化 IO 错误: {e}"),
            Self::Json(e) => write!(f, "remix 持久化行 JSON 解析错误: {e}"),
        }
    }
}

impl std::error::Error for RemixError {}

impl RemixRecord {
    /// 再混合：原始块 + 接收方对它的字段级评估 + 接收方自己的理解。
    /// 等价于 [`create_with_time`](Self::create_with_time) 并传入 `now_iso()`。
    pub fn create(
        block: &Cat7Block,
        assessment: &SvafAssessment,
        assessor: &str,
        understanding: &str,
    ) -> Result<Self, RemixError> {
        Self::create_with_time(block, assessment, assessor, understanding, &now_iso())
    }

    /// 再混合（时间可注入，纯函数可测）。
    pub fn create_with_time(
        block: &Cat7Block,
        assessment: &SvafAssessment,
        assessor: &str,
        understanding: &str,
        now: &str,
    ) -> Result<Self, RemixError> {
        // 1. 原始块完整性（篡改 fail-closed）。
        if !block.verify() {
            return Err(RemixError::BlockNotVerified(block.claim_hash.clone()));
        }
        // 2. 评估锚点必须匹配原始块。
        if assessment.claim_hash != block.claim_hash {
            return Err(RemixError::AnchorMismatch);
        }
        // 3. 评估方必须是接收方本人。
        if assessment.assessor != assessor {
            return Err(RemixError::AssessorMismatch);
        }
        // 4. 决策为 Reject 不产生再混合。
        let decision = assessment.decision();
        if decision == SvafDecision::Reject {
            return Err(RemixError::RejectedNoRemix);
        }
        // 5. 自己的理解不允许空白。
        if understanding.trim().is_empty() {
            return Err(RemixError::EmptyUnderstanding);
        }
        // 6. 形成接收方自己的新块：核心 kind 被接受故沿用；author 与 content
        //    都是自己的；source 指回来源 claim_hash（指针）。
        let kind = block.kind;
        let remix_block = Cat7Block {
            claim_hash: fleet_claim_hash(kind, assessor, understanding),
            kind,
            author: assessor.to_string(),
            content: understanding.to_string(),
            source: Some(block.claim_hash.clone()),
            // 诚实：不编造评估置信度（本版本不自动填）。
            confidence: None,
            created_at: now.to_string(),
        };
        // 新块由我们确定性构造，verify 必须成立。
        assert!(
            remix_block.verify(),
            "再混合块由 fleet_claim_hash 确定性构造，verify 必须成立"
        );
        Ok(Self {
            origin_claim_hash: block.claim_hash.clone(),
            remix_block,
            assessor: assessor.to_string(),
            decision,
            created_at: now.to_string(),
        })
    }

    /// 再混合新块的 claim_hash（便利访问）。
    pub fn remix_claim_hash(&self) -> &str {
        &self.remix_block.claim_hash
    }
}

/// 再混合存储（`remix.jsonl`，每行一个 [`RemixRecord`]）。
#[derive(Debug, Clone, Default)]
pub struct RemixStore {
    records: Vec<RemixRecord>,
    path: Option<PathBuf>,
}

impl RemixStore {
    /// 空存储（未绑定路径）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 打开（回读 `remix.jsonl` 逐行重放；损坏 JSON → [`RemixError::Json`]）。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RemixError> {
        let p = path.as_ref();
        let mut store = Self {
            records: Vec::new(),
            path: Some(p.to_path_buf()),
        };
        if let Some(text) = match std::fs::read_to_string(p) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(RemixError::Io(e.to_string())),
        } {
            for (i, line) in text.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let rec: RemixRecord = serde_json::from_str(line)
                    .map_err(|e| RemixError::Json(format!("第 {} 行: {e}", i + 1)))?;
                store.records.push(rec);
            }
        }
        Ok(store)
    }

    /// 加入一条记录（按再混合 claim_hash 去重；`true`=新增，`false`=已存在跳过）。
    pub fn add(&mut self, record: RemixRecord) -> bool {
        let h = record.remix_block.claim_hash.clone();
        if self.by_remix(&h).is_some() {
            return false;
        }
        self.records.push(record);
        true
    }

    /// 查某来源块被再混合成的所有记录（不同接收方/不同理解）。
    pub fn by_origin(&self, origin_claim_hash: &str) -> Vec<&RemixRecord> {
        self.records
            .iter()
            .filter(|r| r.origin_claim_hash == origin_claim_hash)
            .collect()
    }

    /// 按再混合新块 claim_hash 查记录。
    pub fn by_remix(&self, remix_claim_hash: &str) -> Option<&RemixRecord> {
        self.records
            .iter()
            .find(|r| r.remix_block.claim_hash == remix_claim_hash)
    }

    /// 全部记录。
    pub fn records(&self) -> &[RemixRecord] {
        &self.records
    }

    /// 绑定的持久化路径。
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// 全量重写到绑定路径（未绑定 → [`RemixError::Io`]）。
    pub fn save(&self) -> Result<(), RemixError> {
        match &self.path {
            Some(p) => self.save_to(p),
            None => Err(RemixError::Io("存储未绑定路径，无法 save".to_string())),
        }
    }

    /// 全量重写到指定路径（创建父目录，每行一个紧凑 JSON）。
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), RemixError> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| RemixError::Io(e.to_string()))?;
            }
        }
        let mut text = String::new();
        for r in &self.records {
            let line = serde_json::to_string(r).map_err(|e| RemixError::Json(e.to_string()))?;
            text.push_str(&line);
            text.push('\n');
        }
        std::fs::write(p, text).map_err(|e| RemixError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::fleet::{FleetAccess, FleetEntry, FleetKind};
    use crate::memory::svaf::{Cat7Field, FieldVerdict};

    /// 构造同伴原始块（带前驱）。
    fn origin_block(kind: FleetKind, author: &str, content: &str) -> Cat7Block {
        let e = FleetEntry::new(kind, author, FleetAccess::Contributor, content);
        Cat7Block::from_fleet_entry(&e, Some("upstream-hash"))
    }

    /// 构造评估：verdicts 为 (字段,决定) 列表。
    fn assessment(
        block: &Cat7Block,
        assessor: &str,
        verdicts: &[(Cat7Field, FieldVerdict)],
    ) -> SvafAssessment {
        let mut a = SvafAssessment::for_block(block, assessor).unwrap();
        for (f, v) in verdicts {
            a.set(*f, *v);
        }
        a
    }

    /// 六字段全接受。
    fn accept_all(block: &Cat7Block, assessor: &str) -> SvafAssessment {
        let all: Vec<_> = Cat7Field::all()
            .iter()
            .map(|f| (*f, FieldVerdict::Accept))
            .collect();
        assessment(block, assessor, &all)
    }

    /// 核心三字段接受、可剥离字段拒绝（PartialAccept）。
    fn accept_core(block: &Cat7Block, assessor: &str) -> SvafAssessment {
        let mut v: Vec<_> = [Cat7Field::Kind, Cat7Field::Author, Cat7Field::Content]
            .iter()
            .map(|f| (*f, FieldVerdict::Accept))
            .collect();
        for f in [
            Cat7Field::Source,
            Cat7Field::Confidence,
            Cat7Field::CreatedAt,
        ] {
            v.push((f, FieldVerdict::Reject));
        }
        assessment(block, assessor, &v)
    }

    /// AcceptAll + 自己的理解 → 产出接收方自己的新块。
    #[test]
    fn create_accept_all_produces_own_block() {
        let b = origin_block(FleetKind::Fact, "alice", "地球是圆的");
        let a = accept_all(&b, "bob");
        let rec =
            RemixRecord::create_with_time(&b, &a, "bob", "我确认：大地近似球体", "T1").unwrap();
        let rb = &rec.remix_block;
        assert_eq!(rb.author, "bob", "新块作者必须是接收方自己");
        assert_eq!(rb.content, "我确认：大地近似球体", "新块内容是自己的理解");
        assert_eq!(rb.kind, FleetKind::Fact, "被接受的 kind 沿用");
        assert_eq!(
            rb.source.as_deref(),
            Some(b.claim_hash.as_str()),
            "source 指回来源 claim_hash"
        );
        assert_eq!(
            rb.claim_hash,
            fleet_claim_hash(FleetKind::Fact, "bob", "我确认：大地近似球体")
        );
        assert!(rb.verify());
        assert_eq!(rec.origin_claim_hash, b.claim_hash);
        assert_eq!(rec.decision, SvafDecision::AcceptAll);
        assert_eq!(rec.created_at, "T1");
        assert_eq!(rec.remix_claim_hash(), rb.claim_hash);
    }

    /// PartialAccept（核心接受、可剥离拒绝）也能再混合。
    #[test]
    fn create_partial_accept_produces_remix() {
        let b = origin_block(FleetKind::Decision, "alice", "采用方案 B");
        let a = accept_core(&b, "bob");
        let rec = RemixRecord::create_with_time(&b, &a, "bob", "我执行 B 并记录", "T2").unwrap();
        assert_eq!(rec.decision, SvafDecision::PartialAccept);
        assert_eq!(rec.remix_block.author, "bob");
        assert_eq!(rec.remix_block.content, "我执行 B 并记录");
        assert!(rec.remix_block.verify());
    }

    /// Reject（核心 content 被拒）→ RejectedNoRemix。
    #[test]
    fn reject_decision_no_remix() {
        let b = origin_block(FleetKind::Fact, "alice", "可疑内容");
        let mut v: Vec<_> = [Cat7Field::Kind, Cat7Field::Author]
            .iter()
            .map(|f| (*f, FieldVerdict::Accept))
            .collect();
        v.push((Cat7Field::Content, FieldVerdict::Reject));
        for f in [
            Cat7Field::Source,
            Cat7Field::Confidence,
            Cat7Field::CreatedAt,
        ] {
            v.push((f, FieldVerdict::Accept));
        }
        let a = assessment(&b, "bob", &v);
        let err = RemixRecord::create_with_time(&b, &a, "bob", "我的理解", "T").unwrap_err();
        assert!(matches!(err, RemixError::RejectedNoRemix));
    }

    /// 篡改的原始块 → BlockNotVerified（create 第一道完整性校验，纵深防御）。
    #[test]
    fn tampered_origin_block_rejected() {
        let mut b = origin_block(FleetKind::Fact, "alice", "原始内容");
        b.content = "原始内容（被篡改）".to_string();
        // 手动构造 claim_hash 指向篡改块的评估（绕过 for_block 的前置校验），
        // 以验证 RemixRecord::create 自身的第一道 verify 仍 fail-closed。
        let a = SvafAssessment {
            claim_hash: b.claim_hash.clone(),
            assessor: "bob".to_string(),
            verdicts: vec![],
            note: None,
        };
        let err = RemixRecord::create_with_time(&b, &a, "bob", "理解", "T").unwrap_err();
        assert!(matches!(err, RemixError::BlockNotVerified(_)));
    }

    /// 评估锚点与原始块不符 → AnchorMismatch。
    #[test]
    fn anchor_mismatch_rejected() {
        let b1 = origin_block(FleetKind::Fact, "alice", "内容一");
        let b2 = origin_block(FleetKind::Fact, "alice", "内容二");
        // 评估针对 b2，却用于 b1。
        let a = accept_all(&b2, "bob");
        let err = RemixRecord::create_with_time(&b1, &a, "bob", "理解", "T").unwrap_err();
        assert!(matches!(err, RemixError::AnchorMismatch));
    }

    /// 评估方与接收方不一致 → AssessorMismatch。
    #[test]
    fn assessor_mismatch_rejected() {
        let b = origin_block(FleetKind::Fact, "alice", "内容");
        let a = accept_all(&b, "carol");
        let err = RemixRecord::create_with_time(&b, &a, "bob", "理解", "T").unwrap_err();
        assert!(matches!(err, RemixError::AssessorMismatch));
    }

    /// 空白理解 → EmptyUnderstanding。
    #[test]
    fn empty_understanding_rejected() {
        let b = origin_block(FleetKind::Fact, "alice", "内容");
        let a = accept_all(&b, "bob");
        assert!(matches!(
            RemixRecord::create_with_time(&b, &a, "bob", "   ", "T").unwrap_err(),
            RemixError::EmptyUnderstanding
        ));
    }

    /// 关键隐私测试：记录序列化后不含原始 content/author（只含自己的信号）。
    #[test]
    fn record_does_not_store_origin_signal() {
        let b = origin_block(FleetKind::Fact, "alice", "地球是圆的秘密细节X");
        let a = accept_all(&b, "bob");
        let rec =
            RemixRecord::create_with_time(&b, &a, "bob", "我确认：大地近似球体", "T").unwrap();
        let json = serde_json::to_string(&rec).unwrap();
        assert!(
            !json.contains("地球是圆的秘密细节X"),
            "再混合记录不得包含原始 content"
        );
        assert!(
            !json.contains("\"alice\""),
            "再混合记录不得包含原始 author（alice）"
        );
        // 但自己的信号必须在。
        assert!(json.contains("我确认：大地近似球体"));
        assert!(json.contains("\"bob\""));
    }

    /// 来源指针保留、再混合 hash 与来源 hash 不同。
    #[test]
    fn remix_block_anchors_origin_pointer() {
        let b = origin_block(FleetKind::Fact, "alice", "原始");
        let a = accept_all(&b, "bob");
        let rec = RemixRecord::create_with_time(&b, &a, "bob", "我的理解", "T").unwrap();
        assert_eq!(
            rec.remix_block.source.as_deref(),
            Some(b.claim_hash.as_str())
        );
        assert_ne!(rec.remix_block.claim_hash, b.claim_hash);
    }

    /// 记录 serde 往返。
    #[test]
    fn remix_serde_roundtrip() {
        let b = origin_block(FleetKind::OpenProblem, "alice", "如何扩容");
        let a = accept_core(&b, "bob");
        let rec = RemixRecord::create_with_time(&b, &a, "bob", "我的扩容思路", "T").unwrap();
        let json = serde_json::to_string(&rec).unwrap();
        let back: RemixRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.origin_claim_hash, rec.origin_claim_hash);
        assert_eq!(back.remix_block.claim_hash, rec.remix_block.claim_hash);
        assert_eq!(back.decision, SvafDecision::PartialAccept);
        assert!(back.remix_block.verify());
    }

    /// 不同理解产生不同再混合 hash。
    #[test]
    fn distinct_understandings_distinct_hash() {
        let b = origin_block(FleetKind::Fact, "alice", "原始");
        let a = accept_all(&b, "bob");
        let r1 = RemixRecord::create_with_time(&b, &a, "bob", "理解一", "T").unwrap();
        let r2 = RemixRecord::create_with_time(&b, &a, "bob", "理解二", "T").unwrap();
        assert_ne!(r1.remix_block.claim_hash, r2.remix_block.claim_hash);
    }

    /// 两个接收方对同一来源各自再混合，by_origin 返回两条，add 去重。
    #[test]
    fn store_remix_siblings_and_dedupe() {
        let b = origin_block(FleetKind::Fact, "alice", "共享来源");
        let a_bob = accept_all(&b, "bob");
        let a_carol = accept_all(&b, "carol");
        let rb = RemixRecord::create_with_time(&b, &a_bob, "bob", "bob 的理解", "T").unwrap();
        let rc = RemixRecord::create_with_time(&b, &a_carol, "carol", "carol 的理解", "T").unwrap();
        let mut store = RemixStore::new();
        assert!(store.add(rb.clone()));
        assert!(store.add(rc));
        // 重复加入同一条（同 remix hash）→ false。
        assert!(!store.add(rb.clone()));
        let siblings = store.by_origin(&b.claim_hash);
        assert_eq!(siblings.len(), 2);
        assert!(store.by_remix(&rb.remix_block.claim_hash).is_some());
        assert_eq!(store.records().len(), 2);
    }

    /// save/open jsonl 往返。
    #[test]
    fn store_jsonl_save_open_roundtrip() {
        let tmp = std::env::temp_dir().join("au-remix-test/remix.jsonl");
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
        let b = origin_block(FleetKind::Decision, "alice", "决策来源");
        let a = accept_all(&b, "bob");
        let rec = RemixRecord::create_with_time(&b, &a, "bob", "bob 的执行理解", "T").unwrap();
        let mut store = RemixStore::open(&tmp).unwrap();
        store.add(rec.clone());
        store.save().unwrap();
        let reopened = RemixStore::open(&tmp).unwrap();
        assert_eq!(reopened.records().len(), 1);
        assert_eq!(
            reopened.records()[0].remix_block.claim_hash,
            rec.remix_block.claim_hash
        );
        assert_eq!(reopened.path().unwrap(), tmp);
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
    }

    /// open 遇到损坏 JSON 行 → Json 错误（fail-closed）。
    #[test]
    fn open_rejects_corrupt_line() {
        let tmp = std::env::temp_dir().join("au-remix-corrupt/remix.jsonl");
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
        std::fs::create_dir_all(tmp.parent().unwrap()).unwrap();
        std::fs::write(&tmp, "{not valid json}\n").unwrap();
        let err = RemixStore::open(&tmp).unwrap_err();
        assert!(matches!(err, RemixError::Json(_)));
        let _ = std::fs::remove_dir_all(tmp.parent().unwrap());
    }
}
