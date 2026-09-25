//! 多模型 LLM 适配层（v2.4.9）
//!
//! 统一适配 OpenAI / Gemini / Anthropic 三大模型后端，
//! 每个后端都精确复刻官方 API 格式，并接入 v2.4.7 异构协商。

pub mod openai;
pub mod gemini;
pub mod anthropic;
pub mod doubao;
pub mod domestic;
pub mod adapter;

pub use openai::{MockOpenAiClient, OaChatRequest, OaChatResponse, OpenAiClient, OpenAiModel, OaMessage, OaRole};
pub use gemini::{GeRequest, GeResponse, GeminiClient, GeminiModel, MockGeminiClient};
pub use anthropic::{AnRequest, AnResponse, AnthropicClient, AnthropicModel, MockAnthropicClient, AnMessage, AnRole};
pub use doubao::{DbChatRequest, DbChatResponse, DoubaoClient, DoubaoModel, MockDoubaoClient, DbMessage, DbRole};
pub use domestic::{DomesticClient, DomesticModel, DomesticProvider, MockDomesticClient};
pub use adapter::{AnthropicAdapter, DomesticAdapter, DoubaoAdapter, GeminiAdapter, LlmBackend, LlmResult, OpenAiAdapter};
