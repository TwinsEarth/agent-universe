//! MCP 工具定义
//!
//! 兼容 MCP Tools 规范：工具是 Agent 可调用的函数

use serde::{Deserialize, Serialize};

/// 工具参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

/// 工具 JSON Schema（简化版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: serde_json::Map<String, serde_json::Value>,
    pub required: Vec<String>,
}

impl Default for ToolSchema {
    fn default() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: serde_json::Map::new(),
            required: Vec::new(),
        }
    }
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: ToolSchema,
    /// GSN 扩展：能力标签
    #[serde(default)]
    pub gsn_capability: Option<String>,
    /// GSN 扩展：所需验证级别
    #[serde(default)]
    pub gsn_verify_level: u8,
    /// GSN 扩展：是否需要质押
    #[serde(default)]
    pub gsn_requires_stake: bool,
}

impl ToolDefinition {
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            input_schema: ToolSchema::default(),
            gsn_capability: None,
            gsn_verify_level: 0,
            gsn_requires_stake: false,
        }
    }

    pub fn with_param(mut self, param: ToolParameter) -> Self {
        self.input_schema.properties.insert(
            param.name.clone(),
            serde_json::json!({
                "type": param.param_type,
                "description": param.description,
            }),
        );
        if param.required {
            self.input_schema.required.push(param.name);
        }
        self
    }

    pub fn with_capability(mut self, cap: String) -> Self {
        self.gsn_capability = Some(cap);
        self
    }

    pub fn with_verify_level(mut self, level: u8) -> Self {
        self.gsn_verify_level = level;
        self
    }
}

/// 工具调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: bool,
}

/// 工具内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: serde_json::Value },
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: true,
        }
    }
}
