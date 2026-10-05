//! REST API 路由
//!
//! v2.3.5: 完整 marketplace HTTP 端点。
//! 与传输解耦：`route` 接收解析后的 method/path/query/body，返回 (HTTP 状态码, JSON)，
//! gsn-daemon 的 TCP HTTP 服务器与测试都可直接调用。

use crate::api::market_actor::MarketActorHandle;
use crate::plugin::PluginOrchestratorHandle;
use serde_json::{json, Value};

/// 路由结果
pub struct Routed {
    pub status: u16,
    pub status_text: &'static str,
    pub body: Value,
}

impl Routed {
    fn ok(body: Value) -> Self {
        Routed {
            status: 200,
            status_text: "OK",
            body,
        }
    }
    fn bad_request(msg: &str) -> Self {
        Routed {
            status: 400,
            status_text: "Bad Request",
            body: json!({"error": msg}),
        }
    }
    fn not_found(path: &str) -> Self {
        Routed {
            status: 404,
            status_text: "Not Found",
            body: json!({"error": "not_found", "path": path}),
        }
    }
}

/// 把 MarketResponse 转成 Routed
fn from_mr(r: crate::api::market_actor::MarketResponse, success_status: u16) -> Routed {
    use crate::api::market_actor::MarketResponse::*;
    match r {
        Ok(v) => {
            let text = if success_status == 201 {
                "Created"
            } else {
                "OK"
            };
            Routed {
                status: success_status,
                status_text: text,
                body: v,
            }
        }
        Err(e) => classify_error(&e),
    }
}

/// v2.6.6：错误分类由机器可读前缀决定，不再匹配中文字符串子串。
/// 业务层错误以 `NOT_FOUND:` / `CONFLICT:` / `BAD_REQUEST:` 前缀开头；
/// 未知前缀统一 422（语义错误）。错误体原样回传，前缀即机器可读错误码。
fn classify_error(e: &str) -> Routed {
    if let Some(rest) = e.strip_prefix("NOT_FOUND:") {
        Routed {
            status: 404,
            status_text: "Not Found",
            body: json!({"error": "not_found", "message": rest}),
        }
    } else if e.starts_with("CONFLICT:") {
        Routed {
            status: 409,
            status_text: "Conflict",
            body: json!({"error": e}),
        }
    } else if e.starts_with("BAD_REQUEST:") {
        Routed {
            status: 400,
            status_text: "Bad Request",
            body: json!({"error": e}),
        }
    } else {
        Routed {
            status: 422,
            status_text: "Unprocessable Entity",
            body: json!({"error": e}),
        }
    }
}

/// v2.6.6：变更状态的端点只接受 POST；GET 等只读方法返回 405，不再误触发写操作。
fn method_not_allowed(allow: &'static str) -> Routed {
    Routed {
        status: 405,
        status_text: "Method Not Allowed",
        body: json!({"error": "method_not_allowed", "allow": allow}),
    }
}

