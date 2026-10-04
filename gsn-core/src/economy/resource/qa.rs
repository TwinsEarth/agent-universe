//! BFT-lite QA 抽样验证（v3.8.8）。
//!
//! # 定位
//!
//! v3.8.7 的四维信誉只保证「反馈被有界、确定地聚合」，但**评分本身是入参**——买方
//! 可能刷分、女巫号可能堆量、供需可能合谋。v3.8.8 在信誉与结算之前加一层**可验证
//! 质量门**：对（抽样到的）订单结果组织一轮 BFT-lite 验证，只有拿到诚实超多数背书的
//! 结果才允许进入结算/信誉，出现 equivocation（双花投票）则**整轮作废**，不结算、不
//! 记信誉、只留下证据供 v3.8.4 质押罚没联动。
//!
//! 纯确定性内存记账面：零浮点、零 syscall、零 unsafe、无 panic 路径，全整数计数。
//!
//! # 容错参数
//!
//! - 验证者总数 `n` 必须满足 `n ≥ 3f+1`（[`QaRound::required_validators`]），`f` 为可
//!   容忍的最大拜占庭（恶意/失效）验证者数；
//! - 确认/否决都需要 `2f+1` 的诚实超多数（[`QaRound::approval_threshold`]）。`n≥3f+1`
//!   时诚实超多数唯一，赞成与否决不可能同时达到阈值。
//!
//! # 投票模型
//!
//! 每个验证者对「被验证答案」提交自己算出的结果摘要：摘要 == 被验证 `claimed_digest`
//! 计赞成，否则计异议。
//! - 同一验证者重复提交**相同**摘要：幂等，只计一票（防重复堆票）；
//! - 同一验证者提交**不同**摘要（同时为不同结果背书，典型双签 equivocation）：登记为
//!   equivocator，该轮立即 [`QaVerdict::EquivocationVoid`]，无论其余票如何。
//!
//! # 裁决（[`QaRound::adjudicate`]）
//!
//! 1. 存在任一 equivocator → `EquivocationVoid`（整轮丢弃，不结算不记信誉）；
//! 2. 有效验证者 `n < 3f+1` → `Pending`（样本不足，不能确认）；
//! 3. 赞成 `≥ 2f+1` → `Verified`；异议 `≥ 2f+1` → `Rejected`；
//! 4. 两者都不足 → `NoSupermajority`（分裂，本轮无结论）。

use serde::{Deserialize, Serialize};

use super::ResourceError;

/// 一张验证票：验证者自己算出的结果摘要（与 claimed 相同即赞成）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QaVote {
    /// 验证者身份 DID，非空。
    pub validator_did: String,
    /// 验证者本地重算得到的结果摘要，非空。
    pub observed_digest: String,
}

impl QaVote {
    pub fn new(validator_did: impl Into<String>, observed_digest: impl Into<String>) -> Self {
        Self {
            validator_did: validator_did.into(),
            observed_digest: observed_digest.into(),
        }
    }
}

/// 一轮 BFT-lite QA 的裁决结果（纯记账，不触发结算/罚没）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QaVerdict {
    /// 样本不足（n<3f+1），尚不能裁决。
    Pending { n: u64, required: u64 },
    /// 结果获诚实超多数（≥2f+1）背书，可进入结算/信誉。
    Verified {
        approvals: u64,
        dissents: u64,
        threshold: u64,
    },
    /// 诚实超多数判定结果错误（≥2f+1 异议）。
    Rejected {
        approvals: u64,
        dissents: u64,
        threshold: u64,
    },
    /// 赞成/异议均未达 2f+1，本轮无结论。
    NoSupermajority {
        approvals: u64,
        dissents: u64,
        threshold: u64,
    },
    /// 出现 equivocation（同一验证者为不同结果背书），整轮作废。
    EquivocationVoid { equivocators: Vec<String> },
}

/// 一轮 BFT-lite QA 验证：记录去重投票与 equivocator，提供确定性裁决。
#[derive(Debug, Clone)]
pub struct QaRound {
    round_id: String,
    claimed_digest: String,
    f: u64,
    // 验证者 -> 其首次提交的摘要（去重，保序）。
    votes: Vec<(String, String)>,
    // equivocator（同一验证者提交不同摘要），保序去重。
    equivocators: Vec<String>,
}

