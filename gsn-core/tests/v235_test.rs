//! v2.3.6 MCP / CLI / API 三层集成测试
//!
//! 不依赖真实网络端口：直接调用与传输解耦的
//! - REST: `api::rest::route`
//! - MCP 桥接: `mcp::market_tools::MarketMcpBridge`
//! - MCP HTTP: `mcp::sse::handle_post`
//! 验证完整市场闭环真实执行（非占位）。

use gsn_core::api::market_actor::MarketActorHandle;
use gsn_core::api::rest::{route, NodeInfo};
use gsn_core::mcp::market_tools::MarketMcpBridge;
use gsn_core::mcp::sse;
use serde_json::{json, Value};

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn info() -> NodeInfo {
    NodeInfo {
        version: "0.2.36".to_string(),
        mode: "full".to_string(),
        p2p_port: 4001,
        connected_peers: 0,
        uptime_ms: 0,
    }
}

/// 通过 REST route 发请求，返回 (status, body)
async fn rest(
    market: &MarketActorHandle,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, Value) {
    let r = route(method, path, body, market, &info()).await;
    (r.status, r.body)
}

/// 断言 2xx
fn assert_ok(status: u16) {
    assert!((200..300).contains(&status), "期望 2xx，实际 {status}");
}

// ───────────────────────── REST 完整市场闭环 ─────────────────────────

#[test]
fn test_rest_full_market_lifecycle() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();

        // 1. 调用方充值
        let (s, _) = rest(&market, "POST",
            "/api/v1/accounts/caller-1/deposit",
            r#"{"amount":1000.0}"#).await;
        assert_ok(s);

        // 2. 发布者注册智能体（stake=100 ≥ min_stake）
        let register = json!({
            "agent_id": "agent-translate",
            "name": "翻译智能体",
            "description": "中英互译",
            "skills": ["translation", "english"],
            "stake": 100.0,
            "price": 10.0,
            "currency": "credit"
        });
        let (s, body) = rest(&market, "POST", "/api/v1/agents",
            &register.to_string()).await;
        assert_eq!(s, 201, "注册应 201，body={body}");
        assert_eq!(body["status"], "registered");

        // 3. 查询智能体
        let (s, body) = rest(&market, "GET",
            "/api/v1/agents/agent-translate", "").await;
        assert_ok(s);
        assert_eq!(body["name"], "翻译智能体");
        assert_eq!(body["stake"], 100.0);

        // 4. 按技能发现
        let (s, body) = rest(&market, "GET",
            "/api/v1/agents?skill=translation", "").await;
        assert_ok(s);
        // discover 返回数组或含 agents/results 字段
        let has_results = body.is_array()
            || body.get("agents").is_some()
            || body.get("results").is_some();
        assert!(has_results, "发现结果应含智能体列表: {body}");

        // 5. 发布任务
        let task = json!({
            "task_id": "task-1",
            "goal": "把这段中文翻译成英文",
            "context": "你好世界",
            "done": [],
            "todo": ["translate"],
            "trace": [],
            "owner": "caller-1",
            "budget": 50.0,
            "deadline": 2000000000000u64,
            "required_skills": ["translation"],
            "verification_policy": {"BftLite": {"n": 4, "f": 1}},
            "requester": "caller-1",
            "state": "Open",
            "created_at": 0
        });
        let (s, body) = rest(&market, "POST", "/api/v1/tasks",
            &task.to_string()).await;
        assert_eq!(s, 201, "发布任务应 201，body={body}");

        // 6. 查询任务
        let (s, _) = rest(&market, "GET", "/api/v1/tasks/task-1", "").await;
        assert_ok(s);

        // 7. 投标
        let bid = json!({
            "agent_id": "agent-translate",
            "task_id": "task-1",
            "proposed_price": 10.0,
            "estimated_latency_ms": 500,
            "score": 0.9
        });
        let (s, body) = rest(&market, "POST",
            "/api/v1/tasks/task-1/bids", &bid.to_string()).await;
        assert_eq!(s, 201, "投标应 201，body={body}");

        // 8. 匹配
        let (s, body) = rest(&market, "POST",
            "/api/v1/tasks/task-1/match", "").await;
        assert_ok(s);
        assert_eq!(body["status"], "matched");

        // 9. 提交结果
        let envelope = json!({
            "task_id": "task-1",
            "agent_id": "agent-translate",
            "report": "{\"translation\":\"Hello world\"}",
            "confidence": 0.95,
            "error_type": "None",
            "trace_ref": "trace-1",
            "evidence_grade": "CpuProto",
            "latency_ms": 420
        });
        let (s, body) = rest(&market, "POST",
            "/api/v1/tasks/task-1/results", &envelope.to_string()).await;
        assert_eq!(s, 201, "提交结果应 201，body={body}");

        // 10. QA 验证（3/4 通过，f=1，quorum=3 → Stop）
        let (s, body) = rest(&market, "POST",
            "/api/v1/tasks/task-1/verify?approvals=3&committee_size=4", "").await;
        assert_ok(s);
        assert_eq!(body["accepted"], true, "验证应通过: {body}");

        // 11. 结算
        let (s, body) = rest(&market, "POST",
            "/api/v1/tasks/task-1/settle", "").await;
        assert_ok(s);
        assert_eq!(body["status"], "settled");

        // 12. 守恒检查
        let (s, body) = rest(&market, "GET",
            "/api/v1/conservation", "").await;
        assert_ok(s);
        assert_eq!(body["conserved"], true, "结算应守恒: {body}");
    });
}

