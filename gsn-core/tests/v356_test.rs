//! v3.5.6（#74）市场层结算回归。
//!
//! 背景：`test_end_to_end_market_flow`（v234_test.rs）已钉住「按中标价付款 + 预算余款
//! 退回需求方」的逐账户余额与 `conservation_check`，但有两处市场层行为此前没有任何
//! 断言：
//!   1. 端到端结算（含中标价差额退款）后，**独立审计** `independent_audit()` 是否仍
//!      `passed`（它从只追加流水独立重放，并比对重放集与当前集的并集，比单纯守恒更严，
//!      能抓到「自洽但未入账」的变动）。SettlementEngine 层有测试，但市场层（经
//!      `settle_task` 完整状态机 + escrow/refund 真实账户）这条路径没有。
//!   2. 市场层对**同一任务二次结算**必须返回 `Err`。SettlementEngine 层有
//!      `test_settlement_duplicate_payment_prevented`，但市场层靠任务状态机
//!      （Settled 无入边）拒绝，此前无测试钉住；一旦状态机放宽，二次结算会铸出第二笔款。
//!
//! 这两条用例在「修复前的代码」上的意义：它们把「中标价差额退款」与「防重复结算」
//! 从隐含行为提升为市场层可执行断言，任何回退都会立即变红。

use gsn_core::marketplace::*;

fn agent_card(id: &str, stake: i64) -> MarketAgentCard {
    MarketAgentCard {
        agent_id: id.to_string(),
        version: "1.0.0".to_string(),
        name: format!("Agent {}", id),
        description: "测试智能体".to_string(),
        skills: vec!["translation".to_string()],
        modalities: vec!["text".to_string()],
        models: vec!["test-model".to_string()],
        endpoint: "a2a://test".to_string(),
        pricing: Pricing {
            model: PricingModel::PerCall,
            price: Money::new(10),
            currency: Currency::Credit,
        },
        sla: Sla::default(),
        owner: format!("owner-{}", id),
        stake: Money::new(stake),
        reputation_score: 0.5,
        total_calls: 0,
        success_rate: 0.0,
        evidence_grade: EvidenceGrade::CpuProto,
        verified: false,
        created_at: 1000,
        updated_at: 1000,
    }
}

fn task(id: &str, budget: i64, requester: &str) -> TaskSpec {
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
        verification_policy: VerificationPolicy::None,
        requester: requester.to_string(),
        state: TaskState::Draft,
        created_at: 1000,
    }
}

/// 推进到「已结算」：充值质押注册 → 发布托管 → 投标(10) → 匹配 → 验收 → 结算。
fn settle_once(market: &mut AgentMarket) -> Money {
    market.deposit("agent-1", Money::new(100)).unwrap();
    market.register_agent(agent_card("agent-1", 100)).unwrap();
    market.deposit("requester-1", Money::new(50)).unwrap();
    market
        .publish_task(task("task-1", 50, "requester-1"))
        .unwrap();
    market
        .submit_bid(Bid {
            agent_id: "agent-1".to_string(),
            task_id: "task-1".to_string(),
            proposed_price: Money::new(10),
            estimated_latency_ms: 500,
            score: 0.0,
        })
        .unwrap();
    assert_eq!(market.match_task("task-1").unwrap(), "agent-1");
    market
        .submit_result(ResultEnvelope {
            task_id: "task-1".to_string(),
            agent_id: "agent-1".to_string(),
            report: r#"{"translated":"hello"}"#.to_string(),
            confidence: 0.95,
            error_type: ErrorType::None,
            trace_ref: "trace://task-1/1".to_string(),
            evidence_grade: EvidenceGrade::Unverified,
            latency_ms: 300,
        })
        .unwrap();
    market.settle_task("task-1").unwrap()
}

/// 中标价差额退款后，市场层独立审计必须通过，且重放总额与当前余额并集精确相等。
#[test]
fn test_market_independent_audit_passes_after_winner_price_refund() {
    let mut market = AgentMarket::new();

    let paid = settle_once(&mut market);
    assert_eq!(paid, Money::new(10));

    // 逐账户：执行者收 10、需求方收回余款 40、托管清零、质押锁定 100。
    assert_eq!(market.balance("agent-1"), Money::new(10));
    assert_eq!(market.balance("requester-1"), Money::new(40));
    assert_eq!(market.balance("__escrow__:task-1"), Money::ZERO);
    assert_eq!(market.balance("__stake__:agent-1"), Money::new(100));

    // 关键断言：经完整市场状态机结算（含 50→10 的差额退款）后，
    // 从只追加流水独立重放的审计必须 passed，且重放集 ∪ 当前集总额精确为总充值 150。
    let audit = market.independent_audit();
    assert!(
        audit.passed,
        "中标价差额退款后独立审计失败: passed={}, expected={:?}, actual={:?}, replayed_records={}",
        audit.passed, audit.expected_total, audit.actual_total, audit.replayed_records
    );
    assert_eq!(audit.expected_total, Money::new(150));
    assert_eq!(audit.actual_total, Money::new(150));
    assert!(audit.replayed_records > 0, "结算后流水不应为空");

    // 守恒同时成立（无罚没）。
    let conservation = market.conservation_check();
    assert!(conservation.conserved);
    assert_eq!(conservation.balance_sum, Money::new(150));
}

/// 市场层二次结算同一任务必须被状态机拒绝（Settled 为终态），不得付出第二笔款。
#[test]
fn test_market_settle_task_twice_rejected() {
    let mut market = AgentMarket::new();

    let paid = settle_once(&mut market);
    assert_eq!(paid, Money::new(10));
    assert_eq!(market.get_task("task-1").unwrap().state, TaskState::Settled);

    let before_agent = market.balance("agent-1");
    let before_requester = market.balance("requester-1");

    // 第二次结算必须返回 Err（终态无入边），且账户余额不得再变动。
    let second = market.settle_task("task-1");
    assert!(
        second.is_err(),
        "二次结算应被拒绝，实际返回 Ok: {:?}",
        second
    );
    assert!(
        second.unwrap_err().contains("不能结算"),
        "拒绝原因应指向任务状态"
    );
    assert_eq!(market.balance("agent-1"), before_agent);
    assert_eq!(market.balance("requester-1"), before_requester);
    assert_eq!(market.settled_count(), 1, "结算计数不应因二次调用增加");
}
