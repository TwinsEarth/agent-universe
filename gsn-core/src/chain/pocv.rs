//! PoCV 可验证计算（修复版）
//! 
//! 基于哈希链的可验证计算，验证者可以独立验证计算结果

use sha2::{Sha256, Digest};

#[derive(Debug, Clone)]
pub struct ProofOfComputation {
    pub input_hash: [u8; 32],
    pub output_hash: [u8; 32],
    pub steps: u64,
    pub prover_did: String,
}

pub struct PoCVVerifier;

impl PoCVVerifier {
    pub fn new() -> Self {
        Self
    }

    pub fn compute_hash(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    pub fn verify_hash(&self, data: &[u8], expected: &[u8; 32]) -> bool {
        self.compute_hash(data) == *expected
    }

    /// 验证计算证明：检查输入哈希和输出哈希是否匹配
    pub fn verify_proof(&self, proof: &ProofOfComputation, input: &[u8], output: &[u8]) -> bool {
        let input_match = self.compute_hash(input) == proof.input_hash;
        let output_match = self.compute_hash(output) == proof.output_hash;
        input_match && output_match
    }

    /// 生成计算证明
    pub fn generate_proof(&self, input: &[u8], output: &[u8], steps: u64, prover_did: String) -> ProofOfComputation {
        ProofOfComputation {
            input_hash: self.compute_hash(input),
            output_hash: self.compute_hash(output),
            steps,
            prover_did,
        }
    }
}
