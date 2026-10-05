//! v2.8.6 测试（GAP §3.5 / §4.1）：
//! - MCP 参数校验接入**生产传输**（sse / stdio），不再只存在于零调用的 McpServer；
//! - 金额非法（字符串 / 浮点 / 缺失）在进入执行器前返回 -32602，
//!   不再经 `get_money` 静默存 0 并返回 deposited；
//! - ACA Receipt 资源计量整数化，规范签名载荷不再含 f64，跨语言字节一致。

use gsn_core::api::market_actor::MarketActorHandle;
use gsn_core::mcp::market_tools::MarketMcpBridge;
use gsn_core::mcp::sandbox_tools::SandboxMcpBridge;
use gsn_core::mcp::sse;
use gsn_core::mcp::sse::McpHttp;
use gsn_core::mcp::tool::{find_tool, validate_arguments, ToolDefinition};
use gsn_core::sandbox::config::SandboxConfig;
use gsn_core::sandbox::manager::SandboxManager;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// dev/faucet 治理成员（P0-4：授信须受权治理签名命令；生产须链上凭证，未验证）。
fn faucet() -> (String, gsn_core::Keypair) {
    let mut seed = [0u8; 32];
    seed[0] = 7;
    let kp = gsn_core::Keypair::from_seed(&seed);
    (gsn_core::Did::from_public_key(kp.public_key()).to_string(), kp)
}
fn spawn_market() -> MarketActorHandle {
    let (did, kp) = faucet();
    let pk: [u8; 32] = kp.public_key().try_into().unwrap();
    MarketActorHandle::spawn_with_governance_members(vec![(did, pk)])
}
fn deposit_cmd(account: &str, amount: i64) -> Value {
    let (did, kp) = faucet();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let cmd = gsn_core::marketplace::SignedGovernanceCommand::sign(
        "v286", &did, &kp, gsn_core::marketplace::GOV_CAP_CREDIT, account,
        serde_json::json!({"amount": amount}), &format!("v286-{account}-{amount}"), now - 10, 300,
    );
    serde_json::to_value(cmd).unwrap()
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn sandbox_mgr() -> Arc<Mutex<SandboxManager>> {
    let dir = std::env::temp_dir().join(format!("au-v286-mcp-sb-{}", std::process::id()));
    Arc::new(Mutex::new(SandboxManager::new(
        dir,
        SandboxConfig::default(),
        0,
    )))
}

fn all_tools() -> Vec<ToolDefinition> {
    let mut t = MarketMcpBridge::tool_definitions();
    t.extend(SandboxMcpBridge::tool_definitions());
    t
}

/// 发送一条带正确 Bearer 的 tools/call（生产 sse 路径）。
async fn call(
    market: &MarketActorHandle,
    sb: &Arc<Mutex<SandboxManager>>,
    params: Value,
) -> McpHttp {
    let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":params});
    sse::handle_post(
        &body.to_string(),
        market,
        sb,
        Some("Bearer tok"),
        Some("tok"),
    )
    .await
}

// ── 金额非法必须被 -32602 拒绝，而不是静默存 0 ──

#[test]
fn deposit_amount_string_rejected() {
    rt().block_on(async {
        let market = spawn_market();
        let sb = sandbox_mgr();
        let r = call(
            &market,
            &sb,
            json!({"name":"market_deposit","arguments":{"account":"a","amount":"lots"}}),
        )
        .await;
        assert_eq!(r.status, 400);
        assert!(r.body.contains("-32602"), "应返回 -32602: {}", r.body);
        // 关键：旧缺陷会存 0 并返回 deposited；现在不能进入成功分支。
        assert!(
            !r.body.contains("deposited"),
            "不得对非法金额静默存 0: {}",
            r.body
        );
    });
}

#[test]
fn deposit_amount_fractional_float_rejected() {
    rt().block_on(async {
        let market = spawn_market();
        let sb = sandbox_mgr();
        let r = call(
            &market,
            &sb,
            json!({"name":"market_deposit","arguments":{"account":"a","amount":10.5}}),
        )
        .await;
        assert_eq!(r.status, 400);
        assert!(r.body.contains("-32602"), "浮点 10.5 应被拒: {}", r.body);
    });
}

#[test]
fn deposit_amount_dot_zero_float_rejected() {
    rt().block_on(async {
        let market = spawn_market();
        let sb = sandbox_mgr();
        // serde_json 中 10.0 也是 f64，is_i64() 为 false，必须拒绝。
        let r = call(
            &market,
            &sb,
            json!({"name":"market_deposit","arguments":{"account":"a","amount":10.0}}),
        )
        .await;
        assert_eq!(r.status, 400);
        assert!(r.body.contains("-32602"), "10.0 浮点应被拒: {}", r.body);
    });
}

#[test]
fn deposit_missing_amount_rejected() {
    rt().block_on(async {
        let market = spawn_market();
        let sb = sandbox_mgr();
        let r = call(
            &market,
            &sb,
            json!({"name":"market_deposit","arguments":{"account":"a"}}),
        )
        .await;
        assert_eq!(r.status, 400);
        assert!(r.body.contains("-32602"));
        assert!(r.body.contains("amount"));
    });
}

#[test]
fn arbitrate_guilty_wrong_type_rejected() {
    rt().block_on(async {
        let market = spawn_market();
        let sb = sandbox_mgr();
        // guilty 必须是 boolean；传数字 1 应被 -32602 拒绝，不再 unwrap_or(false)。
        let r = call(
            &market,
            &sb,
            json!({"name":"market_arbitrate","arguments":{"dispute_id":"d","arbitrator":"arb","guilty":1}}),
        )
        .await;
        assert_eq!(r.status, 400);
        assert!(r.body.contains("-32602"), "guilty 类型错误应被拒: {}", r.body);
        assert!(r.body.contains("guilty"));
    });
}

#[test]
fn deposit_valid_integer_succeeds() {
    rt().block_on(async {
        let market = spawn_market();
        let sb = sandbox_mgr();
        let r = call(
            &market,
            &sb,
            json!({"name":"market_deposit","arguments":{"account":"a","amount":250,"governance": deposit_cmd("a",250)}}),
        )
        .await;
        assert_eq!(r.status, 200);
        assert!(r.body.contains("deposit") || r.body.contains("ok"));
    });
}

// ── find_tool / validate_arguments 统一入口 ──

#[test]
fn find_tool_and_validate_arguments() {
    let tools = all_tools();
    assert!(find_tool(&tools, "market_deposit").is_some());
    assert!(find_tool(&tools, "market_balance").is_some());
    assert!(find_tool(&tools, "definitely_not_a_tool").is_none());

    let td = find_tool(&tools, "market_deposit").unwrap();
    // 字符串 / 浮点金额校验失败
    assert!(validate_arguments(&td.input_schema, &json!({"account":"a","amount":"lots"})).is_err());
    assert!(validate_arguments(&td.input_schema, &json!({"account":"a","amount":10.5})).is_err());
    // 缺失必填
    assert!(validate_arguments(&td.input_schema, &json!({"account":"a"})).is_err());
    // 整数金额通过
    assert!(validate_arguments(&td.input_schema, &json!({"account":"a","amount":10})).is_ok());
}

// ── ACA Receipt 计量整数化，规范载荷无浮点 ──

#[test]
fn receipt_metering_integers_no_float_in_payload() {
    use gsn_core::aca::receipt::{Receipt, ResourceMetering};
    let metering = ResourceMetering {
        bandwidth_mb: 100,
        energy_joules: 500,
        ..ResourceMetering::default()
    };
    let receipt = Receipt::new("t".to_string(), "did".to_string(), b"data").with_metering(metering);
    let payload = receipt.signing_payload().unwrap();
    let text = String::from_utf8(payload).unwrap();
    assert!(
        text.contains("\"bandwidth_mb\":100"),
        "带宽应为整数: {text}"
    );
    assert!(
        text.contains("\"energy_joules\":500"),
        "能量应为整数: {text}"
    );
    // 不应出现浮点写法
    assert!(!text.contains("100.0"));
    assert!(!text.contains("500.0"));
}
