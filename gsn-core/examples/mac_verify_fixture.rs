//! Mac 真机 QA 认证投票夹具：用治理委员密钥对签发 STOP 票，生成 verify.json。
//! 运行：cargo run --example mac_verify_fixture --release

use gsn_core::identity::{Did, Keypair};
use gsn_core::marketplace::{QaVote, SignedQaVote};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let out = "/tmp/macfixture";
    fs::create_dir_all(out).unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 4 名治理委员（seed 7/8/9/10），满足 n≥4、f=1、quorum≥3
    let mut members = Vec::new();
    let mut votes = Vec::new();
    for seed in [7u8, 8, 9, 10] {
        let kp = kp_from_seed(seed);
        let did = Did::from_public_key(kp.public_key()).to_string();
        let pk: [u8; 32] = kp.public_key().try_into().unwrap();
        members.push(serde_json::json!({"did": did.clone(), "public_key": hex::encode(pk)}));
        let sv = SignedQaVote::sign(
            "mac-realtask-1",
            1,
            &did,
            QaVote::Stop,
            &format!("mac-3921-verify-nonce-{seed}"),
            now - 5,
            now + 600,
            &kp,
        );
        votes.push(sv);
    }

    let payload = serde_json::json!({
        "round": 1,
        "members": members,
        "signed_votes": votes
    });
    fs::write(
        format!("{out}/verify.json"),
        serde_json::to_string_pretty(&payload).unwrap(),
    )
    .unwrap();
    println!("verify fixture written: {out}/verify.json");
    println!("committee members = 4（seed 7/8/9/10）");
}

fn kp_from_seed(seed_byte: u8) -> Keypair {
    let mut seed = [0u8; 32];
    seed[0] = seed_byte;
    Keypair::from_seed(&seed)
}
