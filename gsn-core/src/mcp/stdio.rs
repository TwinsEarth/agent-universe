//! MCP stdio 传输
//!
//! MCP 标准的本地传输：从 stdin 逐行读取 JSON-RPC，向 stdout 写响应，
//! 日志写 stderr。Claude Desktop / Cursor 等通过 `command + args` 启动本服务。
//!
//! 启动：`gsn mcp`（stdio）或 `gsn mcp --transport stdio`

use crate::api::market_actor::MarketActorHandle;
use crate::mcp::market_tools::MarketMcpBridge;
use crate::mcp::protocol::*;
use crate::mcp::sandbox_tools::SandboxMcpBridge;
use crate::mcp::tool::ToolResult;
use crate::mcp::tool::{find_tool, validate_arguments, ToolDefinition};
use crate::sandbox::SandboxManager;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// 运行 stdio MCP 服务器（阻塞直到 stdin 关闭）
pub async fn run_stdio() -> anyhow::Result<()> {
    let market = MarketActorHandle::spawn();
    let bridge = MarketMcpBridge::new(market.clone());
    let sb_dir = crate::sandbox::default_sandbox_dir(&crate::node::default_data_dir());
    let sandbox_mgr = Arc::new(Mutex::new(SandboxManager::new(
        sb_dir,
        crate::sandbox::config::SandboxConfig::default(),
        0,
    )));
    let sb_bridge = SandboxMcpBridge::new(sandbox_mgr.clone());
    let mut tools = MarketMcpBridge::tool_definitions();
    tools.extend(SandboxMcpBridge::tool_definitions());

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    eprintln!(
        "[gsn-mcp] stdio server 启动，{} 个市场工具可用",
        tools.len()
    );

    while let Some(line) = reader.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        // 解析：可能是请求或通知
        let raw: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp =
                    McpResponse::error(RequestId::Number(0), McpError::ParseError(e.to_string()));
                write_line(&resp).await?;
                continue;
            }
        };

        // 通知（无 id）不响应，如 notifications/initialized
        if raw.get("id").is_none() {
            let method = raw.get("method").and_then(|v| v.as_str()).unwrap_or("");
            eprintln!("[gsn-mcp] 通知: {}", method);
            continue;
        }

        // 解析为请求
        let req: McpRequest = match serde_json::from_value(raw) {
            Ok(r) => r,
            Err(e) => {
                let resp = McpResponse::error(
                    RequestId::Number(0),
                    McpError::InvalidRequest(e.to_string()),
                );
                write_line(&resp).await?;
                continue;
            }
        };

        let id = req.id.clone();
        let method = req.method_enum();
        let response = match method {
            McpMethod::Initialize => initialize_response(id, &tools),
            McpMethod::Ping => McpResponse::success(id, json!({})),
            McpMethod::ToolsList => {
                let result = json!({ "tools": tools });
                McpResponse::success(id, result)
            }
            McpMethod::ToolsCall => {
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = req.params.get("arguments").cloned().unwrap_or(json!({}));
                // v2.8.6（GAP §3.5）：统一在执行前定位 inputSchema 并校验，
                // 缺必填 / 类型错误（amount="lots"）返回 -32602；未知工具 isError:true。
                match find_tool(&tools, name) {
                    Some(td) => {
                        if let Err(msg) = validate_arguments(&td.input_schema, &args) {
                            McpResponse::error(id, McpError::InvalidParams(msg))
                        } else {
                            // stdio 为本机受信传输（无网络），使用固定本地主体
                            let tool_result = if SandboxMcpBridge::is_sandbox_tool(name) {
                                sb_bridge.call(name, &args, Some("local:stdio")).await
                            } else {
                                bridge.call(name, &args).await
                            };
                            McpResponse::success(
                                id,
                                serde_json::to_value(tool_result).unwrap_or(json!({})),
                            )
                        }
                    }
                    None => {
                        let tr = ToolResult::error(format!("未知工具: {name}"));
                        McpResponse::success(id, serde_json::to_value(tr).unwrap_or(json!({})))
                    }
                }
            }
            McpMethod::ResourcesList => McpResponse::success(id, json!({"resources": []})),
            McpMethod::ResourcesRead => {
                McpResponse::error(id, McpError::InvalidRequest("resource 不存在".to_string()))
            }
            McpMethod::PromptsList => McpResponse::success(id, json!({"prompts": []})),
            McpMethod::PromptsGet => {
                McpResponse::error(id, McpError::InvalidRequest("prompt 不存在".to_string()))
            }
            McpMethod::Custom => {
                McpResponse::error(id, McpError::MethodNotFound(req.method.clone()))
            }
        };

        write_line(&response).await?;
    }

    eprintln!("[gsn-mcp] stdin 关闭，退出");
    Ok(())
}

/// 构造 initialize 响应（标准部分复用统一构造，追加 gsn 扩展）
fn initialize_response(id: RequestId, tools: &[ToolDefinition]) -> McpResponse {
    let caps = json!({ "tools": { "listChanged": false } });
    let mut result = initialize_result_value("gsn-agent-market", caps, None);
    if let Some(map) = result.as_object_mut() {
        map.insert(
            "gsn".to_string(),
            json!({
                "tool_count": tools.len(),
                "description": "Agent Universe 智能体市场 MCP 服务",
            }),
        );
    }
    McpResponse::success(id, result)
}

/// 写一行 JSON 响应到 stdout
async fn write_line(resp: &McpResponse) -> anyhow::Result<()> {
    let s = serde_json::to_string(resp)?;
    let mut out = tokio::io::stdout();
    out.write_all(s.as_bytes()).await?;
    out.write_all(b"\n").await?;
    out.flush().await?;
    Ok(())
}
