//! AgentCard & SkillManifest
//!
//! 智能体注册卡片与技能清单
//! Agent 上架前必须质押、签名、声明能力

use crate::marketplace::evidence::EvidenceGrade;
use serde::{Deserialize, Serialize};

/// 定价模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PricingModel {
    /// 按次调用
    PerCall,
    /// 按 token
    PerToken,
    /// 按小时
    PerHour,
    /// 订阅制
    Subscription,
}

/// 货币类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    /// 内部积分
    Credit,
    /// Token
    Token,
    /// 法币
    Fiat,
}

/// 定价信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pricing {
    pub model: PricingModel,
    pub price: f64,
    pub currency: Currency,
}

/// SLA 服务等级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sla {
    pub latency_p95_ms: u64,
    pub availability: f64,
    pub max_concurrency: u32,
}

impl Default for Sla {
    fn default() -> Self {
        Self {
            latency_p95_ms: 2000,
            availability: 0.95,
            max_concurrency: 10,
        }
    }
}

/// 智能体卡片（Agent Card）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketAgentCard {
    /// 全局唯一 DID
    pub agent_id: String,
    /// 语义化版本
    pub version: String,
    /// 名称
    pub name: String,
    /// 能力描述
    pub description: String,
    /// 技能标签
    pub skills: Vec<String>,
    /// 支持模态
    pub modalities: Vec<String>,
    /// 使用的模型
    pub models: Vec<String>,
    /// 调用端点
    pub endpoint: String,
    /// 定价
    pub pricing: Pricing,
    /// SLA
    pub sla: Sla,
    /// 发布者 DID
    pub owner: String,
    /// 质押金额
    pub stake: f64,
    /// 信誉分（0-1）
    pub reputation_score: f64,
    /// 总调用次数
    pub total_calls: u64,
    /// 成功率
    pub success_rate: f64,
    /// 证据等级
    pub evidence_grade: EvidenceGrade,
    /// 是否已验证
    pub verified: bool,
    /// 创建时间
    pub created_at: u64,
    /// 更新时间
    pub updated_at: u64,
}

/// 技能清单（Skill Manifest）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// 技能 ID
    pub skill_id: String,
    /// 技能名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 输入 JSON Schema（简化为字段描述）
    pub input_fields: Vec<SchemaField>,
    /// 输出 JSON Schema
    pub output_fields: Vec<SchemaField>,
    /// 所需工具权限
    pub tool_permissions: Vec<String>,
    /// 超时（毫秒）
    pub timeout_ms: u64,
    /// 预估成本
    pub estimated_cost: f64,
    /// 证据等级
    pub evidence_grade: EvidenceGrade,
}

/// Schema 字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    pub field_type: String,
    pub required: bool,
    pub description: String,
}

/// Agent 分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentCategory {
    Productivity,
    Creative,
    Research,
    Education,
    Entertainment,
    Finance,
    Health,
    Developer,
    Other,
}

impl AgentCategory {
    pub fn label(&self) -> &'static str {
        match self {
            AgentCategory::Productivity => "效率",
            AgentCategory::Creative => "创意",
            AgentCategory::Research => "研究",
            AgentCategory::Education => "教育",
            AgentCategory::Entertainment => "娱乐",
            AgentCategory::Finance => "金融",
            AgentCategory::Health => "健康",
            AgentCategory::Developer => "开发",
            AgentCategory::Other => "其他",
        }
    }
}
