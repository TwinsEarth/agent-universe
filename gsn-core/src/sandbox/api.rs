//! Agent Sandbox 的 REST / E2B 兼容 API（v2.8.0）
//!
//! 纯函数处理器：接收解析后的 method/path/body 与一个 `&mut SandboxManager`，
//! 返回 `(HTTP 状态码, JSON)`，与 `api::rest::route` 风格一致，便于无网络测试。
//!
//! 路径（自有 + E2B 兼容别名）：
//! - `POST /api/v1/sandboxes`（E2B 别名 `/v1/sandboxes`）：创建/acquire；
//! - `GET  /api/v1/sandboxes/{id}`：查询状态；
//! - `GET  /api/v1/sandboxes`：列出受管沙箱；
//! - `POST /api/v1/sandboxes/{id}/exec`（E2B 别名 `/commands`）：执行代码；
//! - `POST /api/v1/sandboxes/{id}/pause` / `/resume`：休眠/唤醒；
//! - `DELETE /api/v1/sandboxes/{id}`：销毁。

use super::manager::SandboxManager;
use super::runtime::CodeLanguage;
use super::state::SandboxState;
use serde_json::{json, Value};
use std::path::PathBuf;

/// 把沙箱错误映射为 HTTP 状态码
fn error_status(e: &crate::sandbox::error::SandboxError) -> u16 {
    use crate::sandbox::error::SandboxError::*;
    match e {
        NotFound(_) => 404,
        NetworkDenied(_) | IsolationViolation(_) | EnvBlocked(_) => 403,
        InvalidConfig(_) | InvalidLifecycle { .. } => 400,
        ResourceLimitExceeded { .. } => 429,
        ExecFailed { .. } => 422,
        Internal(_) => 500,
    }
}

fn err_json(e: crate::sandbox::error::SandboxError) -> (u16, Value) {
    let status = error_status(&e);
    (status, json!({ "error": e.to_string() }))
}

/// 解析沙箱路径为结构化目标
/// 返回 (action, sandbox_id)；action 标识操作类型
fn parse_path(method: &str, seg: &[&str]) -> Option<(&'static str, Option<String>)> {
    // 去掉前导空段
    let seg: Vec<&str> = seg.iter().copied().filter(|s| !s.is_empty()).collect();
    // 形如 api/v1/sandboxes[/id][/action] 或 v1/sandboxes... 或 sandboxes...
    let idx = seg.iter().position(|s| *s == "sandboxes")?;
    let mut rest: Vec<&str> = seg[idx + 1..].to_vec();

    let id = if rest.is_empty() {
        None
    } else {
        Some(rest.remove(0).to_string())
    };
    let sub = if rest.is_empty() { None } else { Some(rest.remove(0)) };

    let action: &'static str = match (method, id.is_some(), sub) {
        ("POST", false, None) => "create",
        ("GET", false, None) => "list",
        ("GET", true, None) => "get",
        ("DELETE", true, None) => "destroy",
        ("POST", true, Some("exec")) | ("POST", true, Some("commands")) | ("POST", true, Some("run")) => "exec",
        ("POST", true, Some("pause")) => "pause",
        ("POST", true, Some("resume")) => "resume",
        _ => return None,
    };
    Some((action, id))
}

