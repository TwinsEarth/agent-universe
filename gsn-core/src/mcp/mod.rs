//! MCP（Model Context Protocol）兼容层
//!
//! 实现 MCP 2024-11-05 核心概念：Tools / Resources / Prompts
//! 不替代 MCP，而是兼容并扩展：GSN 节点可作为 MCP Server 暴露能力，
//! 也可作为 MCP Client 调用外部工具。

pub mod market_tools;
pub mod prompt;
pub mod protocol;
pub mod resource;
pub mod sandbox_tools;
pub mod server;
pub mod sse;
pub mod stdio;
pub mod tool;

pub use prompt::{PromptArgument, PromptDefinition, PromptMessage, PromptRole};
pub use protocol::{McpError, McpMessage, McpMethod, McpRequest, McpResponse, RequestId};
pub use resource::{ResourceContents, ResourceDefinition, ResourceUri};
pub use server::McpServer;
pub use tool::{ToolDefinition, ToolParameter, ToolResult, ToolSchema};
