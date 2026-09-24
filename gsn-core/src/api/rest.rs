//! REST API 路由
//!
//! v2.3.5: 完整 marketplace HTTP 端点。
//! 与传输解耦：`route` 接收解析后的 method/path/query/body，返回 (HTTP 状态码, JSON)，
//! gsn-daemon 的 TCP HTTP 服务器与测试都可直接调用。

use crate::api::market_actor::MarketActorHandle;
use serde_json::{json, Value};

/// 路由结果
pub struct Routed {
    pub status: u16,
    pub status_text: &'static str,
    pub body: Value,
}

impl Routed {
    fn ok(body: Value) -> Self {
        Routed { status: 200, status_text: "OK", body }
    }
    fn bad_request(msg: &str) -> Self {
        Routed { status: 400, status_text: "Bad Request", body: json!({"error": msg}) }
    }
    fn not_found(path: &str) -> Self {
        Routed { status: 404, status_text: "Not Found", body: json!({"error": "not_found", "path": path}) }
    }
}

/// 把 MarketResponse 转成 Routed
fn from_mr(r: crate::api::market_actor::MarketResponse, success_status: u16) -> Routed {
    use crate::api::market_actor::MarketResponse::*;
    match r {
        Ok(v) => {
            let text = if success_status == 201 { "Created" } else { "OK" };
            Routed { status: success_status, status_text: text, body: v }
        }
        Err(e) => {
            // 业务错误统一 422（语义错误），包含"不存在"用 404
            if e.contains("不存在") {
                Routed { status: 404, status_text: "Not Found", body: json!({"error": e}) }
            } else {
                Routed { status: 422, status_text: "Unprocessable Entity", body: json!({"error": e}) }
            }
        }
    }
}

/// 解析 query string 为简单 map（不处理重复键/URL 编码的完整场景，够用）
fn parse_query(q: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded = |s: &str| {
                url_decode(s)
            };
            map.insert(decoded(k), decoded(v));
        }
    }
    map
}

/// 极简 URL 解码
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(b) = u8::from_str_radix(h, 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => { out.push(b' '); i += 1; }
            b => { out.push(b); i += 1; }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// 节点元信息（由 daemon 提供）
pub struct NodeInfo {
    pub version: String,
    pub mode: String,
    pub p2p_port: u16,
    pub connected_peers: usize,
    pub uptime_ms: u128,
}

/// 核心路由函数
///
/// - `method`: HTTP 方法
/// - `full_path`: 含 query 的路径
/// - `body`: 已读取的请求体（POST/PUT）
pub async fn route(
    method: &str,
    full_path: &str,
    body: &str,
    market: &MarketActorHandle,
    info: &NodeInfo,
) -> Routed {
    let (path, query) = full_path.split_once('?').unwrap_or((full_path, ""));
    let q = parse_query(query);
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    // ───── CORS 预检 ─────
    if method == "OPTIONS" {
        return Routed { status: 204, status_text: "No Content", body: Value::Null };
    }

    // ───── 节点级（向后兼容旧路径） ─────
    match (method, path) {
        ("GET", "/") | ("GET", "/health") => {
            return Routed::ok(json!({
                "status": "ok",
                "service": "gsn-daemon",
                "version": info.version,
                "mode": info.mode,
                "p2p_port": info.p2p_port,
                "connected_peers": info.connected_peers,
                "uptime_ms": info.uptime_ms,
            }));
        }
        ("GET", "/version") => {
            return Routed::ok(json!({"name": "gsn-daemon", "version": info.version}));
        }
        ("GET", "/peers") => {
            return Routed::ok(json!({
                "version": info.version,
                "mode": info.mode,
                "connected_peers": info.connected_peers,
            }));
        }
        _ => {}
    }

    // 解析 body JSON
    let parsed_body: Option<Value> = if body.is_empty() {
        None
    } else {
        match serde_json::from_str(body) {
            Ok(v) => Some(v),
            Err(_) => return Routed::bad_request("请求体不是合法 JSON"),
        }
    };

    // ───── 新版 API: /api/v1/... 与旧版 /agents /tasks 双兼容 ─────

    // 归一化：把旧路径映射到新路径
    let normalized = normalize_legacy(method, path, segments.as_slice());

    match normalized {
        Some(RouteTarget::AgentsCollection) => {
            // POST /api/v1/agents → 注册（用 body）
            if method == "POST" {
                if let Some(v) = parsed_body {
                    return from_mr(market.register_agent(v).await, 201);
                }
                return Routed::bad_request("缺少注册卡片");
            }
            // GET /api/v1/agents?skill= 或 ?q=
            if let Some(skill) = q.get("skill") {
                return from_mr(market.discover(skill.clone()).await, 200);
            }
            if let Some(query_str) = q.get("q") {
                return from_mr(market.search(query_str.clone()).await, 200);
            }
            // 无参数：返回市场统计
            return from_mr(market.stats().await, 200);
        }
        Some(RouteTarget::AgentItem(id)) => {
            return from_mr(market.get_agent(id).await, 200);
        }
        Some(RouteTarget::TasksCollection) => {
            if method == "POST" {
                if let Some(v) = parsed_body {
                    return from_mr(market.publish_task(v).await, 201);
                }
                return Routed::bad_request("缺少请求体");
            }
            // GET: 列表统计
            return from_mr(market.list_tasks().await, 200);
        }
        Some(RouteTarget::TaskItem(id)) => {
            return from_mr(market.get_task(id).await, 200);
        }
        Some(RouteTarget::TaskBids(id)) => {
            if let Some(mut v) = parsed_body {
                // 注入 task_id
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("task_id".to_string(), json!(id));
                }
                return from_mr(market.submit_bid(v).await, 201);
            }
            return Routed::bad_request("缺少投标数据")
        }
        Some(RouteTarget::TaskMatch(id)) => {
            return from_mr(market.match_task(id).await, 200);
        }
        Some(RouteTarget::TaskResults(id)) => {
            if let Some(mut v) = parsed_body {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("task_id".to_string(), json!(id));
                }
                return from_mr(market.submit_result(v).await, 201);
            }
            return Routed::bad_request("缺少结果数据")
        }
        Some(RouteTarget::TaskVerify(id)) => {
            let approvals = q.get("approvals").and_then(|s| s.parse::<u32>().ok())
                .or_else(|| parsed_body.as_ref().and_then(|v| v.get("approvals")).and_then(|x| x.as_u64()).map(|x| x as u32))
                .unwrap_or(3);
            let size = q.get("committee_size").and_then(|s| s.parse::<u32>().ok())
                .or_else(|| parsed_body.as_ref().and_then(|v| v.get("committee_size")).and_then(|x| x.as_u64()).map(|x| x as u32))
                .unwrap_or(4);
            return from_mr(market.verify_result(id, approvals, size).await, 200);
        }
        Some(RouteTarget::TaskSettle(id)) => {
            return from_mr(market.settle_task(id).await, 200);
        }
        Some(RouteTarget::DisputesCollection) => {
            if let Some(v) = parsed_body {
                return from_mr(market.open_dispute(v).await, 201);
            }
            return Routed::bad_request("缺少争议数据")
        }
        Some(RouteTarget::DisputeArbitrate(id)) => {
            if let Some(v) = parsed_body {
                let guilty = v.get("guilty").and_then(|x| x.as_bool()).unwrap_or(false);
                let slash = v.get("slash_amount").and_then(|x| x.as_f64()).unwrap_or(0.0);
                return from_mr(market.arbitrate(id, guilty, slash).await, 200);
            }
            return Routed::bad_request("缺少仲裁数据")
        }
        Some(RouteTarget::AccountDeposit(account)) => {
            let amount = parsed_body.as_ref()
                .and_then(|v| v.get("amount"))
                .and_then(|x| x.as_f64())
                .or_else(|| q.get("amount").and_then(|s| s.parse::<f64>().ok()));
            if let Some(amount) = amount {
                return from_mr(market.deposit(account, amount).await, 200);
            }
            return Routed::bad_request("缺少 amount")
        }
        Some(RouteTarget::AccountBalance(account)) => {
            return from_mr(market.balance(account).await, 200);
        }
        Some(RouteTarget::Conservation) => {
            return from_mr(market.conservation().await, 200);
        }
        Some(RouteTarget::Leaderboard) => {
            let limit = q.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
            return from_mr(market.leaderboard(limit).await, 200);
        }
        Some(RouteTarget::Stats) => {
            return from_mr(market.stats().await, 200);
        }
        None => {}
    }

    Routed::not_found(path)
}

