//! Agent Market v2.3.4 测试
//!
//! 覆盖：
//! - Agent 注册 / 重复 / 非法字段 / 质押不足
//! - 任务发布 / 六字段校验
//! - 发现与匹配
//! - BFT-lite QA 委员会
//! - 结算守恒 / 防重复支付
//! - 信誉与质押罚没
//! - 争议仲裁
//! - 端到端流程

use gsn_core::marketplace::*;

// ===== 辅助函数 =====

fn make_agent_card(id: &str, stake: f64) -> MarketAgentCard {
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
            price: 10.0,
            currency: Currency::Credit,
        },
        sla: Sla::default(),
        owner: format!("owner-{}", id),
        stake,
        reputation_score: 0.5,
        total_calls: 0,
        success_rate: 0.0,
        evidence_grade: EvidenceGrade::CpuProto,
        verified: false,
        created_at: 1000,
        updated_at: 1000,
    }
}

fn make_task(id: &str, budget: f64, requester: &str) -> TaskSpec {
    TaskSpec {
        task_id: id.to_string(),
        goal: "翻译一段文字".to_string(),
        context: "中译英".to_string(),
        done: vec![],
        todo: vec!["translate".to_string()],
        trace: vec![],
        owner: None,
        budget,
        deadline: 9999999999,
        required_skills: vec!["translation".to_string()],
        verification_policy: VerificationPolicy::BftLite { n: 4, f: 1 },
        requester: requester.to_string(),
        state: TaskState::Draft,
        created_at: 1000,
    }
}

// ===== F1: Agent 注册测试 =====

#[test]
fn test_register_agent_success() {
    let mut market = AgentMarket::new();
    let card = make_agent_card("agent-1", 100.0);
    let result = market.register_agent(card);
    assert!(result.is_ok());
    assert_eq!(market.agent_count(), 1);
}

#[test]
fn test_register_agent_insufficient_stake() {
    let mut market = AgentMarket::new();
    let card = make_agent_card("agent-1", 50.0); // 最低 100
    let result = market.register_agent(card);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("质押不足"));
}

#[test]
fn test_register_agent_empty_id() {
    let mut market = AgentMarket::new();
    let mut card = make_agent_card("agent-1", 100.0);
    card.agent_id = "".to_string();
    let result = market.register_agent(card);
    assert!(result.is_err());
}

#[test]
fn test_register_agent_empty_name() {
    let mut market = AgentMarket::new();
    let mut card = make_agent_card("agent-1", 100.0);
    card.name = "".to_string();
    let result = market.register_agent(card);
    assert!(result.is_err());
}

#[test]
fn test_register_agent_duplicate() {
    let mut market = AgentMarket::new();
    let card1 = make_agent_card("agent-1", 100.0);
    let card2 = make_agent_card("agent-1", 100.0);
    assert!(market.register_agent(card1).is_ok());
    // 重复注册会覆盖（HashMap 行为），但不报错
    assert!(market.register_agent(card2).is_ok());
    assert_eq!(market.agent_count(), 1);
}

#[test]
fn test_skill_index_created() {
    let mut market = AgentMarket::new();
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();
    market.register_agent(make_agent_card("a2", 100.0)).unwrap();

    let found = market.discover_by_skill("translation");
    assert_eq!(found.len(), 2);
}

// ===== F2: 任务发布测试 =====

#[test]
fn test_publish_task_success() {
    let mut market = AgentMarket::new();
    let task = make_task("task-1", 50.0, "user-1");
    let result = market.publish_task(task);
    assert!(result.is_ok());
    assert_eq!(market.task_count(), 1);
}

#[test]
fn test_publish_task_empty_goal() {
    let mut market = AgentMarket::new();
    let mut task = make_task("task-1", 50.0, "user-1");
    task.goal = "".to_string();
    let result = market.publish_task(task);
    assert!(result.is_err());
    let gaps = result.unwrap_err();
    assert!(gaps.iter().any(|g| g.contains("goal")));
}

#[test]
fn test_publish_task_empty_todo() {
    let mut market = AgentMarket::new();
    let mut task = make_task("task-1", 50.0, "user-1");
    task.todo = vec![];
    let result = market.publish_task(task);
    assert!(result.is_err());
}

#[test]
fn test_publish_task_zero_budget() {
    let mut market = AgentMarket::new();
    let task = make_task("task-1", 0.0, "user-1");
    let result = market.publish_task(task);
    assert!(result.is_err());
}

#[test]
fn test_task_state_open_after_publish() {
    let mut market = AgentMarket::new();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();
    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Open);
}

// ===== F3: 发现与匹配测试 =====

