//! 账户抽象（ERC-4337 Paymaster）与 x402 支付网关的载荷构造 / 响应解析。
//!
//! # 边界
//!
//! 本模块**只**做结构化载荷与解析，**不发任何 HTTP 网络请求**。真实的 Paymaster /
//! x402 facilitator 调用由 [`crate::chain::rpc`] 在拿到显式端点后执行；这里只定义
//! 清楚请求/响应 schema 与 trait。
//!
//! # 概念
//!
//! - **ERC-4337 Paymaster**：代付 gas 的合约；本节点作为赞助方，构造一个
//!   `pmUserOp` 载荷描述「愿意为哪笔 UserOperation 出多少 gas」。
//! - **x402**：HTTP 402 Payment Required 协议——服务端回 402 并附带
//!   `accept` 支付要求，客户端据此付小额 USDC 后重放请求。
//!
//! # 需外部审计
//!
//! 任何与真实 Paymaster / facilitator 的资金交互上线前需外部审计；本模块仅为 schema。

use serde::{Deserialize, Serialize};

use crate::chain::config::ChainError;

/// ERC-4337 UserOperation 的最小必要字段（本节点只构造，不校验 calldata）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserOp {
    pub sender: String,
    pub nonce: String,
    pub call_data: String,
    pub call_gas_limit: u64,
    pub verification_gas_limit: u64,
    pub pre_verification_gas: u64,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
}

/// 请求 Paymaster 赞助的载荷。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SponsorRequest {
    /// 要赞助的 UserOperation。
    pub user_op: UserOp,
    /// 赞助方愿意出的 gas 上限（单位 gas）。
    pub gas_sponsor_limit: u64,
    /// 赞助政策标识（如 "free-for-agents-v1"）。
    pub policy: String,
}

impl SponsorRequest {
    /// 序列化为 JSON-RPC 风格 body（供 rpc.rs POST 给 paymaster 端点）。
    pub fn to_json_body(&self) -> Result<String, ChainError> {
        serde_json::to_string(self)
            .map_err(|e| ChainError::Rpc(format!("SponsorRequest 序列化失败: {e}")))
    }
}

/// Paymaster 批准/拒绝的响应。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SponsorResponse {
    pub approved: bool,
    /// 批准后返回的 paymasterAndData（hex）。
    #[serde(default)]
    pub paymaster_and_data: Option<String>,
    /// 拒绝原因（机器可读码）。
    #[serde(default)]
    pub reason: Option<String>,
}

/// Paymaster 抽象 trait：真实实现走 rpc.rs，测试用内存实现。
pub trait PaymasterClient {
    fn sponsor(&self, req: &SponsorRequest) -> Result<SponsorResponse, ChainError>;
}

// ── x402 ────────────────────────────────────────────────────────────────────

/// x402 服务端 402 响应里的一条支付要求。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentRequirement {
    /// 支付网络（e.g. "base-sepolia"）。
    pub network: String,
    /// 接收方地址。
    #[serde(rename = "payTo")]
    pub pay_to: String,
    /// 金额（最小单位字符串，避免浮点）。
    pub amount: String,
    /// 代币（e.g. "USDC"）。
    pub asset: String,
    /// 可选资源路径。
    #[serde(default)]
    pub resource: Option<String>,
}

/// 完整 402 Payment Required 信封。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentRequired402 {
    pub accept: Vec<PaymentRequirement>,
    /// facilitator 校验端点（可选）。
    #[serde(default)]
    pub facilitator: Option<String>,
}

impl PaymentRequired402 {
    /// 从 HTTP 响应体（JSON）解析 402 信封。
    pub fn from_json(body: &str) -> Result<Self, ChainError> {
        serde_json::from_str(body).map_err(|e| ChainError::Rpc(format!("402 信封解析失败: {e}")))
    }

    /// 选第一条支付要求（本 MVP 不做多要求选择）。
    pub fn first_requirement(&self) -> Result<&PaymentRequirement, ChainError> {
        self.accept
            .first()
            .ok_or_else(|| ChainError::Rpc("402 信封 accept 数组为空".into()))
    }
}

/// 客户端满足 402 后回附的支付凭证（EIP-3009 授权签名）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProof {
    pub network: String,
    pub pay_to: String,
    pub amount: String,
    /// EIP-3009 transferWithAuthorization 签名 (r,s,v) 与 from/nonce。
    pub authorization: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_op() -> UserOp {
        UserOp {
            sender: "0x1111111111111111111111111111111111111111".into(),
            nonce: "0x0".into(),
            call_data: "0x".into(),
            call_gas_limit: 21000,
            verification_gas_limit: 100000,
            pre_verification_gas: 21000,
            max_fee_per_gas: 2_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
        }
    }

    #[test]
    fn sponsor_request_serializes_roundtrip() {
        let req = SponsorRequest {
            user_op: sample_op(),
            gas_sponsor_limit: 500_000,
            policy: "free-for-agents-v1".into(),
        };
        let body = req.to_json_body().unwrap();
        let back: SponsorRequest = serde_json::from_str(&body).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn parses_402_envelope() {
        let body = r#"{
            "accept": [
              {"network":"base-sepolia","payTo":"0xabc","amount":"100000","asset":"USDC","resource":"/api/x"}
            ],
            "facilitator":"https://fac.example/rpc"
        }"#;
        let env = PaymentRequired402::from_json(body).unwrap();
        let r = env.first_requirement().unwrap();
        assert_eq!(r.network, "base-sepolia");
        assert_eq!(r.amount, "100000");
    }

    #[test]
    fn rejects_empty_accept() {
        let env = PaymentRequired402 {
            accept: vec![],
            facilitator: None,
        };
        assert!(env.first_requirement().is_err());
    }
}
