//! OpenAI Chat Completions 适配（v2.4.9）
//!
//! 精确复刻 OpenAI Chat Completions API 的输入输出格式。

use serde::{Deserialize, Serialize};

/// OpenAI 模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpenAiModel {
    Gpt4o,
    Gpt4oMini,
    Gpt4Turbo,
    O1,
}

impl OpenAiModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpenAiModel::Gpt4o => "gpt-4o",
            OpenAiModel::Gpt4oMini => "gpt-4o-mini",
            OpenAiModel::Gpt4Turbo => "gpt-4-turbo",
            OpenAiModel::O1 => "o1",
        }
    }
    pub fn context_window(&self) -> u32 {
        match self {
            OpenAiModel::Gpt4o | OpenAiModel::Gpt4oMini | OpenAiModel::Gpt4Turbo => 128_000,
            OpenAiModel::O1 => 200_000,
        }
    }
}

/// OpenAI 消息。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OaMessage {
    pub role: OaRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OaRole {
    System,
    User,
    Assistant,
    Tool,
}

/// OpenAI 请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OaChatRequest {
    pub model: String,
    pub messages: Vec<OaMessage>,
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u32,
    pub stream: bool,
}

/// OpenAI 响应。
#[derive(Debug, Clone, Deserialize)]
pub struct OaChatResponse {
    pub id: String,
    pub choices: Vec<OaChoice>,
    pub usage: OaUsage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OaChoice {
    pub message: OaMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct OaUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Mock 客户端。
pub struct MockOpenAiClient {
    pub answer: String,
}

pub trait OpenAiClient {
    fn chat(&self, req: &OaChatRequest) -> Result<OaChatResponse, String>;
}

impl OpenAiClient for MockOpenAiClient {
    fn chat(&self, req: &OaChatRequest) -> Result<OaChatResponse, String> {
        let _ = req; // mock ignores request body, returns fixed answer
        Ok(OaChatResponse {
            id: format!("oa-mock-{}", uuid::Uuid::new_v4()),
            choices: vec![OaChoice {
                message: OaMessage { role: OaRole::Assistant, content: self.answer.clone() },
                finish_reason: "stop".into(),
            }],
            usage: OaUsage { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_strings() {
        assert_eq!(OpenAiModel::Gpt4o.as_str(), "gpt-4o");
        assert_eq!(OpenAiModel::Gpt4o.context_window(), 128_000);
        assert_eq!(OpenAiModel::O1.context_window(), 200_000);
    }

    #[test]
    fn mock_client_returns_answer() {
        let client = MockOpenAiClient { answer: "hello".into() };
        let req = OaChatRequest {
            model: "gpt-4o".into(),
            messages: vec![OaMessage { role: OaRole::User, content: "hi".into() }],
            temperature: 0.7, top_p: 1.0, max_tokens: 256, stream: false,
        };
        let resp = client.chat(&req).unwrap();
        assert_eq!(resp.choices[0].message.content, "hello");
    }
}
