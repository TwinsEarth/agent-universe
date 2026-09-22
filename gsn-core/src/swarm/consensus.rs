//! 轻量级共识机制
//! 
//! 不追求 PoW 的安全性，而是追求快速、低成本的网络共识
//! 适合智能体网络的日常决策

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
        self.proposals.insert(id.clone(), Proposal {
            id,
            proposer,
            description,
            votes: Vec::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl_blocks,
        });
    }

    pub fn vote(&mut self, proposal_id: &str, voter: String, approve: bool, weight: u64) {
        if let Some(proposal) = self.proposals.get_mut(proposal_id) {
            proposal.votes.push(Vote { voter, approve, weight });
        }
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
