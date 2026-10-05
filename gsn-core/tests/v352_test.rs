//! v3.5.2 健壮性/隔离诚实化补丁回归测试
//!
//! 对应审计条目（每条都断言「旧实现会失败」）：
//! - AU-04：认证 QA 投票 nonce 跨请求去重（旧实现 seen_nonces 随每次新建委员会清空，
//!   同一组签名票可在第二次验收调用里重放）。
//!
//! 其余条目（AU-06/07/08/09/10/21/22/23/24/25/26/28）的单测就近放在各模块
//! `#[cfg(test)]` 内或本文件后续增补。

use gsn_core::marketplace::*;

#[path = "common/mod.rs"]
mod helpers;

use gsn_core::Keypair;

// ===== 辅助构造（与 v351_test 对齐，集成测试为独立 crate，需各自定义）=====

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

const NOW: u64 = 1_000_000;

/// 把任务推进到 Verifying 状态，返回 (task_id, executor_did)。
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
            evidence_grade: EvidenceGrade::Unverified,
            latency_ms: 100,
            pocv: None,
        })
        .unwrap();
    ("task-1".to_string(), exec.to_string())
}

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

// ===== AU-04：nonce 跨请求去重 =====

/// 同一组合法 Stop 票在第一次验收成功后，第二次原样重放必须被 QA_NONCE_REPLAY 拒绝。
/// 旧实现：seen_nonces 在新建 QaCommittee 时清空，第二次调用不记得这组票 → 重放成功。
#[test]
fn au04_nonce_replayed_across_requests_rejected() {
    let mut market = AgentMarket::new();
    let (tid, _exec) = setup_task_awaiting_qa(&mut market);

    let qa = staked_committee(&mut market, 4);
    let members: Vec<(String, [u8; 32])> = qa.iter().map(|(d, _, pk)| (d.clone(), *pk)).collect();

    // 第一次：前 3 个委员用固定 nonce 签 Stop → 合法通过。
    let mut first_votes = Vec::new();
    for (i, (did, kp, _)) in qa.iter().take(3).enumerate() {
        first_votes.push(SignedQaVote::sign(
            &tid,
            0,
            did,
            QaVote::Stop,
            &format!("round0-nonce-{i}"),
            NOW - 10,
            NOW + 60,
            kp,
        ));
    }

    let decision = market
        .verify_result_authenticated(&tid, 0, members.clone(), first_votes.clone(), NOW)
        .expect("第一次合法验收应通过");
    assert_eq!(decision, QaDecision::Stop);

    // 第二次：原样重放同一组票（同一 task/round/voter/nonce）。
    let err = market
        .verify_result_authenticated(&tid, 0, members, first_votes, NOW)
        .unwrap_err();
    assert!(
        err.contains("QA_NONCE_REPLAY"),
        "跨请求重放应被 QA_NONCE_REPLAY 拒绝，实际: {err}"
    );
}

// ===== AU-28：请求隔离级别高于后端可达级别不得静默降级 =====

/// 进程后端如实只交付 Process（strength=1）。请求 MicroVM 且无豁免 → 具名拒绝；
/// 带非空理由的 IsolationLevel 豁免 → 按现状接受。
/// 旧实现：cfg.isolation 从不与实际可达级别比对，请求 MicroVM 也被静默按 Process 跑。
#[test]
fn au28_microvm_on_process_backend_rejected_without_waiver() {
    use gsn_core::sandbox::capability::{Capability, Waiver};
    use gsn_core::sandbox::config::{IsolationLevel, SandboxConfig};
    use gsn_core::sandbox::error::SandboxError;
    use gsn_core::sandbox::runtime::ProcessSandbox;
    use gsn_core::sandbox::Sandbox;

    let base = std::env::temp_dir().join(format!("au28-{}", std::process::id()));
    let mut cfg = SandboxConfig::trusted_local("au28 test");
    cfg.work_dir_base = Some(base);
    cfg.isolation = IsolationLevel::MicroVM;

    // 无 IsolationLevel 豁免 → 进程后端如实报 Process，拒绝静默降级。
    let mut sb = ProcessSandbox::new("au28-no-waiver");
    let err = sb.create(&cfg).unwrap_err();
    match err {
        SandboxError::PolicyNotEnforceable { boundary, .. } => {
            assert_eq!(boundary, Capability::IsolationLevel);
        }
        other => panic!("expected PolicyNotEnforceable(IsolationLevel), got {other}"),
    }

    // 带非空理由的豁免 → 按现状接受。
    cfg.waivers.push(Waiver::new(
        Capability::IsolationLevel,
        "test: 显式接受从 MicroVM 降级到 Process",
    ));
    cfg.sandbox_id = "au28-waived".to_string();
    let mut sb2 = ProcessSandbox::new("au28-waived");
    assert!(sb2.create(&cfg).is_ok());
    let _ = sb2.destroy();
}

// ===== AU-10：waiver 边界与理由落沙箱审计 =====

/// create 走 API 成功后，审计条目必须带上 cfg.waivers 的边界名与理由。
/// 旧实现：AuditEntry 无 waivers 字段，"带理由放弃某条边界"不留痕。
#[test]
fn au10_waivers_recorded_in_create_audit() {
    use gsn_core::sandbox::config::SandboxConfig;
    use gsn_core::sandbox::handle_sandbox_api;
    use gsn_core::sandbox::manager::SandboxManager;

    let base = std::env::temp_dir().join(format!("au10-{}", std::process::id()));
    let cfg = SandboxConfig::trusted_local("au10 test");
    let mut mgr = SandboxManager::new(base, cfg, 0);
    // 空请求体 → 走 default_cfg（trusted_local，含网络/FS/磁盘豁免）。
    let (status, _body) = handle_sandbox_api(
        "POST",
        "/api/v1/sandboxes",
        "{}",
        Some("did:caller"),
        &mut mgr,
    );
    assert_eq!(status, 201, "create 应成功");
    // create 审计条目应携带 waiver（边界名 + 理由）。
    let create = mgr
        .audit_entries()
        .iter()
        .find(|e| e.action == "create")
        .expect("应有 create 审计条目");
    assert!(
        !create.waivers.is_empty(),
        "trusted_local 的豁免应落审计，got {create:?}"
    );
    assert!(
        create
            .waivers
            .iter()
            .any(|w| w.boundary == "network_deny_all" && !w.justification.trim().is_empty()),
        "应含 network_deny_all 边界与非空理由，got {:?}",
        create.waivers
    );
}
