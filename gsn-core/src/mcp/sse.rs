//! MCP over HTTP（Streamable HTTP / SSE）
//!
//! v2.3.5: 让 AI 客户端通过 HTTP 调用市场工具，无需本地启动进程。
//! 支持两种模式：
//! - **POST /api/v1/mcp**（无状态 Streamable HTTP）：每次请求一条 JSON-RPC，
//!   直接返回一条 JSON-RPC 响应。最简单可靠，适合服务端/远程场景。
//! - **GET /api/v1/mcp**（SSE，v2.6.9 诚实标注，GAP §8.6）：当前 `handle_get`
//!   只发送一帧 `event: ready` 后即关闭连接，**并非真正的长连接流 / 服务端持续推送**。
//!   客户端应优先使用 POST 无状态模式；真长连接（事件 id、重连续传、服务端推送）
//!   尚未实现。
//!
//! 远程客户端（Cursor / 自定义 Agent）把 MCP server URL 指向
//! `http://<node>:4002/api/v1/mcp` 即可。

use crate::api::market_actor::MarketActorHandle;
use crate::mcp::market_tools::MarketMcpBridge;
use crate::mcp::protocol::*;
use crate::mcp::sandbox_tools::SandboxMcpBridge;
use crate::mcp::tool::{find_tool, validate_arguments, ToolResult};
use crate::sandbox::manager::SandboxManager;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// MCP HTTP 处理结果
pub struct McpHttp {
    pub status: u16,
    pub status_text: &'static str,
    pub content_type: String,
    pub body: String,
}

/// 处理 POST /api/v1/mcp（无状态 JSON-RPC）
///
/// v2.6.8（GAP §8.8 严重）：此前不读任何 header/token/origin 就把请求分发到
/// market_deposit / market_arbitrate / market_settle_task / market_register_agent 等
/// **动钱工具**，任何能访问端口的人都能提款/罚没。现在：
/// - `expected_token = Some(t)`：必须带 `Authorization: Bearer <t>`，否则 401；
/// - `expected_token = None`（未配置令牌）：所有写/动钱工具被拒绝（默认安全），
///   只放行只读工具。
pub async fn handle_post(
    body: &str,
    market: &MarketActorHandle,
    sandbox_mgr: &Arc<Mutex<SandboxManager>>,
    auth_header: Option<&str>,
    expected_token: Option<&str>,
) -> McpHttp {
    // ── 认证闸门（先于任何分发）──
    if let Some(t) = expected_token {
        let want = format!("Bearer {t}");
        // v3.5.1（AU-32）：常量时间比较，不提前短路，避免时序侧信道
        let authed = auth_header
            .map(|h| crate::security::constant_time_eq_str(h.trim(), &want))
            .unwrap_or(false);
        if !authed {
            return McpHttp {
                status: 401,
                status_text: "Unauthorized",
                content_type: "application/json".to_string(),
                body: serde_json::to_string(&McpResponse::error(
                    RequestId::Null,
                    McpError::Unauthorized("invalid or missing bearer token".to_string()),
                ))
                .unwrap_or_default(),
            };
        }
    }

    let bridge = MarketMcpBridge::new(market.clone());
    let sb_bridge = SandboxMcpBridge::new(sandbox_mgr.clone());
    let mut tools = MarketMcpBridge::tool_definitions();
    tools.extend(SandboxMcpBridge::tool_definitions());

    // 解析 JSON-RPC（v2.6.8：解析失败回 id=null，符合 JSON-RPC 规范 §8.3）
    let raw: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            return jsonrpc_response(McpResponse::error(
                RequestId::Null,
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
                RequestId::Null,
                McpError::InvalidRequest(e.to_string()),
            ));
        }
    };

    let id = req.id.clone();

    // 未配置访问令牌时，写/动钱工具一律拒绝（默认安全失败）
    if expected_token.is_none() && req.method_enum() == McpMethod::ToolsCall {
        let name = req
            .params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if is_mutating_tool(name) {
            return jsonrpc_response(McpResponse::error(
                id,
                McpError::Unauthorized(
                    "写/动钱工具被禁用：节点未配置 MCP_BEARER_TOKEN".to_string(),
                ),
            ));
        }
    }

    let response = match req.method_enum() {
        McpMethod::Initialize => {
            let caps = json!({ "tools": { "listChanged": false } });
            let result = initialize_result_value("gsn-agent-market-http", caps, None);
            McpResponse::success(id, result)
        }
        McpMethod::Ping => McpResponse::success(id, json!({})),
        McpMethod::ToolsList => McpResponse::success(id, json!({"tools": tools})),
        McpMethod::ToolsCall => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = req.params.get("arguments").cloned().unwrap_or(json!({}));
            // v2.6.8（GAP §8.4）：未知工具返回正常 result + isError:true，而非协议级错误。
            if !is_known_tool(name) {
                let tr = ToolResult::error(format!("未知工具: {name}"));
                return jsonrpc_response(McpResponse::success(
                    id,
                    serde_json::to_value(tr).unwrap_or(json!({})),
                ));
            }
            // v2.8.6（GAP §3.5）：生产传输统一在进入执行器前校验 inputSchema，
            // 缺必填 / 类型错误（如 amount="lots"、guilty=1）返回 -32602，
            // 不再直接调 bridge 造成静默降级（存 0 返 deposited）。
            if let Some(td) = find_tool(&tools, name) {
                if let Err(msg) = validate_arguments(&td.input_schema, &args) {
                    return jsonrpc_response(McpResponse::error(id, McpError::InvalidParams(msg)));
                }
            }
            // MCP 已通过 Bearer 认证，据此派生沙箱所有权主体（与 REST 同一逻辑）
            let mcp_caller = crate::node::extract_caller(auth_header);
            let tool_result = if SandboxMcpBridge::is_sandbox_tool(name) {
                sb_bridge.call(name, &args, mcp_caller.as_deref()).await
            } else {
                bridge.call(name, &args).await
            };
            McpResponse::success(id, serde_json::to_value(tool_result).unwrap_or(json!({})))
        }
        McpMethod::ResourcesList => McpResponse::success(id, json!({"resources": []})),
        McpMethod::ResourcesRead => {
            McpResponse::error(id, McpError::InvalidRequest("resource 不存在".to_string()))
        }
        McpMethod::PromptsList => McpResponse::success(id, json!({"prompts": []})),
        McpMethod::PromptsGet => {
            McpResponse::error(id, McpError::InvalidRequest("prompt 不存在".to_string()))
        }
        McpMethod::Custom => McpResponse::error(id, McpError::MethodNotFound(req.method.clone())),
    };

    jsonrpc_response(response)
}

