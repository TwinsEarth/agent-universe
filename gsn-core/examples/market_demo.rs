//! Agent Market 端到端真实运行演示
//!
//! 运行：cargo run --example market_demo --release
//!
//! 完整演示：充值 → 注册 → 发布 → 投标 → 匹配 → 执行 → BFT验证 → 结算 → 信誉
//! 以及：作恶 → 争议 → 仲裁 → 罚没
//!
//! v2.5.8：金额为精确整数（Money），发布即托管预算，注册即锁定质押，
//! 付款方余额不足一律拒绝（不凭空铸币），守恒为精确相等。

use gsn_core::identity::{Did, Keypair};
use gsn_core::marketplace::{
    AgentMarket, Bid, Currency, ErrorType, EvidenceGrade, Governance, MarketAgentCard, Money,
    Pricing, PricingModel, QaCommittee, QaVote, ResultEnvelope, SignedGovernanceCommand, Sla,
    TaskSpec, TaskState, VerificationPolicy, GOV_CAP_ARBITRATE, GOV_CAP_CREDIT,
};

/// 演示用 faucet 治理成员（dev/faucet：生产授信须链上凭证，未验证）。
fn faucet() -> (String, Keypair) {
    let mut seed = [0u8; 32];
    seed[0] = 7;
    let kp = Keypair::from_seed(&seed);
    (Did::from_public_key(kp.public_key()).to_string(), kp)
}

/// dev/faucet 授信（治理签名命令）。
fn deposit(market: &mut AgentMarket, account: &str, amount: Money) {
    let (did, kp) = faucet();
    let pk: [u8; 32] = kp.public_key().try_into().unwrap();
    market.set_governance(Governance::from_members(vec![(did.clone(), pk)]));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let cmd = SignedGovernanceCommand::sign(
        "demo",
        &did,
        &kp,
        GOV_CAP_CREDIT,
        account,
        serde_json::json!({"amount": amount.as_i64()}),
        "demo-nonce",
        now - 10,
        300,
    );
    market.deposit_signed(&cmd, now).unwrap();
}

fn make_agent(id: &str, name: &str, skill: &str, price: i64, stake: i64) -> MarketAgentCard {
    MarketAgentCard {
        agent_id: id.to_string(),
        version: "1.0.0".to_string(),
        name: name.to_string(),
        description: format!("{} 专家，擅长 {}", name, skill),
        skills: vec![skill.to_string()],
        modalities: vec!["text".to_string()],
        models: vec!["model-base".to_string()],
        endpoint: format!("a2a://{}", id),
        pricing: Pricing {
            model: PricingModel::PerCall,
            price: Money::new(price),
            currency: Currency::Credit,
        },
        sla: Sla::default(),
        owner: format!("owner-{}", id),
        stake: Money::new(stake),
        reputation_score: 0.5,
        total_calls: 0,
        success_rate: 1.0,
        evidence_grade: EvidenceGrade::CpuProto,
        verified: false,
        created_at: 1000,
        updated_at: 1000,
    }
}

fn make_task(id: &str, goal: &str, skill: &str, budget: i64, requester: &str) -> TaskSpec {
    TaskSpec {
        task_id: id.to_string(),
        goal: goal.to_string(),
        context: "教育场景作业辅导".to_string(),
        done: vec![],
        todo: vec!["分析".to_string(), "生成".to_string()],
        trace: vec![],
        owner: None,
        budget: Money::new(budget),
        winner_price: None,
        deadline: 2_000_000_000,
        required_skills: vec![skill.to_string()],
        verification_policy: VerificationPolicy::BftLite { n: 4, f: 1 },
        requester: requester.to_string(),
        state: TaskState::Draft,
        created_at: 1000,
    }
}

fn line(c: char) {
    println!("{}", c.to_string().repeat(64));
}

