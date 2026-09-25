//! 豆包 / 火山引擎方舟适配（v2.5.0）
//!
//! 复刻火山引擎方舟（Ark）Chat Completions API：
//! - base_url: https://ark.cn-beijing.volces.com/api/v3
//! - 兼容 OpenAI Chat Completions 格式
//! - 认证: Bearer ARK_API_KEY
//! - 最新模型: doubao-seed-2-1-pro / turbo / lite, doubao-1-5-pro-32k, seed-character

use serde::{Deserialize, Serialize};

/// 豆包方舟模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DoubaoModel {
    /// doubao-seed-2-1-pro-260915，深度思考，1024K 上下文
    Seed21Pro,
    /// doubao-seed-2-1-turbo-260628，快速版，256K
    Seed21Turbo,
    /// doubao-seed-2-1-lite-260915，轻量版，256K
    Seed21Lite,
    /// doubao-1-5-pro-32k-250115，上一代 Pro
    Doubao15Pro32k,
    /// doubao-seed-1-6-251015
    Seed16,
    /// doubao-seed-character，角色对话
    SeedCharacter,
}

impl DoubaoModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DoubaoModel::Seed21Pro => "doubao-seed-2-1-pro-260915",
            DoubaoModel::Seed21Turbo => "doubao-seed-2-1-turbo-260628",
            DoubaoModel::Seed21Lite => "doubao-seed-2-1-lite-260915",
            DoubaoModel::Doubao15Pro32k => "doubao-1-5-pro-32k-250115",
            DoubaoModel::Seed16 => "doubao-seed-1-6-251015",
            DoubaoModel::SeedCharacter => "doubao-seed-character",
        }
    }

    pub fn context_window(&self) -> u32 {
        match self {
            DoubaoModel::Seed21Pro => 1_024_000,
            DoubaoModel::Seed21Turbo | DoubaoModel::Seed21Lite => 256_000,
            DoubaoModel::Doubao15Pro32k => 32_000,
            DoubaoModel::Seed16 => 256_000,
            DoubaoModel::SeedCharacter => 32_000,
        }
    }

    /// 是否支持深度思考（返回 reasoning_content）。
    pub fn supports_reasoning(&self) -> bool {
        matches!(self, DoubaoModel::Seed21Pro | DoubaoModel::Seed21Turbo)
    }
}

/// 豆包消息（复用 OpenAI 格式：system/user/assistant）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DbMessage {
    pub role: DbRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbRole {
    System,
    User,
    Assistant,
}

/// 豆包请求体（兼容 OpenAI Chat Completions）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbChatRequest {
    pub model: String,
    pub messages: Vec<DbMessage>,
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u32,
    pub stream: bool,
    /// 深度思考开关（仅 seed-2-1 系列支持）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<DbThinking>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DbThinking {
    #[serde(rename = "type")]
    pub ty: DbThinkingType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbThinkingType {
    Enabled,
    Disabled,
}

/// 豆包响应。
#[derive(Debug, Clone, Deserialize)]
pub struct DbChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<DbChoice>,
    pub usage: DbUsage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbChoice {
    pub message: DbAssistantMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbAssistantMessage {
    pub role: String,
    pub content: String,
    /// 深度思考链（仅 seed-2-1 开启 thinking 时返回）。
    #[serde(default)]
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct DbUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Mock 客户端。
pub struct MockDoubaoClient {
    pub answer: String,
    pub reasoning: Option<String>,
}

pub trait DoubaoClient {
    fn chat(&self, req: &DbChatRequest) -> Result<DbChatResponse, String>;
}

impl DoubaoClient for MockDoubaoClient {
    fn chat(&self, req: &DbChatRequest) -> Result<DbChatResponse, String> {
        let _ = req;
        Ok(DbChatResponse {
            id: "db-mock".into(),
            model: req.model.clone(),
            choices: vec![DbChoice {
                message: DbAssistantMessage {
                    role: "assistant".into(),
                    content: self.answer.clone(),
                    reasoning_content: self.reasoning.clone(),
                },
                finish_reason: "stop".into(),
            }],
            usage: DbUsage { prompt_tokens: 20, completion_tokens: 10, total_tokens: 30 },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_strings_and_windows() {
        assert_eq!(DoubaoModel::Seed21Pro.as_str(), "doubao-seed-2-1-pro-260915");
        assert_eq!(DoubaoModel::Seed21Pro.context_window(), 1_024_000);
        assert!(DoubaoModel::Seed21Pro.supports_reasoning());
        assert!(!DoubaoModel::Doubao15Pro32k.supports_reasoning());
    }

    #[test]
    fn thinking_field_serialized_when_enabled() {
        let req = DbChatRequest {
            model: "doubao-seed-2-1-pro-260915".into(),
            messages: vec![DbMessage { role: DbRole::User, content: "hi".into() }],
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 4096,
            stream: false,
            thinking: Some(DbThinking { ty: DbThinkingType::Enabled }),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"thinking\""));
        assert!(json.contains("enabled"));
    }

    #[test]
    fn thinking_field_skipped_when_none() {
        let req = DbChatRequest {
            model: "doubao-1-5-pro-32k-250115".into(),
            messages: vec![],
            temperature: 0.7, top_p: 1.0, max_tokens: 4096, stream: false,
            thinking: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("thinking"));
    }

    #[test]
    fn mock_returns_answer_and_reasoning() {
        let client = MockDoubaoClient {
            answer: "答案".into(),
            reasoning: Some("思考过程...".into()),
        };
        let req = DbChatRequest {
            model: "doubao-seed-2-1-pro-260915".into(),
            messages: vec![],
            temperature: 0.7, top_p: 1.0, max_tokens: 4096, stream: false,
            thinking: Some(DbThinking { ty: DbThinkingType::Enabled }),
        };
        let resp = client.chat(&req).unwrap();
        assert_eq!(resp.choices[0].message.content, "答案");
        assert_eq!(resp.choices[0].message.reasoning_content, Some("思考过程...".into()));
        assert_eq!(resp.model, "doubao-seed-2-1-pro-260915");
    }
}
