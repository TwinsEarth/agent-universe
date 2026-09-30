//! Agent Market v2.3.4 测试（v2.5.8 整数账本版）
//!
//! 覆盖：
//! - Agent 注册 / 重复 / 非法字段 / 质押不足 / 未充值拒绝（防铸币）
//! - 任务发布 / 六字段校验 / 未充值拒绝（发布即托管）
//! - 发现与匹配
//! - BFT-lite QA 委员会
//! - 结算守恒 / 防重复支付（精确相等，无容差）
//! - 信誉与质押罚没
//! - 争议仲裁
//! - 端到端流程

use gsn_core::marketplace::*;

// ===== 辅助函数 =====

fn make_agent_card(id: &str, stake: i64) -> MarketAgentCard {
    MarketAgentCard {
        agent_id: id.to_string(),
        version: "1.0.0".to_string(),
        name: format!("Agent {}", id),
        description: "测试智能体".to_string(),
        skills: vec!["translation".to_string(), "writing".to_string()],
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

/// 正常路径：先充值质押金，再注册
fn fund_and_register(market: &mut AgentMarket, id: &str, stake: i64) {
    market.deposit(id, Money::new(stake)).unwrap();
    market.register_agent(make_agent_card(id, stake)).unwrap();
}

/// 正常路径：先给需求方充值预算，再发布（发布即托管）
fn fund_and_publish(market: &mut AgentMarket, task: TaskSpec, funds: i64) -> String {
    let requester = task.requester.clone();
    market.deposit(&requester, Money::new(funds)).unwrap();
    market.publish_task(task).unwrap()
}

/// 将任务推进到 Accepted（投标 → 匹配 → 提交结果，policy=None 直接验收）。
///
/// v2.8.5 后 Disputed 仅可由 Accepted/Running/Verifying/Rework 进入，
/// 因此发起争议前需先让任务离开 Open/Matched。
fn match_and_accept(market: &mut AgentMarket, task_id: &str, agent: &str, price: i64) {
    market.submit_bid(make_bid(agent, task_id, price)).unwrap();
    market.match_task(task_id).unwrap();
    market
        .submit_result(ResultEnvelope {
            task_id: task_id.to_string(),
            agent_id: agent.to_string(),
            report: r#"{"ok":true}"#.to_string(),
            confidence: 0.95,
            error_type: ErrorType::None,
            trace_ref: format!("trace://{}/1", task_id),
            evidence_grade: EvidenceGrade::CpuProto,
            latency_ms: 100,
        })
        .unwrap();
}

// ===== F1: Agent 注册测试 =====

#[test]
fn test_register_agent_success() {
    let mut market = AgentMarket::new();
    market.deposit("agent-1", Money::new(100)).unwrap();
    let result = market.register_agent(make_agent_card("agent-1", 100));
    assert!(result.is_ok());
    assert_eq!(market.agent_count(), 1);
}

#[test]
fn test_register_agent_insufficient_stake() {
    let mut market = AgentMarket::new();
    market.deposit("agent-1", Money::new(50)).unwrap();
    let result = market.register_agent(make_agent_card("agent-1", 50)); // 最低 100
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("质押不足"));
}

#[test]
fn test_register_agent_empty_id() {
    let mut market = AgentMarket::new();
    market.deposit("agent-1", Money::new(100)).unwrap();
    let mut card = make_agent_card("agent-1", 100);
    card.agent_id = "".to_string();
    let result = market.register_agent(card);
    assert!(result.is_err());
}

#[test]
fn test_register_agent_empty_name() {
    let mut market = AgentMarket::new();
    market.deposit("agent-1", Money::new(100)).unwrap();
    let mut card = make_agent_card("agent-1", 100);
    card.name = "".to_string();
    let result = market.register_agent(card);
    assert!(result.is_err());
}

#[test]
fn test_register_agent_duplicate() {
    let mut market = AgentMarket::new();
    market.deposit("agent-1", Money::new(100)).unwrap();
    assert!(market
        .register_agent(make_agent_card("agent-1", 100))
        .is_ok());
    // 第二次注册：质押金已锁定、自有余额为 0 → 拒绝（防止重复占用），不覆盖
    let result = market.register_agent(make_agent_card("agent-1", 100));
    assert!(result.is_err());
    assert_eq!(market.agent_count(), 1);
}

#[test]
fn test_skill_index_created() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    fund_and_register(&mut market, "a2", 100);

    let found = market.discover_by_skill("translation");
    assert_eq!(found.len(), 2);
}

// v2.5.8: 防铸币 —— 未充值即注册必须拒绝，且系统不产生任何余额
#[test]
fn test_register_without_deposit_rejected_no_minting() {
    let mut market = AgentMarket::new();
    let result = market.register_agent(make_agent_card("a1", 100));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("请先 deposit"));
    assert_eq!(market.agent_count(), 0);

    let report = market.conservation_check();
    assert_eq!(report.balance_sum, Money::ZERO);
    assert_eq!(report.total_deposits, Money::ZERO);
}

// ===== F2: 任务发布测试 =====

#[test]
fn test_publish_task_success() {
    let mut market = AgentMarket::new();
    let id = fund_and_publish(&mut market, make_task("task-1", 50, "user-1"), 50);
    assert_eq!(id, "task-1");
    assert_eq!(market.task_count(), 1);
}

#[test]
fn test_publish_task_empty_goal() {
    let mut market = AgentMarket::new();
    market.deposit("user-1", Money::new(50)).unwrap();
    let mut task = make_task("task-1", 50, "user-1");
    task.goal = "".to_string();
    let result = market.publish_task(task);
    assert!(result.is_err());
    let gaps = result.unwrap_err();
    assert!(gaps.iter().any(|g| g.contains("goal")));
}

