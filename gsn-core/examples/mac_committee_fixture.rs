//! Mac 真机 QA 委员质押夹具：为 4 名治理委员（seed 7/8/9/10）生成
//! deposit 信封 + AgentCard（stake 100），满足 verify 的 COMMITTEE_NOT_STAKED 闸门。
//! 运行：cargo run --example mac_committee_fixture --release

use gsn_core::identity::{Did, Keypair};
use gsn_core::marketplace::{
    Currency, EvidenceGrade, MarketAgentCard, Money, Pricing, PricingModel,
    SignedGovernanceCommand, Sla,
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

    let gov_kp = kp_from_seed(7);
    let gov_did = did_of(&gov_kp);

    for (i, seed) in [7u8, 8, 9, 10].iter().enumerate() {
        let kp = kp_from_seed(*seed);
        let did = did_of(&kp);

        // deposit 信封（治理 credit 200）
        let signed = SignedGovernanceCommand::sign(
            "mac-committee-fixture",
            &gov_did,
            &gov_kp,
            "governance:credit",
            &did,
            serde_json::json!({ "amount": 200 }),
            &format!("mac-3921-committee-{seed}"),
            now - 5,
            600,
        );
        fs::write(
            format!("{out}/committee-{i}-deposit.json"),
            serde_json::to_string_pretty(&signed).unwrap(),
        )
        .unwrap();

        // AgentCard（stake 100）
        let card = MarketAgentCard {
            agent_id: did.clone(),
            version: "1.0.0".into(),
            name: format!("MacQA委员{i}"),
            description: "Mac 真机 QA 委员（BFT-lite 验证）".into(),
            skills: vec!["qa".into()],
            modalities: vec!["text".into()],
            models: vec!["model-base".into()],
            endpoint: format!("a2a://{did}"),
            pricing: Pricing {
                model: PricingModel::PerCall,
                price: Money::new(1),
                currency: Currency::Credit,
            },
            sla: Sla {
                latency_p95_ms: 1000,
                availability: 0.99,
                max_concurrency: 5,
            },
            owner: format!("owner-{did}"),
            stake: Money::new(100),
            reputation_score: 0.5,
            total_calls: 0,
            success_rate: 1.0,
            evidence_grade: EvidenceGrade::CpuProto,
            verified: false,
            created_at: now,
            updated_at: now,
        };
        fs::write(
            format!("{out}/committee-{i}-card.json"),
            serde_json::to_string_pretty(&card).unwrap(),
        )
        .unwrap();
        println!("committee[{i}] did = {did}");
    }
    println!("committee fixtures written to {out}/");
}
