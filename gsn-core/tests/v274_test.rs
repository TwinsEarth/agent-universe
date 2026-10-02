//! v2.7.4 实机部署修复回归测试
//!
//! 缺陷来自 Mac 真机部署：重启 daemon 后，账本/余额/守恒从 SQLite 正确恢复，
//! 但 HTTP `/agents`、`/tasks/{id}`、`/stats` 全空/0/not_found。
//! 根因：`market_actor::spawn_with_store` 对 `load_agents()/load_tasks()` 只取了
//! `.len()` 打日志，并未把业务对象注入内存 market——只有账本走了 `restore_ledger`。
//!
//! 修复三处：
//! 1. `task.rs` 新增 `TaskState::from_label()`（大写 label 反解析，坏值回 Open）。
//! 2. `mod.rs` 新增 `restore_agents_from_store()` / `restore_tasks_from_store()`。
//! 3. `market_actor.rs` 启动时真正调这两个方法注入内存。
//!
//! 本测试不依赖网络端口：直接构造 `AgentMarket` 与 `StoredAgent/StoredTask`。

use gsn_core::marketplace::{AgentMarket, PricingModel, TaskState, VerificationPolicy};
use gsn_core::storage::{StoredAgent, StoredTask};

/// ① `label()` 与 `from_label()` 双向往返一致；未知/坏值一律回 `Open`。
#[test]
fn test_from_label_roundtrip_and_unknown_falls_back_open() {
    let cases = [
        TaskState::Draft,
        TaskState::Open,
        TaskState::Matched,
        TaskState::Running,
        TaskState::Verifying,
        TaskState::Accepted,
        TaskState::Settled,
        TaskState::Rework,
        TaskState::Disputed,
        TaskState::Arbitration,
        TaskState::Slashed,
        TaskState::NoQuorum,
        TaskState::Rejected,
    ];
    for s in cases {
        let label = s.label();
        let back = TaskState::from_label(label);
        assert_eq!(back, s, "label={label} 往返失败");
    }
    // 坏值 / 空串 / 小写 → Open
    assert_eq!(TaskState::from_label(""), TaskState::Open);
    assert_eq!(TaskState::from_label("garbage"), TaskState::Open);
    assert_eq!(TaskState::from_label("settled"), TaskState::Settled); // 大小写不敏感
    assert_eq!(TaskState::from_label("no_quorum"), TaskState::NoQuorum);
}

/// ② 恢复 tasks：磁盘快照的 SETTLED 任务应注入内存且状态正确。
#[test]
fn test_restore_tasks_from_store_visible() {
    let mut market = AgentMarket::new();
    assert_eq!(market.task_count(), 0);

    let spec1 = r#"{"task_id":"task-restore-1","goal":"restored task","context":"rich-context",
        "done":["step0"],"todo":["step1","step2"],"trace":["tr"],"owner":"worker-1",
        "budget":5000,"winner_price":4200,"deadline":1800000000000,
        "required_skills":["rust","network"],"verification_policy":{"BftLite":{"n":3,"f":1}},
        "requester":"requester-1","state":"Settled","created_at":1700000000000}"#;
    let spec2 = r#"{"task_id":"task-restore-2","goal":"open task","context":"ctx2",
        "done":[],"todo":["only"],"trace":[],"owner":null,"budget":3000,
        "winner_price":null,"deadline":0,"required_skills":["ops"],
        "verification_policy":"None","requester":"","state":"Open","created_at":1700000001000}"#;
    let stored = vec![
        StoredTask {
            task_id: "task-restore-1".to_string(),
            goal: "restored task".to_string(),
            state: "SETTLED".to_string(),
            owner: Some("worker-1".to_string()),
            budget: 5000,
            created_at: "1700000000000".to_string(),
            winner_price: Some(4200),
            verification_policy: r#"{"BftLite":{"n":3,"f":1}}"#.to_string(),
            requester: "requester-1".to_string(),
            deadline: 1800000000000,
            spec_json: Some(spec1.to_string()),
        },
        StoredTask {
            task_id: "task-restore-2".to_string(),
            goal: "open task".to_string(),
            state: "OPEN".to_string(),
            owner: None,
            budget: 3000,
            created_at: "1700000001000".to_string(),
            winner_price: None,
            verification_policy: r#""None""#.to_string(),
            requester: String::new(),
            deadline: 0,
            spec_json: Some(spec2.to_string()),
        },
    ];
    market.restore_tasks_from_store(stored);

    assert_eq!(market.task_count(), 2);
    let t1 = market
        .get_task("task-restore-1")
        .expect("task-restore-1 应在内存");
    assert_eq!(t1.state, TaskState::Settled, "SETTLED 应被正确反解析");
    assert_eq!(t1.owner.as_deref(), Some("worker-1"));
    // v2.8.4: 恢复不放宽证据闸门（GAP §3.2），BftLite 不回 None
    assert!(
        matches!(
            t1.verification_policy,
            VerificationPolicy::BftLite { n: 3, f: 1 }
        ),
        "BftLite 策略应正确恢复，实际 {:?}",
        t1.verification_policy
    );
    assert_eq!(t1.winner_price.map(|m| m.as_i64()), Some(4200));
    assert_eq!(t1.requester, "requester-1");
    // v3.5.4（W-04）：完整规格字段不再在重启后丢失。
    assert_eq!(t1.context, "rich-context");
    assert_eq!(t1.todo, vec!["step1".to_string(), "step2".to_string()]);
    assert_eq!(t1.done, vec!["step0".to_string()]);
    assert_eq!(
        t1.required_skills,
        vec!["rust".to_string(), "network".to_string()]
    );
    let t2 = market
        .get_task("task-restore-2")
        .expect("task-restore-2 应在内存");
    assert_eq!(t2.state, TaskState::Open);
    assert!(matches!(t2.verification_policy, VerificationPolicy::None));
    // v3.5.4（W-04）：第二条任务的丰富字段同样存活。
    assert_eq!(t2.context, "ctx2");
    assert_eq!(t2.required_skills, vec!["ops".to_string()]);
}

