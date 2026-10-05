//! 轻量级共识机制
//!
//! 不追求 PoW 的安全性，而是追求快速、低成本的网络共识
//! 适合智能体网络的日常决策
//!
//! # ⚠️ 未接线（v3.5.3，AU-20）
//!
//! 本模块当前**未接入 daemon 运行图**，无任何生产调用点。接入前**必须**满足三项前提：
//! 投票权重等于已质押 stake（而非调用方自报的任意 weight）；每票/每轮 nonce 去重以防重放；
//! 每票用委员 DID 私钥验签（拒绝自报投票）。在这三项落地前，不得把本模块的"通过"结果
//! 当作可信共识。
//!
//! # 已落地的正确性约束（v3.5.5，P1）
//!
//! - **一人一票**：同一 voter 对同一 proposal 只能投一次，重复投票返回 `Err`，
//!   既不替换旧票也不把权重按票数累加（防重放 / 多投凑票）。
//! - 本模块**仍是轻量共识而非 BFT**：`weight` 仍由调用方传入（自报），
//!   超多数口径也只是 `total_stake * quorum_ratio` 的简单阈值。
//!   去重只保证"同一票不被重复计数"，不保证 weight 本身可信——后者需绑定 stake + 验签。

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Vote {
    pub voter: String,
    pub approve: bool,
    pub weight: u64,
}

#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: String,
    pub proposer: String,
    pub description: String,
    pub votes: Vec<Vote>,
    pub created_at: u64,
    pub ttl_blocks: u64,
}

#[derive(Debug, Clone)]
pub struct ConsensusResult {
    pub proposal_id: String,
    pub total_votes: u64,
    pub approve_weight: u64,
    pub reject_weight: u64,
    pub quorum: u64,
    pub consensus_reached: bool,
    pub accepted: bool,
}

pub struct LightweightConsensus {
    proposals: HashMap<String, Proposal>,
    total_stake: u64,
    quorum_ratio: f64,
}

impl LightweightConsensus {
    pub fn new(total_stake: u64, quorum_ratio: f64) -> Self {
        Self {
            proposals: HashMap::new(),
            total_stake,
            quorum_ratio,
        }
    }

    pub fn propose(&mut self, id: String, proposer: String, description: String, ttl_blocks: u64) {
        self.proposals.insert(
            id.clone(),
            Proposal {
                id,
                proposer,
                description,
                votes: Vec::new(),
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                ttl_blocks,
            },
        );
    }

    /// 投一票。
    ///
    /// 去重口径（一人一票，v3.5.5 P1）：
    /// - 同一 `voter` 对同一 `proposal_id` **只能投一次**；重复投票返回 `Err`，
    ///   不替换旧票、也不把权重按票数累加（防重放 / 多投凑票）。
    /// - 对不存在的 proposal 投票也返回 `Err`（静默丢弃会让调用方误以为已计入）。
    ///
    /// 注意：`weight` 仍由调用方自报，本函数只负责"同一票不重复计数"，
    /// 不负责校验 weight 是否等于真实质押——那是接入生产前必须补的前提。
    pub fn vote(
        &mut self,
        proposal_id: &str,
        voter: String,
        approve: bool,
        weight: u64,
    ) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| format!("投票失败：提案不存在 '{proposal_id}'"))?;
        if proposal.votes.iter().any(|v| v.voter == voter) {
            return Err(format!(
                "重复投票：voter '{voter}' 已对提案 '{proposal_id}' 投过票（一人一票，拒绝重放）"
            ));
        }
        proposal.votes.push(Vote {
            voter,
            approve,
            weight,
        });
        Ok(())
    }

    pub fn tally(&self, proposal_id: &str) -> Option<ConsensusResult> {
        let proposal = self.proposals.get(proposal_id)?;

        let mut approve_weight = 0u64;
        let mut reject_weight = 0u64;
        let mut total_votes = 0u64;

        for vote in &proposal.votes {
            total_votes += vote.weight;
            if vote.approve {
                approve_weight += vote.weight;
            } else {
                reject_weight += vote.weight;
            }
        }

        let quorum = (self.total_stake as f64 * self.quorum_ratio) as u64;
        let consensus_reached = total_votes >= quorum;
        let accepted = consensus_reached && approve_weight > reject_weight;

        Some(ConsensusResult {
            proposal_id: proposal_id.to_string(),
            total_votes,
            approve_weight,
            reject_weight,
            quorum,
            consensus_reached,
            accepted,
        })
    }

    pub fn active_proposals(&self) -> Vec<&Proposal> {
        self.proposals.values().collect()
    }

    pub fn set_total_stake(&mut self, stake: u64) {
        self.total_stake = stake;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个 total_stake=100、quorum_ratio=0.5（quorum=50）、含提案 "p1" 的共识器。
    fn setup() -> LightweightConsensus {
        let mut c = LightweightConsensus::new(100, 0.5);
        c.propose(
            "p1".into(),
            "proposer".into(),
            "some proposal".into(),
            100,
        );
        c
    }

    #[test]
    fn duplicate_voter_is_rejected_and_not_double_counted() {
        let mut c = setup();
        c.vote("p1", "v1".into(), true, 10).unwrap();
        // 同 voter 再次投票必须返回 Err（拒绝重放/多投）
        let dup = c.vote("p1", "v1".into(), true, 10);
        assert!(dup.is_err(), "同一 voter 重复投票必须被拒绝");
        let r = c.tally("p1").expect("p1 存在");
        // 权重不得按票数累加：仍只计一票的 10。
        assert_eq!(r.total_votes, 10, "重复投票不得让权重按票数累加");
        assert_eq!(r.approve_weight, 10);
        assert_eq!(r.reject_weight, 0);
    }

    #[test]
    fn different_voters_accumulate_weight() {
        let mut c = setup();
        c.vote("p1", "v1".into(), true, 10).unwrap();
        c.vote("p1", "v2".into(), true, 20).unwrap();
        c.vote("p1", "v3".into(), false, 5).unwrap();
        let r = c.tally("p1").unwrap();
        assert_eq!(r.approve_weight, 30);
        assert_eq!(r.reject_weight, 5);
        assert_eq!(r.total_votes, 35);
        // 35 < quorum(50)，未达 quorum，accepted=false（轻量、非 BFT）。
        assert!(!r.consensus_reached);
        assert!(!r.accepted);
    }

    #[test]
    fn reaching_quorum_then_accepting_when_approve_majority() {
        let mut c = setup();
        c.vote("p1", "v1".into(), true, 30).unwrap();
        c.vote("p1", "v2".into(), true, 30).unwrap();
        c.vote("p1", "v3".into(), false, 10).unwrap();
        let r = c.tally("p1").unwrap();
        assert_eq!(r.quorum, 50);
        assert!(r.consensus_reached, "60 >= 50 应达 quorum");
        assert!(r.accepted, "approve(60) > reject(10) 应通过");
    }

    #[test]
    fn vote_on_unknown_proposal_errors() {
        let mut c = setup();
        assert!(c.vote("no-such-proposal", "v1".into(), true, 1).is_err());
        assert!(c.tally("no-such-proposal").is_none());
    }
}
