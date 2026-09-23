//! MCP 服务器端
//!
//! 一个 GSN 节点可作为 MCP Server，将其能力暴露为标准 MCP 接口
//! 同时兼容 GSN 自己的扩展字段

use std::collections::HashMap;
use serde_json::Value;

use super::protocol::*;
use super::tool::*;
use super::resource::*;
use super::prompt::*;

/// MCP 服务器
pub struct McpServer {
    server_name: String,
    server_version: String,
    tools: HashMap<String, ToolDefinition>,
    resources: HashMap<String, ResourceDefinition>,
    prompts: HashMap<String, PromptDefinition>,
    /// 已注册的提示模板消息
    prompt_templates: HashMap<String, Vec<PromptMessage>>,
    request_count: u64,
}

impl McpServer {
    pub fn new(server_name: String) -> Self {
        Self {
            server_name,
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            tools: HashMap::new(),
            resources: HashMap::new(),
            prompts: HashMap::new(),
            prompt_templates: HashMap::new(),
            request_count: 0,
        }
    }

    pub fn register_tool(&mut self, tool: ToolDefinition) {
        self.tools.insert(tool.name.clone(), tool);
    }

    pub fn register_resource(&mut self, resource: ResourceDefinition) {
        self.resources.insert(resource.uri.to_string(), resource);
    }

    pub fn register_prompt(&mut self, prompt: PromptDefinition, messages: Vec<PromptMessage>) {
        self.prompt_templates.insert(prompt.name.clone(), messages);
        self.prompts.insert(prompt.name.clone(), prompt);
    }

    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    pub fn prompt_count(&self) -> usize {
        self.prompts.len()
    }

    pub fn request_count(&self) -> u64 {
        self.request_count
    }

    /// 处理 MCP 请求，返回响应
    pub fn handle(&mut self, req: &McpRequest) -> McpResponse {
        self.request_count += 1;
        let id = req.id.clone();

        match req.method_enum() {
            McpMethod::Initialize => self.handle_initialize(id),
            McpMethod::ToolsList => self.handle_tools_list(id),
            McpMethod::ToolsCall => self.handle_tools_call(id, &req.params),
            McpMethod::ResourcesList => self.handle_resources_list(id),
            McpMethod::ResourcesRead => self.handle_resources_read(id, &req.params),
            McpMethod::PromptsList => self.handle_prompts_list(id),
            McpMethod::PromptsGet => self.handle_prompts_get(id, &req.params),
            McpMethod::Custom => McpResponse::error(id, McpError::MethodNotFound(req.method.clone())),
        }
    }

    fn handle_initialize(&self, id: RequestId) -> McpResponse {
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            server_name: self.server_name.clone(),
            server_version: self.server_version.clone(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: false }),
                resources: Some(ResourcesCapability { subscribe: false, list_changed: false }),
                prompts: Some(PromptsCapability { list_changed: false }),
            },
        };
        McpResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn handle_tools_list(&self, id: RequestId) -> McpResponse {
        let tools: Vec<&ToolDefinition> = self.tools.values().collect();
        let result = serde_json::json!({ "tools": tools });
        McpResponse::success(id, result)
    }

    fn handle_tools_call(&self, id: RequestId, params: &Value) -> McpResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        match self.tools.get(name) {
            Some(_tool) => {
                // 实际执行由 GSN 运行时注入；这里返回占位成功
                let result = ToolResult::text(format!("tool '{}' executed", name));
                McpResponse::success(id, serde_json::to_value(result).unwrap())
            }
            None => McpResponse::error(id, McpError::ToolNotFound(name.to_string())),
        }
    }

    fn handle_resources_list(&self, id: RequestId) -> McpResponse {
        let resources: Vec<&ResourceDefinition> = self.resources.values().collect();
        let result = serde_json::json!({ "resources": resources });
        McpResponse::success(id, result)
    }

    fn handle_resources_read(&self, id: RequestId, params: &Value) -> McpResponse {
        let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        match self.resources.get(uri) {
            Some(res) => {
                let contents = ResourceContents::text(
                    res.uri.clone(),
                    res.mime_type.clone(),
                    format!("content of {}", res.name),
                );
                let result = serde_json::json!({ "contents": [contents] });
                McpResponse::success(id, result)
            }
            None => McpResponse::error(id, McpError::InvalidRequest(format!("resource not found: {}", uri))),
        }
    }

    fn handle_prompts_list(&self, id: RequestId) -> McpResponse {
        let prompts: Vec<&PromptDefinition> = self.prompts.values().collect();
        let result = serde_json::json!({ "prompts": prompts });
        McpResponse::success(id, result)
    }

    fn handle_prompts_get(&self, id: RequestId, params: &Value) -> McpResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

        match (self.prompts.get(name), self.prompt_templates.get(name)) {
            (Some(prompt), Some(template)) => {
                let args_map = args.as_object().cloned().unwrap_or_default();
                let messages = prompt.render(template, &args_map);
                let result = serde_json::json!({
                    "description": prompt.description,
                    "messages": messages,
                });
                McpResponse::success(id, result)
            }
            _ => McpResponse::error(id, McpError::InvalidRequest(format!("prompt not found: {}", name))),
        }
    }
}
