//! API 层
//!
//! v2.3.5: 将智能体市场能力通过统一的 Actor + REST API 暴露
//! - MarketActor: 独占 AgentMarket，通过 mpsc 通道接收命令（无锁死风险）
//! - REST: 完整 marketplace HTTP 端点
//! - MCP over SSE: 标准化 AI 工具连接

pub mod market_actor;
pub mod rest;

pub use market_actor::{MarketCommand, MarketActorHandle, MarketResponse};
