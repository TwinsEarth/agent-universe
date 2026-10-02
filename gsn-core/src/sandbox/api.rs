//! Agent Sandbox HTTP API（v2.8.7：认证 + 所有权 + 请求体配置 + 审计）
//!
//! 安全边界（v2.8.7 修复未认证 RCE）：
//! - 变更类操作（create/exec/pause/resume/destroy）必须带认证主体 `caller`；
//! - create 时把沙箱绑定到 caller，后续 exec/pause/resume/destroy 校验
//!   调用者即所有者，否则 403；
//! - create 请求体中的配置（template / timeout / env / resources /
//!   allowed_domains / initial_files）会真正解析并生效，不再被丢弃；
//! - 每个变更操作写一条审计记录。

use super::config::SandboxConfig;
use super::manager::SandboxManager;
use super::runtime::CodeLanguage;
use super::state::SandboxState;
use super::SandboxError;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 处理沙箱 API
///
/// `caller`：认证后的调用主体（None = 未认证；GET 放行，变更类拒绝）
pub fn handle_api(
    method: &str,
    full_path: &str,
    body: &str,
    caller: Option<&str>,
    manager: &mut SandboxManager,
) -> (u16, Value) {
    let parsed_path = parse_path(method, full_path);
    let (action, id) = match parsed_path {
        Some(v) => v,
        None => return (404, json!({ "error": "not found" })),
    };

    // GET 列表 / 单个详情：只读放行（与 REST 一致）
    if action == "list" {
        let list: Vec<Value> = manager
            .list_sandboxes()
            .iter()
            .map(|s| json!({ "id": s }))
            .collect();
        return (200, json!({ "sandboxes": list, "count": list.len() }));
    }
    if action == "get" {
        let id = id.unwrap_or_default();
        if !manager.exists(&id) {
            return (404, json!({ "error": format!("sandbox {id} not found") }));
        }
        let state = manager.state_of(&id).unwrap_or(SandboxState::Pending);
        return (
            200,
            json!({
                "id": id,
                "state": state_label(state),
                "owner": manager.owner_of(&id).unwrap_or_default(),
            }),
        );
    }

    // 以下为变更类操作：必须认证
    let caller = match caller.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => c,
        None => {
            return (
                401,
                json!({ "error": "未认证：变更类沙箱操作需要 Bearer 认证" }),
            )
        }
    };

    match action {
        "create" => {
            let parsed = if body.trim().is_empty() {
                json!({})
            } else {
                match serde_json::from_str::<Value>(body) {
                    Ok(v) => v,
                    Err(e) => {
                        return (400, json!({ "error": format!("请求体不是合法 JSON: {e}") }))
                    }
                }
            };
            let cfg = match config_from_body(&parsed, manager.default_config()) {
                Ok(c) => c,
                Err(e) => return (400, json!({ "error": e })),
            };
            // v3.5.2（AU-10）：在 cfg 被 acquire 消费前快照其豁免边界与理由，
            // 无论 create 成功/失败都把"放弃了哪条边界、为什么"落审计。
            // config_from_body 返回 Option：None 表示走 default_cfg。
            let waivers_audit: Vec<(String, String)> = match &cfg {
                Some(c) => c.waivers.iter(),
                None => manager.default_config().waivers.iter(),
            }
            .map(|w| (w.boundary.as_str().to_string(), w.justification.clone()))
            .collect();
            // 构造一条带豁免边界与理由的审计记录（成功/失败各一）。
            let build_entry = |sandbox_id: &str, outcome: &str, grade| {
                let mut e = super::security::audit_entry(
                    sandbox_id, caller, "create", sandbox_id, outcome, grade,
                );
                e.waivers = waivers_audit
                    .iter()
                    .map(
                        |(boundary, justification)| super::security::WaiverAuditEntry {
                            boundary: boundary.clone(),
                            justification: justification.clone(),
                        },
                    )
                    .collect();
                e
            };
            match manager.acquire(cfg) {
                Ok(id) => {
                    if let Err(e) = manager.bind_owner(&id, caller) {
                        return (error_status(&e), json!({ "error": e.to_string() }));
                    }
                    let state = manager.state_of(&id).unwrap_or(SandboxState::Pending);
                    let owner = manager
                        .owner_of(&id)
                        .map(str::to_string)
                        .unwrap_or_default();
                    let _ = manager.log_audit(build_entry(
                        &id,
                        "created",
                        super::security::EvidenceGrade::Unverified,
                    ));
                    (
                        201,
                        json!({
                            "id": id,
                            "state": state_label(state),
                            "owner": owner,
                            "status": "created",
                        }),
                    )
                }
                Err(e) => {
                    // v3.5.2（AU-10）：create 失败也留审计（含豁免信息），避免失败路径无痕。
                    let _ = manager.log_audit(build_entry(
                        "",
                        "failed",
                        super::security::EvidenceGrade::Unverifiable,
                    ));
                    (error_status(&e), json!({ "error": e.to_string() }))
                }
            }
        }

        "exec" => {
            let id = id.clone().unwrap_or_default();
            if let Err(e) = manager.check_owner(&id, Some(caller)) {
                return (error_status(&e), json!({ "error": e.to_string() }));
            }
            let parsed = match serde_json::from_str::<Value>(body) {
                Ok(v) => v,
                Err(e) => return (400, json!({ "error": format!("请求体不是合法 JSON: {e}") })),
            };
            let language = parsed
                .get("language")
                .and_then(|v| v.as_str())
                .unwrap_or("python");
            let code = match parsed.get("code").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return (400, json!({ "error": "缺少 'code' 字段" })),
            };
            let lang = match CodeLanguage::from_label(language) {
                Some(l) => l,
                None => return (400, json!({ "error": format!("不支持的语言: {language}") })),
            };

            // 预热池沙箱取出时是 Paused，先 wake
            if manager.state_of(&id) == Some(SandboxState::Paused) {
                if let Err(e) = manager.wake(&id) {
                    return (error_status(&e), json!({ "error": e.to_string() }));
                }
            }

            let result = match manager.sandbox_mut(&id) {
                Ok(sandbox) => sandbox.run_code(lang, code),
                Err(e) => return (error_status(&e), json!({ "error": e.to_string() })),
            };
            match result {
                Ok(r) => {
                    let grade = super::security::EvidenceGrade::Unverified;
                    let _ = manager.record_audit(
                        &id,
                        caller,
                        "exec",
                        language,
                        &format!("exit={}", r.exit_code),
                        grade,
                    );
                    (
                        200,
                        json!({
                            "id": id,
                            "exit_code": r.exit_code,
                            "stdout": r.stdout,
                            "stderr": r.stderr,
                            "wall_ms": r.wall_ms,
                        }),
                    )
                }
                Err(e) => {
                    let _ = manager.record_audit(
                        &id,
                        caller,
                        "exec",
                        language,
                        &format!("error: {e}"),
                        super::security::EvidenceGrade::Unverifiable,
                    );
                    (error_status(&e), json!({ "error": e.to_string() }))
                }
            }
        }

        "pause" => {
            let id = id.unwrap_or_default();
            if let Err(e) = manager.check_owner(&id, Some(caller)) {
                return (error_status(&e), json!({ "error": e.to_string() }));
            }
            match manager.pause(&id) {
                Ok(_) => {
                    let _ = manager.record_audit(
                        &id,
                        caller,
                        "pause",
                        &id,
                        "paused",
                        super::security::EvidenceGrade::Unverified,
                    );
                    (200, json!({ "id": id, "state": "paused" }))
                }
                Err(e) => (error_status(&e), json!({ "error": e.to_string() })),
            }
        }

        "resume" => {
            let id = id.unwrap_or_default();
            if let Err(e) = manager.check_owner(&id, Some(caller)) {
                return (error_status(&e), json!({ "error": e.to_string() }));
            }
            match manager.wake(&id) {
                Ok(_) => {
                    let _ = manager.record_audit(
                        &id,
                        caller,
                        "resume",
                        &id,
                        "running",
                        super::security::EvidenceGrade::Unverified,
                    );
                    (200, json!({ "id": id, "state": "running" }))
                }
                Err(e) => (error_status(&e), json!({ "error": e.to_string() })),
            }
        }

        "destroy" => {
            let id = id.unwrap_or_default();
            if let Err(e) = manager.check_owner(&id, Some(caller)) {
                return (error_status(&e), json!({ "error": e.to_string() }));
            }
            match manager.destroy(&id) {
                Ok(_) => {
                    let _ = manager.record_audit(
                        &id,
                        caller,
                        "destroy",
                        &id,
                        "destroyed",
                        super::security::EvidenceGrade::Unverified,
                    );
                    (200, json!({ "id": id, "status": "destroyed" }))
                }
                Err(e) => (error_status(&e), json!({ "error": e.to_string() })),
            }
        }

        _ => (404, json!({ "error": "unknown action" })),
    }
}

