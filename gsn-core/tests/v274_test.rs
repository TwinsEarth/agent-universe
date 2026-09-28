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

use gsn_core::marketplace::{AgentMarket, TaskState};
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

    let stored = vec![
        StoredTask {
            task_id: "task-restore-1".to_string(),
            goal: "restored task".to_string(),
            state: "SETTLED".to_string(),
            owner: Some("worker-1".to_string()),
            budget: 5000,
            created_at: "1700000000000".to_string(),
        },
        StoredTask {
            task_id: "task-restore-2".to_string(),
            goal: "open task".to_string(),
            state: "OPEN".to_string(),
            owner: None,
            budget: 3000,
            created_at: "1700000001000".to_string(),
        },
    ];
    market.restore_tasks_from_store(stored);

    assert_eq!(market.task_count(), 2);
    let t1 = market.get_task("task-restore-1").expect("task-restore-1 应在内存");
    assert_eq!(t1.state, TaskState::Settled, "SETTLED 应被正确反解析");
    assert_eq!(t1.owner.as_deref(), Some("worker-1"));
    let t2 = market.get_task("task-restore-2").expect("task-restore-2 应在内存");
    assert_eq!(t2.state, TaskState::Open);
}

/// ③ 恢复 agents：磁盘快照的 agent 应注入内存，stake/reputation 正确还原。
#[test]
fn test_restore_agents_from_store_visible() {
    let mut market = AgentMarket::new();
    assert_eq!(market.agent_count(), 0);

    let stored = vec![StoredAgent {
        agent_id: "did:nau:restore-worker".to_string(),
        name: "restore-worker".to_string(),
        skills: "rust,network".to_string(),
        stake: 10000,
        reputation: 0.87,
        created_at: "1700000000000".to_string(),
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
}