#[test]
fn test_publish_task_empty_todo() {
    let mut market = AgentMarket::new();
    market.deposit("user-1", Money::new(50)).unwrap();
    let mut task = make_task("task-1", 50, "user-1");
    task.todo = vec![];
    let result = market.publish_task(task);
    assert!(result.is_err());
}

#[test]
fn test_publish_task_zero_budget() {
    let mut market = AgentMarket::new();
    let result = market.publish_task(make_task("task-1", 0, "user-1"));
    assert!(result.is_err());
}

#[test]
fn test_task_state_open_after_publish() {
    let mut market = AgentMarket::new();
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);
    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Open);
}

// v2.5.8: 发布即托管 —— 未充值即发布必须拒绝，任务不入库，余额不产生
#[test]
fn test_publish_without_deposit_rejected_escrow() {
    let mut market = AgentMarket::new();
    let result = market.publish_task(make_task("t1", 50, "u1"));
    assert!(result.is_err());
    let gaps = result.unwrap_err();
    assert!(gaps.iter().any(|g| g.contains("余额不足以覆盖预算")));
    assert_eq!(market.task_count(), 0);

    let report = market.conservation_check();
    assert_eq!(report.balance_sum, Money::ZERO);
}

// v2.5.8: 发布即托管 —— 发布后预算已从需求方余额转入托管账户锁定
#[test]
fn test_publish_locks_budget_in_escrow() {
    let mut market = AgentMarket::new();
    market.deposit("u1", Money::new(80)).unwrap();
    market.publish_task(make_task("t1", 50, "u1")).unwrap();

    // 需求方自有余额只剩 30，预算 50 已锁定
    assert_eq!(market.balance("u1"), Money::new(30));
    assert_eq!(market.balance("__escrow__:t1"), Money::new(50));
}

// ===== F3: 发现与匹配测试 =====

#[test]
fn test_discover_by_skill_case_insensitive() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);

    let found = market.discover_by_skill("TRANSLATION");
    assert_eq!(found.len(), 1);
}

#[test]
fn test_discover_nonexistent_skill() {
    let market = AgentMarket::new();
    let found = market.discover_by_skill("nonexistent");
    assert_eq!(found.len(), 0);
}

#[test]
fn test_search_agents_by_name() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    let found = market.search_agents("Agent a1");
    assert_eq!(found.len(), 1);
}

#[test]
fn test_submit_bid_success() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);

    assert!(market.submit_bid(make_bid("a1", "t1", 10)).is_ok());
}

#[test]
fn test_submit_bid_unregistered_agent() {
    let mut market = AgentMarket::new();
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);

    assert!(market.submit_bid(make_bid("fake", "t1", 10)).is_err());
}

#[test]
fn test_match_task_selects_winner() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);
    market.submit_bid(make_bid("a1", "t1", 10)).unwrap();

    let winner = market.match_task("t1").unwrap();
    assert_eq!(winner, "a1");

    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Matched);
    assert_eq!(task.owner, Some("a1".to_string()));
    assert_eq!(task.winner_price, Some(Money::new(10)));
}

#[test]
fn test_match_task_no_bids() {
    let mut market = AgentMarket::new();
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);
    let result = market.match_task("t1");
    assert!(result.is_err());
}

// ===== BFT-lite QA 委员会测试 =====

#[test]
fn test_bft_committee_valid_config() {
    let committee = QaCommittee::new(4, 1);
    assert!(committee.is_ok());
}

#[test]
fn test_bft_committee_invalid_config() {
    // n=3, f=1 → 需要 n≥4
    let committee = QaCommittee::new(3, 1);
    assert!(committee.is_err());
}

#[test]
fn test_bft_committee_quorum() {
    let committee = QaCommittee::new(7, 2).unwrap();
    assert_eq!(committee.quorum(), 5); // 2f+1 = 5
}

#[test]
fn test_bft_stop_decision() {
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("m{}", i));
    }
    // 3 票 STOP（≥ quorum=3）
    committee.cast_vote("m0", QaVote::Stop).unwrap();
    committee.cast_vote("m1", QaVote::Stop).unwrap();
    committee.cast_vote("m2", QaVote::Stop).unwrap();
    committee.cast_vote("m3", QaVote::Continue).unwrap();

    assert_eq!(committee.tally(), QaDecision::Stop);
}

#[test]
fn test_bft_continue_decision() {
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("m{}", i));
    }
    committee.cast_vote("m0", QaVote::Continue).unwrap();
    committee.cast_vote("m1", QaVote::Continue).unwrap();
    committee.cast_vote("m2", QaVote::Continue).unwrap();
    committee.cast_vote("m3", QaVote::Stop).unwrap();

    assert_eq!(committee.tally(), QaDecision::Continue);
}

#[test]
fn test_bft_equivocation_invalidates_round() {
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("m{}", i));
    }
    // m0 先投 Stop 再投 Continue → equivocation
    committee.cast_vote("m0", QaVote::Stop).unwrap();
    committee.cast_vote("m0", QaVote::Continue).unwrap();

    assert_eq!(committee.tally(), QaDecision::NoQuorum);
}

#[test]
fn test_bft_silent_no_quorum() {
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("m{}", i));
    }
    // 只有 2 票 Stop，其余沉默
    committee.cast_vote("m0", QaVote::Stop).unwrap();
    committee.cast_vote("m1", QaVote::Stop).unwrap();
    // m2, m3 沉默
    assert_eq!(committee.tally(), QaDecision::NoQuorum);
}

