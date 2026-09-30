//! v2.7.3 实机部署修复回归测试
//!
//! 两个缺陷均来自 Mac/Windows 真机部署：
//! 1. 发布任务 JSON 缺 `state` 字段时，服务端返回 422（反序列化缺字段），
//!    而 `market_actor` 反序列化成功后又强制把 state 置为 Open——不传必 422、
//!    传了被覆盖。修复：`TaskSpec.state` 加 `#[serde(default)]`，缺省即 Open。
//! 2. Unverified 结果经认证式 BFT 委员会签名投票判定 Stop、state=Accepted 后，
//!    `evidence_grade` 仍停在 Unverified，`settle_task` 的可信闸门返回 422，
//!    且系统无升级 API，路径不可达。修复：认证验收 Stop 后把证据提升为 Verified。
//!
//! 不依赖真实网络端口：直接走 `api::rest::route`。

use gsn_core::api::market_actor::MarketActorHandle;
use gsn_core::api::rest::{route, NodeInfo};
use serde_json::{json, Value};

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn info() -> NodeInfo {
    NodeInfo {
        version: "0.2.73".to_string(),
        mode: "full".to_string(),
        p2p_port: 4001,
        connected_peers: 0,
        uptime_ms: 0,
    }
}

async fn rest(market: &MarketActorHandle, method: &str, path: &str, body: &str) -> (u16, Value) {
    let r = route(method, path, body, market, &info()).await;
    (r.status, r.body)
}

fn assert_ok(status: u16) {
    assert!((200..300).contains(&status), "期望 2xx，实际 {status}");
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// 修复①：发布任务缺 `state` 字段应默认 Open，返回 201（而非 422）。
#[test]
fn test_publish_task_without_state_defaults_open() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();

        let (s, _) = rest(
            &market,
            "POST",
            "/api/v1/accounts/caller-1/deposit",
            r#"{"amount":1000}"#,
        )
        .await;
        assert_ok(s);

        let (s, _) = rest(
            &market,
            "POST",
            "/api/v1/accounts/agent-translate/deposit",
            r#"{"amount":100}"#,
        )
        .await;
        assert_ok(s);

        let register = json!({
            "agent_id": "agent-translate",
            "name": "翻译智能体",
            "description": "中英互译",
            "skills": ["translation"],
            "stake": 100,
            "price": 10,
            "currency": "credit"
        });
        let (s, _) = rest(&market, "POST", "/api/v1/agents", &register.to_string()).await;
        assert_eq!(s, 201);

        // 注意：JSON 中【故意不含 state】字段
        let task = json!({
            "task_id": "task-1",
            "goal": "把这段中文翻译成英文",
            "context": "你好世界",
            "done": [],
            "todo": ["translate"],
            "trace": [],
            "owner": "caller-1",
            "budget": 50,
            "deadline": 2000000000000u64,
            "required_skills": ["translation"],
            "verification_policy": {"BftLite": {"n": 4, "f": 1}},
            "requester": "caller-1",
            "created_at": 0
        });
        let (s, body) = rest(&market, "POST", "/api/v1/tasks", &task.to_string()).await;
        assert_eq!(s, 201, "缺 state 应默认 Open 并 201，body={body}");

        // 回读确认状态为 open
        let (s, body) = rest(&market, "GET", "/api/v1/tasks/task-1", "").await;
        assert_ok(s);
        assert_eq!(body["state"], "Open", "缺省 state 应为 Open: {body}");
    });
}

