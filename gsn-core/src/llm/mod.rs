//! 多模型 LLM 适配层（v2.5.2）
//!
//! # 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │           collaboration::hetero_llm                 │
//! │         Deliberation（加权投票协商）                  │
//! └──────────────────────┬──────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────┐
//! │              adapter.rs（统一适配器）                  │
//! │  OpenAiAdapter / GeminiAdapter / AnthropicAdapter  │
//! │  DoubaoAdapter / DomesticAdapter                    │
//! └──────┬──────────┬──────────┬──────────┬─────────────┘
//!        │          │          │          │
//! ┌──────▼──┐ ┌────▼────┐ ┌──▼─────┐ ┌─▼────────┐
//! │ openai  │ │ gemini  │ │anthropic│ │ doubao/   │
//! │ .rs     │ │ .rs     │ │ .rs     │ │ domestic  │
//! │ (126行) │ │ (152行) │ │ (145行) │ │ .rs(432行)│
//! └─────────┘ └─────────┘ └────────┘ └───────────┘
//!
//! ┌─────────────────────────────────────────────────────┐
//! │              network.rs（网络分区路由）                │
//! │  DomesticRegistry(7端点) / OverseasRegistry(4端点)  │
//! │  NetworkRouter（URL路由/健康检查/代理故障转移）        │
//! │  RegionAwareDeliberation（外网不可达自动降级）         │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # 已接入的 11 个模型家族
//!
//! | family | 厂商 | 网络组 |
//! |---|---|---|
//! | openai | OpenAI GPT-4o/o1 | 国外 |
//! | gemini | Google Gemini 1.5/2.0 Pro | 国外 |
//! | anthropic | Claude Sonnet/Opus/Haiku 4 | 国外 |
//! | deepseek | DeepSeek Chat/Reasoner | 国内 |
//! | doubao | 豆包 Seed 2.1 | 国内 |
//! | kimi | Kimi/Moonshot | 国内 |
//! | qwen | 通义千问 | 国内 |
//! | zhipu | 智谱 GLM-4 | 国内 |
//! | minimax | MiniMax abab6.5 | 国内 |
//! | hunyuan | 腾讯混元 | 国内 |
//! | xiaomi | 小米 MiLM | 国内 |

pub mod openai;
pub mod gemini;
pub mod anthropic;
pub mod doubao;
pub mod domestic;
pub mod network;
pub mod adapter;

pub use openai::{MockOpenAiClient, OaChatRequest, OaChatResponse, OpenAiClient, OpenAiModel, OaMessage, OaRole};
pub use gemini::{GeRequest, GeResponse, GeminiClient, GeminiModel, MockGeminiClient};
pub use anthropic::{AnRequest, AnResponse, AnthropicClient, AnthropicModel, MockAnthropicClient, AnMessage, AnRole};
pub use doubao::{DbChatRequest, DbChatResponse, DoubaoClient, DoubaoModel, MockDoubaoClient, DbMessage, DbRole};
pub use domestic::{DomesticClient, DomesticModel, DomesticProvider, MockDomesticClient};
pub use network::{DomesticRegistry, DowngradeEvent, EndpointHealth, LlmEndpoint, NetworkRegion, NetworkRouter, OverseasRegistry, RegionAwareDeliberation};
pub use adapter::{AnthropicAdapter, DomesticAdapter, DoubaoAdapter, GeminiAdapter, LlmBackend, LlmResult, OpenAiAdapter};
