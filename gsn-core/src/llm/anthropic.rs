//! Anthropic Messages API 适配（v2.4.9）
//!
//! 复刻 Anthropic Messages API 格式：
//! - model: claude-sonnet-4 / claude-opus-4 等
//! - system: 顶级字符串参数（不是 messages 里的 role）
//! - messages: [{role: user/assistant, content: "..."}]
//! - max_tokens: 必填
//! - headers: x-api-key + anthropic-version

use serde::{Deserialize, Serialize};

/// Anthropic 模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnthropicModel {
    ClaudeSonnet4,
    ClaudeOpus4,
    ClaudeHaiku4,
}

impl AnthropicModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnthropicModel::ClaudeSonnet4 => "claude-sonnet-4-20250514",
            AnthropicModel::ClaudeOpus4 => "claude-opus-4-20250514",
            AnthropicModel::ClaudeHaiku4 => "claude-haiku-4-20250514",
        }
    }
    pub fn context_window(&self) -> u32 {
        match self {
            AnthropicModel::ClaudeSonnet4 | AnthropicModel::ClaudeOpus4 => 200_000,
            AnthropicModel::ClaudeHaiku4 => 200_000,
        }
    }
}

/// Anthropic 消息。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnMessage {
    pub role: AnRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnRole {
    User,
    Assistant,
}

/// Anthropic 请求体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnRequest {
    pub model: String,
    pub messages: Vec<AnMessage>,
    /// system 是顶级参数，不在 messages 里。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub max_tokens: u32,
    pub temperature: f64,
    pub top_p: f64,
}

/// Anthropic 响应。
#[derive(Debug, Clone, Deserialize)]
pub struct AnResponse {
    pub id: String,
    pub content: Vec<AnContentBlock>,
    pub usage: AnUsage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnContentBlock {
    #[serde(rename = "type")]
    pub ty: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AnUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Mock 客户端。
pub struct MockAnthropicClient {
    pub answer: String,
}

pub trait AnthropicClient {
    fn messages(&self, req: &AnRequest) -> Result<AnResponse, String>;
}

impl AnthropicClient for MockAnthropicClient {
    fn messages(&self, req: &AnRequest) -> Result<AnResponse, String> {
        let _ = req;
        Ok(AnResponse {
            id: format!("an-mock-{}", uuid::Uuid::new_v4()),
            content: vec![AnContentBlock { ty: "text".into(), text: self.answer.clone() }],
            usage: AnUsage { input_tokens: 15, output_tokens: 6 },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_strings() {
        assert_eq!(AnthropicModel::ClaudeSonnet4.as_str(), "claude-sonnet-4-20250514");
        assert_eq!(AnthropicModel::ClaudeSonnet4.context_window(), 200_000);
    }

    #[test]
    fn system_is_top_level_not_in_messages() {
        let req = AnRequest {
            model: "claude-sonnet-4-20250514".into(),
            messages: vec![AnMessage { role: AnRole::User, content: "hi".into() }],
            system: Some("you are helpful".into()),
            max_tokens: 512,
            temperature: 0.7,
            top_p: 1.0,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"system\":\"you are helpful\""));
        // messages 里不应该有 system role
        assert!(!json.contains("\"role\":\"system\""));
    }

    #[test]
    fn mock_returns_answer() {
        let client = MockAnthropicClient { answer: "hello".into() };
        let req = AnRequest {
            model: "claude-sonnet-4-20250514".into(),
            messages: vec![],
            system: None,
            max_tokens: 256,
            temperature: 0.7,
            top_p: 1.0,
        };
        let resp = client.messages(&req).unwrap();
        assert_eq!(resp.content[0].text, "hello");
        assert_eq!(resp.content[0].ty, "text");
    }
}
