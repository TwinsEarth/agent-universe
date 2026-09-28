//! Proof of Contribution（贡献证明）
//! 
//! 不同于 PoW 的计算浪费，PoC 证明你为网络做了实际有用的贡献
//! 贡献越大，获得的信誉和奖励越多

use sha2::{Sha256, Digest};
use crate::identity::Ed25519Signer;

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
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut hasher = Sha256::new();
        hasher.update(agent_did.as_bytes());
        hasher.update(task_id.as_bytes());
        hasher.update(contribution_type.as_bytes());
        hasher.update(value.to_le_bytes());
        hasher.update(now.to_le_bytes());
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

    /// 本地受信环境下的贡献验证（无签名）。
    ///
    /// v2.6.4（GAP §3.6）已加：去重（同一验证者不重复计）、禁自验、饱和计数。
    /// 跨网络不可信环境请用 [`verify_contribution_signed`](Self::verify_contribution_signed)。
    pub fn verify_contribution(&mut self, hash: &[u8; 32], verifier_did: String) -> bool {
        let record = match self.records.iter_mut().find(|r| &r.hash == hash) {
            Some(r) => r,
            None => return false,
        };
        // 禁自验
        if verifier_did == record.agent_did {
            return false;
        }
        // 去重：同一验证者只计一次，禁止重复凑数
        if record.verifiers.contains(&verifier_did) {
            return record.verification_count >= self.min_verifications;
        }
        record.verifiers.push(verifier_did);
        record.verification_count = record.verification_count.saturating_add(1);
        record.verification_count >= self.min_verifications
    }

    /// 带签名身份的贡献验证（GAP §3.6，跨网络不可信环境）。
    ///
    /// - `verifier_did`：验证者 DID；
    /// - `verifier_pubkey`：验证者 32 字节公钥；
    /// - `signature`：验证者对 `hash || verifier_did` 的 Ed25519 签名。
    ///
    /// 规则：记录存在、禁自验、去重、验签通过、饱和计数。任一不满足返回 `Err`。
    pub fn verify_contribution_signed(
        &mut self,
        hash: &[u8; 32],
        verifier_did: &str,
        verifier_pubkey: &[u8; 32],
        signature: &[u8],
    ) -> Result<bool, String> {
        let record = self
            .records
            .iter_mut()
            .find(|r| &r.hash == hash)
            .ok_or_else(|| "贡献记录不存在".to_string())?;

        if verifier_did == record.agent_did {
            return Err("禁止自验：贡献者不能验证自己的贡献".to_string());
        }
        if record.verifiers.iter().any(|d| d == verifier_did) {
            return Err("重复验证：同一验证者已计过此贡献（禁止凑数）".to_string());
        }

        // 签名覆盖 hash || verifier_did
        let mut msg = Vec::with_capacity(32 + verifier_did.len());
        msg.extend_from_slice(hash);
        msg.extend_from_slice(verifier_did.as_bytes());
        if !Ed25519Signer::verify_with_pubkey(verifier_pubkey, &msg, signature) {
            return Err("验证者签名无效".to_string());
        }

        record.verifiers.push(verifier_did.to_string());
        record.verification_count = record.verification_count.saturating_add(1);
        Ok(record.verification_count >= self.min_verifications)
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