/// 路由目标（归一化后）
enum RouteTarget {
    AgentsCollection,
    AgentItem(String),
    TasksCollection,
    TaskItem(String),
    TaskBids(String),
    TaskMatch(String),
    TaskResults(String),
    TaskVerify(String),
    TaskSettle(String),
    DisputesCollection,
    DisputeArbitrate(String),
    AccountDeposit(String),
    AccountBalance(String),
    Conservation,
    Leaderboard,
    Stats,
}

/// 把路径（含旧版兼容）映射到路由目标
fn normalize_legacy(method: &str, path: &str, seg: &[&str]) -> Option<RouteTarget> {
    // 新版 /api/v1/...
    if seg.first() == Some(&"api") && seg.get(1) == Some(&"v1") {
        let rest = &seg[2..];
        return map_api_segments(rest);
    }

    // 旧版兼容：POST /agents 注册, GET /agents 列表
    match path {
        "/agents" if method == "POST" => return Some(RouteTarget::AgentsCollection),
        "/agents" if method == "GET" => return Some(RouteTarget::AgentsCollection),
        "/tasks" if method == "GET" => return Some(RouteTarget::TasksCollection),
        _ => {}
    }
    None
}

/// 映射 /api/v1 之后的段
fn map_api_segments(seg: &[&str]) -> Option<RouteTarget> {
    match seg {
        ["agents"] => Some(RouteTarget::AgentsCollection),
        ["agents", "search"] => {
            // q 通过 query 传递，这里给占位；实际 query 在 route 中处理
            Some(RouteTarget::AgentsCollection)
        }
        ["agents", id] => Some(RouteTarget::AgentItem((*id).to_string())),
        ["tasks"] => Some(RouteTarget::TasksCollection),
        ["tasks", id] => Some(RouteTarget::TaskItem((*id).to_string())),
        ["tasks", id, "bids"] => Some(RouteTarget::TaskBids((*id).to_string())),
        ["tasks", id, "match"] => Some(RouteTarget::TaskMatch((*id).to_string())),
        ["tasks", id, "results"] => Some(RouteTarget::TaskResults((*id).to_string())),
        ["tasks", id, "verify"] => Some(RouteTarget::TaskVerify((*id).to_string())),
        ["tasks", id, "settle"] => Some(RouteTarget::TaskSettle((*id).to_string())),
        ["disputes"] => Some(RouteTarget::DisputesCollection),
        ["disputes", id, "arbitrate"] => Some(RouteTarget::DisputeArbitrate((*id).to_string())),
        ["accounts", account, "deposit"] => Some(RouteTarget::AccountDeposit((*account).to_string())),
        ["accounts", account, "balance"] => Some(RouteTarget::AccountBalance((*account).to_string())),
        ["conservation"] => Some(RouteTarget::Conservation),
        ["leaderboard"] => Some(RouteTarget::Leaderboard),
        ["stats"] => Some(RouteTarget::Stats),
        _ => None,
    }
}
