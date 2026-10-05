//! Agent Market v2.8.5 测试（GAP §3.5 REST / §3.7 / §3.8 / §2.2.6）
//!
//! 覆盖：
//! - 仲裁必须显式指定仲裁者（拒绝匿名）
//! - 仲裁罚没金额由服务端规则决定（作恶罚全部质押，不接受请求体指定）
//! - 重复仲裁被拒绝
//! - 状态变更只经状态转换表：Open 状态 / 终态 Settled 不能发起争议
//! - 终局拒绝 reject_task 经 transition（与 REST/MCP 暴露对应）
//! - 仲裁作恶罚没后预算退还、守恒成立
//!
//! 注：HTTP 层认证（rest_authorize）与 CORS（compute_cors）是 node.rs
//! 私有函数，在 node.rs 内 `#[cfg(test)]` 单元测试中覆盖。

use gsn_core::marketplace::*;

#[path = "common/mod.rs"]
mod helpers;

// ── 辅助函数（本文件独立，与 v234 同构） ──

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

fn make_task(id: &str, budget: i64, requester: &str, policy: VerificationPolicy) -> TaskSpec {
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
        verification_policy: policy,
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

fn ok_envelope(task: &str, agent: &str) -> ResultEnvelope {
    ResultEnvelope {
        task_id: task.to_string(),
        agent_id: agent.to_string(),
        report: "translated result".to_string(),
        confidence: 0.9,
        error_type: ErrorType::None,
        trace_ref: "trace://t1/1".to_string(),
        evidence_grade: EvidenceGrade::CpuProto,
        latency_ms: 100,
        pocv: None,
    }
}

fn weak_envelope(task: &str, agent: &str) -> ResultEnvelope {
    ResultEnvelope {
        task_id: task.to_string(),
        agent_id: agent.to_string(),
        report: "low confidence result".to_string(),
        confidence: 0.3,
        error_type: ErrorType::LowConfidence,
        trace_ref: "trace://t1/1".to_string(),
        evidence_grade: EvidenceGrade::Unverified,
        latency_ms: 1000,
        pocv: None,
    }
}

/// 充值质押金并注册
fn fund_and_register(m: &mut AgentMarket, id: &str, stake: i64) {
    helpers::deposit(m, id, Money::new(stake));
    m.register_agent(make_agent_card(id, stake)).unwrap();
}

/// 充值预算并发布
fn fund_and_publish(m: &mut AgentMarket, task: TaskSpec, budget: i64) {
    let requester = task.requester.clone();
    helpers::deposit(m, &requester, Money::new(budget));
    m.publish_task(task).unwrap();
}

/// 投标 → 匹配 → 提交成功结果（policy None 直接 Accepted）
fn match_and_accept(m: &mut AgentMarket, task: &str, agent: &str, price: i64) {
    m.submit_bid(make_bid(agent, task, price)).unwrap();
    m.match_task(task).unwrap();
    m.submit_result(ok_envelope(task, agent)).unwrap();
}

/// 构造一个已进入 Disputed 的任务（共同前置）
fn setup_disputed(m: &mut AgentMarket) {
    fund_and_register(m, "a1", 100);
    fund_and_publish(m, make_task("t1", 50, "u1", VerificationPolicy::None), 50);
    match_and_accept(m, "t1", "a1", 10);
    m.open_dispute("d1", "t1", "u1", "质量不达标").unwrap();
}

// ===== 1. 仲裁必须有受权治理签名信封（P0-4：fail-closed，拒绝自报） =====

#[test]
fn test_arbitrate_requires_governance_signature() {
    let mut m = AgentMarket::new();
    setup_disputed(&mut m);
    // 显式清空治理集（helpers::deposit 已注入 faucet）→ fail-closed
    m.set_governance(gsn_core::marketplace::Governance::empty());

    // 空治理集 → 任何仲裁命令都被 fail-closed 拒绝
    let err = helpers::arbitrate_signed_by_stranger(&mut m, "d1", true).unwrap_err();
    assert!(err.contains("NO_MEMBERS"), "错误信息: {err}");

    // 注入治理集后，受权成员签名命令 → 成功
    let (verdict, _) = helpers::arbitrate(&mut m, "d1", true);
    assert_eq!(verdict, "guilty");
}

// ===== 2. 仲裁作恶：罚没全部质押（服务端规则） =====

#[test]
fn test_arbitration_guilty_slash_all_server_decided() {
    let mut m = AgentMarket::new();
    setup_disputed(&mut m);

    let (verdict, slashed) = helpers::arbitrate(&mut m, "d1", true);
    assert_eq!(verdict, "guilty");
    // 罚没全部质押 100（由服务端 SLASH_RATE_ARBITRATION 规则决定，
    // 调用方无法通过请求体指定 slash_amount）
    assert_eq!(slashed, Money::new(100));
    assert_eq!(m.get_task("t1").unwrap().state, TaskState::Slashed);

    // 预算全额退还需求方
    assert_eq!(m.balance("u1"), Money::new(50));
    assert_eq!(m.balance("__escrow__:t1"), Money::ZERO);
    // 质押全部罚没
    assert_eq!(m.balance("__stake__:a1"), Money::ZERO);

    // 守恒：总充值 150，罚没 100，余额和 50
    let r = m.conservation_check();
    assert!(r.conserved, "作恶仲裁守恒失败: {:?}", r);
    assert_eq!(r.total_slashed, Money::new(100));
    assert_eq!(r.balance_sum, Money::new(50));
}

// ===== 3. 重复仲裁被拒绝 =====

#[test]
fn test_duplicate_arbitration_rejected() {
    let mut m = AgentMarket::new();
    setup_disputed(&mut m);

    helpers::arbitrate(&mut m, "d1", false);
    // 已仲裁 → 不能再次仲裁（即使换仲裁者/改结论）
    let err = helpers::arbitrate_result(&mut m, "d1", true).unwrap_err();
    assert!(err.contains("已仲裁"), "错误信息应指明已仲裁: {err}");
}

// ===== 4. 仲裁无争议 / 不存在 → NOT_FOUND =====

#[test]
fn test_arbitration_missing_dispute_not_found() {
    let mut m = AgentMarket::new();
    helpers::grant_governance(&mut m);
    // 受权成员签名命令打不存在的争议 → NOT_FOUND（治理校验已过）
    let err = helpers::arbitrate_result(&mut m, "nope", true).unwrap_err();
    assert!(err.contains("NOT_FOUND"), "错误信息应指明 NOT_FOUND: {err}");
}

// ===== 5. 仲裁无过：不罚没，任务回到 Accepted =====

#[test]
fn test_arbitration_not_guilty_no_slash() {
    let mut m = AgentMarket::new();
    setup_disputed(&mut m);

    let (verdict, slashed) = helpers::arbitrate(&mut m, "d1", false);
    assert_eq!(verdict, "not_guilty");
    assert_eq!(slashed, Money::ZERO);
    assert_eq!(m.get_task("t1").unwrap().state, TaskState::Accepted);
    // 质押不受影响
    assert_eq!(m.balance("__stake__:a1"), Money::new(100));
}

// ===== 6. Open 状态不能发起争议（状态转换表） =====

#[test]
fn test_open_state_cannot_dispute() {
    let mut m = AgentMarket::new();
    fund_and_register(&mut m, "a1", 100);
    fund_and_publish(
        &mut m,
        make_task("t1", 50, "u1", VerificationPolicy::None),
        50,
    );

    // 刚发布，Open 状态
    assert_eq!(m.get_task("t1").unwrap().state, TaskState::Open);
    let err = m.open_dispute("d1", "t1", "u1", "理由").unwrap_err();
    assert!(err.contains("非法"), "错误信息应指明非法状态: {}", err);
}

// ===== 7. 终态 Settled 不能发起争议 =====

#[test]
fn test_settled_state_cannot_dispute() {
    let mut m = AgentMarket::new();
    fund_and_register(&mut m, "a1", 100);
    fund_and_publish(
        &mut m,
        make_task("t1", 50, "u1", VerificationPolicy::None),
        50,
    );
    match_and_accept(&mut m, "t1", "a1", 10);
    m.settle_task("t1").unwrap();
    assert_eq!(m.get_task("t1").unwrap().state, TaskState::Settled);

    // 终态不能改、不能争议
    let err = m.open_dispute("d1", "t1", "u1", "理由").unwrap_err();
    assert!(err.contains("非法"), "错误信息应指明非法状态: {}", err);
}

// ===== 8. reject_task 经 transition，settle 罚 10% 质押 =====

#[test]
fn test_reject_task_transition_and_slash_rate() {
    let mut m = AgentMarket::new();
    fund_and_register(&mut m, "a1", 100);
    fund_and_publish(
        &mut m,
        make_task("t1", 50, "u1", VerificationPolicy::BftLite { n: 4, f: 1 }),
        50,
    );
    m.submit_bid(make_bid("a1", "t1", 10)).unwrap();
    m.match_task("t1").unwrap();

    // 提交低置信结果 → Verifying
    m.submit_result(weak_envelope("t1", "a1")).unwrap();
    assert_eq!(m.get_task("t1").unwrap().state, TaskState::Verifying);

    // 终局拒绝 → Rejected
    m.reject_task("t1").unwrap();
    assert_eq!(m.get_task("t1").unwrap().state, TaskState::Rejected);

    // 结算：付 0、退预算、罚 10% 质押（100 → 90）
    let paid = m.settle_task("t1").unwrap();
    assert_eq!(paid, Money::ZERO);
    assert_eq!(m.balance("u1"), Money::new(50));
    assert_eq!(m.balance("__stake__:a1"), Money::new(90));

    // 守恒：总充值 150，罚没 10，余额和 140
    let r = m.conservation_check();
    assert!(r.conserved, "拒绝流程守恒失败: {:?}", r);
    assert_eq!(r.total_slashed, Money::new(10));
}