#[test]
fn test_discover_by_skill_case_insensitive() {
    let mut market = AgentMarket::new();
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();

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
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();
    let found = market.search_agents("Agent a1");
    assert_eq!(found.len(), 1);
}

#[test]
fn test_submit_bid_success() {
    let mut market = AgentMarket::new();
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();

    let bid = Bid {
        agent_id: "a1".to_string(),
        task_id: "t1".to_string(),
        proposed_price: 10.0,
        estimated_latency_ms: 500,
        score: 0.0,
    };
    assert!(market.submit_bid(bid).is_ok());
}

#[test]
fn test_submit_bid_unregistered_agent() {
    let mut market = AgentMarket::new();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();

    let bid = Bid {
        agent_id: "fake".to_string(),
        task_id: "t1".to_string(),
        proposed_price: 10.0,
        estimated_latency_ms: 500,
        score: 0.0,
    };
    assert!(market.submit_bid(bid).is_err());
}

#[test]
fn test_match_task_selects_winner() {
    let mut market = AgentMarket::new();
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();

    market
        .submit_bid(Bid {
            agent_id: "a1".to_string(),
            task_id: "t1".to_string(),
            proposed_price: 10.0,
            estimated_latency_ms: 500,
            score: 0.0,
        })
        .unwrap();

    let winner = market.match_task("t1").unwrap();
    assert_eq!(winner, "a1");

    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Matched);
    assert_eq!(task.owner, Some("a1".to_string()));
}

#[test]
fn test_match_task_no_bids() {
    let mut market = AgentMarket::new();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();
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
    // 只有 1 票 Stop，2 票沉默（沉默 > f=1）
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

// ===== 结算守恒测试 =====

#[test]
fn test_settlement_deposit_and_balance() {
    let mut engine = SettlementEngine::new();
    engine.deposit("u1", 100.0);
    assert_eq!(engine.balance("u1"), 100.0);
}

#[test]
fn test_settlement_completed() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", 100.0);
    let paid = engine
        .settle("t1", "payer", "payee", 50.0, SettlementReason::Completed)
        .unwrap();
    assert_eq!(paid, 50.0);
    assert_eq!(engine.balance("payer"), 50.0);
    assert_eq!(engine.balance("payee"), 50.0);
}

#[test]
fn test_settlement_duplicate_payment_prevented() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", 100.0);

    engine
        .settle("t1", "payer", "payee", 50.0, SettlementReason::Completed)
        .unwrap();
    // 第二次结算同一任务 → 付 0
    let paid = engine
        .settle("t1", "payer", "payee", 50.0, SettlementReason::Completed)
        .unwrap();
    assert_eq!(paid, 0.0);
    assert_eq!(engine.balance("payee"), 50.0); // 没有多付
}

#[test]
fn test_settlement_rejected_pays_zero() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", 100.0);
    let paid = engine
        .settle("t1", "payer", "payee", 50.0, SettlementReason::Rejected)
        .unwrap();
    assert_eq!(paid, 0.0);
}

#[test]
fn test_settlement_insufficient_balance() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", 10.0);
    let result = engine.settle("t1", "payer", "payee", 50.0, SettlementReason::Completed);
    assert!(result.is_err());
}

#[test]
fn test_settlement_slash() {
    let mut engine = SettlementEngine::new();
    engine.deposit("agent", 100.0);
    let slashed = engine.slash("agent", 30.0).unwrap();
    assert_eq!(slashed, 30.0);
    assert_eq!(engine.balance("agent"), 70.0);
}

#[test]
fn test_settlement_slash_insufficient() {
    let mut engine = SettlementEngine::new();
    engine.deposit("agent", 10.0);
    let result = engine.slash("agent", 50.0);
    assert!(result.is_err());
}

#[test]
fn test_conservation_check_valid() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", 100.0);
    engine
        .settle("t1", "payer", "payee", 40.0, SettlementReason::Completed)
        .unwrap();

    let report = engine.conservation_check();
    assert!(report.conserved, "守恒检查失败: {:?}", report);
    assert_eq!(report.total_budget, 100.0);
    assert_eq!(report.total_paid, 40.0);
}

#[test]
fn test_conservation_with_slash() {
    let mut engine = SettlementEngine::new();
    engine.deposit("payer", 100.0);
    engine
        .settle("t1", "payer", "payee", 40.0, SettlementReason::Completed)
        .unwrap();
    engine.slash("payee", 10.0).unwrap();

    let report = engine.conservation_check();
    assert!(report.conserved, "守恒检查失败: {:?}", report);
    assert_eq!(report.total_slashed, 10.0);
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
    let mut mgr = ReputationManager::new(100.0);
    assert!(mgr.register_stake("a1", 100.0).is_ok());
}