#[test]
fn test_bft_reset_votes() {
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("m{}", i));
    }
    committee.cast_vote("m0", QaVote::Stop).unwrap();
    committee.reset_votes();
    assert_eq!(committee.tally(), QaDecision::NoQuorum);
}

#[test]
fn test_bft_unknown_voter_rejected() {
    let mut committee = QaCommittee::new(4, 1).unwrap();
    let result = committee.cast_vote("fake", QaVote::Stop);
    assert!(result.is_err());
}

// ===== 结算守恒测试（精确整数，无容差）=====

#[test]
fn test_settlement_deposit_and_balance() {
    let mut engine = SettlementEngine::new();
    engine.deposit("u1", Money::new(100)).unwrap();
    assert_eq!(engine.balance("u1"), Money::new(100));
}

#[test]
fn test_settlement_deposit_negative_rejected() {
    let mut engine = SettlementEngine::new();
    assert!(engine.deposit("u1", Money::new(-1)).is_err());
}

#[test]
fn test_settlement_completed() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", Money::new(100)).unwrap();
    let paid = engine
        .settle(
            "t1",
            "payer",
            "payee",
            Money::new(50),
            SettlementReason::Completed,
        )
        .unwrap();
    assert_eq!(paid, Money::new(50));
    assert_eq!(engine.balance("payer"), Money::new(50));
    assert_eq!(engine.balance("payee"), Money::new(50));
}

#[test]
fn test_settlement_duplicate_payment_prevented() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", Money::new(100)).unwrap();

    engine
        .settle(
            "t1",
            "payer",
            "payee",
            Money::new(50),
            SettlementReason::Completed,
        )
        .unwrap();
    // 第二次结算同一任务 → 付 0
    let paid = engine
        .settle(
            "t1",
            "payer",
            "payee",
            Money::new(50),
            SettlementReason::Completed,
        )
        .unwrap();
    assert_eq!(paid, Money::ZERO);
    assert_eq!(engine.balance("payee"), Money::new(50)); // 没有多付
}

#[test]
fn test_settlement_rejected_pays_zero() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", Money::new(100)).unwrap();
    let paid = engine
        .settle(
            "t1",
            "payer",
            "payee",
            Money::new(50),
            SettlementReason::Rejected,
        )
        .unwrap();
    assert_eq!(paid, Money::ZERO);
    // 被拒不付款，付款方余额不变
    assert_eq!(engine.balance("payer"), Money::new(100));
    assert_eq!(engine.balance("payee"), Money::ZERO);
}

#[test]
fn test_settlement_insufficient_balance_no_minting() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", Money::new(10)).unwrap();
    // 付款方余额不足 → 报错，绝不铸币
    let result = engine.settle(
        "t1",
        "payer",
        "payee",
        Money::new(50),
        SettlementReason::Completed,
    );
    assert!(result.is_err());
    // 收款方没有凭空收到钱
    assert_eq!(engine.balance("payee"), Money::ZERO);
    assert_eq!(engine.balance("payer"), Money::new(10));
}

#[test]
fn test_settlement_slash() {
    let mut engine = SettlementEngine::new();
    engine.deposit("agent", Money::new(100)).unwrap();
    let slashed = engine.slash("agent", Money::new(30)).unwrap();
    assert_eq!(slashed, Money::new(30));
    assert_eq!(engine.balance("agent"), Money::new(70));
}

#[test]
fn test_settlement_slash_insufficient() {
    let mut engine = SettlementEngine::new();
    engine.deposit("agent", Money::new(10)).unwrap();
    let result = engine.slash("agent", Money::new(50));
    assert!(result.is_err());
    assert_eq!(engine.balance("agent"), Money::new(10));
}

#[test]
fn test_conservation_check_valid() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", Money::new(100)).unwrap();
    engine
        .settle(
            "t1",
            "payer",
            "payee",
            Money::new(40),
            SettlementReason::Completed,
        )
        .unwrap();

    let report = engine.conservation_check();
    assert!(report.conserved, "守恒检查失败: {:?}", report);
    assert_eq!(report.total_deposits, Money::new(100));
    assert_eq!(report.total_paid, Money::new(40));
}

#[test]
fn test_conservation_with_slash() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", Money::new(100)).unwrap();
    engine
        .settle(
            "t1",
            "payer",
            "payee",
            Money::new(40),
            SettlementReason::Completed,
        )
        .unwrap();
    engine.slash("payee", Money::new(10)).unwrap();

    let report = engine.conservation_check();
    assert!(report.conserved, "守恒检查失败: {:?}", report);
    assert_eq!(report.total_slashed, Money::new(10));
    // 精确核对：余额和 = 总充值 - 总罚没
    assert_eq!(report.balance_sum, Money::new(90));
}

// ===== 信誉测试 =====

#[test]
fn test_reputation_initial() {
    let rep = MarketReputation::new();
    assert_eq!(rep.quality, 0.5);
    assert_eq!(rep.total_calls, 0);
}

#[test]
fn test_reputation_record_success() {
    let mut rep = MarketReputation::new();
    rep.record_call(true, 0.5);
    assert_eq!(rep.total_calls, 1);
    assert_eq!(rep.success_calls, 1);
    assert!(rep.quality > 0.5);
}

#[test]
fn test_reputation_record_failure() {
    let mut rep = MarketReputation::new();
    rep.record_call(false, 2.0);
    assert_eq!(rep.success_calls, 0);
    assert!(rep.quality < 0.5);
}

