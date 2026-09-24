//! ACA Agent 间通讯消息
//!
//! 兼容 A2A（Agent-to-Agent）协议的消息格式

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::manifest::AgentManifest;
use super::envelope::TaskEnvelope;
use super::receipt::Receipt;

/// 消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// 握手
    Handshake,
    /// 能力发现请求
    DiscoveryRequest,
    /// 能力发现响应
    DiscoveryResponse,
    /// 任务提议
    TaskProposal,
    /// 任务接受
    TaskAccept,
    /// 任务拒绝
    TaskReject,
    /// 任务结果
    TaskResult,
    /// 验证请求
    VerificationRequest,
    /// 验证结果
    VerificationResult,
    /// 收据
    Receipt,
    /// 声誉查询
    ReputationQuery,
    /// 声誉响应
    ReputationResponse,
}

/// ACA 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcaMessage {
    pub message_id: String,
    pub msg_type: MessageType,
    pub from_did: String,
    pub to_did: String,
    /// 消息内容（根据类型解析为对应对象）
    pub payload: serde_json::Value,
    /// 时间戳
    pub timestamp: u64,
    /// 签名
    pub signature: String,
}

impl AcaMessage {
    pub fn new(msg_type: MessageType, from_did: String, to_did: String, payload: serde_json::Value) -> Self {
        Self {
            message_id: Uuid::new_v4().to_string(),
            msg_type,
            from_did,
            to_did,
            payload,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: String::new(),
        }
    }

    /// 创建握手消息（携带 Manifest）
    pub fn handshake(from: String, manifest: AgentManifest) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(&manifest)?;
        Ok(Self::new(MessageType::Handshake, from, "*".to_string(), payload))
    }

    /// 创建任务提议
    pub fn propose_task(from: String, to: String, envelope: TaskEnvelope) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(&envelope)?;
        Ok(Self::new(MessageType::TaskProposal, from, to, payload))
    }

    /// 创建任务结果（携带 Receipt）
    pub fn deliver_receipt(from: String, to: String, receipt: Receipt) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(&receipt)?;
        Ok(Self::new(MessageType::Receipt, from, to, payload))
    }

    /// 解析为 AgentManifest
    pub fn parse_manifest(&self) -> Result<AgentManifest, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }

    /// 解析为 TaskEnvelope
    pub fn parse_envelope(&self) -> Result<TaskEnvelope, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }

    /// 解析为 Receipt
    pub fn parse_receipt(&self) -> Result<Receipt, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }

    /// 检查是否是广播消息
    pub fn is_broadcast(&self) -> bool {
        self.to_did == "*"
    }
}
