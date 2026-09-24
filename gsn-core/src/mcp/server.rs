//! MCP 服务器端
//!
//! 一个 GSN 节点可作为 MCP Server，将其能力暴露为标准 MCP 接口，
//! 同时兼容 GSN 自己的扩展字段。
//!
//! v2.3.5 重构（去除占位）：
//! - 工具必须绑定执行器（handler）才会真正执行；仅注册定义而无执行器时，
//!   tools/call 返回明确错误，而不是伪造 "tool executed"。
//! - 资源必须绑定读取器（reader）才会真正读取；否则返回明确错误。
//! - initialize 输出规范的 camelCase，能力声明按实际注册动态生成。
//! - 支持 MCP 规范的 `ping` 心跳。

use std::collections::HashMap;
use serde_json::{json, Value};

use super::protocol::*;
use super::tool::*;
use super::resource::*;
use super::prompt::*;

/// 同步工具执行器：接收 arguments JSON，返回 ToolResult
pub type ToolHandler = Box<dyn Fn(&Value) -> ToolResult + Send + Sync>;
/// 同步资源读取器：返回资源文本，或错误
pub type ResourceReader = Box<dyn Fn() -> Result<String, String> + Send + Sync>;

/// MCP 服务器
pub struct McpServer {
    server_name: String,
    tools: HashMap<String, ToolDefinition>,
    tool_handlers: HashMap<String, ToolHandler>,
    resources: HashMap<String, ResourceDefinition>,
    resource_readers: HashMap<String, ResourceReader>,
    prompts: HashMap<String, PromptDefinition>,
    prompt_templates: HashMap<String, Vec<PromptMessage>>,
    request_count: u64,
}

impl McpServer {
    pub fn new(server_name: String) -> Self {
        Self {
            server_name,
            tools: HashMap::new(),
            tool_handlers: HashMap::new(),
            resources: HashMap::new(),
            resource_readers: HashMap::new(),
            prompts: HashMap::new(),
            prompt_templates: HashMap::new(),
            request_count: 0,
        }
    }

    /// 仅注册工具定义（tools/list 可见，但 tools/call 会提示未绑定执行器）
    pub fn register_tool(&mut self, tool: ToolDefinition) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// 注册工具定义并绑定真实执行器
    pub fn register_tool_with_handler(&mut self, tool: ToolDefinition, handler: ToolHandler) {
        let name = tool.name.clone();
        self.tools.insert(name.clone(), tool);
        self.tool_handlers.insert(name, handler);
    }

    /// 仅注册资源定义
    pub fn register_resource(&mut self, resource: ResourceDefinition) {
        self.resources.insert(resource.uri.to_string(), resource);
    }

    /// 注册资源定义并绑定真实读取器
    pub fn register_resource_with_reader(
        &mut self,
        resource: ResourceDefinition,
        reader: ResourceReader,
    ) {
        let uri = resource.uri.to_string();
        self.resources.insert(uri.clone(), resource);
        self.resource_readers.insert(uri, reader);
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
            McpMethod::Ping => McpResponse::success(id, json!({})),
            McpMethod::ToolsList => self.handle_tools_list(id),
            McpMethod::ToolsCall => self.handle_tools_call(id, &req.params),
            McpMethod::ResourcesList => self.handle_resources_list(id),
            McpMethod::ResourcesRead => self.handle_resources_read(id, &req.params),
            McpMethod::PromptsList => self.handle_prompts_list(id),
            McpMethod::PromptsGet => self.handle_prompts_get(id, &req.params),
            McpMethod::Custom => {
                McpResponse::error(id, McpError::MethodNotFound(req.method.clone()))
            }
        }
    }

    /// 按实际注册动态生成能力声明
    fn capabilities_value(&self) -> Value {
        let mut caps = serde_json::Map::new();
        if !self.tools.is_empty() {
            caps.insert("tools".to_string(), json!({ "listChanged": false }));
        }
        if !self.resources.is_empty() {
            caps.insert(
                "resources".to_string(),
                json!({ "subscribe": false, "listChanged": false }),
            );
        }
        if !self.prompts.is_empty() {
            caps.insert("prompts".to_string(), json!({ "listChanged": false }));
        }
        Value::Object(caps)
    }

    fn handle_initialize(&self, id: RequestId) -> McpResponse {
        let result = initialize_result_value(
            &self.server_name,
            self.capabilities_value(),
            None,
        );
        McpResponse::success(id, result)
    }

    fn handle_tools_list(&self, id: RequestId) -> McpResponse {
        let tools: Vec<&ToolDefinition> = self.tools.values().collect();
        let result = json!({ "tools": tools });
        McpResponse::success(id, result)
    }

    fn handle_tools_call(&self, id: RequestId, params: &Value) -> McpResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        // 工具不存在
        if !self.tools.contains_key(name) {
            return McpResponse::error(id, McpError::ToolNotFound(name.to_string()));
        }

        // 工具已注册但未绑定执行器：诚实报错，不伪造成功
        let handler = match self.tool_handlers.get(name) {
            Some(h) => h,
            None => {
                return McpResponse::error(
                    id,
                    McpError::InvalidRequest(format!(
                        "工具 '{name}' 未绑定执行器，无法执行"
                    )),
                );
            }
        };

        let tool_result = handler(&arguments);
        McpResponse::success(id, serde_json::to_value(tool_result).unwrap_or_else(|e| {
            json!({ "content": [{ "type": "text", "text": format!("结果序列化失败: {e}") }], "isError": true })
        }))
    }

    fn handle_resources_list(&self, id: RequestId) -> McpResponse {
        let resources: Vec<&ResourceDefinition> = self.resources.values().collect();
        let result = json!({ "resources": resources });
        McpResponse::success(id, result)
    }

    fn handle_resources_read(&self, id: RequestId, params: &Value) -> McpResponse {
        let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");

        let definition = match self.resources.get(uri) {
            Some(d) => d,
            None => {
                return McpResponse::error(
                    id,
                    McpError::InvalidRequest(format!("resource not found: {uri}")),
                );
            }
        };

        // 未绑定读取器：诚实报错，不伪造内容
        let reader = match self.resource_readers.get(uri) {
            Some(r) => r,
            None => {
                return McpResponse::error(
                    id,
                    McpError::InvalidRequest(format!(
                        "资源 '{uri}' 未绑定读取器，无法读取"
                    )),
                );
            }
        };

        match reader() {
            Ok(text) => {
                let contents = ResourceContents::text(
                    definition.uri.clone(),
                    definition.mime_type.clone(),
                    text,
                );
                let result = json!({ "contents": [contents] });
                McpResponse::success(id, result)
            }
            Err(e) => McpResponse::error(
                id,
                McpError::InternalError(format!("读取资源 '{uri}' 失败: {e}")),
            ),
        }
    }

    fn handle_prompts_list(&self, id: RequestId) -> McpResponse {
        let prompts: Vec<&PromptDefinition> = self.prompts.values().collect();
        let result = json!({ "prompts": prompts });
        McpResponse::success(id, result)
    }

    fn handle_prompts_get(&self, id: RequestId, params: &Value) -> McpResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        match (self.prompts.get(name), self.prompt_templates.get(name)) {
            (Some(prompt), Some(template)) => {
                let args_map = args.as_object().cloned().unwrap_or_default();
                let messages = prompt.render(template, &args_map);
                let result = json!({
                    "description": prompt.description,
                    "messages": messages,
                });
                McpResponse::success(id, result)
            }
            _ => McpResponse::error(
                id,
                McpError::InvalidRequest(format!("prompt not found: {name}")),
            ),
        }
    }
}