#[test]
fn test_reputation_success_rate() {
    let mut rep = MarketReputation::new();
    rep.record_call(true, 0.5);
    rep.record_call(true, 0.5);
    rep.record_call(false, 2.0);
    assert!((rep.success_rate() - 2.0 / 3.0).abs() < 0.001);
}

#[test]
fn test_reputation_overall_weighted() {
    let rep = MarketReputation {
        quality: 1.0,
        speed: 1.0,
        honesty: 1.0,
        availability: 1.0,
        total_calls: 10,
        success_calls: 10,
    };
    assert!((rep.overall() - 1.0).abs() < 0.001);
}

#[test]
fn test_dishonesty_penalty() {
    let mut rep = MarketReputation::new();
    let initial = rep.honesty;
    rep.penalize_dishonesty(0.3);
    assert_eq!(rep.honesty, initial - 0.3);
}

#[test]
fn test_honesty_reward() {
    let mut rep = MarketReputation::new();
    rep.honesty = 0.5;
    rep.reward_honesty();
    assert_eq!(rep.honesty, 0.55);
}

// ===== 质押测试 =====

#[test]
fn test_stake_registration() {
    let mut mgr = ReputationManager::new(Money::new(100));
    assert!(mgr.register_stake("a1", Money::new(100)).is_ok());
}

#[test]
fn test_stake_below_minimum() {
    let mut mgr = ReputationManager::new(Money::new(100));
    assert!(mgr.register_stake("a1", Money::new(50)).is_err());
}

#[test]
fn test_stake_slash() {
    let mut mgr = ReputationManager::new(Money::new(100));
    mgr.register_stake("a1", Money::new(100)).unwrap();
    let slashed = mgr.slash_stake("a1", Money::new(30)).unwrap();
    assert_eq!(slashed, Money::new(30));
    assert_eq!(mgr.stake("a1").unwrap().amount, Money::new(70));
}

#[test]
fn test_stake_fully_slashed() {
    let mut mgr = ReputationManager::new(Money::new(100));
    mgr.register_stake("a1", Money::new(100)).unwrap();
    mgr.slash_stake("a1", Money::new(100)).unwrap();
    assert_eq!(mgr.stake("a1").unwrap().status, StakeStatus::Slashed);
}

#[test]
fn test_eligibility_check() {
    let mut mgr = ReputationManager::new(Money::new(100));
    mgr.register_stake("a1", Money::new(100)).unwrap();
    // 初始信誉 0.5 ≥ 0.3
    assert!(mgr.is_eligible("a1", 0.3));
    assert!(!mgr.is_eligible("a1", 0.9));
}

#[test]
fn test_leaderboard() {
    let mut mgr = ReputationManager::new(Money::new(100));
    mgr.register_stake("a1", Money::new(100)).unwrap();
    mgr.register_stake("a2", Money::new(100)).unwrap();

    // a1 多次成功
    if let Some(rep) = mgr.reputation_mut("a1") {
        for _ in 0..10 {
            rep.record_call(true, 0.3);
        }
    }

    let board = mgr.leaderboard(10);
    assert_eq!(board.len(), 2);
    assert_eq!(board[0].0, "a1"); // a1 应该排第一
}

// ===== 证据分级测试 =====

#[test]
fn test_evidence_grade_labels() {
    assert_eq!(EvidenceGrade::Verified.label(), "verified");
    assert_eq!(EvidenceGrade::CpuProto.label(), "cpu-proto");
    assert_eq!(EvidenceGrade::Unverified.label(), "unverified");
}

#[test]
fn test_evidence_trustworthy() {
    assert!(EvidenceGrade::Verified.is_trustworthy());
    assert!(EvidenceGrade::CpuProto.is_trustworthy());
    assert!(!EvidenceGrade::Unverified.is_trustworthy());
}

// ===== 争议仲裁测试 =====

#[test]
fn test_open_dispute() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);
    match_and_accept(&mut market, "t1", "a1", 10);

    assert!(market.open_dispute("d1", "t1", "u1", "结果不合格").is_ok());
    assert_eq!(market.dispute_count(), 1);

    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Disputed);
}

#[test]
fn test_arbitration_guilty() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);
    match_and_accept(&mut market, "t1", "a1", 10);
    market.open_dispute("d1", "t1", "u1", "作弊").unwrap();

    // v2.8.5：仲裁者显式指定，罚没金额由服务端规则决定（不接受请求体）。
    let (verdict, slashed) = market.arbitrate("d1", "arb-1", true).unwrap();
    assert_eq!(verdict, "guilty");
    assert_eq!(slashed, Money::new(100), "仲裁作恶应罚没全部质押 100");

    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Slashed);

    // 作恶方不获报酬，托管预算全额退回需求方
    assert_eq!(market.balance("u1"), Money::new(50));
    assert_eq!(market.balance("__escrow__:t1"), Money::ZERO);
    // 仲裁作恶罚没全部质押：质押锁定账户清零
    assert_eq!(market.balance("__stake__:a1"), Money::ZERO);

    // 精确守恒：余额和 = 总充值 - 总罚没
    let report = market.conservation_check();
    assert!(report.conserved, "仲裁后守恒失败: {:?}", report);
    assert_eq!(report.total_slashed, Money::new(100));
}