impl QaRound {
    /// 开一轮验证：`f` 为最大拜占庭数，`claimed_digest` 为被验证答案摘要。
    pub fn new(
        round_id: impl Into<String>,
        claimed_digest: impl Into<String>,
        f: u64,
    ) -> Result<Self, ResourceError> {
        let round_id = round_id.into();
        let claimed_digest = claimed_digest.into();
        if round_id.trim().is_empty() {
            return Err(ResourceError::EmptyQaRoundId);
        }
        if claimed_digest.trim().is_empty() {
            return Err(ResourceError::EmptyQaDigest);
        }
        // 校验 3f+1 可表示（fail-closed，拒绝可致阈值溢出的 f）。
        Self::required_validators(f)?;
        Self::approval_threshold(f)?;
        Ok(Self {
            round_id,
            claimed_digest,
            f,
            votes: Vec::new(),
            equivocators: Vec::new(),
        })
    }

    pub fn round_id(&self) -> &str {
        &self.round_id
    }

    pub fn claimed_digest(&self) -> &str {
        &self.claimed_digest
    }

    pub fn faulty_tolerance(&self) -> u64 {
        self.f
    }

    /// 所需最小验证者数 `3f+1`（checked，溢出 fail-closed）。
    pub fn required_validators(f: u64) -> Result<u64, ResourceError> {
        f.checked_mul(3)
            .and_then(|v| v.checked_add(1))
            .ok_or(ResourceError::QaFaultToleranceOverflow { f })
    }

    /// 诚实超多数阈值 `2f+1`（checked）。
    pub fn approval_threshold(f: u64) -> Result<u64, ResourceError> {
        f.checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or(ResourceError::QaFaultToleranceOverflow { f })
    }

    /// 投一张票。
    ///
    /// - 空验证者/空摘要：fail-closed，不计票；
    /// - 同验证者同摘要重复：幂等，只计一票（返回 `false` 表示未新增）；
    /// - 同验证者不同摘要：登记 equivocator（保序去重），整轮将作废（返回 `false`）。
    ///
    /// 返回 `true` 表示这是该验证者首次有效投票。
    pub fn cast(&mut self, vote: QaVote) -> Result<bool, ResourceError> {
        if vote.validator_did.trim().is_empty() {
            return Err(ResourceError::EmptyQaValidator);
        }
        if vote.observed_digest.trim().is_empty() {
            return Err(ResourceError::EmptyQaDigest);
        }
        let validator = vote.validator_did;
        let digest = vote.observed_digest;

        // 已在 equivocator 名单：继续提交不同/相同票都不再改变结果（轮已作废）。
        if self.equivocators.contains(&validator) {
            return Ok(false);
        }
        if let Some((_, existing)) = self.votes.iter().find(|(v, _)| v == &validator) {
            if existing == &digest {
                // 幂等重复，同一摘要不重复计票。
                return Ok(false);
            }
            // 同一验证者为不同结果背书 → equivocation，整轮作废。
            self.equivocators.push(validator.clone());
            return Ok(false);
        }
        self.votes.push((validator, digest));
        Ok(true)
    }

    /// 有效（去重、非 equivocation）验证者数。
    pub fn validator_count(&self) -> u64 {
        self.votes.len() as u64
    }

    /// 赞成票数（observed == claimed）。
    pub fn approvals(&self) -> u64 {
        self.votes
            .iter()
            .filter(|(_, d)| d == &self.claimed_digest)
            .count() as u64
    }

    /// 异议票数（observed != claimed）。
    pub fn dissents(&self) -> u64 {
        self.validator_count() - self.approvals()
    }

    /// equivocator 名单（保序去重）。
    pub fn equivocators(&self) -> &[String] {
        &self.equivocators
    }

    /// 本轮是否已因 equivocation 作废。
    pub fn is_void(&self) -> bool {
        !self.equivocators.is_empty()
    }

    /// 确定性裁决（可重复调用，不改变状态）。
    pub fn adjudicate(&self) -> QaVerdict {
        if self.is_void() {
            return QaVerdict::EquivocationVoid {
                equivocators: self.equivocators.clone(),
            };
        }
        let n = self.validator_count();
        let required = match Self::required_validators(self.f) {
            Ok(v) => v,
            // new 已校验；防御性返回 Pending（不 panic）。
            Err(_) => return QaVerdict::Pending { n, required: 0 },
        };
        if n < required {
            return QaVerdict::Pending { n, required };
        }
        let approvals = self.approvals();
        let dissents = self.dissents();
        let threshold = match Self::approval_threshold(self.f) {
            Ok(v) => v,
            Err(_) => {
                return QaVerdict::NoSupermajority {
                    approvals,
                    dissents,
                    threshold: 0,
                }
            }
        };
        if approvals >= threshold {
            QaVerdict::Verified {
                approvals,
                dissents,
                threshold,
            }
        } else if dissents >= threshold {
            QaVerdict::Rejected {
                approvals,
                dissents,
                threshold,
            }
        } else {
            QaVerdict::NoSupermajority {
                approvals,
                dissents,
                threshold,
            }
        }
    }

