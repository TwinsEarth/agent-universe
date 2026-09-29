//! 沙箱 MCP 工具桥接（v2.8.0）
//!
//! 把 Agent Sandbox 的能力注册为标准 MCP 工具，让大模型能够：
//! 创建沙箱、在沙箱内执行代码、查询状态、休眠/唤醒、销毁。
//!
//! 桥接持有 `Arc<Mutex<SandboxManager>>`，tools/call 时经 `spawn_blocking`
//! 复用 `sandbox::handle_sandbox_api`（与 HTTP REST 同一处理器，单一来源）。

use crate::mcp::tool::*;
use crate::sandbox::manager::SandboxManager;
use crate::sandbox::handle_sandbox_api;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// 沙箱 MCP 桥接
pub struct SandboxMcpBridge {
    manager: Arc<Mutex<SandboxManager>>,
}

impl SandboxMcpBridge {
    pub fn new(manager: Arc<Mutex<SandboxManager>>) -> Self {
        Self { manager }
    }

    /// 全部沙箱工具定义
    pub fn tool_definitions() -> Vec<ToolDefinition> {
        vec![
            tool("sandbox_create", "创建并启动一个隔离沙箱（进程级隔离，返回 sandbox_id）"),
            tool("sandbox_list", "列出受管沙箱与预热池大小（无参数）"),
            tool("sandbox_get", "按 sandbox_id 查询沙箱状态")
                .param("sandbox_id", "string", "沙箱 ID", true),
            tool("sandbox_run_code", "在指定沙箱内执行代码（Python/JavaScript）并返回输出")
                .param("sandbox_id", "string", "沙箱 ID", true)
                .param("language", "string", "语言：python 或 javascript", true)
                .param("code", "string", "要执行的源代码", true),
            tool("sandbox_pause", "休眠沙箱（释放资源，保留文件）")
                .param("sandbox_id", "string", "沙箱 ID", true),
            tool("sandbox_resume", "唤醒休眠沙箱")
                .param("sandbox_id", "string", "沙箱 ID", true),
            tool("sandbox_destroy", "销毁沙箱并清理临时文件")
                .param("sandbox_id", "string", "沙箱 ID", true),
        ]
    }

    /// 执行工具调用
    pub async fn call(&self, name: &str, args: &Value) -> ToolResult {
        let get_str = |k: &str| -> String {
            args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
        };

        // 构造 (method, path, body) 复用 REST 处理器
        let (method, path, body): (&str, String, String) = match name {
            "sandbox_create" => ("POST", "/api/v1/sandboxes".to_string(), "{}".to_string()),
            "sandbox_list" => ("GET", "/api/v1/sandboxes".to_string(), String::new()),
            "sandbox_get" => (
                "GET",
                format!("/api/v1/sandboxes/{}", get_str("sandbox_id")),
                String::new(),
            ),
            "sandbox_run_code" => {
                let id = get_str("sandbox_id");
                let lang = get_str("language");
                let code = args.get("code").and_then(|v| v.as_str()).unwrap_or("");
                let payload = json!({ "language": lang, "code": code }).to_string();
                (
                    "POST",
                    format!("/api/v1/sandboxes/{id}/exec"),
                    payload,
                )
            }
            "sandbox_pause" => (
                "POST",
                format!("/api/v1/sandboxes/{}/pause", get_str("sandbox_id")),
                String::new(),
            ),
            "sandbox_resume" => (
                "POST",
                format!("/api/v1/sandboxes/{}/resume", get_str("sandbox_id")),
                String::new(),
            ),
            "sandbox_destroy" => (
                "DELETE",
                format!("/api/v1/sandboxes/{}", get_str("sandbox_id")),
                String::new(),
            ),
            _ => return ToolResult::error(format!("未知沙箱工具: {name}")),
        };

        let mgr = self.manager.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut guard = mgr.lock().unwrap();
            handle_sandbox_api(method, &path, &body, &mut guard)
        })
        .await;

        match result {
            Ok((status, payload)) if (200..300).contains(&status) => {
                ToolResult::text(payload.to_string())
            }
            Ok((_status, payload)) => {
                let msg = payload
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("沙箱工具执行失败")
                    .to_string();
                ToolResult::error(msg)
            }
            Err(_) => ToolResult::error("沙箱任务异常"),
        }
    }

    /// 名称是否为沙箱工具
    pub fn is_sandbox_tool(name: &str) -> bool {
        Self::tool_definitions().iter().any(|t| t.name == name)
    }
}