#[test]
fn test_arbitration_not_guilty() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);
    match_and_accept(&mut market, "t1", "a1", 10);
    market.open_dispute("d1", "t1", "u1", "争议").unwrap();

    let (verdict, slashed) = market.arbitrate("d1", "arb-1", false).unwrap();
    assert_eq!(verdict, "not_guilty");
    assert_eq!(slashed, Money::ZERO);
}

// ===== 端到端流程测试 =====

#[test]
fn test_end_to_end_market_flow() {
    let mut market = AgentMarket::new();

    // 1. 充值并注册 Agent（质押 100）
    market.deposit("agent-1", Money::new(100)).unwrap();
    market
        .register_agent(make_agent_card("agent-1", 100))
        .unwrap();

    // 2. 充值需求方并发布任务（预算 50，发布即托管）
    market.deposit("requester-1", Money::new(50)).unwrap();
    market
        .publish_task(make_task("task-1", 50, "requester-1"))
        .unwrap();

    // 3. 投标（报价 10）
    market
        .submit_bid(make_bid("agent-1", "task-1", 10))
        .unwrap();

    // 4. 匹配
    let winner = market.match_task("task-1").unwrap();
    assert_eq!(winner, "agent-1");

    // 5. 提交结果
    let envelope = ResultEnvelope {
        task_id: "task-1".to_string(),
        agent_id: "agent-1".to_string(),
        report: r#"{"translated": "hello"}"#.to_string(),
        confidence: 0.95,
        error_type: ErrorType::None,
        trace_ref: "trace://t1/1".to_string(),
        evidence_grade: EvidenceGrade::CpuProto,
        latency_ms: 300,
    };
    market.submit_result(envelope).unwrap();

    // 6. QA 验证（3 票 Stop）
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("qa-{}", i));
    }
    committee.cast_vote("qa-0", QaVote::Stop).unwrap();
    committee.cast_vote("qa-1", QaVote::Stop).unwrap();
    committee.cast_vote("qa-2", QaVote::Stop).unwrap();
    committee.cast_vote("qa-3", QaVote::Continue).unwrap();

    let decision = market.verify_result("task-1", &committee).unwrap();
    assert_eq!(decision, QaDecision::Stop);

    // 7. 结算：按中标价 10 支付（min(10,50)），余款 40 退回
    let paid = market.settle_task("task-1").unwrap();
    assert_eq!(paid, Money::new(10));

    // 8. 验证终态
    let task = market.get_task("task-1").unwrap();
    assert_eq!(task.state, TaskState::Settled);
    assert_eq!(market.settled_count(), 1);

    // 9. 逐账户精确核对（无容差）
    // 质押锁定 100；执行者收款 10；需求方收回余款 40；托管账户清零
    assert_eq!(market.balance("__stake__:agent-1"), Money::new(100));
    assert_eq!(market.balance("agent-1"), Money::new(10));
    assert_eq!(market.balance("requester-1"), Money::new(40));
    assert_eq!(market.balance("__escrow__:task-1"), Money::ZERO);

    // 10. 守恒：总充值 150，无罚没，余额和精确等于 150
    let report = market.conservation_check();
    assert!(report.conserved, "端到端守恒失败: {:?}", report);
    assert_eq!(report.total_deposits, Money::new(150));
    assert_eq!(report.balance_sum, Money::new(150));
    assert_eq!(report.total_slashed, Money::ZERO);
}

#[test]
fn test_end_to_end_rework_flow() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    fund_and_publish(&mut market, make_task("t1", 50, "u1"), 50);
    market.submit_bid(make_bid("a1", "t1", 10)).unwrap();
    market.match_task("t1").unwrap();

    // 提交不合格结果
    let envelope = ResultEnvelope {
        task_id: "t1".to_string(),
        agent_id: "a1".to_string(),
        report: "{}".to_string(),
        confidence: 0.3,
        error_type: ErrorType::LowConfidence,
        trace_ref: "trace://t1/1".to_string(),
        evidence_grade: EvidenceGrade::CpuProto,
        latency_ms: 1000,
    };
    market.submit_result(envelope).unwrap();

    // QA 投票 Continue
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("m{}", i));
    }
    committee.cast_vote("m0", QaVote::Continue).unwrap();
    committee.cast_vote("m1", QaVote::Continue).unwrap();
    committee.cast_vote("m2", QaVote::Continue).unwrap();
    committee.cast_vote("m3", QaVote::Stop).unwrap();

    let decision = market.verify_result("t1", &committee).unwrap();
    assert_eq!(decision, QaDecision::Continue);

    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Rework);
}

// ===== v2.6.3：投标价校验 / Rejected / DuplicateWork =====

#[test]
fn test_bid_price_validation() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    market.deposit("u1", Money::new(50)).unwrap();
    market.publish_task(make_task("t1", 50, "u1")).unwrap();

    // 0 价拒绝
    assert!(market.submit_bid(make_bid("a1", "t1", 0)).is_err());
    // 负价拒绝
    assert!(market.submit_bid(make_bid("a1", "t1", -5)).is_err());
    // 超预算拒绝
    assert!(market.submit_bid(make_bid("a1", "t1", 60)).is_err());
    // 合法价接受
    assert!(market.submit_bid(make_bid("a1", "t1", 10)).is_ok());
}

fn failing_envelope(report: &str) -> ResultEnvelope {
    ResultEnvelope {
        task_id: "t1".to_string(),
        agent_id: "a1".to_string(),
        report: report.to_string(),
        confidence: 0.3,
        error_type: ErrorType::LowConfidence,
        trace_ref: "trace://t1/1".to_string(),
        evidence_grade: EvidenceGrade::CpuProto,
        latency_ms: 1000,
    }
}