// ───────────────────────── REST 错误与边界 ─────────────────────────

#[test]
fn test_rest_health_and_404() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();

        let (s, body) = rest(&market, "GET", "/health", "").await;
        assert_ok(s);
        assert_eq!(body["status"], "ok");

        let (s, _) = rest(&market, "GET", "/api/v1/agents/nope", "").await;
        assert_eq!(s, 404, "不存在的 agent 应 404");

        let (s, _) = rest(&market, "GET", "/totally/unknown", "").await;
        assert_eq!(s, 404);

        // 非法 JSON body
        let (s, _) = rest(&market, "POST", "/api/v1/tasks", "{not json").await;
        assert_eq!(s, 400);
    });
}

#[test]
fn test_rest_register_validation() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();

        // 质押不足应 422
        let bad = json!({"agent_id":"a1","name":"X","stake": 1.0});
        let (s, _) = rest(&market, "POST", "/api/v1/agents", &bad.to_string()).await;
        assert_eq!(s, 422, "质押不足应 422");

        // 缺 name 应 422/400
        let noname = json!({"agent_id":"a2","stake":100.0});
        let (s, _) = rest(&market, "POST", "/api/v1/agents", &noname.to_string()).await;
        assert!((400..=422).contains(&s));
    });
}

// ───────────────────────── MCP 工具桥接 ─────────────────────────

#[test]
fn test_mcp_tool_definitions() {
    let tools = MarketMcpBridge::tool_definitions();
    assert!(tools.len() >= 15, "应注册至少 15 个市场工具，实际 {}", tools.len());

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    for required in [
        "market_register_agent",
        "market_publish_task",
        "market_match_task",
        "market_settle_task",
        "market_conservation",
        "market_stats",
    ] {
        assert!(names.contains(&required), "缺少工具 {required}");
    }

    // 每个工具应有非空 input schema 描述
    for t in &tools {
        assert_eq!(t.input_schema.schema_type, "object");
    }
}