#[test]
fn test_stake_below_minimum() {
    let mut mgr = ReputationManager::new(100.0);
    assert!(mgr.register_stake("a1", 50.0).is_err());
}

#[test]
fn test_stake_slash() {
    let mut mgr = ReputationManager::new(100.0);
    mgr.register_stake("a1", 100.0).unwrap();
    let slashed = mgr.slash_stake("a1", 30.0).unwrap();
    assert_eq!(slashed, 30.0);
    assert_eq!(mgr.stake("a1").unwrap().amount, 70.0);
}

#[test]
fn test_stake_fully_slashed() {
    let mut mgr = ReputationManager::new(100.0);
    mgr.register_stake("a1", 100.0).unwrap();
    mgr.slash_stake("a1", 100.0).unwrap();
    assert_eq!(mgr.stake("a1").unwrap().status, StakeStatus::Slashed);
}

#[test]
fn test_eligibility_check() {
    let mut mgr = ReputationManager::new(100.0);
    mgr.register_stake("a1", 100.0).unwrap();
    // 初始信誉 0.5 ≥ 0.3
    assert!(mgr.is_eligible("a1", 0.3));
    assert!(!mgr.is_eligible("a1", 0.9));
}

#[test]
fn test_leaderboard() {
    let mut mgr = ReputationManager::new(100.0);
    mgr.register_stake("a1", 100.0).unwrap();
    mgr.register_stake("a2", 100.0).unwrap();

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
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();

    assert!(market.open_dispute("d1", "t1", "u1", "结果不合格").is_ok());
    assert_eq!(market.dispute_count(), 1);

    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Disputed);
}

#[test]
fn test_arbitration_guilty() {
    let mut market = AgentMarket::new();
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();
    market
        .submit_bid(Bid {
            agent_id: "a1".to_string(),
            task_id: "t1".to_string(),
            proposed_price: 10.0,
            estimated_latency_ms: 500,
            score: 0.0,
        })
        .unwrap();
    market.match_task("t1").unwrap();
    market.open_dispute("d1", "t1", "u1", "作弊").unwrap();

    let verdict = market.arbitrate("d1", true, 50.0).unwrap();
    assert_eq!(verdict, "guilty");

    let task = market.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Slashed);
}

#[test]
fn test_arbitration_not_guilty() {
    let mut market = AgentMarket::new();
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();
    market.open_dispute("d1", "t1", "u1", "争议").unwrap();

    let verdict = market.arbitrate("d1", false, 0.0).unwrap();
    assert_eq!(verdict, "not_guilty");
}

// ===== 端到端流程测试 =====

#[test]
fn test_end_to_end_market_flow() {
    let mut market = AgentMarket::new();

    // 1. 注册 Agent
    market.register_agent(make_agent_card("agent-1", 100.0)).unwrap();

    // 2. 发布任务
    market.publish_task(make_task("task-1", 50.0, "requester-1")).unwrap();

    // 3. 投标
    market
        .submit_bid(Bid {
            agent_id: "agent-1".to_string(),
            task_id: "task-1".to_string(),
            proposed_price: 10.0,
            estimated_latency_ms: 500,
            score: 0.0,
        })
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

    // 6. QA 验证
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

    // 7. 结算
    let paid = market.settle_task("task-1").unwrap();
    assert_eq!(paid, 50.0);

    // 8. 验证终态
    let task = market.get_task("task-1").unwrap();
    assert_eq!(task.state, TaskState::Settled);
    assert_eq!(market.settled_count(), 1);

    // 9. 守恒检查
    let report = market.conservation_check();
    assert!(report.conserved, "端到端守恒失败: {:?}", report);
}

#[test]
fn test_end_to_end_rework_flow() {
    let mut market = AgentMarket::new();
    market.register_agent(make_agent_card("a1", 100.0)).unwrap();
    market.publish_task(make_task("t1", 50.0, "u1")).unwrap();
    market
        .submit_bid(Bid {
            agent_id: "a1".to_string(),
            task_id: "t1".to_string(),
            proposed_price: 10.0,
            estimated_latency_ms: 500,
            score: 0.0,
        })
        .unwrap();
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

// ===== TaskSpec validate 测试 =====

#[test]
fn test_task_spec_validate_valid() {
    let task = make_task("t1", 50.0, "u1");
    assert!(task.validate().is_ok());
}

#[test]
fn test_task_spec_multiple_gaps() {
    let mut task = make_task("t1", 50.0, "u1");
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