#[test]
fn test_end_to_end_rejected_flow() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    market.deposit("u1", Money::new(50)).unwrap();
    market.publish_task(make_task("t1", 50, "u1")).unwrap();
    market.submit_bid(make_bid("a1", "t1", 10)).unwrap();
    market.match_task("t1").unwrap();

    // 提交失败结果（低置信度）→ Verifying
    market.submit_result(failing_envelope("{}")).unwrap();

    // 终局拒绝
    market.reject_task("t1").unwrap();
    assert_eq!(market.get_task("t1").unwrap().state, TaskState::Rejected);

    // 结算：付执行者 0
    let paid = market.settle_task("t1").unwrap();
    assert_eq!(paid, Money::ZERO);

    // 终态 + 逐账户核对
    assert_eq!(market.get_task("t1").unwrap().state, TaskState::Settled);
    assert_eq!(market.balance("u1"), Money::new(50));
    assert_eq!(market.balance("a1"), Money::ZERO);
    assert_eq!(market.balance("__escrow__:t1"), Money::ZERO);
    // 罚没 10% 质押：100 → 90
    assert_eq!(market.balance("__stake__:a1"), Money::new(90));

    // 守恒：总充值 150，罚没 10，余额和 140
    let report = market.conservation_check();
    assert!(report.conserved, "拒绝流程守恒失败: {:?}", report);
    assert_eq!(report.total_slashed, Money::new(10));
    assert_eq!(report.balance_sum, Money::new(140));

    // Rejected 原因确实进入流水（GAP §2.8：变体可达）
    assert!(market
        .settlement_records()
        .iter()
        .any(|r| r.reason == SettlementReason::Rejected));
}

#[test]
fn test_end_to_end_duplicate_work_flow() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "a1", 100);
    market.deposit("u1", Money::new(50)).unwrap();
    market.publish_task(make_task("t1", 50, "u1")).unwrap();
    market.submit_bid(make_bid("a1", "t1", 10)).unwrap();
    market.match_task("t1").unwrap();

    // 第一次提交结果 → Verifying
    market.submit_result(failing_envelope("SAME")).unwrap();

    // QA 投票 Continue（3 Continue + 1 Stop）→ Rework
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("m{}", i));
    }
    committee.cast_vote("m0", QaVote::Continue).unwrap();
    committee.cast_vote("m1", QaVote::Continue).unwrap();
    committee.cast_vote("m2", QaVote::Continue).unwrap();
    committee.cast_vote("m3", QaVote::Stop).unwrap();
    let decision = market.verify_result("t1", &committee).unwrap();
    assert_eq!(decision, QaDecision::Continue);

    // 返工 → 回到执行中
    market.resume_after_rework("t1").unwrap();

    // 返工后提交与上次完全相同的结果 = 重复劳动，自动转 Rejected
    market.submit_result(failing_envelope("SAME")).unwrap();
    assert_eq!(market.get_task("t1").unwrap().state, TaskState::Rejected);

    // 结算：DuplicateWork，付 0
    let paid = market.settle_task("t1").unwrap();
    assert_eq!(paid, Money::ZERO);
    assert!(market
        .settlement_records()
        .iter()
        .any(|r| r.reason == SettlementReason::DuplicateWork));

    // 预算全退、罚没 10% 质押、守恒
    assert_eq!(market.balance("u1"), Money::new(50));
    assert_eq!(market.balance("__stake__:a1"), Money::new(90));
    let report = market.conservation_check();
    assert!(report.conserved, "重复劳动流程守恒失败: {:?}", report);
}

// ===== TaskSpec validate 测试 =====

#[test]
fn test_task_spec_validate_valid() {
    let task = make_task("t1", 50, "u1");
    assert!(task.validate().is_ok());
}

#[test]
fn test_task_spec_multiple_gaps() {
    let mut task = make_task("t1", 50, "u1");
    task.goal = "".to_string();
    task.context = "".to_string();
    task.required_skills = vec![];

    let result = task.validate();
    assert!(result.is_err());
    let gaps = result.unwrap_err();
    assert!(gaps.len() >= 3);
}

// ===== ResultEnvelope 测试 =====

#[test]
fn test_result_envelope_success() {
    let envelope = ResultEnvelope {
        task_id: "t1".to_string(),
        agent_id: "a1".to_string(),
        report: "{}".to_string(),
        confidence: 0.9,
        error_type: ErrorType::None,
        trace_ref: "".to_string(),
        evidence_grade: EvidenceGrade::Verified,
        latency_ms: 100,
    };
    assert!(envelope.is_success());
}

#[test]
fn test_result_envelope_low_confidence_failure() {
    let envelope = ResultEnvelope {
        task_id: "t1".to_string(),
        agent_id: "a1".to_string(),
        report: "{}".to_string(),
        confidence: 0.3,
        error_type: ErrorType::None,
        trace_ref: "".to_string(),
        evidence_grade: EvidenceGrade::Unverified,
        latency_ms: 100,
    };
    assert!(!envelope.is_success());
}

// ===== TaskState 测试 =====

#[test]
fn test_task_state_terminal() {
    assert!(TaskState::Settled.is_terminal());
    assert!(TaskState::Slashed.is_terminal());
    assert!(!TaskState::Open.is_terminal());
    assert!(!TaskState::Running.is_terminal());
}

// ===== v2.5.8 跨语言金额向量：期望值为 conformance/money-vectors.json（单一来源）=====

