#![allow(dead_code)]
//! 集成测试共享辅助（P0-3/P0-4 后，特权写需签名治理命令）。
//!
//! 设计：每个测试用一个确定性 faucet 治理成员（固定 seed），
//! `deposit` / `arbitrate` 辅助在调用前把该成员注入市场治理集，
//! 然后构造 Ed25519 签名治理命令并走 `deposit_signed` / `arbitrate_signed`。
//! 非特权路径（submit_result 等）行为不变；ResultEnvelope 需补 `pocv: None`。

use gsn_core::identity::{Did, Keypair};
use gsn_core::marketplace::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn next_nonce(prefix: &str) -> String {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{prefix}-{n}")
}

/// 确定性 faucet 治理成员。
pub fn faucet() -> (String, Keypair) {
    let mut s = [0u8; 32];
    s[0] = 7;
    let kp = Keypair::from_seed(&s);
    (Did::from_public_key(kp.public_key()).to_string(), kp)
}

/// 把 faucet 成员注入市场治理集（幂等，可重复调用）。
pub fn grant_governance(market: &mut AgentMarket) {
    let (did, kp) = faucet();
    let pk: [u8; 32] = kp.public_key().try_into().unwrap();
    market.set_governance(Governance::from_members(vec![(did, pk)]));
}

/// off-chain 授信（dev/faucet）：治理签名命令授信到 account。
pub fn deposit(market: &mut AgentMarket, account: &str, amount: Money) {
    grant_governance(market);
    let (did, kp) = faucet();
    let now = now_secs();
    let cmd = SignedGovernanceCommand::sign(
        "test",
        &did,
        &kp,
        GOV_CAP_CREDIT,
        account,
        json!({"amount": amount.as_i64()}),
        &next_nonce("dep"),
        now - 10,
        300,
    );
    market.deposit_signed(&cmd, now).unwrap();
}

/// 仲裁：治理签名命令（capability=governance:arbitrate）执行罚没/放行。
pub fn arbitrate(market: &mut AgentMarket, dispute_id: &str, guilty: bool) -> (String, Money) {
    arbitrate_result(market, dispute_id, guilty).unwrap()
}

/// 仲裁的 fallible 版本（用于断言已仲裁/不存在等业务错误）。
pub fn arbitrate_result(
    market: &mut AgentMarket,
    dispute_id: &str,
    guilty: bool,
) -> Result<(String, Money), String> {
    grant_governance(market);
    let (did, kp) = faucet();
    let now = now_secs();
    let cmd = SignedGovernanceCommand::sign(
        "test",
        &did,
        &kp,
        GOV_CAP_ARBITRATE,
        dispute_id,
        json!({"guilty": guilty}),
        &next_nonce("arb"),
        now - 10,
        300,
    );
    market.arbitrate_signed(&cmd, now)
}

/// 构造一个非治理成员签名的仲裁命令（用于负例：应被 NOT_MEMBER 拒绝）。
pub fn arbitrate_signed_by_stranger(
    market: &mut AgentMarket,
    dispute_id: &str,
    guilty: bool,
) -> Result<(String, Money), String> {
    // 注意：market 的治理集仍是 faucet 成员；陌生人不在其中。
    let mut s = [0u8; 32];
    s[0] = 99;
    let kp = Keypair::from_seed(&s);
    let did = Did::from_public_key(kp.public_key()).to_string();
    let now = now_secs();
    let cmd = SignedGovernanceCommand::sign(
        "test",
        &did,
        &kp,
        GOV_CAP_ARBITRATE,
        dispute_id,
        json!({"guilty": guilty}),
        &next_nonce("arb-stranger"),
        now - 10,
        300,
    );
    market.arbitrate_signed(&cmd, now)
}
