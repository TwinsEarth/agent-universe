//! v5.1.2：SVAF 字段级评估（Subject-Value Assessment Field，v5-memory-roadmap §二）。
//!
//! 接收方对 [`Cat7Block`] **逐字段**决定接受/拒绝，而非整条接受或拒绝。
//!
//! 字段分级（fail-closed）：
//! | 分级 | 字段 | 聚合效果 |
//! |------|------|----------|
//! | 核心 | kind / author / content | 任一**未显式接受**（拒绝或未表态）→ 整条 [`Reject`](SvafDecision::Reject) |
//! | 可剥离 | source / confidence / created_at | 核心全接受、任一**未显式接受** → [`PartialAccept`](SvafDecision::PartialAccept)，剥离该字段 |
//! | 锚点 | claim_hash | 不是主张字段，不参与字段级评估；由 [`Cat7Block::verify`] 前置保证（篡改块不能进入评估） |
//!
//! 诚实边界：
//! - 不默认接受任何字段——未表态的核心字段一律按 Reject 处理（防止"未评估被默认接受"）。
//! - 部分接受剥离后，核心字段不变，`claim_hash` 仍成立（它只依赖 kind/author/content），
//!   剥离块仍可通过 `verify`，故能安全导入。
//! - 剥离 `created_at` 时以**接收方重建时间**替换（不伪造原始发布时间，决策本身留痕）。

use serde::{Deserialize, Serialize};

use crate::memory::cat7::Cat7Block;
use crate::memory::fleet::now_iso;

/// 可评估字段（六个"主张字段"）；`claim_hash` 是锚点，不参与字段级评估。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cat7Field {
    /// 策展分类：fact / decision / open_problem。
    Kind,
    /// 来源 Agent id。
    Author,
    /// 记忆内容本体。
    Content,
    /// 血缘前驱 claim_hash。
    Source,
    /// 接收方角色评估置信度。
    Confidence,
    /// 创建时间。
    CreatedAt,
}

impl Cat7Field {
    /// 从 CLI 字符串解析（未知 → None，调用方 fail-closed 拒绝）。
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "kind" => Some(Self::Kind),
            "author" => Some(Self::Author),
            "content" => Some(Self::Content),
            "source" => Some(Self::Source),
            "confidence" => Some(Self::Confidence),
            "created_at" | "created-at" | "createdat" => Some(Self::CreatedAt),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kind => "kind",
            Self::Author => "author",
            Self::Content => "content",
            Self::Source => "source",
            Self::Confidence => "confidence",
            Self::CreatedAt => "created_at",
        }
    }

    /// 核心字段（拒绝/未表态即整条拒绝）。
    pub fn is_core(self) -> bool {
        matches!(self, Self::Kind | Self::Author | Self::Content)
    }

    /// 全部可评估字段（核心在前）。
    pub fn all() -> [Self; 6] {
        [
            Self::Kind,
            Self::Author,
            Self::Content,
            Self::Source,
            Self::Confidence,
            Self::CreatedAt,
        ]
    }
}

/// 字段级决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldVerdict {
    /// 接受该字段。
    Accept,
    /// 拒绝该字段。
    Reject,
}

/// 整体决策（字段级评估聚合结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SvafDecision {
    /// 六字段全部显式接受：整条接受（字段完整）。
    AcceptAll,
    /// 核心字段全接受，存在可剥离字段未接受：剥离后接受。
    PartialAccept,
    /// 任一核心字段未显式接受：整条拒绝。
    Reject,
}

/// 字段级评估（一份针对单个 CAT7 块的逐字段决定）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvafAssessment {
    /// 被评估块的内容锚点（关联键，非评估字段）。
    pub claim_hash: String,
    /// 评估方（接收方）agent id。
    pub assessor: String,
    /// 字段决定（可部分覆盖；未覆盖字段按 fail-closed 聚合）。
    pub verdicts: Vec<(Cat7Field, FieldVerdict)>,
    /// 备注（可选，留痕）。
    pub note: Option<String>,
}

/// SVAF 错误（fail-closed）。
#[derive(Debug)]
pub enum SvafError {
    /// 块未通过完整性校验（篡改），不能进入字段评估。
    BlockNotVerified(String),
    /// 评估锚点与待应用块不匹配（评估的是另一块）。
    AnchorMismatch,
}

