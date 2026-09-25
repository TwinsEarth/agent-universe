//! Google Gemini 适配（v2.4.9）
//!
//! 复刻 Gemini API 的 generateContent 请求格式：
//! - role: user / model（无 system 角色，system 作为 system_instruction 顶级参数）
//! - content.parts: [{text: "...}]
//! - generationConfig: temperature / topP / maxOutputTokens

use serde::{Deserialize, Serialize};

/// Gemini 模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeminiModel {
    Gemini15Pro,
    Gemini15Flash,
    Gemini20Pro,
}

impl GeminiModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            GeminiModel::Gemini15Pro => "gemini-1.5-pro",
            GeminiModel::Gemini15Flash => "gemini-1.5-flash",
            GeminiModel::Gemini20Pro => "gemini-2.0-pro",
        }
    }
    pub fn context_window(&self) -> u32 {
        match self {
            GeminiModel::Gemini15Pro | GeminiModel::Gemini15Flash => 1_000_000,
            GeminiModel::Gemini20Pro => 1_000_000,
        }
    }
}

/// Gemini 内容块。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GePart {
    pub text: String,
}

/// Gemini 消息（role: user / model）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeContent {
    pub role: String,
    pub parts: Vec<GePart>,
}

/// Gemini 请求体（generateContent）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeRequest {
    pub contents: Vec<GeContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeContent>,
    pub generation_config: GeGenConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeGenConfig {
    pub temperature: f64,
    pub top_p: f64,
    pub max_output_tokens: u32,
}

/// Gemini 响应。
#[derive(Debug, Clone, Deserialize)]
pub struct GeResponse {
    pub candidates: Vec<GeCandidate>,
    pub usage_metadata: GeUsage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeCandidate {
    pub content: GeContent,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct GeUsage {
    pub prompt_token_count: u32,
    pub candidates_token_count: u32,
    pub total_token_count: u32,
}

/// Mock 客户端。
pub struct MockGeminiClient {
    pub answer: String,
}

pub trait GeminiClient {
    fn generate(&self, req: &GeRequest) -> Result<GeResponse, String>;
}

impl GeminiClient for MockGeminiClient {
    fn generate(&self, req: &GeRequest) -> Result<GeResponse, String> {
        let _ = req;
        Ok(GeResponse {
            candidates: vec![GeCandidate {
                content: GeContent {
                    role: "model".into(),
                    parts: vec![GePart { text: self.answer.clone() }],
                },
                finish_reason: "STOP".into(),
            }],
            usage_metadata: GeUsage {
                prompt_token_count: 20,
                candidates_token_count: 8,
                total_token_count: 28,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_strings() {
        assert_eq!(GeminiModel::Gemini15Pro.as_str(), "gemini-1.5-pro");
        assert_eq!(GeminiModel::Gemini15Flash.context_window(), 1_000_000);
    }

    #[test]
    fn request_uses_system_instruction_not_role() {
        let req = GeRequest {
            contents: vec![GeContent {
                role: "user".into(),
                parts: vec![GePart { text: "hi".into() }],
            }],
            system_instruction: Some(GeContent {
                role: "user".into(),
                parts: vec![GePart { text: "you are helpful".into() }],
            }),
            generation_config: GeGenConfig { temperature: 0.7, top_p: 0.9, max_output_tokens: 512 },
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("system_instruction"));
        assert!(json.contains("gemini-1.5-pro") == false); // model in URL, not body
    }

    #[test]
    fn mock_returns_answer() {
        let client = MockGeminiClient { answer: "42".into() };
        let req = GeRequest {
            contents: vec![],
            system_instruction: None,
            generation_config: GeGenConfig { temperature: 0.7, top_p: 0.9, max_output_tokens: 256 },
        };
        let resp = client.generate(&req).unwrap();
        assert_eq!(resp.candidates[0].content.parts[0].text, "42");
        assert_eq!(resp.candidates[0].content.role, "model");
    }
}
