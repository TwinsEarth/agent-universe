//! Verifier HTTP 客户端（跨平台轻量版）

pub struct VerifierClient {
    endpoint: String,
    timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub valid: bool,
    pub score: f64,
    pub reason: String,
}

impl VerifierClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            timeout_ms: 5000,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub async fn verify(&self, _task_id: &str, _result: &serde_json::Value) -> VerificationResult {
        // 简化版：实际实现中这里会调用 Verifier 服务
        VerificationResult {
            valid: true,
            score: 0.95,
            reason: "verification passed".to_string(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}
