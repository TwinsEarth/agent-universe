//! v3.5.1 安全补丁回归测试
//!
//! 对应审计条目：
//! - AU-01 / AU-05：认证式 QA 验收的服务端质押锚定闸门（独立、已质押、非执行者 + BFT 下限）
//! - AU-03：账本哈希链恢复 fail-closed（断链默认拒服，GSN_ALLOW_TAMPERED_LEDGER=1 逃生）
//! - AU-18：submit_result 入库即降为 Unverified
//!
//! 每个用例都断言「在旧实现上会失败」：
//! - AU-01 闸门是 v3.5.1 新增，旧实现无闸门 → 自造小委员会/未质押/执行者自批都会放行；
//! - AU-18 旧实现原样落库客户端自报等级；
//! - AU-03 旧实现断链仍容错恢复并进入可写/结算数据面。

use gsn_core::api::market_actor::{MarketActorHandle, MarketResponse};
use gsn_core::marketplace::*;

#[path = "common/mod.rs"]
mod helpers;

use gsn_core::Keypair;
use std::sync::Arc;

// ===== 辅助构造 =====

fn make_agent_card(id: &str, stake: i64) -> MarketAgentCard {
    MarketAgentCard {
        agent_id: id.to_string(),
        version: "1.0.0".to_string(),
        name: format!("Agent {id}"),
        description: "测试智能体".to_string(),
        skills: vec!["translation".to_string()],
        modalities: vec![],
        models: vec![],
        endpoint: String::new(),
        pricing: Pricing {
            model: PricingModel::PerCall,
            price: Money::new(1),
            currency: Currency::Credit,
        },
        sla: Sla::default(),
        owner: id.to_string(),
        stake: Money::new(stake),
        reputation_score: 0.5,
        total_calls: 0,
        success_rate: 0.0,
        evidence_grade: EvidenceGrade::Unverified,
        verified: false,
        created_at: 1000,
        updated_at: 1000,
    }
}

fn make_task(id: &str, budget: i64, requester: &str) -> TaskSpec {
    TaskSpec {
        task_id: id.to_string(),
        goal: "翻译一段文字".to_string(),
        context: "中译英".to_string(),
        done: vec![],
        todo: vec!["translate".to_string()],
        trace: vec![],
        owner: None,
        budget: Money::new(budget),
        winner_price: None,
        deadline: 9999999999,
        required_skills: vec!["translation".to_string()],
        verification_policy: VerificationPolicy::BftLite { n: 4, f: 1 },
        requester: requester.to_string(),
        state: TaskState::Draft,
        created_at: 1000,
    }
}

fn make_bid(agent: &str, task: &str, price: i64) -> Bid {
    Bid {
        agent_id: agent.to_string(),
        task_id: task.to_string(),
        proposed_price: Money::new(price),
        estimated_latency_ms: 500,
        score: 0.0,
    }
}

fn deposit_and_register(market: &mut AgentMarket, id: &str, stake: i64) {
    helpers::deposit(market, id, Money::new(stake));
    market.register_agent(make_agent_card(id, stake)).unwrap();
}

/// 把任务推进到「已提交结果、等待认证 QA」状态（Verifying）。
///
/// 执行者 `exec` 中标并提交一个结果信封；返回 (task_id, executor_did)。
fn setup_task_awaiting_qa(market: &mut AgentMarket) -> (String, String) {
    let exec = "did:nau:executor";
    deposit_and_register(market, exec, 100);
    helpers::deposit(market, "did:nau:requester", Money::new(50));
    market
        .publish_task(make_task("task-1", 50, "did:nau:requester"))
        .unwrap();
    market.submit_bid(make_bid(exec, "task-1", 10)).unwrap();
    market.match_task("task-1").unwrap();
    market
        .submit_result(ResultEnvelope {
            task_id: "task-1".to_string(),
            agent_id: exec.to_string(),
            report: r#"{"ok":true}"#.to_string(),
            confidence: 0.95,
            error_type: ErrorType::None,
            trace_ref: "trace://t1/1".to_string(),
            evidence_grade: EvidenceGrade::Verified, // 自报 Verified（AU-18 会被降为 Unverified）
            latency_ms: 100,
            pocv: None,
        })
        .unwrap();
    ("task-1".to_string(), exec.to_string())
}