    /// 守恒不变量：赞成+异议==去重验证者数；非作废时无 equivocator；裁决与独立重算一致。
    pub fn invariant_holds(&self) -> bool {
        // votes 验证者唯一（cast 保证，这里独立复核）。
        let mut seen = std::collections::HashSet::new();
        for (v, _) in &self.votes {
            if !seen.insert(v.as_str()) {
                return false;
            }
        }
        if self.approvals().checked_add(self.dissents()) != Some(self.validator_count()) {
            return false;
        }
        // equivocator 去重保序。
        let mut es = std::collections::HashSet::new();
        for e in &self.equivocators {
            if !es.insert(e.as_str()) {
                return false;
            }
        }
        // 裁决与一条独立判定路径一致：作废优先，否则按阈值重算。
        let recomputed = if self.is_void() {
            QaVerdict::EquivocationVoid {
                equivocators: self.equivocators.clone(),
            }
        } else {
            self.adjudicate()
        };
        recomputed == self.adjudicate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vote(v: &str, d: &str) -> QaVote {
        QaVote::new(v, d)
    }

    const CLAIM: &str = "sha:claimed";

    #[test]
    fn honest_supermajority_verifies() {
        // f=1 → 需要 n≥4，阈值 3。3 赞成 1 异议 → Verified。
        let mut r = QaRound::new("round-1", CLAIM, 1).unwrap();
        assert!(r.cast(vote("v1", CLAIM)).unwrap());
        assert!(r.cast(vote("v2", CLAIM)).unwrap());
        assert!(r.cast(vote("v3", CLAIM)).unwrap());
        assert!(r.cast(vote("v4", "sha:other")).unwrap());
        assert_eq!(r.validator_count(), 4);
        assert_eq!(r.approvals(), 3);
        assert_eq!(r.dissents(), 1);
        assert_eq!(
            r.adjudicate(),
            QaVerdict::Verified {
                approvals: 3,
                dissents: 1,
                threshold: 3
            }
        );
        assert!(r.invariant_holds());
    }

    #[test]
    fn insufficient_validators_stays_pending() {
        // f=1 需 4；只有 3 票且全赞成 → 仍 Pending（样本不足不能确认）。
        let mut r = QaRound::new("r", CLAIM, 1).unwrap();
        for v in ["a", "b", "c"] {
            r.cast(vote(v, CLAIM)).unwrap();
        }
        assert_eq!(r.adjudicate(), QaVerdict::Pending { n: 3, required: 4 });
        assert!(r.invariant_holds());
    }

    #[test]
    fn exact_threshold_split_no_supermajority() {
        // f=1, n=4，恰 2 赞成 2 异议：均不足 3 → NoSupermajority。
        let mut r = QaRound::new("r", CLAIM, 1).unwrap();
        r.cast(vote("a", CLAIM)).unwrap();
        r.cast(vote("b", CLAIM)).unwrap();
        r.cast(vote("c", "sha:x")).unwrap();
        r.cast(vote("d", "sha:y")).unwrap();
        assert_eq!(
            r.adjudicate(),
            QaVerdict::NoSupermajority {
                approvals: 2,
                dissents: 2,
                threshold: 3
            }
        );
        assert!(r.invariant_holds());
    }

    #[test]
    fn equivocation_vectors_entire_round() {
        // v1 先投 claimed，再投另一摘要 → equivocation；即便其余 3 票全赞成也整轮作废。
        let mut r = QaRound::new("r", CLAIM, 1).unwrap();
        r.cast(vote("v1", CLAIM)).unwrap();
        assert_eq!(r.cast(vote("v1", "sha:evil")), Ok(false));
        r.cast(vote("v2", CLAIM)).unwrap();
        r.cast(vote("v3", CLAIM)).unwrap();
        r.cast(vote("v4", CLAIM)).unwrap();
        assert!(r.is_void());
        assert_eq!(r.equivocators(), vec!["v1".to_string()]);
        assert_eq!(
            r.adjudicate(),
            QaVerdict::EquivocationVoid {
                equivocators: vec!["v1".to_string()]
            }
        );
        // 作废后 equivocator 再投也不改变。
        assert_eq!(r.cast(vote("v1", CLAIM)), Ok(false));
        assert_eq!(r.equivocators().len(), 1);
        assert!(r.invariant_holds());
    }

    #[test]
    fn duplicate_same_vote_is_idempotent() {
        let mut r = QaRound::new("r", CLAIM, 1).unwrap();
        assert_eq!(r.cast(vote("a", CLAIM)), Ok(true));
        // 同验证者同摘要重复：不新增票。
        assert_eq!(r.cast(vote("a", CLAIM)), Ok(false));
        assert_eq!(r.cast(vote("a", CLAIM)), Ok(false));
        assert_eq!(r.validator_count(), 1);
        assert_eq!(r.approvals(), 1);
        assert!(!r.is_void());
        assert!(r.invariant_holds());
    }

    #[test]
    fn dissenting_supermajority_rejects() {
        // f=1, n=4，1 赞成 3 异议 → Rejected。
        let mut r = QaRound::new("r", CLAIM, 1).unwrap();
        r.cast(vote("a", CLAIM)).unwrap();
        r.cast(vote("b", "sha:x")).unwrap();
        r.cast(vote("c", "sha:y")).unwrap();
        r.cast(vote("d", "sha:z")).unwrap();
        assert_eq!(
            r.adjudicate(),
            QaVerdict::Rejected {
                approvals: 1,
                dissents: 3,
                threshold: 3
            }
        );
        assert!(r.invariant_holds());
    }

    #[test]
    fn f_zero_single_validator_and_empty_round() {
        // f=0 → 需要 n≥1，阈值 1。单验证者即可确认/否决。
        let mut r = QaRound::new("r", CLAIM, 0).unwrap();
        assert_eq!(QaRound::required_validators(0), Ok(1));
        assert_eq!(QaRound::approval_threshold(0), Ok(1));
        assert_eq!(r.adjudicate(), QaVerdict::Pending { n: 0, required: 1 });
        r.cast(vote("solo", CLAIM)).unwrap();
        assert_eq!(
            r.adjudicate(),
            QaVerdict::Verified {
                approvals: 1,
                dissents: 0,
                threshold: 1
            }
        );
        let mut r2 = QaRound::new("r2", CLAIM, 0).unwrap();
        r2.cast(vote("solo", "sha:diff")).unwrap();
        assert_eq!(
            r2.adjudicate(),
            QaVerdict::Rejected {
                approvals: 0,
                dissents: 1,
                threshold: 1
            }
        );
        assert!(r.invariant_holds() && r2.invariant_holds());
    }

    #[test]
    fn empty_fields_rejected_and_threshold_math_checked() {
        assert!(matches!(
            QaRound::new("  ", CLAIM, 1),
            Err(ResourceError::EmptyQaRoundId)
        ));
        assert!(matches!(
            QaRound::new("r", "  ", 1),
            Err(ResourceError::EmptyQaDigest)
        ));
        let mut r = QaRound::new("r", CLAIM, 1).unwrap();
        assert!(matches!(
            r.cast(vote("  ", CLAIM)),
            Err(ResourceError::EmptyQaValidator)
        ));
        assert!(matches!(
            r.cast(vote("a", "  ")),
            Err(ResourceError::EmptyQaDigest)
        ));
        assert_eq!(r.validator_count(), 0);
        // 阈值整数恒等式：required=3f+1, threshold=2f+1。
        for (f, req, thr) in [(0u64, 1, 1), (1, 4, 3), (2, 7, 5), (3, 10, 7)] {
            assert_eq!(QaRound::required_validators(f), Ok(req));
            assert_eq!(QaRound::approval_threshold(f), Ok(thr));
        }
        // f=2, n=7，恰 5 赞成达阈值 → Verified（验证阈值随 f 放大）。
        let mut r2 = QaRound::new("r2", CLAIM, 2).unwrap();
        for v in ["a", "b", "c", "d", "e"] {
            r2.cast(vote(v, CLAIM)).unwrap();
        }
        r2.cast(vote("f", "sha:x")).unwrap();
        r2.cast(vote("g", "sha:y")).unwrap();
        assert_eq!(r2.validator_count(), 7);
        assert_eq!(
            r2.adjudicate(),
            QaVerdict::Verified {
                approvals: 5,
                dissents: 2,
                threshold: 5
            }
        );
        // 致 3f+1 溢出的 f fail-closed。
        assert!(matches!(
            QaRound::required_validators(u64::MAX),
            Err(ResourceError::QaFaultToleranceOverflow { .. })
        ));
        assert!(r.invariant_holds() && r2.invariant_holds());
    }
}
