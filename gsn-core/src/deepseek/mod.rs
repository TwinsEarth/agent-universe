//! DeepSeek 模型适配层（v2.4.8）
//!
//! deepseek-recipe：深度优先的模型适配层，只解决"如何与 DeepSeek 模型正确对话"：
//! - recipe：精确复刻官方提示词编码（角色、采样参数区间、思考模式）
//! - protocol：OpenAI / DeepSeek / gsn 内部格式三向互转
//! - tokenizer：token 估算与上下文窗口预算（截断策略）
//! - adapter：把 DeepSeek 后端包装成异构 LLM 协商可用的节点
//!
//! 分工：agent-universe 解决"智能体如何组网与经济协作"，
//! deepseek 模块解决"单个智能体如何与模型正确交互"。

pub mod adapter;
pub mod protocol;
pub mod recipe;
pub mod tokenizer;

pub use adapter::{DeepSeekAdapter, DeepSeekClient, DeepSeekConfig, MockDeepSeekClient};
pub use protocol::Protocol;
pub use recipe::{
    DeepSeekModel, DsChatRequest, DsChatResponse, DsChoice, DsMessage, DsRole, DsSampling,
    DsUsage, Recipe,
};
pub use tokenizer::{ContextBudget, estimate_tokens, message_tokens};