/// 从请求 body 解析自定义配置；返回 None 表示无自定义（用默认/预热池）
fn config_from_body(
    parsed: &Value,
    default: &SandboxConfig,
) -> Result<Option<SandboxConfig>, String> {
    let custom_keys = [
        "template",
        "timeout_ms",
        "env",
        "allowed_domains",
        "initial_files",
        "cpu_millis",
        "mem_mb",
        "disk_mb",
    ];
    let has_custom = custom_keys.iter().any(|k| parsed.get(k).is_some());
    if !has_custom {
        return Ok(None);
    }

    let mut cfg = default.clone();
    if let Some(t) = parsed.get("template").and_then(|v| v.as_str()) {
        if !t.trim().is_empty() {
            cfg.template = t.to_string();
        }
    }
    if let Some(t) = parsed.get("timeout_ms").and_then(|v| v.as_u64()) {
        if t == 0 {
            return Err("timeout_ms 必须 > 0".into());
        }
        cfg.resources.timeout_ms = t;
    }
    if let Some(c) = parsed.get("cpu_millis").and_then(|v| v.as_u64()) {
        if c == 0 {
            return Err("cpu_millis 必须 > 0".into());
        }
        cfg.resources.cpu_millis = c as u32;
    }
    if let Some(m) = parsed.get("mem_mb").and_then(|v| v.as_u64()) {
        if m == 0 {
            return Err("mem_mb 必须 > 0".into());
        }
        cfg.resources.mem_mb = m as u32;
    }
    if let Some(d) = parsed.get("disk_mb").and_then(|v| v.as_u64()) {
        if d == 0 {
            return Err("disk_mb 必须 > 0".into());
        }
        cfg.resources.disk_mb = d as u32;
    }
    if let Some(env) = parsed.get("env") {
        cfg.env = parse_env(env)?;
    }
    if let Some(files) = parsed.get("initial_files").and_then(|v| v.as_object()) {
        let mut vf = Vec::new();
        for (k, val) in files {
            let content = val.as_str().ok_or("initial_files 值必须是字符串")?;
            vf.push((k.clone(), content.to_string()));
        }
        cfg.initial_files = vf;
    }
    if let Some(domains) = parsed.get("allowed_domains").and_then(|v| v.as_array()) {
        let list: Vec<String> = domains
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        if !list.is_empty() {
            // 显式白名单 → 放行这些出站目标
            cfg.network.allow_egress = true;
            cfg.network.egress_allowlist = list;
        }
    }

    if let Err(errs) = cfg.validate() {
        return Err(errs.join("; "));
    }
    Ok(Some(cfg))
}