/// 注册 n 个相互独立、各自足额（100）锁定质押的委员，返回 (did, keypair, pubkey)。
fn staked_committee(market: &mut AgentMarket, n: u8) -> Vec<(String, Keypair, [u8; 32])> {
    let mut out = Vec::new();
    for i in 1..=n {
        let mut seed = [0u8; 32];
        seed[0] = i;
        let kp = Keypair::from_seed(&seed);
        let pk: [u8; 32] = kp.public_key().try_into().unwrap();
        let did = format!("did:nau:qa{i}");
        deposit_and_register(market, &did, 100);
        out.push((did, kp, pk));
    }
    out
}

fn as_member_set(committee: &[(String, Keypair, [u8; 32])]) -> Vec<(String, [u8; 32])> {
    committee
        .iter()
        .map(|(d, _, pk)| (d.clone(), *pk))
        .collect()
}

const NOW: u64 = 1_000_000;

// ===== AU-01 / AU-05：服务端质押闸门 =====

/// ① 单把自造密钥 n=1 自签 Stop → 必须被 COMMITTEE_TOO_SMALL 拒绝，
///    不得升 Verified、不得放款。旧实现无人数下限，n=1 即 f=0 单人自批通过。
#[test]
fn au01_n1_self_signed_stop_rejected() {
    let mut market = AgentMarket::new();
    let (tid, _exec) = setup_task_awaiting_qa(&mut market);

    // 攻击者带一把自造密钥，把自己列为唯一委员并自签 Stop。
    let mut seed = [0u8; 32];
    seed[0] = 77;
    let kp = Keypair::from_seed(&seed);
    let pk: [u8; 32] = kp.public_key().try_into().unwrap();
    let bad = "did:nau:bad-actor";
    let members = vec![(bad.to_string(), pk)];
    let vote = SignedQaVote::sign(&tid, 0, bad, QaVote::Stop, "n1", NOW - 10, NOW + 60, &kp);

    let err = market
        .verify_result_authenticated(&tid, 0, members, vec![vote], NOW)
        .unwrap_err();
    assert!(
        err.contains("COMMITTEE_TOO_SMALL"),
        "应因人数不足被拒，实际: {err}"
    );

    // 证据未被提升、任务仍在 Verifying、不得结算付款。
    assert!(!market
        .get_result(&tid)
        .unwrap()
        .evidence_grade
        .is_trustworthy());
    assert_eq!(market.get_task(&tid).unwrap().state, TaskState::Verifying);
    assert!(market.settle_task(&tid).is_err(), "被拒后不得结算放款");
}

/// ② 委员会里混入任务执行者（中标 owner）→ COMMITTEE_EXECUTOR_CONFLICT。
///    旧实现不做执行者回避，执行者可给自己打分。
#[test]
fn au02_committee_including_executor_rejected() {
    let mut market = AgentMarket::new();
    let (tid, exec) = setup_task_awaiting_qa(&mut market);

    // 3 个足额质押的独立委员 + 把执行者本人塞进第 4 席。
    let qa = staked_committee(&mut market, 3);
    let mut members = as_member_set(&qa);
    // 执行者已注册且足额质押，但利益冲突仍须拒绝。
    members.push((exec.clone(), [0u8; 32]));

    let err = market
        .verify_result_authenticated(&tid, 0, members, vec![], NOW)
        .unwrap_err();
    assert!(
        err.contains("COMMITTEE_EXECUTOR_CONFLICT"),
        "应因执行者回避被拒，实际: {err}"
    );
    assert!(market.settle_task(&tid).is_err());
}

/// ③ 委员未在服务端持有有效锁定质押 → COMMITTEE_NOT_STAKED。
///    旧实现不查服务端质押，自带未质押的自造密钥即可当评委。
#[test]
fn au03_committee_member_not_staked_rejected() {
    let mut market = AgentMarket::new();
    let (tid, _exec) = setup_task_awaiting_qa(&mut market);

    let qa = staked_committee(&mut market, 3);
    let mut members = as_member_set(&qa);
    // 第 4 席：有密钥对，但从未在服务端注册/质押。
    let mut seed = [0u8; 32];
    seed[0] = 99;
    let ghost = Keypair::from_seed(&seed);
    let ghost_pk: [u8; 32] = ghost.public_key().try_into().unwrap();
    members.push(("did:nau:ghost".to_string(), ghost_pk));

    let err = market
        .verify_result_authenticated(&tid, 0, members, vec![], NOW)
        .unwrap_err();
    assert!(
        err.contains("COMMITTEE_NOT_STAKED"),
        "应因委员未质押被拒，实际: {err}"
    );
    assert!(market.settle_task(&tid).is_err());
}