/// 写/动钱工具集合：会改账户余额、质押、任务状态或罚没。
fn is_mutating_tool(name: &str) -> bool {
    matches!(
        name,
        "market_register_agent"
            | "market_publish_task"
            | "market_submit_bid"
            | "market_match_task"
            | "market_submit_result"
            | "market_verify_result"
            | "market_settle_task"
            | "market_open_dispute"
            | "market_arbitrate"
            | "market_deposit"
            | "sandbox_create"
            | "sandbox_destroy"
            | "sandbox_pause"
            | "sandbox_resume"
            | "sandbox_run_code"
    )
}

fn is_known_tool(name: &str) -> bool {
    MarketMcpBridge::tool_definitions()
        .iter()
        .any(|t| t.name == name)
        || SandboxMcpBridge::tool_definitions()
            .iter()
            .any(|t| t.name == name)
}

/// 处理 GET /api/v1/mcp（SSE 流的首帧说明）
///
/// 完整 SSE 长连接需要服务器保持连接；在无状态响应中返回一个 event-stream
/// 的初始化帧，告知客户端工具数量与改用 POST 的无状态端点。
pub fn handle_get() -> McpHttp {
    let mut tools = MarketMcpBridge::tool_definitions();
    tools.extend(SandboxMcpBridge::tool_definitions());
    // SSE 格式：event: xxx\ndata: yyy\n\n
    let init_data = serde_json::json!({
        "transport": "streamable-http",
        "stateless_endpoint": "/api/v1/mcp",
        "tool_count": tools.len(),
        "tools": tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
    });
    let stream = format!("event: ready\ndata: {}\n\nretry: 3000\n", init_data);
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
        status_text: if resp.is_success() {
            "OK"
        } else {
            "Bad Request"
        },
        content_type: "application/json".to_string(),
        body: serde_json::to_string(&resp).unwrap_or_default(),
    }
}