/// 解析 env（对象 {KEY: VAL} 或数组 ["KEY=VAL"]）
fn parse_env(env: &Value) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    if let Some(obj) = env.as_object() {
        for (k, v) in obj {
            let vs = v.as_str().ok_or("env 值必须是字符串")?;
            out.push((k.clone(), vs.to_string()));
        }
    } else if let Some(arr) = env.as_array() {
        for item in arr {
            let s = item.as_str().ok_or("env 数组元素必须形如 'KEY=VAL'")?;
            let (k, v) = s.split_once('=').ok_or("env 数组元素必须形如 KEY=VAL")?;
            out.push((k.to_string(), v.to_string()));
        }
    } else {
        return Err("env 必须是对象或数组".into());
    }
    Ok(out)
}

/// 解析路径
fn parse_path(method: &str, full_path: &str) -> Option<(&'static str, Option<String>)> {
    let segments: Vec<&str> = full_path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    // 找 "sandboxes" 段
    let sb_idx = segments.iter().position(|s| *s == "sandboxes")?;
    let after = &segments[sb_idx + 1..];

    match after.len() {
        0 => {
            if method == "POST" {
                Some(("create", None))
            } else if method == "GET" {
                Some(("list", None))
            } else {
                None
            }
        }
        1 => {
            let id = after[0].to_string();
            match method {
                "GET" => Some(("get", Some(id))),
                "DELETE" => Some(("destroy", Some(id))),
                _ => None,
            }
        }
        2 => {
            let id = after[0].to_string();
            let sub = after[1];
            let action = match (method, sub) {
                ("POST", "exec") | ("POST", "commands") | ("POST", "run") => "exec",
                ("POST", "pause") => "pause",
                ("POST", "resume") => "resume",
                _ => return None,
            };
            Some((action, Some(id)))
        }
        _ => None,
    }
}

/// 判断是否为沙箱路由
pub fn is_sandbox_route(path: &str) -> bool {
    path.contains("/sandboxes")
}

/// 默认沙箱目录
pub fn default_sandbox_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("sandboxes")
}

/// 错误类型 → HTTP 状态码
fn error_status(e: &SandboxError) -> u16 {
    match e {
        SandboxError::NotFound(_) => 404,
        SandboxError::NetworkDenied(_)
        | SandboxError::IsolationViolation(_)
        | SandboxError::EnvBlocked(_) => 403,
        SandboxError::InvalidConfig(_) | SandboxError::InvalidLifecycle { .. } => 400,
        SandboxError::ResourceLimitExceeded { .. } => 429,
        SandboxError::ExecFailed { .. } => 422,
        // 配置语法合法，但请求的隔离边界后端无法强制：需换后端或显式豁免
        SandboxError::PolicyNotEnforceable { .. } => 422,
        SandboxError::Internal(_) => 500,
    }
}

/// 状态 → 可读标签
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
