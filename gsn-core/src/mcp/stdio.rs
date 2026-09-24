//! MCP stdio 传输
//!
//! MCP 标准的本地传输：从 stdin 逐行读取 JSON-RPC，向 stdout 写响应，
//! 日志写 stderr。Claude Desktop / Cursor 等通过 `command + args` 启动本服务。
//!
//! 启动：`gsn mcp`（stdio）或 `gsn mcp --transport stdio`

use crate::api::market_actor::MarketActorHandle;
use crate::mcp::market_tools::MarketMcpBridge;
use crate::mcp::protocol::*;
use crate::mcp::tool::ToolDefinition;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// 运行 stdio MCP 服务器（阻塞直到 stdin 关闭）
pub async fn run_stdio() -> anyhow::Result<()> {
    let market = MarketActorHandle::spawn();
    let bridge = MarketMcpBridge::new(market.clone());
    let tools = MarketMcpBridge::tool_definitions();

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    eprintln!("[gsn-mcp] stdio server 启动，{} 个市场工具可用", tools.len());

    while let Some(line) = reader.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        // 解析：可能是请求或通知
        let raw: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp = McpResponse::error(
                    RequestId::Number(0),
                    McpError::ParseError(e.to_string()),
                );
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
            McpMethod::ToolsList => {
                let result = json!({ "tools": tools });
                McpResponse::success(id, result)
            }
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

        write_line(&response).await?;
    }

    eprintln!("[gsn-mcp] stdin 关闭，退出");
    Ok(())
}

/// 构造 initialize 响应
fn initialize_response(id: RequestId, tools: &[ToolDefinition]) -> McpResponse {
    let result = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "gsn-agent-market",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": { "listChanged": false },
        },
        "gsn": {
            "tool_count": tools.len(),
            "description": "Agent Universe 智能体市场 MCP 服务",
        }
    });
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
