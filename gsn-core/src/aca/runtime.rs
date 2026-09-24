//! ACA 运行时编排
//!
//! 把 ACA 的静态协议对象串成真实、可验证的运行流程：
//!
//! ```text
//! register_peer（带外公钥，校验 DID=公钥指纹）
//!   → Handshake：验消息签名 → 验 manifest 签名 → 存 manifest
//!   → TaskProposal：验 envelope → 能力/预算/验证/隐私决策 accept/reject
//!   → execute_task：执行回调 → 生成签名 Receipt → 更新声誉
//!   → 对端 Receipt：验签 → 按可验证收据客观记账
//!   → submit_review：主观评价
//! ```
//!
//! 所有跨节点对象（消息 / manifest / receipt）都经过 Ed25519 验签；
//! 公钥由带外或 DHT 获得，且必须与其 DID 指纹一致，防止冒名。

use std::collections::HashMap;

use crate::identity::{Did, Ed25519Signer, Keypair};

use super::envelope::{PrivacyRequirement, TaskEnvelope};
use super::manifest::AgentManifest;
use super::message::{AcaMessage, MessageType};
use super::receipt::{Receipt, ReceiptStatus, ResourceMetering};
use super::reputation::{MultiReputation, ReputationDimension};

/// 任务执行回调的输出
#[derive(Debug, Clone)]
pub struct ExecOutput {
    /// 执行结果字节
    pub result: Vec<u8>,
    /// 是否成功
    pub success: bool,
    /// 计算耗时（毫秒）
    pub compute_ms: u64,
    /// 峰值内存（MB）
    pub memory_peak_mb: u64,
}

impl ExecOutput {
    pub fn ok(result: impl Into<Vec<u8>>) -> Self {
        Self {
            result: result.into(),
            success: true,
            compute_ms: 0,
            memory_peak_mb: 0,
        }
    }

    pub fn failed() -> Self {
        Self {
            result: Vec::new(),
            success: false,
            compute_ms: 0,
            memory_peak_mb: 0,
        }
    }
}

/// 任务执行回调：接收 envelope，返回执行输出
pub type TaskExecutor = Box<dyn Fn(&TaskEnvelope) -> ExecOutput + Send + Sync>;

/// 对任务提议的决策
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcaDecision {
    Accept,
    Reject(String),
}

/// 消息/任务处理结果
#[derive(Debug, Clone)]
pub enum ProcessOutcome {
    /// 握手已验证（对端 DID）
    HandshakeVerified(String),
    /// 任务已接受
    TaskAccepted { task_id: String },
    /// 任务被拒绝
    TaskRejected { task_id: String, reason: String },
    /// 任务已执行，产出收据
    TaskCompleted { task_id: String, receipt: Receipt },
    /// 消息/请求被拒绝（未进入流程）
    Rejected { reason: String },
}

/// ACA 协议编排处理器
pub struct AcaProcessor {
    /// 本节点 DID
    local_did: String,
    /// 本节点密钥
    keypair: Keypair,
    /// 本节点声明的能力
    capabilities: Vec<String>,
    /// 已知对端公钥：DID -> 32 字节公钥
    peers: HashMap<String, Vec<u8>>,
    /// 已知对端 manifest：DID -> manifest
    manifests: HashMap<String, AgentManifest>,
    /// 各方声誉档案：DID -> reputation
    reputations: HashMap<String, MultiReputation>,
    /// 已接受、待执行的任务：task_id -> envelope
    pending: HashMap<String, TaskEnvelope>,
    /// 任务执行器
    executor: Option<TaskExecutor>,
    /// 是否支持 TEE
    supports_tee: bool,
    /// 是否支持 ZK
    supports_zk: bool,
    /// 是否支持差分隐私
    supports_dp: bool,
}

impl AcaProcessor {
    /// 用本节点密钥创建处理器，DID 由公钥指纹派生
    pub fn new(keypair: Keypair) -> Self {
        let local_did = Did::from_public_key(keypair.public_key()).to_string();
        Self {
            local_did,
            keypair,
            capabilities: Vec::new(),
            peers: HashMap::new(),
            manifests: HashMap::new(),
            reputations: HashMap::new(),
            pending: HashMap::new(),
            executor: None,
            supports_tee: false,
            supports_zk: false,
            supports_dp: false,
        }
    }

    /// 声明本节点能力
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// 注册任务执行器
    pub fn with_executor(mut self, executor: TaskExecutor) -> Self {
        self.executor = Some(executor);
        self
    }

    pub fn with_tee(mut self) -> Self {
        self.supports_tee = true;
        self
    }

    pub fn with_zk(mut self) -> Self {
        self.supports_zk = true;
        self
    }

    pub fn with_differential_privacy(mut self) -> Self {
        self.supports_dp = true;
        self
    }