/// ③ 恢复 agents：磁盘快照的 agent 应注入内存，stake/reputation 正确还原。
#[test]
fn test_restore_agents_from_store_visible() {
    let mut market = AgentMarket::new();
    assert_eq!(market.agent_count(), 0);

    let card_json = r#"{"agent_id":"did:nau:restore-worker","version":"9.9.9-w05",
        "name":"restore-worker","description":"d","skills":["rust","network"],
        "modalities":["text","image"],"models":["m1"],"endpoint":"ep",
        "pricing":{"model":"PerCall","price":150,"currency":"Credit"},
        "sla":{"latency_p95_ms":1234,"availability":0.9,"max_concurrency":7},
        "owner":"did:nau:restore-worker","stake":10000,"reputation_score":0.87,
        "total_calls":42,"success_rate":0.9,"evidence_grade":"Verified",
        "verified":true,"created_at":1700000000000,"updated_at":1700000000001}"#;
    let stored = vec![StoredAgent {
        agent_id: "did:nau:restore-worker".to_string(),
        name: "restore-worker".to_string(),
        skills: "rust,network".to_string(),
        stake: 10000,
        reputation: 0.87,
        created_at: "1700000000000".to_string(),
        card_json: Some(card_json.to_string()),
    }];
    market.restore_agents_from_store(stored);

    assert_eq!(market.agent_count(), 1);
    let card = market
        .get_agent("did:nau:restore-worker")
        .expect("agent 应在内存");
    assert_eq!(card.name, "restore-worker");
    assert_eq!(card.stake.as_i64(), 10000);
    assert!((card.reputation_score - 0.87).abs() < 1e-9);
    assert!(card.skills.contains(&"rust".to_string()));
    assert!(card.skills.contains(&"network".to_string()));
    // v3.5.4（W-05）：完整卡片声明字段不再在重启后退化为 0.0.0-restored。
    assert_eq!(card.version, "9.9.9-w05");
    assert_eq!(
        card.modalities,
        vec!["text".to_string(), "image".to_string()]
    );
    assert_eq!(card.models, vec!["m1".to_string()]);
    assert_eq!(card.pricing.price.as_i64(), 150);
    assert!(matches!(card.pricing.model, PricingModel::PerCall));
    assert_eq!(card.sla.latency_p95_ms, 1234);
    assert_eq!(card.sla.max_concurrency, 7);
    assert_eq!(card.total_calls, 42);
    assert!(card.verified);
}