impl std::fmt::Display for SvafError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlockNotVerified(h) => {
                write!(f, "块未通过完整性校验（篡改），不能进入字段评估: {h}")
            }
            Self::AnchorMismatch => write!(f, "评估锚点与待应用块不匹配"),
        }
    }
}

impl std::error::Error for SvafError {}

impl SvafAssessment {
    /// 创建评估：**前置完整性校验**——块必须通过 `verify`，否则拒绝进入评估（fail-closed）。
    pub fn for_block(block: &Cat7Block, assessor: &str) -> Result<Self, SvafError> {
        if !block.verify() {
            return Err(SvafError::BlockNotVerified(block.claim_hash.clone()));
        }
        Ok(Self {
            claim_hash: block.claim_hash.clone(),
            assessor: assessor.to_string(),
            verdicts: Vec::new(),
            note: None,
        })
    }

    /// 设置字段决定（同字段后设覆盖先设）。
    pub fn set(&mut self, field: Cat7Field, verdict: FieldVerdict) -> &mut Self {
        self.verdicts.retain(|(f, _)| *f != field);
        self.verdicts.push((field, verdict));
        self
    }

    /// 读取某字段的决定（未表态 → None）。
    pub fn verdict_of(&self, field: Cat7Field) -> Option<FieldVerdict> {
        self.verdicts
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, v)| *v)
    }

    /// 聚合决策（fail-closed）：
    /// 1. 核心字段任一未显式接受 → Reject；
    /// 2. 可剥离字段任一未显式接受 → PartialAccept；
    /// 3. 六字段全显式接受 → AcceptAll。
    pub fn decision(&self) -> SvafDecision {
        for f in Cat7Field::all() {
            if f.is_core() && self.verdict_of(f) != Some(FieldVerdict::Accept) {
                return SvafDecision::Reject;
            }
        }
        for f in Cat7Field::all() {
            if !f.is_core() && self.verdict_of(f) != Some(FieldVerdict::Accept) {
                return SvafDecision::PartialAccept;
            }
        }
        SvafDecision::AcceptAll
    }

    /// 应用评估到块（便利方法）：剥离 `created_at` 时以**当前重建时间**替换。
    /// 等价于 [`apply_with`](Self::apply_with) 并传入 `now_iso()`。
    pub fn apply(&self, block: &Cat7Block) -> Result<Option<Cat7Block>, SvafError> {
        self.apply_with(block, &now_iso())
    }

    /// 应用评估到块（时间可注入，纯函数可测）：
    /// - Reject → `None`（拒绝采纳）；
    /// - AcceptAll → 原块（字段完整）；
    /// - PartialAccept → 剥离未接受的可剥离字段后的块（核心字段不变，仍通过 `verify`）；
    ///   其中 `created_at` 未被接受时，用调用方提供的 `recreated_at`（接收方重建时间）替换。
    pub fn apply_with(
        &self,
        block: &Cat7Block,
        recreated_at: &str,
    ) -> Result<Option<Cat7Block>, SvafError> {
        if block.claim_hash != self.claim_hash {
            return Err(SvafError::AnchorMismatch);
        }
        if !block.verify() {
            return Err(SvafError::BlockNotVerified(block.claim_hash.clone()));
        }
        match self.decision() {
            SvafDecision::Reject => Ok(None),
            SvafDecision::AcceptAll => Ok(Some(block.clone())),
            SvafDecision::PartialAccept => {
                let mut b = block.clone();
                if self.verdict_of(Cat7Field::Source) != Some(FieldVerdict::Accept) {
                    b.source = None;
                }
                if self.verdict_of(Cat7Field::Confidence) != Some(FieldVerdict::Accept) {
                    b.confidence = None;
                }
                if self.verdict_of(Cat7Field::CreatedAt) != Some(FieldVerdict::Accept) {
                    // 接收方不采纳原始时间：以调用方提供的重建时间替换（不伪造原始发布时间，决策留痕）。
                    b.created_at = recreated_at.to_string();
                }
                // 核心字段未变，claim_hash 仍成立：剥离块必须通过完整性校验。
                assert!(b.verify(), "剥离后核心字段不变，verify 必须成立");
                Ok(Some(b))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::fleet::{FleetAccess, FleetEntry, FleetKind};

    fn block(kind: FleetKind, author: &str, content: &str) -> Cat7Block {
        let e = FleetEntry::new(kind, author, FleetAccess::Contributor, content);
        Cat7Block::from_fleet_entry(&e, Some("prev-hash"))
    }

    /// 六字段全显式接受 → AcceptAll，apply 返回字段完整的原块。
    #[test]
    fn svaf_accept_all_keeps_full_block() {
        let b = block(FleetKind::Fact, "alice", "地球是圆的");
        let mut a = SvafAssessment::for_block(&b, "bob").unwrap();
        for f in Cat7Field::all() {
            a.set(f, FieldVerdict::Accept);
        }
        assert_eq!(a.decision(), SvafDecision::AcceptAll);
        let out = a.apply(&b).unwrap().expect("全接受必须产出块");
        assert_eq!(out.source.as_deref(), Some("prev-hash"));
        assert_eq!(out.content, "地球是圆的");
        assert_eq!(out.created_at, b.created_at);
        assert!(out.verify());
    }

    /// 核心接受、可剥离拒绝 → PartialAccept，剥离 source/confidence/created_at，verify 仍成立。
    #[test]
    fn svaf_partial_strips_optional_fields() {
        let b = block(FleetKind::Decision, "alice", "采用方案 B");
        let orig_ts = b.created_at.clone();
        let mut a = SvafAssessment::for_block(&b, "bob").unwrap();
        for f in [Cat7Field::Kind, Cat7Field::Author, Cat7Field::Content] {
            a.set(f, FieldVerdict::Accept);
        }
        for f in [
            Cat7Field::Source,
            Cat7Field::Confidence,
            Cat7Field::CreatedAt,
        ] {
            a.set(f, FieldVerdict::Reject);
        }
        assert_eq!(a.decision(), SvafDecision::PartialAccept);
        // 注入明确不同的重建时间（避免依赖系统时钟秒级精度）。
        let out = a
            .apply_with(&b, "9999999999")
            .unwrap()
            .expect("部分接受必须产出剥离块");
        assert_eq!(out.source, None, "拒绝 source 必须剥离");
        assert_eq!(out.confidence, None, "拒绝 confidence 必须剥离");
        assert_eq!(
            out.created_at, "9999999999",
            "拒绝 created_at 必须以调用方提供的重建时间替换"
        );
        assert_ne!(out.created_at, orig_ts);
        assert_eq!(out.content, "采用方案 B");
        assert!(out.verify(), "剥离可剥离字段不影响核心 claim_hash");
    }

    /// 仅拒绝一个可剥离字段（source）→ PartialAccept，只剥离 source。
    #[test]
    fn svaf_partial_strips_only_rejected_field() {
        let b = block(FleetKind::Fact, "alice", "事实 1");
        let mut a = SvafAssessment::for_block(&b, "bob").unwrap();
        for f in [
            Cat7Field::Kind,
            Cat7Field::Author,
            Cat7Field::Content,
            Cat7Field::Confidence,
            Cat7Field::CreatedAt,
        ] {
            a.set(f, FieldVerdict::Accept);
        }
        a.set(Cat7Field::Source, FieldVerdict::Reject);
        assert_eq!(a.decision(), SvafDecision::PartialAccept);
        let out = a.apply(&b).unwrap().unwrap();
        assert_eq!(out.source, None);
        assert_eq!(out.created_at, b.created_at, "未拒绝 created_at 必须保留");
        assert!(out.verify());
    }

    /// 核心字段被拒 → Reject，apply 返回 None。
    #[test]
    fn svaf_reject_core_field() {
        let b = block(FleetKind::Fact, "alice", "可疑内容");
        let mut a = SvafAssessment::for_block(&b, "bob").unwrap();
        for f in Cat7Field::all() {
            a.set(f, FieldVerdict::Accept);
        }
        a.set(Cat7Field::Content, FieldVerdict::Reject);
        assert_eq!(a.decision(), SvafDecision::Reject);
        assert!(a.apply(&b).unwrap().is_none(), "拒绝核心字段必须返回 None");
    }

    /// 核心字段未表态（漏设 content）→ fail-closed 按 Reject，不默认接受。
    #[test]
    fn svaf_unset_core_fails_closed() {
        let b = block(FleetKind::Fact, "alice", "地球是圆的");
        let mut a = SvafAssessment::for_block(&b, "bob").unwrap();
        a.set(Cat7Field::Kind, FieldVerdict::Accept);
        a.set(Cat7Field::Author, FieldVerdict::Accept);
        // content 未表态。
        assert_eq!(
            a.decision(),
            SvafDecision::Reject,
            "未表态的核心字段必须按 Reject"
        );
        assert!(a.apply(&b).unwrap().is_none());
    }

    /// 核心全接受、可剥离字段未表态（而非显式拒绝）→ PartialAccept（未表态不强行算接受）。
    #[test]
    fn svaf_unset_optional_is_partial_not_full() {
        let b = block(FleetKind::Fact, "alice", "事实 1");
        let mut a = SvafAssessment::for_block(&b, "bob").unwrap();
        for f in [Cat7Field::Kind, Cat7Field::Author, Cat7Field::Content] {
            a.set(f, FieldVerdict::Accept);
        }
        // source/confidence/created_at 完全未表态。
        assert_eq!(a.decision(), SvafDecision::PartialAccept);
        let out = a.apply(&b).unwrap().unwrap();
        assert_eq!(out.source, None);
        assert_eq!(out.confidence, None);
    }

    /// 篡改块（verify 失败）不能进入字段评估（for_block 即 fail-closed）。
    #[test]
    fn svaf_tampered_block_cannot_be_assessed() {
        let mut b = block(FleetKind::Fact, "alice", "原始内容");
        b.content = "原始内容（被篡改）".to_string();
        assert!(matches!(
            SvafAssessment::for_block(&b, "bob"),
            Err(SvafError::BlockNotVerified(_))
        ));
    }

    /// 评估锚点与待应用块不匹配 → AnchorMismatch。
    #[test]
    fn svaf_anchor_mismatch_on_apply() {
        let b1 = block(FleetKind::Fact, "alice", "内容一");
        let b2 = block(FleetKind::Fact, "alice", "内容二");
        let mut a = SvafAssessment::for_block(&b1, "bob").unwrap();
        for f in Cat7Field::all() {
            a.set(f, FieldVerdict::Accept);
        }
        assert!(matches!(a.apply(&b2), Err(SvafError::AnchorMismatch)));
    }

    /// 评估 serde 往返：决策与字段决定保持一致。
    #[test]
    fn svaf_serde_roundtrip() {
        let b = block(FleetKind::OpenProblem, "carol", "如何扩容");
        let mut a = SvafAssessment::for_block(&b, "bob").unwrap();
        a.set(Cat7Field::Kind, FieldVerdict::Accept);
        a.set(Cat7Field::Author, FieldVerdict::Accept);
        a.set(Cat7Field::Content, FieldVerdict::Accept);
        a.set(Cat7Field::Source, FieldVerdict::Reject);
        let before = a.decision();
        let json = serde_json::to_string(&a).unwrap();
        let back: SvafAssessment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.claim_hash, a.claim_hash);
        assert_eq!(back.assessor, "bob");
        assert_eq!(back.decision(), before);
        assert_eq!(
            back.verdict_of(Cat7Field::Source),
            Some(FieldVerdict::Reject)
        );
    }

    /// 字段解析/分类与核心判定。
    #[test]
    fn field_parse_and_classification() {
        assert_eq!(Cat7Field::parse("CONTENT"), Some(Cat7Field::Content));
        assert_eq!(Cat7Field::parse("created-at"), Some(Cat7Field::CreatedAt));
        assert_eq!(
            Cat7Field::parse("claim_hash"),
            None,
            "锚点字段不参与字段级评估"
        );
        assert!(Cat7Field::Author.is_core());
        assert!(!Cat7Field::Confidence.is_core());
        assert_eq!(Cat7Field::all().len(), 6);
        assert_eq!(Cat7Field::Kind.as_str(), "kind");
    }
}
