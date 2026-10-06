//! v3.9.12 回归：签名治理授信命令 nonce 必须跨重启持久化去重。
//!
//! 真机（Linux 云机三节点 SIGKILL + 同 data-dir 重启）发现：授信命令的 nonce
//! 防重放此前只存在于进程内 HashSet，daemon 崩溃重启后内存集清空，在命令有效
//! 期窗口内重放同一条已签名授信命令会被再次接受，导致同一账户被重复授信、
//! 破坏资金守恒（实测 100 → 400 重复入账 300）。
//!
//! 本测试用同一个 SQLite 文件先后启动两个市场 actor（丢弃第一个即模拟进程退出，
//! 第二个重开同一 data-dir 即模拟重启），断言：
//!   1. 首次授信成功；
//!   2. 同进程重放同一条签名命令被拒；
//!   3. 重启（重开同一持久库）后，在窗口内重放仍被拒，余额不翻倍、资金守恒；
//!   4. 重启后换一个全新 nonce 的合法授信仍可成功（去重不影响正常命令）。
//!
//! 本文件为独立集成测试二进制，仅含一个用例，因此在进程内设置
//! GSN_GOVERNANCE_FILE 不会与其他测试的环境变量并发竞争。
//!
//! 注：涉及 Ed25519 治理签名/防重放路径，仍需外部安全审计（REQUIRE EXTERNAL AUDIT）。

use std::sync::Arc;

use gsn_core::api::market_actor::{MarketActorHandle, MarketResponse};
use gsn_core::marketplace::{GovernanceMemberEntry, SignedGovernanceCommand, GOV_CAP_CREDIT};
use gsn_core::storage::PersistentStore;
use gsn_core::{Did, Keypair};
use serde_json::json;

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "gsn_v3912_govnonce_{}_{}_{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn member() -> (String, Keypair) {
    let mut seed = [0u8; 32];
    seed[0] = 42;
    let kp = Keypair::from_seed(&seed);
    (Did::from_public_key(kp.public_key()).to_string(), kp)
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn signed_credit(
    did: &str,
    kp: &Keypair,
    account: &str,
    amount: i64,
    nonce: &str,
    now: u64,
) -> SignedGovernanceCommand {
    SignedGovernanceCommand::sign(
        "v3912-cmd-1",
        did,
        kp,
        GOV_CAP_CREDIT,
        account,
        json!({ "amount": amount }),
        nonce,
        now - 10,
        now + 300,
    )
}

fn is_replay(resp: &MarketResponse) -> bool {
    matches!(resp, MarketResponse::Err(m) if m.contains("GOV_REPLAY"))
}

async fn balance_of(handle: &MarketActorHandle, account: &str) -> i64 {
    match handle.balance(account.to_string()).await {
        MarketResponse::Ok(v) => v
            .get("balance")
            .and_then(|b| b.as_i64())
            .expect("balance 应为整数"),
        MarketResponse::Err(e) => panic!("查询余额失败: {e}"),
    }
}

#[test]
fn governance_credit_nonce_survives_restart_and_blocks_replay() {
    let rt = rt();
    rt.block_on(async move {
        let dir = unique_dir("db");
        let db_path = dir.join("market.db");

        // 治理成员文件 + 环境变量（独立测试二进制，无跨用例 env 竞争）。
        let (did, kp) = member();
        let pk_bytes: [u8; 32] = kp.public_key().try_into().unwrap();
        let members = vec![GovernanceMemberEntry {
            did: did.clone(),
            pubkey_hex: hex::encode(pk_bytes),
        }];
        let gov_path = dir.join("governance.json");
        std::fs::write(&gov_path, serde_json::to_vec(&members).unwrap()).unwrap();
        std::env::set_var("GSN_GOVERNANCE_FILE", &gov_path);

        let account = "v3912-bob";
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cmd = signed_credit(&did, &kp, account, 300, "nonce-req-1", now);

        // ── 第一阶段：首次进程，授信成功 + 同进程重放被拒 ──
        let store1 = Arc::new(PersistentStore::open(&db_path).unwrap());
        let h1 = MarketActorHandle::spawn_with_store(store1);

        let first = h1.deposit_signed(cmd.clone(), now).await;
        assert!(
            matches!(first, MarketResponse::Ok(_)),
            "首次授信应成功: {first:?}"
        );
        assert_eq!(
            balance_of(&h1, account).await,
            300,
            "首次授信后余额应为 300"
        );

        let replay_same_proc = h1.deposit_signed(cmd.clone(), now).await;
        assert!(
            is_replay(&replay_same_proc),
            "同进程重放必须被 GOV_REPLAY 拒绝"
        );
        assert_eq!(
            balance_of(&h1, account).await,
            300,
            "同进程重放不得改变余额"
        );

        // 丢弃 actor 与连接，模拟进程退出。
        drop(h1);

        // ── 第二阶段：重开同一 data-dir（模拟 daemon 重启），窗口内重放仍须拒绝 ──
        let store2 = Arc::new(PersistentStore::open(&db_path).unwrap());
        let h2 = MarketActorHandle::spawn_with_store(store2);
        // 给 actor 一点时间完成启动重放。
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert_eq!(
            balance_of(&h2, account).await,
            300,
            "重启后余额应从账本恢复为 300，而非被污染成 600"
        );

        let replay_after_restart = h2.deposit_signed(cmd.clone(), now + 1).await;
        assert!(
            is_replay(&replay_after_restart),
            "重启后在有效期窗口内重放同一签名命令，必须仍被 GOV_REPLAY 拒绝（核心回归点）"
        );
        assert_eq!(
            balance_of(&h2, account).await,
            300,
            "重启后重放被拒，余额必须保持 300，不得重复授信"
        );

        // ── 全新 nonce 的合法授信不受去重影响 ──
        let cmd2 = signed_credit(&did, &kp, account, 100, "nonce-req-2", now + 2);
        let second_ok = h2.deposit_signed(cmd2, now + 2).await;
        assert!(
            matches!(second_ok, MarketResponse::Ok(_)),
            "全新 nonce 的合法授信应成功: {second_ok:?}"
        );
        assert_eq!(
            balance_of(&h2, account).await,
            400,
            "第二次合法授信后余额应为 300+100=400"
        );

        std::env::remove_var("GSN_GOVERNANCE_FILE");
        let _ = std::fs::remove_dir_all(&dir);
    });
}