/// ④ 4 个相互独立、各自足额质押、且均非执行者的委员，3 张 Stop 票 →
///    合法通过、升 Verified、可结算（证明闸门不误伤正确流程）。
#[test]
fn au04_four_independently_staked_committee_accepts_and_settles() {
    let mut market = AgentMarket::new();
    let (tid, _exec) = setup_task_awaiting_qa(&mut market);

    let qa = staked_committee(&mut market, 4);
    let members = as_member_set(&qa);

    // n=4 → f=1 → quorum=3：前 3 个委员签 Stop。
    let mut votes = Vec::new();
    for (did, kp, _) in qa.iter().take(3) {
        votes.push(SignedQaVote::sign(
            &tid,
            0,
            did,
            QaVote::Stop,
            &format!("nonce-{did}"),
            NOW - 10,
            NOW + 60,
            kp,
        ));
    }

    let decision = market
        .verify_result_authenticated(&tid, 0, members, votes, NOW)
        .expect("合法的独立足额质押委员会应通过");
    assert_eq!(decision, QaDecision::Stop);

    // 证据被服务端提升为可信，任务 Accepted。
    assert!(market
        .get_result(&tid)
        .unwrap()
        .evidence_grade
        .is_trustworthy());
    assert_eq!(market.get_task(&tid).unwrap().state, TaskState::Accepted);

    // 可结算（按中标价 10）。
    let paid = market.settle_task(&tid).expect("验收后应可结算");
    assert_eq!(paid, Money::new(10));
    assert_eq!(market.get_task(&tid).unwrap().state, TaskState::Settled);
}

// ===== AU-18：提交即降为 Unverified =====

/// 提交时客户端自报 Verified，入库后必须是 Unverified，且不能直接结算。
/// 旧实现原样落库自报 Verified。
#[test]
fn au18_submitted_self_reported_verified_is_downgraded() {
    let mut market = AgentMarket::new();
    let exec = "did:nau:executor";
    deposit_and_register(&mut market, exec, 100);
    helpers::deposit(&mut market, "did:nau:requester", Money::new(50));
    market
        .publish_task(make_task("task-1", 50, "did:nau:requester"))
        .unwrap();
    market.submit_bid(make_bid(exec, "task-1", 10)).unwrap();
    market.match_task("task-1").unwrap();

    // 客户端自报 Verified（企图绕过证据闸门直接结算）。
    market
        .submit_result(ResultEnvelope {
            task_id: "task-1".to_string(),
            agent_id: exec.to_string(),
            report: r#"{"ok":true}"#.to_string(),
            confidence: 0.95,
            error_type: ErrorType::None,
            trace_ref: "trace://t1/1".to_string(),
            evidence_grade: EvidenceGrade::Verified,
            latency_ms: 100,
            pocv: None,
        })
        .unwrap();

    // 入库后强制降为 Unverified（旧实现此处会是 Verified）。
    let stored = market.get_result("task-1").unwrap();
    assert_eq!(
        stored.evidence_grade,
        EvidenceGrade::Unverified,
        "客户端自报等级入库即须降为 Unverified"
    );

    // 不得直接结算（仍在 Verifying，且证据不可信）。
    assert!(
        market.settle_task("task-1").is_err(),
        "自报 Verified 不得直接结算放款"
    );
}

// ===== AU-03：账本哈希链恢复 fail-closed =====

