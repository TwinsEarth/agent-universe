//! 贡献证明（Proof of Contribution）
//! 
//! 智能体为网络做了多少贡献，就获得多少信誉
//! 不同于 PoW 的计算浪费，PoC 是实际有用的贡献

use sha2::{Sha256, Digest};

#[derive(Debug, Clone)]
pub struct ContributionProof {
    pub agent_did: String,
    pub contribution_type: ContributionType,
    pub task_id: String,
    pub value: u64,
    pub timestamp: u64,
    pub proof_hash: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContributionType {
    /// 完成任务
    TaskCompletion,
    /// 提供计算资源
    ComputeProvided,
    /// 提供存储
    StorageProvided,
    /// 验证其他节点
    Verification,
    /// 传播知识
    KnowledgeShare,
    /// 帮助新节点
    Onboarding,
}

impl ContributionType {
    pub fn weight(&self) -> u64 {
        match self {
            ContributionType::TaskCompletion => 10,
            ContributionType::ComputeProvided => 5,
            ContributionType::StorageProvided => 2,
            ContributionType::Verification => 8,
            ContributionType::KnowledgeShare => 6,
            ContributionType::Onboarding => 15,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ContributionType::TaskCompletion => "完成任务",
            ContributionType::ComputeProvided => "提供计算资源",
            ContributionType::StorageProvided => "提供存储",
            ContributionType::Verification => "验证其他节点",
            ContributionType::KnowledgeShare => "传播知识",
            ContributionType::Onboarding => "帮助新节点加入",
        }
    }
}

impl ContributionProof {
    pub fn new(
        agent_did: String,
        contribution_type: ContributionType,
        task_id: String,
        value: u64,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut hasher = Sha256::new();
        hasher.update(agent_did.as_bytes());
        hasher.update(&[contribution_type as u8]);
        hasher.update(task_id.as_bytes());
        hasher.update(&value.to_le_bytes());
        hasher.update(&now.to_le_bytes());
        let proof_hash = hasher.finalize().into();

        Self {
            agent_did,
            contribution_type,
            task_id,
            value,
            timestamp: now,
            proof_hash,
            signature: Vec::new(),
        }
    }

    pub fn sign(&mut self, signature: Vec<u8>) {
        self.signature = signature;
    }

    pub fn effective_value(&self) -> u64 {
        self.value * self.contribution_type.weight()
    }

    pub fn verify_hash(&self) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(self.agent_did.as_bytes());
        hasher.update(&[self.contribution_type as u8]);
        hasher.update(self.task_id.as_bytes());
        hasher.update(&self.value.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.finalize().as_slice() == self.proof_hash
    }
}
