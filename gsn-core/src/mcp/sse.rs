//! MCP over HTTP（Streamable HTTP / SSE）
//!
//! v2.3.5: 让 AI 客户端通过 HTTP 调用市场工具，无需本地启动进程。
//! 支持两种模式：
//! - **POST /api/v1/mcp**（无状态 Streamable HTTP）：每次请求一条 JSON-RPC，
//!   直接返回一条 JSON-RPC 响应。最简单可靠，适合服务端/远程场景。
//! - **GET /api/v1/mcp**（SSE 长连接）：建立 text/event-stream，
//!   用于服务器主动推送（本实现发送初始化事件 + 心跳）。
//!
//! 远程客户端（Cursor / 自定义 Agent）把 MCP server URL 指向
//! `http://<node>:4002/api/v1/mcp` 即可。

use crate::api::market_actor::MarketActorHandle;
use crate::mcp::market_tools::MarketMcpBridge;
use crate::mcp::protocol::*;
use serde_json::{json, Value};

/// MCP HTTP 处理结果
pub struct McpHttp {
    pub status: u16,
    pub status_text: &'static str,
    pub content_type: String,
    pub body: String,
}

/// 处理 POST /api/v1/mcp（无状态 JSON-RPC）
pub async fn handle_post(body: &str, market: &MarketActorHandle) -> McpHttp {
    let bridge = MarketMcpBridge::new(market.clone());
    let tools = MarketMcpBridge::tool_definitions();

    // 解析 JSON-RPC
    let raw: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            return jsonrpc_response(McpResponse::error(
                RequestId::Number(0),
                McpError::ParseError(e.to_string()),
            ));
        }
    };

    // 无 id 的通知：返回 202
    if raw.get("id").is_none() {
        return McpHttp {
            status: 202,
            status_text: "Accepted",
            content_type: "application/json".to_string(),
            body: String::new(),
        };
    }

    let req: McpRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return jsonrpc_response(McpResponse::error(
                RequestId::Number(0),
                McpError::InvalidRequest(e.to_string()),
            ));
        }
    };

    let id = req.id.clone();
    let response = match req.method_enum() {
        McpMethod::Initialize => {
            let caps = json!({ "tools": { "listChanged": false } });
            let result = initialize_result_value("gsn-agent-market-http", caps, None);
            McpResponse::success(id, result)
        }
        McpMethod::Ping => McpResponse::success(id, json!({})),
        McpMethod::ToolsList => McpResponse::success(id, json!({"tools": tools})),
        McpMethod::ToolsCall => {
            let name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = req.params.get("arguments").cloned().unwrap_or(json!({}));
            let tool_result = bridge.call(name, &args).await;
            McpResponse::success(id, serde_json::to_value(tool_result).unwrap_or(json!({})))
        }
        McpMethod::ResourcesList => McpResponse::success(id, json!({"resources": []})),
        McpMethod::ResourcesRead => McpResponse::error(
            id,
            McpError::InvalidRequest("resource 不存在".to_string()),
        ),
        McpMethod::PromptsList => McpResponse::success(id, json!({"prompts": []})),
        McpMethod::PromptsGet => McpResponse::error(
            id,
            McpError::InvalidRequest("prompt 不存在".to_string()),
        ),
        McpMethod::Custom => McpResponse::error(
            id,
            McpError::MethodNotFound(req.method.clone()),
        ),
    };

    jsonrpc_response(response)
}

/// 处理 GET /api/v1/mcp（SSE 流的首帧说明）
///
/// 完整 SSE 长连接需要服务器保持连接；在无状态响应中返回一个 event-stream
/// 的初始化帧，告知客户端工具数量与改用 POST 的无状态端点。
pub fn handle_get() -> McpHttp {
    let tools = MarketMcpBridge::tool_definitions();
    // SSE 格式：event: xxx\ndata: yyy\n\n
    let init_data = serde_json::json!({
        "transport": "streamable-http",
        "stateless_endpoint": "/api/v1/mcp",
        "tool_count": tools.len(),
        "tools": tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
    });
    let stream = format!(
        "event: ready\ndata: {}\n\nretry: 3000\n",
        init_data.to_string()
    );
    McpHttp {
        status: 200,
        status_text: "OK",
        content_type: "text/event-stream".to_string(),
        body: stream,
    }
}

fn jsonrpc_response(resp: McpResponse) -> McpHttp {
    McpHttp {
        status: if resp.is_success() { 200 } else { 400 },
        status_text: if resp.is_success() { "OK" } else { "Bad Request" },
        content_type: "application/json".to_string(),
        body: serde_json::to_string(&resp).unwrap_or_default(),
    }
}