/// 处理沙箱 REST 请求（纯函数）
pub fn handle_api(
    method: &str,
    full_path: &str,
    body: &str,
    manager: &mut SandboxManager,
) -> (u16, Value) {
    let path = full_path.split('?').next().unwrap_or(full_path);
    let seg: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    let (action, id) = match parse_path(method, &seg) {
        Some(x) => x,
        None => {
            // 路径不含 sandboxes，不归本处理器
            return (404, json!({ "error": "not a sandbox route" }));
        }
    };

    let parsed: Option<Value> = if body.is_empty() {
        None
    } else {
        serde_json::from_str(body).ok()
    };

    match action {
        "create" => {
            // body 可带 warm（是否从预热池取）、template 等；当前统一 acquire
            match manager.acquire(None) {
                Ok(id) => (
                    201,
                    json!({
                        "sandbox_id": id,
                        "state": "running",
                        "message": "沙箱已创建并启动（进程级隔离）",
                    }),
                ),
                Err(e) => err_json(e),
            }
        }
        "list" => (
            200,
            json!({
                "count": manager.count(),
                "warm_pool_size": manager.warm_pool_len(),
            }),
        ),
        "get" => {
            let id = id.unwrap_or_default();
            match manager.state_of(&id) {
                Some(state) => (
                    200,
                    json!({ "sandbox_id": id, "state": state_label(state) }),
                ),
                None => (404, json!({ "error": format!("沙箱不存在: {id}") })),
            }
        }
        "destroy" => {
            let id = id.unwrap_or_default();
            match manager.destroy(&id) {
                Ok(()) => (200, json!({ "sandbox_id": id, "destroyed": true })),
                Err(e) => err_json(e),
            }
        }
        "pause" => {
            let id = id.unwrap_or_default();
            match manager.release(&id) {
                Ok(()) => (200, json!({ "sandbox_id": id, "state": "paused" })),
                Err(e) => err_json(e),
            }
        }
        "resume" => {
            let id = id.unwrap_or_default();
            match manager.wake(&id) {
                Ok(()) => (200, json!({ "sandbox_id": id, "state": "running" })),
                Err(e) => err_json(e),
            }
        }
        "exec" => {
            let id = id.unwrap_or_default();
            let v = match parsed {
                Some(v) => v,
                None => return (400, json!({ "error": "缺少请求体 {language, code}" })),
            };
            let lang_str = v
                .get("language")
                .and_then(|x| x.as_str())
                .unwrap_or("python");
            let code = match v.get("code").and_then(|x| x.as_str()) {
                Some(c) => c,
                None => return (400, json!({ "error": "缺少 code 字段" })),
            };
            let lang = match CodeLanguage::from_label(lang_str) {
                Some(l) => l,
                None => return (400, json!({ "error": format!("不支持的语言: {lang_str}") })),
            };
            // 确保沙箱处于 running
            if manager.state_of(&id) == Some(SandboxState::Paused) {
                let _ = manager.wake(&id);
            }
            match manager.sandbox_mut(&id) {
                Ok(sb) => match sb.run_code(lang, code) {
                    Ok(r) => (
                        200,
                        json!({
                            "sandbox_id": id,
                            "exit_code": r.exit_code,
                            "stdout": r.stdout,
                            "stderr": r.stderr,
                            "wall_ms": r.wall_ms,
                        }),
                    ),
                    Err(e) => err_json(e),
                },
                Err(e) => err_json(e),
            }
        }
        _ => (404, json!({ "error": "unknown sandbox action" })),
    }
}

fn state_label(s: SandboxState) -> &'static str {
    match s {
        SandboxState::Pending => "pending",
        SandboxState::Creating => "creating",
        SandboxState::Starting => "starting",
        SandboxState::Running => "running",
        SandboxState::Pausing => "pausing",
        SandboxState::Paused => "paused",
        SandboxState::Resuming => "resuming",
        SandboxState::Stopping => "stopping",
        SandboxState::Stopped => "stopped",
        SandboxState::Failed => "failed",
    }
}

/// 判断一个路径是否属于沙箱 API（供 HTTP 层快速分流）
pub fn is_sandbox_route(path: &str) -> bool {
    let p = path.split('?').next().unwrap_or(path);
    let seg: Vec<&str> = p.trim_start_matches('/').split('/').collect();
    // /sandboxes, /api/v1/sandboxes, /v1/sandboxes
    let joined = seg
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<&str>>()
        .join("/");
    joined == "sandboxes"
        || joined.starts_with("sandboxes/")
        || joined.starts_with("api/v1/sandboxes")
        || joined.starts_with("v1/sandboxes")
}

/// 构建管理器默认使用的沙箱根目录（data-dir 下）
pub fn default_sandbox_dir(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("sandboxes")
}