    pub fn local_did(&self) -> &str {
        &self.local_did
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn manifest_count(&self) -> usize {
        self.manifests.len()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// 注册对端公钥（带外或 DHT 获得）
    ///
    /// 校验公钥的 DID 指纹与传入 DID 一致，不一致则拒绝注册。
    pub fn register_peer(&mut self, did: &str, pubkey: &[u8]) -> Result<(), String> {
        let derived = Did::from_public_key(pubkey).to_string();
        if derived != did {
            return Err(format!(
                "公钥与 DID 不匹配：DID={did}, 公钥指纹对应={derived}"
            ));
        }
        self.peers.insert(did.to_string(), pubkey.to_vec());
        Ok(())
    }

    /// 获取某方综合声誉分
    pub fn reputation_of(&mut self, did: &str) -> f64 {
        self.reputations
            .entry(did.to_string())
            .or_insert_with(|| MultiReputation::new(did.to_string()))
            .overall()
    }

    /// 处理一条入站 ACA 消息
    pub fn process_message(&mut self, msg: &AcaMessage) -> ProcessOutcome {
        // 取发送者公钥
        let pubkey = match self.peers.get(&msg.from_did) {
            Some(k) => k.clone(),
            None => {
                return ProcessOutcome::Rejected {
                    reason: format!("未知对端公钥: {}", msg.from_did),
                };
            }
        };

        // 验消息签名
        if !msg.verify(&pubkey) {
            return ProcessOutcome::Rejected {
                reason: format!("消息签名无效: {}", msg.message_id),
            };
        }

        match msg.msg_type {
            MessageType::Handshake => self.handle_handshake(msg, &pubkey),
            MessageType::TaskProposal => self.handle_proposal(msg),
            MessageType::Receipt => self.handle_incoming_receipt(msg, &pubkey),
            other => ProcessOutcome::Rejected {
                reason: format!("暂不处理的消息类型: {:?}", other),
            },
        }
    }

    /// 处理握手：验 manifest 并登记
    fn handle_handshake(&mut self, msg: &AcaMessage, pubkey: &[u8]) -> ProcessOutcome {
        let manifest = match msg.parse_manifest() {
            Ok(m) => m,
            Err(e) => {
                return ProcessOutcome::Rejected {
                    reason: format!("manifest 解析失败: {e}"),
                };
            }
        };

        // 验 manifest 签名
        if !manifest.verify(pubkey) {
            return ProcessOutcome::Rejected {
                reason: "manifest 签名无效".to_string(),
            };
        }
        // manifest 声称的 DID 必须与发送者一致
        if manifest.did != msg.from_did {
            return ProcessOutcome::Rejected {
                reason: "manifest DID 与发送者不一致".to_string(),
            };
        }

        let did = manifest.did.clone();
        self.manifests.insert(did.clone(), manifest);
        self.reputations
            .entry(did.clone())
            .or_insert_with(|| MultiReputation::new(did.clone()));
        ProcessOutcome::HandshakeVerified(did)
    }

    /// 处理任务提议：解析 envelope 并决策
    fn handle_proposal(&mut self, msg: &AcaMessage) -> ProcessOutcome {
        let envelope = match msg.parse_envelope() {
            Ok(e) => e,
            Err(e) => {
                return ProcessOutcome::Rejected {
                    reason: format!("envelope 解析失败: {e}"),
                };
            }
        };

        if envelope.requester_did != msg.from_did {
            return ProcessOutcome::Rejected {
                reason: "envelope 请求方 DID 与发送者不一致".to_string(),
            };
        }

        let task_id = envelope.task_id.clone();
        match self.decide(&envelope) {
            AcaDecision::Accept => {
                self.pending.insert(task_id.clone(), envelope);
                ProcessOutcome::TaskAccepted { task_id }
            }
            AcaDecision::Reject(reason) => ProcessOutcome::TaskRejected { task_id, reason },
        }
    }

    /// 依据能力 / 预算 / 验证 / 隐私对 envelope 决策
    fn decide(&self, envelope: &TaskEnvelope) -> AcaDecision {
        // 能力匹配
        if !self.capabilities.contains(&envelope.capability) {
            return AcaDecision::Reject(format!(
                "本节点不具备能力: {}",
                envelope.capability
            ));
        }
        // 预算须为正
        if envelope.budget == 0 {
            return AcaDecision::Reject("任务预算为 0".to_string());
        }
        // 验证成本须可负担
        if !envelope.verification_affordable() {
            return AcaDecision::Reject(format!(
                "预算 {} 不足以覆盖 {:?} 级验证成本（预估 {}）",
                envelope.budget,
                envelope.verification_level,
                envelope.estimated_verification_cost()
            ));
        }
        // 隐私要求
        match envelope.privacy {
            PrivacyRequirement::Public | PrivacyRequirement::LocalOnly => {}
            PrivacyRequirement::DifferentialPrivacy if self.supports_dp => {}
            PrivacyRequirement::TEE if self.supports_tee => {}
            PrivacyRequirement::ZK if self.supports_zk => {}
            other => {
                return AcaDecision::Reject(format!("本节点不满足隐私要求: {:?}", other));
            }
        }
        AcaDecision::Accept
    }

    /// 执行已接受的任务，生成签名收据并更新本节点可用性声誉
    pub fn execute_task(&mut self, task_id: &str) -> ProcessOutcome {
        let envelope = match self.pending.get(task_id) {
            Some(e) => e.clone(),
            None => {
                return ProcessOutcome::Rejected {
                    reason: format!("没有已接受的任务: {task_id}"),
                };
            }
        };

        let executor = match &self.executor {
            Some(e) => e,
            None => {
                return ProcessOutcome::Rejected {
                    reason: "未注册任务执行器".to_string(),
                };
            }
        };

        let output = executor(&envelope);

        // 构造并签名收据
        let signer = Ed25519Signer::new(&self.keypair);
        let mut receipt =
            Receipt::new(task_id.to_string(), self.local_did.clone(), &output.result);
        receipt.metering = ResourceMetering {
            compute_ms: output.compute_ms,
            memory_peak_mb: output.memory_peak_mb,
            ..ResourceMetering::default()
        };

        let local = self.local_did.clone();
        let rep = self
            .reputations
            .entry(local.clone())
            .or_insert_with(|| MultiReputation::new(local));

        if output.success {
            receipt.sign(&signer);
            // 客观：成功交付，可用性加分（质量留给请求方主观评价）
            rep.record_success(ReputationDimension::Availability, 50.0);
            self.pending.remove(task_id);
            ProcessOutcome::TaskCompleted {
                task_id: task_id.to_string(),
                receipt,
            }
        } else {
            // 失败：可用性扣分，收据标记 Failed 并签名留证
            receipt.status = ReceiptStatus::Failed;
            receipt.sign(&signer);
            rep.record_failure(ReputationDimension::Availability, 50.0);
            self.pending.remove(task_id);
            ProcessOutcome::TaskRejected {
                task_id: task_id.to_string(),
                reason: "执行器返回失败".to_string(),
            }
        }
    }

    /// 处理对端发来的收据：验签后按可验证收据客观记账
    fn handle_incoming_receipt(
        &mut self,
        msg: &AcaMessage,
        pubkey: &[u8],
    ) -> ProcessOutcome {
        let receipt = match msg.parse_receipt() {
            Ok(r) => r,
            Err(e) => {
                return ProcessOutcome::Rejected {
                    reason: format!("receipt 解析失败: {e}"),
                };
            }
        };

        if !receipt.verify(pubkey) {
            return ProcessOutcome::Rejected {
                reason: "receipt 签名无效".to_string(),
            };
        }
        if receipt.executor_did != msg.from_did {
            return ProcessOutcome::Rejected {
                reason: "receipt 执行方 DID 与发送者不一致".to_string(),
            };
        }

        let executor = receipt.executor_did.clone();
        let rep = self
            .reputations
            .entry(executor.clone())
            .or_insert_with(|| MultiReputation::new(executor.clone()));

        match receipt.status {
            ReceiptStatus::Completed | ReceiptStatus::Verified => {
                rep.record_success(ReputationDimension::Quality, 60.0);
                rep.record_success(ReputationDimension::Availability, 40.0);
            }
            ReceiptStatus::Failed => {
                rep.record_failure(ReputationDimension::Availability, 60.0);
            }
            ReceiptStatus::Disputed | ReceiptStatus::Arbitrated => {
                rep.record_failure(ReputationDimension::Honesty, 80.0);
            }
        }

        ProcessOutcome::TaskCompleted {
            task_id: receipt.task_id.clone(),
            receipt,
        }
    }

    /// 主观评价：调用方对执行方某维度评分
    ///
    /// `score`：正数表示好评（加分），负数表示差评（扣分）。
    pub fn submit_review(
        &mut self,
        target_did: &str,
        dim: ReputationDimension,
        score: f64,
    ) -> Result<(), String> {
        if !self.peers.contains_key(target_did) && !self.manifests.contains_key(target_did) {
            return Err(format!("未知的被评价方: {target_did}"));
        }
        let rep = self
            .reputations
            .entry(target_did.to_string())
            .or_insert_with(|| MultiReputation::new(target_did.to_string()));
        if score >= 0.0 {
            rep.record_success(dim, score.min(200.0));
        } else {
            rep.record_failure(dim, (-score).min(200.0));
        }
        Ok(())
    }
}
