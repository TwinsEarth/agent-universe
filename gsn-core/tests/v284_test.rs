//! v2.8.4 重启恢复证据闸门对抗测试（GAP §3.2）
//!
//! 审计根因（v2.8.2）：`restore_tasks_from_store` 对快照未存字段用安全默认值硬编码：
//!   verification_policy: None（结算闸门豁免证据检查）
//!   winner_price: None（`winner_price.unwrap_or(budget)` 按满额预算支付）
//!   结果信封从不落盘（已验收未结算任务重启后永远无法结算）
//!   信誉/质押从不落盘（重启后 submit_bid 对每个 agent 失败、arbitrate 报无质押，
//!                        但质押资金仍在账，罚没路径是死的）
//!
//! 本测试做"关闭再打开"：构造一个 BftLite 任务走完投标→匹配→提交结果，
//! 持久化全部快照后 drop，重开 store 与新 market 恢复，断言：
//!   1. 验证策略恢复为 BftLite（不回 None 豁免闸门）
//!   2. winner_price 恢复（不付满额）
//!   3. 结果信封存活
//!   4. 信誉/质押恢复后仍满足资格、可出价

use gsn_core::marketplace::{
    AgentMarket, Bid, Currency, ErrorType, EvidenceGrade, Money, Pricing, PricingModel,
    ResultEnvelope, Sla, TaskSpec, TaskState, VerificationPolicy,
};
use gsn_core::storage::PersistentStore;

fn tmp_db(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gsn_v284_{}_{}_{}",
        std::process::id(),
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("gsn.db")
}

fn make_card(agent_id: &str) -> gsn_core::marketplace::MarketAgentCard {
    gsn_core::marketplace::MarketAgentCard {
        agent_id: agent_id.to_string(),
        version: "1.0.0".to_string(),
        name: "Worker".to_string(),
        description: "test worker".to_string(),
        skills: vec!["rust".to_string()],
        modalities: vec!["text".to_string()],
        models: vec!["mock".to_string()],
        endpoint: "http://127.0.0.1:1".to_string(),
        pricing: Pricing {
            model: PricingModel::PerCall,
            price: Money::new(4000),
            currency: Currency::Credit,
        },
        sla: Sla::default(),
        owner: agent_id.to_string(),
        stake: Money::new(1000),
        reputation_score: 0.9,
        total_calls: 0,
        success_rate: 1.0,
        evidence_grade: EvidenceGrade::CpuProto,
        verified: false,
        created_at: 100,
        updated_at: 100,
    }
}

fn make_task(task_id: &str) -> TaskSpec {
    TaskSpec {
        task_id: task_id.to_string(),
        goal: "do it".to_string(),
        context: "task context".to_string(),
        done: vec![],
        todo: vec!["work".to_string()],
        trace: vec![],
        owner: None,
        budget: Money::new(5000),
        winner_price: None,
        deadline: 1000,
        required_skills: vec!["rust".to_string()],
        verification_policy: VerificationPolicy::BftLite { n: 3, f: 1 },
        requester: "requester-1".to_string(),
        state: TaskState::Open,
        created_at: 100,
    }
}