/// 修复②：Unverified 结果经认证 BFT 验收 Stop 后，settle 应成功。
#[test]
fn test_unverified_result_authenticated_verify_then_settle() {
    rt().block_on(async {
        let market = MarketActorHandle::spawn();

        let (s, _) = rest(
            &market,
            "POST",
            "/api/v1/accounts/caller-1/deposit",
            r#"{"amount":1000}"#,
        )
        .await;
        assert_ok(s);
        let (s, _) = rest(
            &market,
            "POST",
            "/api/v1/accounts/agent-translate/deposit",
            r#"{"amount":100}"#,
        )
        .await;
        assert_ok(s);

        let register = json!({
            "agent_id": "agent-translate",
            "name": "翻译智能体",
            "description": "中英互译",
            "skills": ["translation"],
            "stake": 100,
            "price": 10,
            "currency": "credit"
        });
        let (s, _) = rest(&market, "POST", "/api/v1/agents", &register.to_string()).await;
        assert_eq!(s, 201);

        let task = json!({
            "task_id": "task-1",
            "goal": "把这段中文翻译成英文",
            "context": "你好世界",
            "done": [],
            "todo": ["translate"],
            "trace": [],
            "owner": "caller-1",
            "budget": 50,
            "deadline": 2000000000000u64,
            "required_skills": ["translation"],
            "verification_policy": {"BftLite": {"n": 4, "f": 1}},
            "requester": "caller-1",
            "created_at": 0
        });
        let (s, body) = rest(&market, "POST", "/api/v1/tasks", &task.to_string()).await;
        assert_eq!(s, 201, "body={body}");

        let bid = json!({
            "agent_id": "agent-translate",
            "task_id": "task-1",
            "proposed_price": 10,
            "estimated_latency_ms": 500,
            "score": 0.9
        });
        let (s, _) = rest(
            &market,
            "POST",
            "/api/v1/tasks/task-1/bids",
            &bid.to_string(),
        )
        .await;
        assert_eq!(s, 201);

        let (s, body) = rest(&market, "POST", "/api/v1/tasks/task-1/match", "").await;
        assert_ok(s);
        assert_eq!(body["status"], "matched");

        // 提交结果：证据等级为【Unverified】（不可信，单独无法过结算闸门）
        let envelope = json!({
            "task_id": "task-1",
            "agent_id": "agent-translate",
            "report": "{\"translation\":\"Hello world\"}",
            "confidence": 0.95,
            "error_type": "None",
            "trace_ref": "trace-1",
            "evidence_grade": "Unverified",
            "latency_ms": 420
        });
        let (s, body) = rest(
            &market,
            "POST",
            "/api/v1/tasks/task-1/results",
            &envelope.to_string(),
        )
        .await;
        assert_eq!(s, 201, "提交结果应 201，body={body}");

        // 认证式 QA：4 委员固定集，前 3 委员签 Stop，quorum=3 → Stop
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut members_json: Vec<Value> = Vec::new();
        let mut votes_json: Vec<Value> = Vec::new();
        for i in 1u8..=4 {
            let mut seed = [0u8; 32];
            seed[0] = i + 20;
            let kp = gsn_core::Keypair::from_seed(&seed);
            let pk: [u8; 32] = kp.public_key().try_into().unwrap();
            // v2.8.9：DID 必须由公钥派生（GAP §2.4），不再使用伪造的 qa-i
            let did = format!("did:nau:{}", gsn_core::Did::fingerprint(&pk));
            members_json.push(json!({"did": did, "public_key": to_hex(&pk)}));
            if i <= 3 {
                let sv = gsn_core::marketplace::SignedQaVote::sign(
                    "task-1",
                    0,
                    &did,
                    gsn_core::marketplace::QaVote::Stop,
                    &format!("nonce-{}", i),
                    now - 60,
                    now + 3600,
                    &kp,
                );
                votes_json.push(serde_json::to_value(&sv).unwrap());
            }
        }
        let verify_body = json!({
            "round": 0,
            "members": members_json,
            "signed_votes": votes_json
        });
        let (s, body) = rest(
            &market,
            "POST",
            "/api/v1/tasks/task-1/verify",
            &verify_body.to_string(),
        )
        .await;
        assert_ok(s);
        assert_eq!(body["accepted"], true, "认证验收应通过: {body}");

        // 关键：Unverified 结果在认证验收通过后，settle 必须成功
        let (s, body) = rest(&market, "POST", "/api/v1/tasks/task-1/settle", "").await;
        assert_ok(s);
        assert_eq!(body["status"], "settled", "证据提升后应可结算: {body}");
        assert_eq!(body["amount"], 10, "应按中标价 10 结算: {body}");

        // 守恒
        let (s, body) = rest(&market, "GET", "/api/v1/conservation", "").await;
        assert_ok(s);
        assert_eq!(body["conserved"], true, "结算应守恒: {body}");
    });
}