fn main() {
    line('=');
    println!("  Agent Universe · 智能体市场端到端运行演示");
    line('=');

    let mut market = AgentMarket::new();

    // ===== 步骤 1：充值并注册 3 个 Agent =====
    println!("\n【步骤 1】充值质押金并注册 Agent（质押准入，最低 100）");
    deposit(&mut market, "agent-writer", Money::new(100));
    deposit(&mut market, "agent-pro", Money::new(150));
    deposit(&mut market, "agent-data", Money::new(200));
    market
        .register_agent(make_agent("agent-writer", "文案师", "writing", 10, 100))
        .unwrap();
    market
        .register_agent(make_agent("agent-pro", "高级文案", "writing", 8, 150))
        .unwrap();
    market
        .register_agent(make_agent("agent-data", "数据师", "analysis", 20, 200))
        .unwrap();
    println!("  已注册 Agent 数量：{}", market.agent_count());

    // 验证：质押不足被拒绝（钱仍留在自有账户，未锁定）
    deposit(&mut market, "agent-bad", Money::new(50));
    let bad = make_agent("agent-bad", "低质押", "writing", 5, 50);
    match market.register_agent(bad) {
        Ok(_) => println!("  [异常] 低质押应被拒绝！"),
        Err(e) => println!("  防护验证：低质押注册被拒绝 → {}", e),
    }

    // ===== 步骤 2：技能发现 =====
    println!("\n【步骤 2】按技能发现 Agent");
    let writers = market.discover_by_skill("writing");
    println!("  技能 'writing' 下发现 {} 个 Agent：", writers.len());
    for a in writers {
        println!(
            "    - {}（报价 {}，质押 {}）",
            a.name, a.pricing.price, a.stake
        );
    }

    // ===== 步骤 3：需求方充值 =====
    println!("\n【步骤 3】需求方充值");
    deposit(&mut market, "requester-1", Money::new(100));
    println!("  requester-1 余额：{}", market.balance("requester-1"));

    // ===== 步骤 4：发布任务（发布即托管预算 30）=====
    println!("\n【步骤 4】发布写作任务（预算 30，发布即锁定到托管账户）");
    market
        .publish_task(make_task(
            "task-1",
            "撰写一篇科技短文",
            "writing",
            30,
            "requester-1",
        ))
        .unwrap();
    let task = market.get_task("task-1").unwrap();
    println!("  任务状态：{}", task.state.label());
    println!(
        "  托管账户余额：{}，需求方余额：{}",
        market.balance("__escrow__:task-1"),
        market.balance("requester-1")
    );

    // ===== 步骤 5：两个 Agent 投标 =====
    println!("\n【步骤 5】Agent 投标");
    market
        .submit_bid(Bid {
            agent_id: "agent-writer".to_string(),
            task_id: "task-1".to_string(),
            proposed_price: Money::new(10),
            estimated_latency_ms: 800,
            score: 0.0,
        })
        .unwrap();
    market
        .submit_bid(Bid {
            agent_id: "agent-pro".to_string(),
            task_id: "task-1".to_string(),
            proposed_price: Money::new(8),
            estimated_latency_ms: 1200,
            score: 0.0,
        })
        .unwrap();
    println!("  agent-writer 与 agent-pro 均已投标");

    // ===== 步骤 6：匹配（性价比最高） =====
    println!("\n【步骤 6】匹配任务（性价比 = 信誉/价格，含延迟惩罚）");
    let winner = market.match_task("task-1").unwrap();
    println!("  中标 Agent：{}", winner);
    let task = market.get_task("task-1").unwrap();
    println!(
        "  任务状态：{}，负责人：{:?}，中标价：{}",
        task.state.label(),
        task.owner,
        task.winner_price.unwrap_or(Money::ZERO)
    );

    // ===== 步骤 7：提交执行结果 =====
    println!("\n【步骤 7】提交执行结果");
    let envelope = ResultEnvelope {
        task_id: "task-1".to_string(),
        agent_id: winner.clone(),
        report: r#"{"title":"科技改变生活","words":800}"#.to_string(),
        confidence: 0.92,
        error_type: ErrorType::None,
        trace_ref: "trace://task-1/step/3".to_string(),
        evidence_grade: EvidenceGrade::CpuProto,
        latency_ms: 950,
        pocv: None,
    };
    market.submit_result(envelope).unwrap();
    let task = market.get_task("task-1").unwrap();
    println!("  任务状态：{}（等待 QA 验证）", task.state.label());

    // ===== 步骤 8：BFT-lite QA 委员会验证 =====
    println!("\n【步骤 8】BFT-lite QA 委员会验证（n=4, f=1，需 3 票 STOP）");
    let mut committee = QaCommittee::new(4, 1).unwrap();
    for i in 1..=4 {
        committee.add_member(format!("qa-{}", i));
    }
    committee.cast_vote("qa-1", QaVote::Stop).unwrap();
    committee.cast_vote("qa-2", QaVote::Stop).unwrap();
    committee.cast_vote("qa-3", QaVote::Stop).unwrap();
    committee.cast_vote("qa-4", QaVote::Continue).unwrap();
    let decision = market.verify_result("task-1", &committee).unwrap();
    println!("  QA 决策：{:?}（3 STOP 通过）", decision);
    let task = market.get_task("task-1").unwrap();
    println!("  任务状态：{}", task.state.label());

    // ===== 步骤 9：结算（按中标价支付，余款退回）=====
    println!("\n【步骤 9】结算");
    let paid = market.settle_task("task-1").unwrap();
    println!("  实际支付金额：{}", paid);
    println!("  中标方余额：{}", market.balance(&winner));
    let task = market.get_task("task-1").unwrap();
    println!("  任务状态：{}", task.state.label());

    // ===== 步骤 10：信誉与排行榜 =====
    println!("\n【步骤 10】信誉更新与排行榜");
    let board = market.leaderboard(5);
    for (id, score) in &board {
        println!("  {}：信誉 {:.4}", id, score);
    }

    // ===== 步骤 11：守恒检查 =====
    println!("\n【步骤 11】资金守恒检查（精确相等，无容差）");
    let report = market.conservation_check();
    println!("  总充值：{}", report.total_deposits);
    println!("  总支付：{}", report.total_paid);
    println!("  总罚没：{}", report.total_slashed);
    println!("  当前余额总和：{}", report.balance_sum);
    println!("  守恒成立：{}", if report.conserved { "是" } else { "否" });

    line('-');

    // ===== 场景二：作恶 Agent 被仲裁罚没 =====
    println!("\n【场景二】作恶 Agent → 争议 → 仲裁 → 罚没");

    market
        .publish_task(make_task(
            "task-2",
            "数据分析报告",
            "analysis",
            20,
            "requester-1",
        ))
        .unwrap();
    market
        .submit_bid(Bid {
            agent_id: "agent-data".to_string(),
            task_id: "task-2".to_string(),
            proposed_price: Money::new(20),
            estimated_latency_ms: 500,
            score: 0.0,
        })
        .unwrap();
    let data_winner = market.match_task("task-2").unwrap();
    println!("  task-2 中标：{}", data_winner);

    // 数据师提交了低质量结果
    market
        .submit_result(ResultEnvelope {
            task_id: "task-2".to_string(),
            agent_id: data_winner.clone(),
            report: "{}".to_string(),
            confidence: 0.3,
            error_type: ErrorType::LowConfidence,
            trace_ref: "trace://task-2".to_string(),
            evidence_grade: EvidenceGrade::Unverified,
            latency_ms: 1800,
            pocv: None,
        })
        .unwrap();

    // 需求方发起争议
    market
        .open_dispute(
            "dispute-1",
            "task-2",
            "requester-1",
            "结果质量不合格，疑似虚假交付",
        )
        .unwrap();
    println!(
        "  争议已发起，任务状态：{}",
        market.get_task("task-2").unwrap().state.label()
    );

    // 仲裁：有罪，罚没金额由服务端规则决定（全部质押），托管预算全额退回需求方
    let (arb_did, arb_kp) = faucet();
    let arb_pk: [u8; 32] = arb_kp.public_key().try_into().unwrap();
    market.set_governance(Governance::from_members(vec![(arb_did.clone(), arb_pk)]));
    let arb_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let arb_cmd = SignedGovernanceCommand::sign(
        "demo",
        &arb_did,
        &arb_kp,
        GOV_CAP_ARBITRATE,
        "dispute-1",
        serde_json::json!({"guilty": true}),
        "demo-arb-nonce",
        arb_now - 10,
        300,
    );
    let (verdict, slashed) = market.arbitrate_signed(&arb_cmd, arb_now).unwrap();
    println!("  仲裁结果：{}，罚没：{}", verdict, slashed.as_i64());
    println!(
        "  任务状态：{}",
        market.get_task("task-2").unwrap().state.label()
    );
    println!(
        "  需求方余额（已退托管预算）：{}",
        market.balance("requester-1")
    );

    // 最终守恒
    let final_report = market.conservation_check();
    println!(
        "\n  最终守恒检查：{}",
        if final_report.conserved {
            "成立"
        } else {
            "失败"
        }
    );
    println!(
        "  总充值 {} / 总支付 {} / 总罚没 {} / 余额总和 {}",
        final_report.total_deposits,
        final_report.total_paid,
        final_report.total_slashed,
        final_report.balance_sum
    );

    // ===== 统计汇总 =====
    line('=');
    println!("  运行统计：");
    println!("    注册 Agent：{}", market.agent_count());
    println!("    发布任务：{}", market.task_count());
    println!("    已结算：{}", market.settled_count());
    println!("    争议：{}", market.dispute_count());
    line('=');
    println!("  演示完成：Agent Market 全流程功能正常运行");
    line('=');
}
