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
    /// MCP 规范字段为 inputSchema（camelCase）
    #[serde(rename = "inputSchema")]
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
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

/// 工具内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
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

/// 按工具 inputSchema 校验 arguments（v2.6.1，GAP §8.1）。
///
/// schema 是 tools/list 与 tools/call 共用的**单一来源**：缺必填、显式 null
/// 占据必填、或已提供参数类型不符时返回 Err 并指名参数，由调用方转成
/// JSON-RPC `-32602`（InvalidParams），杜绝静默降级。
pub fn validate_arguments(schema: &ToolSchema, args: &serde_json::Value) -> Result<(), String> {
    let map = match args {
        serde_json::Value::Object(m) => m,
        _ => return Err("工具参数必须是一个 JSON 对象".to_string()),
    };

    // 必填校验（缺失或显式 null 都判缺失）
    for name in &schema.required {
        let present = matches!(map.get(name), Some(v) if !v.is_null());
        if !present {
            return Err(format!("缺少必填参数 '{name}'"));
        }
    }

    // 类型校验（只校验 schema 声明且实际提供、非 null 的参数）
    for (name, prop) in &schema.properties {
        if let Some(value) = map.get(name) {
            if value.is_null() {
                continue;
            }
            let ty = prop.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let ok = match ty {
                "string" => value.is_string(),
                "integer" => value.is_i64(),
                "number" => value.is_number(),
                "boolean" => value.is_boolean(),
                "object" => value.is_object(),
                "array" => value.is_array(),
                _ => true, // 未声明 / 复合类型不做强校验
            };
            if !ok {
                return Err(format!("参数 '{name}' 应为 {ty} 类型"));
            }
        }
    }

    Ok(())
}