/// 构造一个「append 两条流水后篡改第 1 条 payload」的 SQLite 账本文件，
/// 使 verify_ledger_chain 返回 Err。返回该库路径。
fn tampered_ledger_path(tag: &str) -> std::path::PathBuf {
    use gsn_core::marketplace::{SettlementReason, SettlementRecord};
    let path = std::env::temp_dir().join(format!(
        "au-v351-ledger-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // 1) 正常 append 两条。
    {
        let store = gsn_core::storage::PersistentStore::open(&path).unwrap();
        store
            .append_ledger_record(&SettlementRecord {
                task_id: "deposit:a1".into(),
                from_account: String::new(),
                to_account: "a1".into(),
                amount: Money::new(100),
                reason: SettlementReason::Deposited,
                timestamp: 1,
            })
            .unwrap();
        store
            .append_ledger_record(&SettlementRecord {
                task_id: "deposit:a2".into(),
                from_account: String::new(),
                to_account: "a2".into(),
                amount: Money::new(50),
                reason: SettlementReason::Deposited,
                timestamp: 2,
            })
            .unwrap();
    }
    // 2) 用独立连接篡改第 1 条 payload（断链）。
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE ledger_entries SET payload = ?1 WHERE seq = 1",
            ["{\"tampered\":true}"],
        )
        .unwrap();
    }
    // 3) 确认链确实断了。
    let store = gsn_core::storage::PersistentStore::open(&path).unwrap();
    assert!(
        store.verify_ledger_chain().is_err(),
        "前置：篡改后账本链必须校验失败"
    );
    drop(store);
    path
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// AU-03：默认（无逃生开关）断链账本 → 演员进入拒服态，一切命令回 LEDGER_TAMPERED。
#[test]
fn au03_tampered_ledger_refuses_service_by_default() {
    std::env::remove_var("GSN_ALLOW_TAMPERED_LEDGER");
    let path = tampered_ledger_path("failclosed");
    let store = Arc::new(gsn_core::storage::PersistentStore::open(&path).unwrap());

    rt().block_on(async {
        let handle = MarketActorHandle::spawn_with_store(store);
        // 任何命令都应被拒服态拦截（包括结算与只读查询）。
        let resp = handle.settle_task("task-1".to_string()).await;
        match resp {
            MarketResponse::Err(msg) => {
                assert!(
                    msg.contains("LEDGER_TAMPERED"),
                    "默认断链应拒服，实际: {msg}"
                );
            }
            other => panic!("断链默认应拒绝服务，实际进入数据面: {other:?}"),
        }
        let resp2 = handle.balance("a1".to_string()).await;
        match resp2 {
            MarketResponse::Err(msg) => assert!(msg.contains("LEDGER_TAMPERED")),
            other => panic!("断链默认连只读查询也应拒服: {other:?}"),
        }
    });
}

/// AU-03：显式 GSN_ALLOW_TAMPERED_LEDGER=1 → 保留旧的容错恢复（进入数据面）。
///
/// 环境变量是进程级全局，为避免污染其它测试，本用例在子进程中执行：
/// 首次运行时以 GSN_ALLOW_TAMPERED_LEDGER=1 + AU03_ESCAPE_CHILD=1 重新拉起测试二进制自身。
#[test]
fn au03_escape_hatch_allows_tampered_recovery() {
    if std::env::var("AU03_ESCAPE_CHILD").as_deref() != Ok("1") {
        let exe = std::env::current_exe().unwrap();
        let status = std::process::Command::new(exe)
            .arg("--exact")
            .arg("au03_escape_hatch_allows_tampered_recovery")
            .env("GSN_ALLOW_TAMPERED_LEDGER", "1")
            .env("AU03_ESCAPE_CHILD", "1")
            .env("RUST_TEST_THREADS", "1")
            .status()
            .expect("拉起子进程验证逃生开关");
        assert!(status.success(), "子进程（逃生开关=1）应通过");
        return;
    }

    // 子进程内：逃生开关已为 1。
    std::env::set_var("GSN_ALLOW_TAMPERED_LEDGER", "1");
    let path = tampered_ledger_path("escape");
    let store = Arc::new(gsn_core::storage::PersistentStore::open(&path).unwrap());

    rt().block_on(async {
        let handle = MarketActorHandle::spawn_with_store(store);
        // 旧容错行为：不拒服，进入正常数据面。task-1 不存在 → 业务错误 NOT_FOUND，
        // 而非 LEDGER_TAMPERED。
        let resp = handle.settle_task("task-1".to_string()).await;
        match resp {
            MarketResponse::Err(msg) => {
                assert!(
                    !msg.contains("LEDGER_TAMPERED"),
                    "逃生开关下应进入数据面（业务错误），不应拒服，实际: {msg}"
                );
                assert!(msg.contains("NOT_FOUND"), "应为任务不存在的业务错误: {msg}");
            }
            other => panic!("逃生开关下应进入数据面: {other:?}"),
        }
    });
}
