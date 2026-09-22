//! Proof of Contribution（贡献证明）
//! 
//! 不同于 PoW 的计算浪费，PoC 证明你为网络做了实际有用的贡献
//! 贡献越大，获得的信誉和奖励越多

use sha2::{Sha256, Digest};

#[derive(Debug, Clone)]
pub struct ContributionRecord {
    pub agent_did: String,
    pub task_id: String,
    pub contribution_type: String,
    pub value: u64,
    pub timestamp: u64,
    pub hash: [u8; 32],
    pub verifiers: Vec<String>,
    pub verification_count: u8,
}

pub struct ProofOfContribution {
    records: Vec<ContributionRecord>,
    min_verifications: u8,
}

impl ProofOfContribution {
    pub fn new(min_verifications: u8) -> Self {
        Self {
            records: Vec::new(),
            min_verifications,
        }
    }

    pub fn submit_contribution(
        &mut self,
        agent_did: String,
        task_id: String,
        contribution_type: String,
        value: u64,
    ) -> [u8; 32] {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut hasher = Sha256::new();
        hasher.update(agent_did.as_bytes());
        hasher.update(task_id.as_bytes());
        hasher.update(contribution_type.as_bytes());
        hasher.update(&value.to_le_bytes());
        hasher.update(&now.to_le_bytes());
        let hash = hasher.finalize().into();

        self.records.push(ContributionRecord {
            agent_did,
            task_id,
            contribution_type,
            value,
            timestamp: now,
            hash,
            verifiers: Vec::new(),
            verification_count: 0,
        });

        hash
    }

    pub fn verify_contribution(&mut self, hash: &[u8; 32], verifier_did: String) -> bool {
        for record in &mut self.records {
            if &record.hash == hash {
                record.verifiers.push(verifier_did);
                record.verification_count += 1;
                return record.verification_count >= self.min_verifications;
            }
        }
        false
    }

    pub fn is_valid(&self, hash: &[u8; 32]) -> bool {
        self.records.iter()
            .find(|r| &r.hash == hash)
            .map(|r| r.verification_count >= self.min_verifications)
            .unwrap_or(false)
    }

    pub fn total_contributions(&self, agent_did: &str) -> u64 {
        self.records.iter()
            .filter(|r| r.agent_did == agent_did && r.verification_count >= self.min_verifications)
            .map(|r| r.value)
            .sum()
    }

    pub fn contribution_count(&self) -> usize {
        self.records.len()
    }

    pub fn verified_count(&self) -> usize {
        self.records.iter()
            .filter(|r| r.verification_count >= self.min_verifications)
            .count()
    }
}
