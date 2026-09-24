//! BFT-lite QA 委员会
//!
//! n ≥ 3f+1，法定人数 q = 2f+1
//! 同轮 equivocation 整轮作废
//! 沉默 > f → NO_QUORUM + view_change

use serde::{Deserialize, Serialize};

/// QA 投票
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QaVote {
    /// 结果合格，停止
    Stop,
    /// 需要继续 / 返工
    Continue,
    /// 未投票（沉默）
    Silent,
}

/// QA 委员
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaMember {
    pub did: String,
    pub vote: QaVote,
    /// 是否在本轮投了多个不同票（equivocation）
    pub equivocated: bool,
}

/// QA 投票结果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QaDecision {
    /// 通过（STOP 票 ≥ q）
    Stop,
    /// 不通过（CONTINUE 票 ≥ q）
    Continue,
    /// 无共识（沉默 > f 或 equivocation）
    NoQuorum,
}

/// BFT-lite QA 委员会
#[derive(Debug, Clone)]
pub struct QaCommittee {
    members: Vec<QaMember>,
    n: u32,
    f: u32,
}

impl QaCommittee {
    /// 创建委员会
    ///
    /// 要求 n ≥ 3f+1
    pub fn new(n: u32, f: u32) -> Result<Self, String> {
        if n < 3 * f + 1 {
            return Err(format!(
                "BFT-lite 要求 n ≥ 3f+1，当前 n={}, f={}, 需要 n≥{}",
                n, f,
                3 * f + 1
            ));
        }
        Ok(Self {
            members: Vec::new(),
            n,
            f,
        })
    }

    /// 添加委员
    pub fn add_member(&mut self, did: String) {
        if self.members.len() < self.n as usize {
            self.members.push(QaMember {
                did,
                vote: QaVote::Silent,
                equivocated: false,
            });
        }
    }

    /// 记录投票
    pub fn cast_vote(&mut self, did: &str, vote: QaVote) -> Result<(), String> {
        let member = self
            .members
            .iter_mut()
            .find(|m| m.did == did)
            .ok_or_else(|| format!("委员 {} 不存在", did))?;

        // 检测 equivocation：已投过票且新票不同
        if member.vote != QaVote::Silent && member.vote != vote {
            member.equivocated = true;
            return Ok(());
        }
        member.vote = vote;
        Ok(())
    }

    /// 统计决策
    pub fn tally(&self) -> QaDecision {
        // 如果有 equivocation，整轮作废
        if self.members.iter().any(|m| m.equivocated) {
            return QaDecision::NoQuorum;
        }

        let stop_count = self
            .members
            .iter()
            .filter(|m| m.vote == QaVote::Stop)
            .count() as u32;
        let continue_count = self
            .members
            .iter()
            .filter(|m| m.vote == QaVote::Continue)
            .count() as u32;
        let silent_count = self
            .members
            .iter()
            .filter(|m| m.vote == QaVote::Silent)
            .count() as u32;

        let quorum = 2 * self.f + 1;

        // 沉默 > f → 无共识
        if silent_count > self.f {
            return QaDecision::NoQuorum;
        }

        if stop_count >= quorum {
            QaDecision::Stop
        } else if continue_count >= quorum {
            QaDecision::Continue
        } else {
            QaDecision::NoQuorum
        }
    }

    /// 重置投票（view change 后）
    pub fn reset_votes(&mut self) {
        for member in &mut self.members {
            member.vote = QaVote::Silent;
            member.equivocated = false;
        }
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn n(&self) -> u32 {
        self.n
    }

    pub fn f(&self) -> u32 {
        self.f
    }

    pub fn quorum(&self) -> u32 {
        2 * self.f + 1
    }
}