#[test]
fn test_money_vector_matches_conformance() {
    let vector: serde_json::Value =
        serde_json::from_str(include_str!("../../conformance/money-vectors.json")).unwrap();
    let exp_bal = &vector["expected_balances"];
    let exp_rep = &vector["expected_report"];

    let mut market = AgentMarket::new();
    // worker：充值 100 并注册质押 100
    fund_and_register(&mut market, "agent-1", 100);
    // requester：充值 50
    market.deposit("requester-1", Money::new(50)).unwrap();
    // 发布任务（预算 50，发布即托管）
    market
        .publish_task(make_task("task-1", 50, "requester-1"))
        .unwrap();
    // 投标 10、匹配
    market
        .submit_bid(make_bid("agent-1", "task-1", 10))
        .unwrap();
    assert_eq!(market.match_task("task-1").unwrap(), "agent-1");
    // 提交结果
    market
        .submit_result(ResultEnvelope {
            task_id: "task-1".to_string(),
            agent_id: "agent-1".to_string(),
            report: r#"{"ok":true}"#.to_string(),
            confidence: 0.95,
            error_type: ErrorType::None,
            trace_ref: "trace://t1".to_string(),
            evidence_grade: EvidenceGrade::CpuProto,
            latency_ms: 300,
        })
        .unwrap();
    // QA 委员会（3 STOP 通过）
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("qa-{}", i));
    }
    committee.cast_vote("qa-0", QaVote::Stop).unwrap();
    committee.cast_vote("qa-1", QaVote::Stop).unwrap();
    committee.cast_vote("qa-2", QaVote::Stop).unwrap();
    committee.cast_vote("qa-3", QaVote::Continue).unwrap();
    assert_eq!(
        market.verify_result("task-1", &committee).unwrap(),
        QaDecision::Stop
    );
    // 结算
    let paid = market.settle_task("task-1").unwrap();
    assert_eq!(paid, Money::new(exp_rep["total_paid"].as_i64().unwrap()));

    // 逐账户：键与期望值全部来自 JSON，与 JS SDK 必须逐值一致
    for (key, val) in exp_bal.as_object().unwrap() {
        assert_eq!(
            market.balance(key),
            Money::new(val.as_i64().unwrap()),
            "账户 {key} 跨语言不一致"
        );
    }
    // 聚合守恒报告
    let report = market.conservation_check();
    assert_eq!(
        report.total_deposits,
        Money::new(exp_rep["total_deposits"].as_i64().unwrap())
    );
    assert_eq!(
        report.total_paid,
        Money::new(exp_rep["total_paid"].as_i64().unwrap())
    );
    assert_eq!(
        report.total_slashed,
        Money::new(exp_rep["total_slashed"].as_i64().unwrap())
    );
    assert_eq!(
        report.balance_sum,
        Money::new(exp_rep["balance_sum"].as_i64().unwrap())
    );
    assert_eq!(report.conserved, exp_rep["conserved"].as_bool().unwrap());
}

// ===== v2.6.0 状态机恢复边 + 证据分级结算闸门（GAP §3.4 / 证据谓词零调用点）=====

fn submit_envelope(market: &mut AgentMarket, task: &str, agent: &str, grade: EvidenceGrade) {
    market
        .submit_result(ResultEnvelope {
            task_id: task.to_string(),
            agent_id: agent.to_string(),
            report: r#"{"ok":true}"#.to_string(),
            confidence: 0.95,
            error_type: ErrorType::None,
            trace_ref: "trace://x".to_string(),
            evidence_grade: grade,
            latency_ms: 300,
        })
        .unwrap();
}

/// 3 票 Continue 的 QA 委员会（n=4,f=1 → Continue）
fn rework_committee() -> QaCommittee {
    let mut c = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        c.add_member(format!("q{}", i));
    }
    c.cast_vote("q0", QaVote::Continue).unwrap();
    c.cast_vote("q1", QaVote::Continue).unwrap();
    c.cast_vote("q2", QaVote::Continue).unwrap();
    c.cast_vote("q3", QaVote::Stop).unwrap();
    c
}

/// 3 票 Stop 的 QA 委员会（n=4,f=1 → Stop）
fn stop_committee(prefix: &str) -> QaCommittee {
    let mut c = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        c.add_member(format!("{}{}", prefix, i));
    }
    c.cast_vote(&format!("{}0", prefix), QaVote::Stop).unwrap();
    c.cast_vote(&format!("{}1", prefix), QaVote::Stop).unwrap();
    c.cast_vote(&format!("{}2", prefix), QaVote::Stop).unwrap();
    c.cast_vote(&format!("{}3", prefix), QaVote::Continue)
        .unwrap();
    c
}

#[test]
fn test_state_transition_table() {
    // 两条恢复边
    assert!(TaskState::Rework.can_transition_to(TaskState::Running));
    assert!(TaskState::NoQuorum.can_transition_to(TaskState::Open));
    // 主生命周期
    assert!(TaskState::Draft.can_transition_to(TaskState::Open));
    assert!(TaskState::Open.can_transition_to(TaskState::Matched));
    assert!(TaskState::Matched.can_transition_to(TaskState::Verifying));
    assert!(TaskState::Running.can_transition_to(TaskState::Verifying));
    assert!(TaskState::Verifying.can_transition_to(TaskState::Accepted));
    assert!(TaskState::Accepted.can_transition_to(TaskState::Settled));
    // 终态无出边、跨级非法
    assert!(!TaskState::Settled.can_transition_to(TaskState::Open));
    assert!(!TaskState::Open.can_transition_to(TaskState::Settled));
    assert!(TaskState::Open.transition(TaskState::Settled).is_err());
    // 合法 transition 改变状态
    assert_eq!(
        TaskState::Rework.transition(TaskState::Running).unwrap(),
        TaskState::Running
    );
    assert_eq!(
        TaskState::NoQuorum.transition(TaskState::Open).unwrap(),
        TaskState::Open
    );
}

