//! MCP JSON-RPC 2.0 消息协议
//!
//! 兼容 MCP 2024-11-05 规范的 JSON-RPC 消息格式

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 请求 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(u64),
    String(String),
}

/// MCP 协议版本（MCP 2024-11-05）
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// MCP 标准方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum McpMethod {
    /// 初始化握手
    Initialize,
    /// 心跳（MCP 规范，初始化后客户端周期性发送）
    Ping,
    /// 列出工具
    ToolsList,
    /// 调用工具
    ToolsCall,
    /// 列出资源
    ResourcesList,
    /// 读取资源
    ResourcesRead,
    /// 列出提示
    PromptsList,
    /// 获取提示
    PromptsGet,
    /// 自定义方法
    Custom,
}

impl McpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            McpMethod::Initialize => "initialize",
            McpMethod::Ping => "ping",
            McpMethod::ToolsList => "tools/list",
            McpMethod::ToolsCall => "tools/call",
            McpMethod::ResourcesList => "resources/list",
            McpMethod::ResourcesRead => "resources/read",
            McpMethod::PromptsList => "prompts/list",
            McpMethod::PromptsGet => "prompts/get",
            McpMethod::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "initialize" => McpMethod::Initialize,
            "ping" => McpMethod::Ping,
            "tools/list" => McpMethod::ToolsList,
            "tools/call" => McpMethod::ToolsCall,
            "resources/list" => McpMethod::ResourcesList,
            "resources/read" => McpMethod::ResourcesRead,
            "prompts/list" => McpMethod::PromptsList,
            "prompts/get" => McpMethod::PromptsGet,
            _ => McpMethod::Custom,
        }
    }
}

/// MCP 错误
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum McpError {
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("method not found: {0}")]
    MethodNotFound(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("internal error: {0}")]
    InternalError(String),
    #[error("tool not found: {0}")]
    ToolNotFound(String),
}

impl McpError {
    pub fn code(&self) -> i32 {
        match self {
            McpError::ParseError(_) => -32700,
            McpError::InvalidRequest(_) => -32600,
            McpError::MethodNotFound(_) => -32601,
            McpError::InvalidParams(_) => -32602,
            McpError::InternalError(_) => -32603,
            McpError::ToolNotFound(_) => -32001,
        }
    }
}

/// JSON-RPC 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpErrorBody>,
}

/// 错误体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpErrorBody {
    pub code: i32,
    pub message: String,
}

/// 统一消息枚举（请求或响应）
#[derive(Debug, Clone)]
pub enum McpMessage {
    Request(McpRequest),
    Response(McpResponse),
}

impl McpRequest {
    pub fn new(id: RequestId, method: McpMethod, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.as_str().to_string(),
            params,
        }
    }

    pub fn method_enum(&self) -> McpMethod {
        McpMethod::from_str(&self.method)
    }
}

impl McpResponse {
    pub fn success(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: RequestId, err: McpError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpErrorBody {
                code: err.code(),
                message: err.to_string(),
            }),
        }
    }

    pub fn is_success(&self) -> bool {
        self.result.is_some() && self.error.is_none()
    }
}

/// MCP 客户端/服务器实体信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInfo {
    pub name: String,
    #[serde(default)]
    pub version: String,
}

/// MCP 初始化参数（MCP 2024-11-05）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub client_info: EntityInfo,
    #[serde(default)]
    pub capabilities: Value,
}

impl Default for InitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            client_info: EntityInfo {
                name: "gsn-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: Value::Object(serde_json::Map::new()),
        }
    }
}

/// MCP 服务器信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// MCP 初始化结果（MCP 2024-11-05）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// 构造标准 initialize 结果 JSON
///
/// server / stdio / sse 三处统一复用，避免手写不一致：
/// - `server_name`：服务器名
/// - `capabilities`：能力声明 JSON
/// - `instructions`：可选的使用说明
pub fn initialize_result_value(
    server_name: &str,
    capabilities: Value,
    instructions: Option<&str>,
) -> Value {
    let mut v = serde_json::json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "serverInfo": {
            "name": server_name,
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": capabilities,
    });
    if let Some(text) = instructions {
        v.as_object_mut()
            .unwrap()
            .insert("instructions".to_string(), serde_json::json!(text));
    }
    v
}

/// 服务器能力声明
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
}

impl ServerCapabilities {
    /// 仅声明 tools 能力（市场 MCP 服务使用）
    pub fn tools_only() -> Self {
        Self {
            tools: Some(ToolsCapability { list_changed: false }),
            resources: None,
            prompts: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    pub subscribe: bool,
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    pub list_changed: bool,
}
