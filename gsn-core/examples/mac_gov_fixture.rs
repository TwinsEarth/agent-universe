//! Mac 真机闭环夹具（dev/faucet，仅本地测试）：
//! 生成治理成员文件 + 两笔 governance:credit 签名授信信封 + AgentCard + TaskSpec。
//!
//! 运行：cargo run --example mac_gov_fixture --release
//! 输出目录：/tmp/macfixture/
//!
//! 安全说明：dev/faucet 授信仅用于本地真机验证；生产授信必须由链上支付凭证支撑
//! （EIP-3009/HTLC 等），该链上兑付路径当前为纯协议内核、只构造不广播，需外部审计。

use gsn_core::identity::{Did, Keypair};
use gsn_core::marketplace::{
    Currency, EvidenceGrade, GovernanceMemberEntry, MarketAgentCard, Money, Pricing, PricingModel,
    SignedGovernanceCommand, Sla, TaskSpec, TaskState, VerificationPolicy, GOV_CAP_CREDIT,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn kp_from_seed(seed_byte: u8) -> Keypair {
    let mut seed = [0u8; 32];
    seed[0] = seed_byte;
    Keypair::from_seed(&seed)
}

fn did_of(kp: &Keypair) -> String {
    Did::from_public_key(kp.public_key()).to_string()
}

fn main() {
    let out = "/tmp/macfixture";
    fs::create_dir_all(out).unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 治理成员（faucet）
    let gov_kp = kp_from_seed(7);
    let gov_pk: [u8; 32] = gov_kp.public_key().try_into().unwrap();
    let gov_did = did_of(&gov_kp);
    let gov_entry = vec![GovernanceMemberEntry {
        did: gov_did.clone(),
        pubkey_hex: hex::encode(gov_pk),
    }];
    fs::write(
        format!("{out}/governance.json"),
        serde_json::to_string_pretty(&gov_entry).unwrap(),
    )
    .unwrap();
    println!("governance did = {gov_did}");

    // Agent 账户与 Requester 账户
    let agent_kp = kp_from_seed(101);
    let agent_did = did_of(&agent_kp);
    let req_kp = kp_from_seed(202);
    let req_did = did_of(&req_kp);
    println!("agent did     = {agent_did}");
    println!("requester did = {req_did}");

    // 两笔授信：agent=1000（够锁 100 质押），requester=300（够托管 200 预算）
    let credit = |target: &str, amount: i64, nonce: &str| {
        SignedGovernanceCommand::sign(
            "mac-fixture",
            &gov_did,
            &gov_kp,
            GOV_CAP_CREDIT,
            target,
            serde_json::json!({ "amount": amount }),
            nonce,
            now - 5,
            600,
        )
    };
    let cmd_agent = credit(&agent_did, 1000, "nonce-agent-1");
    let cmd_req = credit(&req_did, 300, "nonce-req-1");
    fs::write(
        format!("{out}/deposit-agent.json"),
        serde_json::to_string_pretty(&cmd_agent).unwrap(),
    )
    .unwrap();
    fs::write(
        format!("{out}/deposit-req.json"),
        serde_json::to_string_pretty(&cmd_req).unwrap(),
    )
    .unwrap();

    // 第三个账户专供真实 CLI 正向验证（全新 nonce/账户，避免与 HTTP 用例冲突）
    let cli_kp = kp_from_seed(203);
    let cli_did = did_of(&cli_kp);
    println!("cli did      = {cli_did}");
    let cmd_cli = credit(&cli_did, 50, "nonce-cli-1");
    fs::write(
        format!("{out}/deposit-cli.json"),
        serde_json::to_string_pretty(&cmd_cli).unwrap(),
    )
    .unwrap();

    // AgentCard（agent_id 用真实 did:nau）
    let card = MarketAgentCard {
        agent_id: agent_did.clone(),
        version: "1.0.0".to_string(),
        name: "Mac真机文案师".to_string(),
        description: "Mac 真机闭环测试 Agent，擅长 writing".to_string(),
        skills: vec!["writing".to_string()],
        modalities: vec!["text".to_string()],
        models: vec!["model-base".to_string()],
        endpoint: format!("a2a://{agent_did}"),
        pricing: Pricing {
            model: PricingModel::PerCall,
            price: Money::new(10),
            currency: Currency::Credit,
        },
        sla: Sla::default(),
        owner: format!("owner-{agent_did}"),
        stake: Money::new(100),
        reputation_score: 0.5,
        total_calls: 0,
        success_rate: 1.0,
        evidence_grade: EvidenceGrade::CpuProto,
        verified: false,
        created_at: 1000,
        updated_at: 1000,
    };
    fs::write(
        format!("{out}/agent-card.json"),
        serde_json::to_string_pretty(&card).unwrap(),
    )
    .unwrap();

    // TaskSpec（required_skills 非空，budget=200，发布即托管）
    let task = TaskSpec {
        task_id: "mac-realtask-1".to_string(),
        goal: "写一段 100 字产品文案（Mac 真机闭环）".to_string(),
        context: "Mac mini 三节点真机验证".to_string(),
        done: vec![],
        todo: vec!["分析".to_string(), "生成".to_string()],
        trace: vec![],
        owner: None,
        budget: Money::new(200),
        winner_price: None,
        deadline: now + 3600,
        required_skills: vec!["writing".to_string()],
        verification_policy: VerificationPolicy::BftLite { n: 4, f: 1 },
        requester: req_did.clone(),
        state: TaskState::Draft,
        created_at: 1000,
    };
    fs::write(
        format!("{out}/task.json"),
        serde_json::to_string_pretty(&task).unwrap(),
    )
    .unwrap();

    println!("fixtures written to {out}/");
    println!(
        "files: governance.json deposit-agent.json deposit-req.json deposit-cli.json agent-card.json task.json"
    );
}