#[test]
fn test_resume_after_rework() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "agent-1", 100);
    market.deposit("requester-1", Money::new(50)).unwrap();
    market
        .publish_task(make_task("task-1", 50, "requester-1"))
        .unwrap();
    market
        .submit_bid(make_bid("agent-1", "task-1", 10))
        .unwrap();
    market.match_task("task-1").unwrap();
    submit_envelope(&mut market, "task-1", "agent-1", EvidenceGrade::CpuProto);

    // QA 判 Continue → Rework
    let committee = rework_committee();
    assert_eq!(
        market.verify_result("task-1", &committee).unwrap(),
        QaDecision::Continue
    );
    assert_eq!(market.get_task("task-1").unwrap().state, TaskState::Rework);

    // 恢复边：Rework → Running
    market.resume_after_rework("task-1").unwrap();
    assert_eq!(market.get_task("task-1").unwrap().state, TaskState::Running);
    // 已在 Running，再次 resume 非法（转移表无 Running→Running）
    assert!(market.resume_after_rework("task-1").is_err());
    // 不存在任务报错
    assert!(market.resume_after_rework("nope").is_err());
}

#[test]
fn test_reopen_after_no_quorum_then_resettle() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "agent-1", 100);
    market.deposit("requester-1", Money::new(50)).unwrap();
    market
        .publish_task(make_task("task-1", 50, "requester-1"))
        .unwrap();
    market
        .submit_bid(make_bid("agent-1", "task-1", 10))
        .unwrap();
    market.match_task("task-1").unwrap();
    submit_envelope(&mut market, "task-1", "agent-1", EvidenceGrade::CpuProto);

    // 仅 1 票（< quorum=3）→ NoQuorum
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 0..4 {
        committee.add_member(format!("q{}", i));
    }
    committee.cast_vote("q0", QaVote::Stop).unwrap();
    assert_eq!(
        market.verify_result("task-1", &committee).unwrap(),
        QaDecision::NoQuorum
    );
    assert_eq!(
        market.get_task("task-1").unwrap().state,
        TaskState::NoQuorum
    );

    // 恢复边：NoQuorum → Open，消除吸收态
    market.reopen_after_no_quorum("task-1").unwrap();
    assert_eq!(market.get_task("task-1").unwrap().state, TaskState::Open);
    // 非 NoQuorum 状态不能 reopen
    assert!(market.reopen_after_no_quorum("task-1").is_err());

    // reopen 后重新走全链路：匹配 → 提交 → 验收 → 结算
    market.match_task("task-1").unwrap();
    submit_envelope(&mut market, "task-1", "agent-1", EvidenceGrade::CpuProto);
    let c2 = stop_committee("r");
    assert_eq!(
        market.verify_result("task-1", &c2).unwrap(),
        QaDecision::Stop
    );
    let paid = market.settle_task("task-1").unwrap();
    assert_eq!(paid, Money::new(10));
    assert_eq!(market.get_task("task-1").unwrap().state, TaskState::Settled);
}

#[test]
fn test_settle_rejected_for_unverified_evidence() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "agent-1", 100);
    market.deposit("requester-1", Money::new(50)).unwrap();
    market
        .publish_task(make_task("task-1", 50, "requester-1"))
        .unwrap();
    market
        .submit_bid(make_bid("agent-1", "task-1", 10))
        .unwrap();
    market.match_task("task-1").unwrap();
    // 结果证据 Unverified
    submit_envelope(&mut market, "task-1", "agent-1", EvidenceGrade::Unverified);

    // 验收仍可 Stop（票数决定），任务进入 Accepted
    let committee = stop_committee("q");
    assert_eq!(
        market.verify_result("task-1", &committee).unwrap(),
        QaDecision::Stop
    );
    assert_eq!(
        market.get_task("task-1").unwrap().state,
        TaskState::Accepted
    );

    // 证据闸门：Unverified 拒绝结算付款
    let err = market.settle_task("task-1").unwrap_err();
    assert!(err.contains("不可信"), "实际错误：{err}");
    // 未付款、任务状态仍是 Accepted
    assert_eq!(
        market.get_task("task-1").unwrap().state,
        TaskState::Accepted
    );
}

#[test]
fn test_settle_none_policy_exempts_evidence_gate() {
    let mut market = AgentMarket::new();
    fund_and_register(&mut market, "agent-1", 100);
    market.deposit("requester-1", Money::new(50)).unwrap();
    let mut task = make_task("task-1", 50, "requester-1");
    task.verification_policy = VerificationPolicy::None;
    market.publish_task(task).unwrap();
    market
        .submit_bid(make_bid("agent-1", "task-1", 10))
        .unwrap();
    market.match_task("task-1").unwrap();

    // policy None：提交结果即验收（无需 QA 委员会）；证据即使 Unverified，门禁也豁免
    submit_envelope(&mut market, "task-1", "agent-1", EvidenceGrade::Unverified);
    assert_eq!(
        market.get_task("task-1").unwrap().state,
        TaskState::Accepted
    );
    // 无 QA、证据豁免 → 结算成功
    let paid = market.settle_task("task-1").unwrap();
    assert_eq!(paid, Money::new(10));
    assert_eq!(market.get_task("task-1").unwrap().state, TaskState::Settled);
}
