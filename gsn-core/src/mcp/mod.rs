//! MCP（Model Context Protocol）兼容层
//!
//! 实现 MCP 2024-11-05 核心概念：Tools / Resources / Prompts
//! 不替代 MCP，而是兼容并扩展：GSN 节点可作为 MCP Server 暴露能力，
//! 也可作为 MCP Client 调用外部工具。

pub mod protocol;
pub mod tool;
pub mod resource;
pub mod prompt;
pub mod server;

pub use protocol::{McpMessage, McpRequest, McpResponse, McpError, RequestId, McpMethod};
pub use tool::{ToolDefinition, ToolParameter, ToolSchema, ToolResult};
pub use resource::{ResourceDefinition, ResourceContents, ResourceUri};
pub use prompt::{PromptDefinition, PromptMessage, PromptArgument, PromptRole};
pub use server::McpServer;