#[test]
fn evidence_gate_survives_restart() {
    let path = tmp_db("gate");
    let agent_id = "did:nau:v284-worker";
    let task_id = "task-v284";

    // ---- 第一段：建市、走完业务、持久化 ----
    {
        let mut market = AgentMarket::new();
        // 需求方与 agent 各入金（需求方覆盖预算，agent 覆盖质押）
        market.deposit("requester-1", Money::new(5000)).unwrap();
        market.deposit(agent_id, Money::new(1000)).unwrap();
        // 注册 agent（质押 1000 锁定）
        market.register_agent(make_card(agent_id)).unwrap();
        // 发布 BftLite 任务（预算 5000 托管）
        market.publish_task(make_task(task_id)).unwrap();
        // 投标价 4000
        market
            .submit_bid(Bid {
                agent_id: agent_id.to_string(),
                task_id: task_id.to_string(),
                proposed_price: Money::new(4000),
                estimated_latency_ms: 100,
                score: 0.9,
            })
            .unwrap();
        // 匹配（→ Matched，winner_price=4000）
        let winner = market.match_task(task_id).unwrap();
        assert_eq!(winner, agent_id);
        // 提交结果（BftLite → Verifying，结果信封入 results）
        market
            .submit_result(ResultEnvelope {
                task_id: task_id.to_string(),
                agent_id: agent_id.to_string(),
                report: "done-report".to_string(),
                confidence: 0.95,
                error_type: ErrorType::None,
                trace_ref: String::new(),
                evidence_grade: EvidenceGrade::CpuProto,
                latency_ms: 100,
            })
            .unwrap();

        // 持久化全部快照（agents/tasks/results/reputations/stakes）
        let store = PersistentStore::open(&path).unwrap();
        for a in market.snapshot_agents_for_store() {
            store.upsert_agent(&a).unwrap();
        }
        for t in market.snapshot_tasks_for_store() {
            store.upsert_task(&t).unwrap();
        }
        for (id, payload) in market.snapshot_results_for_store() {
            store
                .put_json_row("result_envelopes", "task_id", &id, &payload)
                .unwrap();
        }
        for (id, payload) in market.snapshot_reputations_for_store() {
            store
                .put_json_row("reputations", "agent_id", &id, &payload)
                .unwrap();
        }
        for (id, payload) in market.snapshot_stakes_for_store() {
            store
                .put_json_row("stakes", "agent_id", &id, &payload)
                .unwrap();
        }
    } // market + store drop

    // ---- 第二段：重开，恢复，断言闸门不放宽 ----
    let store = PersistentStore::open(&path).unwrap();
    let mut market = AgentMarket::new();
    market.restore_agents_from_store(store.load_agents().unwrap());
    market.restore_tasks_from_store(store.load_tasks().unwrap());
    market.restore_results_from_store(store.load_json_rows("result_envelopes", "task_id").unwrap());
    let rep_rows = store.load_json_rows("reputations", "agent_id").unwrap();
    let stake_rows = store.load_json_rows("stakes", "agent_id").unwrap();
    market.restore_reputation_state_from_store(rep_rows, stake_rows);

    // 1. 验证策略恢复为 BftLite（不回 None 豁免）
    let task = market.get_task(task_id).expect("任务应恢复");
    assert!(
        matches!(
            task.verification_policy,
            VerificationPolicy::BftLite { n: 3, f: 1 }
        ),
        "BftLite 策略应在重启后存活，实际 {:?}",
        task.verification_policy
    );
    // 2. winner_price 恢复（不付满额预算 5000）
    assert_eq!(
        task.winner_price.map(|m| m.as_i64()),
        Some(4000),
        "winner_price 应恢复为 4000，实际 {:?}",
        task.winner_price
    );
    assert_eq!(task.requester, "requester-1");
    // v3.5.0（Win/Mac 实机 W-04）：完整任务规格字段重启后必须存活。
    // 旧实现把 context 丢成 ""、required_skills 丢成 []、todo 换成 "(restored from disk)" 占位符。
    assert_eq!(task.context, "task context", "context 重启后不应丢失");
    assert_eq!(
        task.todo,
        vec!["work".to_string()],
        "todo 重启后不应被占位符替换"
    );
    assert_eq!(
        task.required_skills,
        vec!["rust".to_string()],
        "required_skills 重启后不应丢失"
    );
    // 3. 结果信封存活
    let env = market.get_result(task_id).expect("结果信封应在重启后存活");
    assert_eq!(env.report, "done-report");
    // 4. 质押恢复后仍满足资格（重启前可出价，重启后也应可出价）
    assert!(
        market.is_eligible(agent_id, 0.3),
        "agent 质押恢复后应仍满足资格"
    );
}