#[test]
fn test_mcp_tools_call_real_execution() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();
        let bridge = MarketMcpBridge::new(market.clone());

        // 调用 stats（无参数）—— 应真实返回而非占位
        let result = bridge.call("market_stats", &json!({})).await;
        assert!(!result.is_error, "stats 不应报错");
        let text = match &result.content[0] {
            gsn_core::mcp::tool::ToolContent::Text { text } => text.clone(),
            _ => panic!("期望 text 内容"),
        };
        assert!(text.contains("agent_count") || text.contains("task_count"),
            "stats 应返回真实统计字段: {text}");
        // 确保不再是旧的占位 "tool executed"
        assert!(!text.contains("tool executed"), "不应再是占位响应");

        // 充值 + 余额查询，验证带参工具真实执行
        let r = bridge.call("market_deposit",
            &json!({"account":"c1","amount":500.0})).await;
        assert!(!r.is_error);

        let r = bridge.call("market_balance",
            &json!({"account":"c1"})).await;
        assert!(!r.is_error);
        let text = match &r.content[0] {
            gsn_core::mcp::tool::ToolContent::Text { text } => text.clone(),
            _ => panic!(),
        };
        assert!(text.contains("500"), "余额应反映充值: {text}");

        // 未知工具应返回错误
        let r = bridge.call("nonexistent_tool", &json!({})).await;
        assert!(r.is_error);
    });
}

#[test]
fn test_mcp_full_flow_via_bridge() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();
        let bridge = MarketMcpBridge::new(market.clone());

        // 注册
        let r = bridge.call("market_register_agent", &json!({
            "agent_id":"a-mcp","name":"MCP智能体","skills":["math"],"stake":100.0
        })).await;
        assert!(!r.is_error);

        // 发现
        let r = bridge.call("market_discover_agents",
            &json!({"skill":"math"})).await;
        assert!(!r.is_error);

        // 守恒
        let r = bridge.call("market_conservation", &json!({})).await;
        assert!(!r.is_error);
    });
}

// ───────────────────────── MCP over HTTP ─────────────────────────

#[test]
fn test_mcp_http_initialize_and_tools_list() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();

        // initialize
        let init = json!({
            "jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2024-11-05"}
        });
        let r = sse::handle_post(&init.to_string(), &market).await;
        assert_eq!(r.status, 200);
        assert!(r.body.contains("serverInfo"));

        // tools/list
        let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
        let r = sse::handle_post(&list.to_string(), &market).await;
        assert_eq!(r.status, 200);
        assert!(r.body.contains("market_register_agent"));
    });
}

#[test]
fn test_mcp_http_tools_call() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();

        let call = json!({
            "jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{
                "name":"market_deposit",
                "arguments":{"account":"http-1","amount":250.0}
            }
        });
        let r = sse::handle_post(&call.to_string(), &market).await;
        assert_eq!(r.status, 200);
        // JSON-RPC result 内应含工具文本，且非占位
        assert!(r.body.contains("deposit") || r.body.contains("ok"));
        assert!(!r.body.contains("tool executed"));

        // GET 返回 SSE
        let g = sse::handle_get();
        assert_eq!(g.content_type, "text/event-stream");
        assert!(g.body.contains("ready"));
    });
}

#[test]
fn test_mcp_http_notification_returns_202() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();
        // notifications/initialized 无 id
        let notif = json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}});
        let r = sse::handle_post(&notif.to_string(), &market).await;
        assert_eq!(r.status, 202);
    });
}

#[test]
fn test_data_dir_tilde_expansion() {
    use gsn_core::node::{default_data_dir, expand_tilde};
    use std::path::PathBuf;

    // 默认数据目录不得是字面 "~"，否则会在 CWD 下创建 ~ 目录。
    let d = default_data_dir();
    assert!(!d.starts_with("~"), "default_data_dir 不应以字面 ~ 开头: {:?}", d);

    // ~/.gsn/data 必须展开到真实 home。
    let expanded = expand_tilde(PathBuf::from("~/.gsn/data"));
    assert!(!expanded.starts_with("~"), "~ 未被展开: {:?}", expanded);
    if let Ok(home) = std::env::var("HOME") {
        assert!(expanded.starts_with(&home), "应位于 HOME 下: {:?} (HOME={})", expanded, home);
        assert!(expanded.ends_with(".gsn/data"));
    }

    // 普通相对路径保持不变。
    let rel = expand_tilde(PathBuf::from("local/data"));
    assert_eq!(rel, PathBuf::from("local/data"));

    // 绝对路径（不以 ~ 开头）保持不变。
    let abs = expand_tilde(PathBuf::from("/var/lib/gsn"));
    assert_eq!(abs, PathBuf::from("/var/lib/gsn"));
}
