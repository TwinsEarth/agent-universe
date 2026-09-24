//! Receipt（执行收据）
//!
//! 任务执行后的可验证收据

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// 收据状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptStatus {
    /// 已完成
    Completed,
    /// 已验证
    Verified,
    /// 有争议
    Disputed,
    /// 已仲裁
    Arbitrated,
    /// 失败
    Failed,
}

/// 资源计量
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceMetering {
    pub compute_ms: u64,
    pub memory_peak_mb: u64,
    pub bandwidth_mb: f64,
    pub storage_bytes: u64,
    pub energy_joules: f64,
}

/// 执行收据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub task_id: String,
    pub executor_did: String,
    pub result_cid: String,
    pub result_hash: [u8; 32],
    pub status: ReceiptStatus,
    pub metering: ResourceMetering,
    /// TEE 证明（如有）
    pub tee_quote: Option<String>,
    /// zk 证明（如有）
    pub zk_proof: Option<String>,
    /// 执行时间戳
    pub completed_at: u64,
    /// 执行方签名
    pub signature: String,
}

impl Receipt {
    pub fn new(
        task_id: String,
        executor_did: String,
        result: &[u8],
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(result);
        let result_hash: [u8; 32] = hasher.finalize().into();

        Self {
            task_id,
            executor_did,
            result_cid: format!("cid:{}", hex::encode(&result_hash[..8])),
            result_hash,
            status: ReceiptStatus::Completed,
            metering: ResourceMetering::default(),
            tee_quote: None,
            zk_proof: None,
            completed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: String::new(),
        }
    }

    pub fn with_metering(mut self, metering: ResourceMetering) -> Self {
        self.metering = metering;
        self
    }

    pub fn with_tee_quote(mut self, quote: String) -> Self {
        self.tee_quote = Some(quote);
        self
    }

    pub fn mark_verified(&mut self) {
        self.status = ReceiptStatus::Verified;
    }

    pub fn mark_disputed(&mut self) {
        self.status = ReceiptStatus::Disputed;
    }

    /// 验证结果哈希
    pub fn verify_result(&self, result: &[u8]) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(result);
        hasher.finalize().as_slice() == self.result_hash.as_slice()
    }

    /// 是否需要仲裁
    pub fn needs_arbitration(&self) -> bool {
        self.status == ReceiptStatus::Disputed
    }
}