/// 解析 query string 为简单 map（不处理重复键/URL 编码的完整场景，够用）
fn parse_query(q: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded = |s: &str| url_decode(s);
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
            // v2.6.6: off-by-one 修正——末尾的 %41 也要能解码（i+3<=len，即 i+2<=len-1 的等价写法）
            b'%' if i + 3 <= bytes.len() => {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(b) = u8::from_str_radix(h, 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
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
    orchestrator: Option<&PluginOrchestratorHandle>,
    info: &NodeInfo,
) -> Routed {
    let (path, query) = full_path.split_once('?').unwrap_or((full_path, ""));
    let q = parse_query(query);
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    // ───── CORS 预检 ─────
    if method == "OPTIONS" {
        return Routed {
            status: 204,
            status_text: "No Content",
            body: Value::Null,
        };
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
                    // B1：编排器可用时先经 agent-card 插件校验卡片再注册；否则走单体 market。
                    let resp = match orchestrator {
                        Some(o) => o.register_agent_gated(v).await,
                        None => market.register_agent(v).await,
                    };
                    return from_mr(resp, 201);
                }
                return Routed::bad_request("缺少注册卡片");
            }
            // GET /api/v1/agents?all=1 列出全部（工作台）
            if q.contains_key("all") {
                return from_mr(market.list_all_agents().await, 200);
            }
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
            // GET: ?all=1 列出全部（工作台），否则返回统计
            if q.contains_key("all") {
                return from_mr(market.list_all_tasks().await, 200);
            }
            return from_mr(market.list_tasks().await, 200);
        }
        Some(RouteTarget::TaskItem(id)) => {
            return from_mr(market.get_task(id).await, 200);
        }
        Some(RouteTarget::TaskBids(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            if let Some(mut v) = parsed_body {
                // 注入 task_id
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("task_id".to_string(), json!(id));
                }
                return from_mr(market.submit_bid(v).await, 201);
            }
            return Routed::bad_request("缺少投标数据");
        }
        Some(RouteTarget::TaskMatch(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            // B1：编排器可用时投标交 market-match 插件决策再应用 winner；否则走单体 market。
            let resp = match orchestrator {
                Some(o) => o.match_task_gated(id).await,
                None => market.match_task(id).await,
            };
            return from_mr(resp, 200);
        }
        Some(RouteTarget::TaskResults(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            if let Some(mut v) = parsed_body {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("task_id".to_string(), json!(id));
                }
                return from_mr(market.submit_result(v).await, 201);
            }
            return Routed::bad_request("缺少结果数据");
        }
        Some(RouteTarget::TaskVerify(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            // v2.5.9 认证式：固定委员集 (did + 公钥) + 委员私钥签名票，
            // 不再接受 approvals / committee_size 合成投票。
            let body = parsed_body.as_ref();
            // v3.5.3（AU-35）：round 必须是 u32 范围内整数；1.5/-1/5e9 一律 400，
            // 旧实现 `as_u64().map(|x| x as u32)` 对非整数静默退 0、对大值静默截断。
            let round: u32 = match body.and_then(|v| v.get("round")) {
                None | Some(serde_json::Value::Null) => 0,
                Some(v) => match v.as_i64().and_then(|n| u32::try_from(n).ok()) {
                    Some(r) => r,
                    None => {
                        return Routed::bad_request(&format!(
                            "INVALID_PARAM: round 必须为 u32 范围内整数，实际: {v}"
                        ));
                    }
                },
            };
            let members = match parse_committee_members(body) {
                Ok(m) => m,
                Err(e) => return Routed::bad_request(&e),
            };
            let votes = match parse_signed_votes(body) {
                Ok(v) => v,
                Err(e) => return Routed::bad_request(&e),
            };
            return from_mr(
                market
                    .verify_result(id, round, members, votes, unix_now())
                    .await,
                200,
            );
        }
        Some(RouteTarget::TaskSettle(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            // B1：编排器可用时先经 market-settle 插件审计账本，passed 才结算；否则走单体 market。
            let resp = match orchestrator {
                Some(o) => o.settle_task_gated(id).await,
                None => market.settle_task(id).await,
            };
            return from_mr(resp, 200);
        }
        Some(RouteTarget::TaskResume(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            // 恢复边：返工任务回到执行中（v2.6.0）
            return from_mr(market.resume_rework(id.clone()).await, 200);
        }
        Some(RouteTarget::TaskReopen(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            // 恢复边：无共识任务重新开放（v2.6.0）
            return from_mr(market.reopen_task(id.clone()).await, 200);
        }
        Some(RouteTarget::TaskReject(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            // 终局拒绝（v2.8.5，GAP §2.2.6：此前 reject_task 仅库可达）
            return from_mr(market.reject_task(id.clone()).await, 200);
        }
        Some(RouteTarget::DisputesCollection) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            if let Some(v) = parsed_body {
                return from_mr(market.open_dispute(v).await, 201);
            }
            return Routed::bad_request("缺少争议数据");
        }
        Some(RouteTarget::DisputeArbitrate(id)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            // P0-4：仲裁是特权写，请求体必须是签名治理信封
            // （SignedGovernanceCommand，capability=governance:arbitrate，
            // target=dispute_id，claim.guilty 为裁决）。无信封/验签失败一律拒绝；
            // 仲裁者身份由信封 sender_did 决定，禁止自报。
            let Some(v) = parsed_body else {
                return Routed::bad_request("缺少签名治理信封（SignedGovernanceCommand）");
            };
            let cmd: crate::marketplace::SignedGovernanceCommand =
                match serde_json::from_value(v.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        return Routed::bad_request(&format!(
                            "治理信封解析失败（需 SignedGovernanceCommand JSON）: {e}"
                        ))
                    }
                };
            if cmd.target != id {
                return Routed::bad_request(&format!(
                    "治理信封 target={} 与路径 dispute_id={} 不一致",
                    cmd.target, id
                ));
            }
            return from_mr(market.arbitrate_signed(cmd, unix_now()).await, 200);
        }
        Some(RouteTarget::AccountDeposit(account)) => {
            if method != "POST" {
                return method_not_allowed("POST");
            }
            // P0-4：off-chain 授信是特权写，请求体必须是签名治理信封
            // （capability=governance:credit，target=account，claim.amount 非负整数）。
            // 生产口径必须由链上支付凭证支持（未验证/需外部审计）。
            let Some(v) = parsed_body else {
                return Routed::bad_request("缺少签名治理信封（SignedGovernanceCommand）");
            };
            let cmd: crate::marketplace::SignedGovernanceCommand =
                match serde_json::from_value(v.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        return Routed::bad_request(&format!(
                            "治理信封解析失败（需 SignedGovernanceCommand JSON）: {e}"
                        ))
                    }
                };
            if cmd.target != account {
                return Routed::bad_request(&format!(
                    "治理信封 target={} 与路径 account={} 不一致",
                    cmd.target, account
                ));
            }
            return from_mr(market.deposit_signed(cmd, unix_now()).await, 200);
        }
        Some(RouteTarget::AccountBalance(account)) => {
            return from_mr(market.balance(account).await, 200);
        }
        Some(RouteTarget::Conservation) => {
            return from_mr(market.conservation().await, 200);
        }
        Some(RouteTarget::Audit) => {
            return from_mr(market.audit().await, 200);
        }
        Some(RouteTarget::Leaderboard) => {
            let limit = q
                .get("limit")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(10);
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
    TaskResume(String),
    TaskReopen(String),
    TaskReject(String),
    DisputesCollection,
    DisputeArbitrate(String),
    AccountDeposit(String),
    AccountBalance(String),
    Conservation,
    Audit,
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
        ["tasks", id, "resume"] => Some(RouteTarget::TaskResume((*id).to_string())),
        ["tasks", id, "reopen"] => Some(RouteTarget::TaskReopen((*id).to_string())),
        ["tasks", id, "reject"] => Some(RouteTarget::TaskReject((*id).to_string())),
        ["disputes"] => Some(RouteTarget::DisputesCollection),
        ["disputes", id, "arbitrate"] => Some(RouteTarget::DisputeArbitrate((*id).to_string())),
        ["accounts", account, "deposit"] => {
            Some(RouteTarget::AccountDeposit((*account).to_string()))
        }
        ["accounts", account, "balance"] => {
            Some(RouteTarget::AccountBalance((*account).to_string()))
        }
        ["conservation"] => Some(RouteTarget::Conservation),
        ["audit"] => Some(RouteTarget::Audit),
        ["leaderboard"] => Some(RouteTarget::Leaderboard),
        ["stats"] => Some(RouteTarget::Stats),
        _ => None,
    }
}

/// 当前 unix 秒
pub(crate) fn unix_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 解析固定委员集：members = [{did, public_key(hex 32)}]
pub(crate) fn parse_committee_members(
    v: Option<&Value>,
) -> Result<Vec<(String, [u8; 32])>, String> {
    let arr = v
        .and_then(|b| b.get("members"))
        .and_then(|m| m.as_array())
        .ok_or("缺少 members：固定委员集 [{\"did\":..,\"public_key\":hex32}]")?;
    let mut out = Vec::new();
    for m in arr {
        let did = m
            .get("did")
            .and_then(|x| x.as_str())
            .ok_or("委员缺少 did")?
            .to_string();
        let pk_hex = m
            .get("public_key")
            .or_else(|| m.get("pubkey"))
            .and_then(|x| x.as_str())
            .ok_or("委员缺少 public_key（hex）")?;
        let pk_bytes = hex::decode(pk_hex).map_err(|e| format!("委员公钥 hex 错误: {e}"))?;
        let pk: [u8; 32] = pk_bytes.try_into().map_err(|_| "委员公钥必须为 32 字节")?;
        // v2.8.9：弱公钥拒绝 + DID↔公钥绑定（GAP §2.2/§2.4 委员授权，防止自造密钥签票）
        if crate::identity::is_weak_pubkey(&pk) {
            return Err(format!("委员 {did} 公钥为弱公钥，拒绝"));
        }
        let parsed =
            crate::identity::Did::parse(&did).map_err(|_| format!("委员 {did} 的 DID 非法"))?;
        if parsed.identifier() != crate::identity::Did::fingerprint(&pk) {
            return Err(format!("委员 {did} 的 DID 与公钥指纹不匹配"));
        }
        out.push((did, pk));
    }
    Ok(out)
}

/// 解析委员签名票：signed_votes = [SignedQaVote...]，vote 可传 "Stop"/"Continue"
pub(crate) fn parse_signed_votes(
    v: Option<&Value>,
) -> Result<Vec<crate::marketplace::SignedQaVote>, String> {
    let arr = match v
        .and_then(|b| b.get("signed_votes"))
        .and_then(|x| x.as_array())
    {
        Some(a) => a,
        None => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for item in arr {
        let mut obj = item.clone();
        if let Some(map) = obj.as_object_mut() {
            if let Some(vtag) = map.get("vote").and_then(|x| x.as_str()) {
                let enum_tag = match vtag.to_lowercase().as_str() {
                    "stop" => json!("Stop"),
                    "continue" => json!("Continue"),
                    "silent" => json!("Silent"),
                    other => return Err(format!("未知投票类型 {other}")),
                };
                map.insert("vote".to_string(), enum_tag);
            }
        }
        let sv = serde_json::from_value::<crate::marketplace::SignedQaVote>(obj)
            .map_err(|e| format!("签名票格式错误: {e}"))?;
        out.push(sv);
    }
    Ok(out)
}

#[cfg(test)]
mod v266_tests {
    use super::*;

    #[test]
    fn url_decode_decodes_trailing_percent_escape() {
        // §6.6：末尾 %41 必须解成 'A'（旧 off-by-one 会漏掉）
        assert_eq!(url_decode("task%41"), "taskA");
        assert_eq!(url_decode("x%41%42"), "xAB");
    }

    #[test]
    fn url_decode_decodes_multibyte_utf8() {
        // %E4%B8%AD = UTF-8 "中"
        assert_eq!(url_decode("%E4%B8%AD"), "中");
        assert_eq!(url_decode("a+b"), "a b");
    }

    #[test]
    fn classify_not_found_is_404_not_chinese_substring() {
        // §6.5：按机器码前缀分类，不再依赖中文"不存在"子串
        let r = classify_error("NOT_FOUND: task 不存在: t1");
        assert_eq!(r.status, 404);
        // 恰好含"不存在"三字但无前缀的校验错误不得被误判为 404
        let r2 = classify_error("字段 x 不存在合理，校验失败");
        assert_eq!(r2.status, 422);
        let r3 = classify_error("CONFLICT: 重复");
        assert_eq!(r3.status, 409);
    }

    #[test]
    fn method_not_allowed_shape() {
        let r = method_not_allowed("POST");
        assert_eq!(r.status, 405);
    }
}
