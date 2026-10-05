//! 插件编排器（Plugin Orchestrator）—— 主数据面「插件决策 → 宿主应用」网关
//!
//! # 它解决什么（B1 gap）
//!
//! v3.0.0–v3.4.2 的官方插件虽已业务化，但在 daemon 主数据面**没有接线**：
//! 单体主业务仍直接走 `market actor`（注册/匹配/结算全部由宿主自决），
//! 插件只在 `/api/v1/plugins/{id}/call` 下被**独立并行调用**。
//!
//! 编排器把这个关系倒过来：**关键业务决策先经插件，宿主再应用结果**。
//!
//! ```text
//!  gated 业务方法：
//!    register_agent_gated  → agent-card/validate 插件校验卡片，valid 才注册
//!    match_task_gated       → market-match/match 插件决定中标方，再应用 winner
//!    settle_task_gated      → market-settle/audit 插件审计账本，passed 才结算
//! ```
//!
//! 宿主（[`MarketActorHandle`](crate::api::market_actor::MarketActorHandle)）
//! 仍持有权威状态（账本/任务/质押/信誉），单体 market 退化为**状态存储 + 执行引擎**；
//! 插件承载的是可替换、可审计的**决策逻辑**。
//!
//! # 为什么这样切线程
//!
//! [`PluginHost`] 是同步、非 `Send`-safe 的内核（内部持实例），被
//! `std::sync::Mutex` 保护；调用插件必须在 `spawn_blocking` 中进行。
//! 而 `market` 是异步 `mpsc` 句柄。编排器同时持有两者，把「插件同步调用」
//! 与「market 异步命令」串成一个异步流程。

use crate::api::market_actor::{MarketActorHandle, MarketResponse};
use crate::plugin::host::PluginHost;
use crate::plugin::official::{
    OFF_AGENT_CARD, OFF_AGENT_SKILL, OFF_ECONOMY_REPUTATION, OFF_MARKET_MATCH, OFF_MARKET_SETTLE,
    OFF_SCHEDULER_TASK,
};
use serde_json::Value;
use std::sync::Arc;

/// 插件编排器句柄（克隆廉价）。
#[derive(Clone)]
pub struct PluginOrchestratorHandle {
    /// 插件宿主（std Mutex，调用需 spawn_blocking）。
    host: Arc<std::sync::Mutex<PluginHost>>,
    /// 市场 actor（权威状态存储 + 执行引擎）。
    market: MarketActorHandle,
}

impl PluginOrchestratorHandle {
    /// 构造编排器（绑定插件宿主与市场 actor）。
    pub fn new(host: Arc<std::sync::Mutex<PluginHost>>, market: MarketActorHandle) -> Self {
        Self { host, market }
    }

    /// 调用一个插件方法（在 blocking 线程中持锁同步调用）。
    ///
    /// 要求插件返回合法 JSON；stdout 非 JSON 即错误。
    async fn call_plugin(
        &self,
        plugin_id: &str,
        method: &str,
        payload: Value,
    ) -> Result<Value, String> {
        let host = self.host.clone();
        let plugin_id = plugin_id.to_string();
        let method = method.to_string();
        let bytes = serde_json::to_vec(&payload).map_err(|e| format!("payload 序列化失败: {e}"))?;
        tokio::task::spawn_blocking(move || {
            let mut guard = host.lock().unwrap_or_else(|e| {
                // 锁毒化：恢复内部数据（内核实例不依赖锁的纯洁性），记录并继续。
                eprintln!("⚠️ 插件宿主锁毒化，已恢复: {e}");
                e.into_inner()
            });
            let out = guard
                .call(&plugin_id, &method, &bytes)
                .map_err(|e| format!("插件 {plugin_id}.{method} 调用失败: {e}"))?;
            serde_json::from_slice::<Value>(&out)
                .map_err(|e| format!("插件 {plugin_id}.{method} 输出非 JSON: {e}"))
        })
        .await
        .map_err(|e| format!("编排任务失败: {e}"))?
    }

    // ===== 纯插件决策（可单独调用，便于观察/测试） =====

    /// agent-card/validate：校验注册卡片。
    pub async fn validate_card(&self, card: Value) -> Result<Value, String> {
        self.call_plugin(
            OFF_AGENT_CARD,
            "validate",
            serde_json::json!({ "card": card }),
        )
        .await
    }

    /// agent-skill/discover：按技能发现智能体。
    pub async fn discover_skill(&self, payload: Value) -> Result<Value, String> {
        self.call_plugin(OFF_AGENT_SKILL, "discover", payload).await
    }

    /// scheduler-task/route：路由候选节点。
    pub async fn route_task(&self, payload: Value) -> Result<Value, String> {
        self.call_plugin(OFF_SCHEDULER_TASK, "route", payload).await
    }

    /// market-match/match：在投标中决定中标方。
    pub async fn match_bids(&self, payload: Value) -> Result<Value, String> {
        self.call_plugin(OFF_MARKET_MATCH, "match", payload).await
    }

    /// market-settle/audit：独立审计账本。
    pub async fn audit_ledger(&self, payload: Value) -> Result<Value, String> {
        self.call_plugin(OFF_MARKET_SETTLE, "audit", payload).await
    }

    /// economy-reputation/overall：四维信誉聚合。
    pub async fn reputation_overall(&self, payload: Value) -> Result<Value, String> {
        self.call_plugin(OFF_ECONOMY_REPUTATION, "overall", payload)
            .await
    }

    // ===== gated 业务闸门（主数据面） =====

    /// 注册智能体：先经 agent-card 插件校验卡片，`valid` 才注册。
    ///
    /// 卡片非法时返回类型化错误（`BAD_REQUEST:`），不进入 market。
    pub async fn register_agent_gated(&self, card: Value) -> MarketResponse {
        match self.validate_card(card.clone()).await {
            Ok(v) => {
                let valid = v.get("valid").and_then(|x| x.as_bool()).unwrap_or(false);
                if !valid {
                    return MarketResponse::err(format!(
                        "BAD_REQUEST: 卡片校验未通过: {}",
                        v.get("errors")
                            .cloned()
                            .unwrap_or_else(|| Value::from("unknown"))
                    ));
                }
                self.market.register_agent(card).await
            }
            Err(e) => MarketResponse::err(e),
        }
    }

    /// 匹配任务：market 取真实投标 → market-match 插件决定中标 → 应用 winner。
    ///
    /// 无有效投标时返回 `CONFLICT:`，不改变任务状态。
    pub async fn match_task_gated(&self, task_id: String) -> MarketResponse {
        let bids_payload = match self.market.bids_for_plugin(task_id.clone()).await {
            MarketResponse::Ok(v) => v,
            MarketResponse::Err(e) => return MarketResponse::err(e),
        };
        let decision = match self.match_bids(bids_payload).await {
            Ok(v) => v,
            Err(e) => return MarketResponse::err(e),
        };
        match decision.get("winner").and_then(|x| x.as_str()) {
            Some(winner) => {
                self.market
                    .match_task_with_winner(task_id, winner.to_string())
                    .await
            }
            None => MarketResponse::err(format!(
                "CONFLICT: 无有效投标（{}）",
                decision
                    .get("reason")
                    .and_then(|x| x.as_str())
                    .unwrap_or("unknown")
            )),
        }
    }

    /// 结算任务：market 取账本快照 → market-settle 插件独立审计 → passed 才结算。
    ///
    /// 审计不通过（守恒失败/篡改/ghost 账户）时返回 `CONFLICT:`，拒绝付款。
    pub async fn settle_task_gated(&self, task_id: String) -> MarketResponse {
        let snapshot = match self.market.ledger_snapshot().await {
            MarketResponse::Ok(v) => v,
            MarketResponse::Err(e) => return MarketResponse::err(e),
        };
        let audit = match self.audit_ledger(snapshot).await {
            Ok(v) => v,
            Err(e) => return MarketResponse::err(e),
        };
        let passed = audit
            .get("passed")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if !passed {
            return MarketResponse::err(format!(
                "CONFLICT: 账本审计未通过（mismatches={}）",
                audit
                    .get("mismatches")
                    .cloned()
                    .unwrap_or_else(|| Value::from("[]"))
            ));
        }
        self.market.settle_task(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::host::PluginHost;

    /// 构造一个装配了系统 + 官方插件的宿主（process 后端在测试环境可用）。
    fn booted_host() -> Arc<std::sync::Mutex<PluginHost>> {
        let mut host = PluginHost::new("3.4.5", None);
        host.boot_system(&crate::plugin::system::SystemHandles::default())
            .unwrap();
        host.boot_official().unwrap();
        Arc::new(std::sync::Mutex::new(host))
    }

    fn valid_card() -> Value {
        serde_json::json!({
            "agent_id": "did:nau:alice", "name": "Alice", "version": "1.0",
            "skills": [], "reputation_score": 0.5, "success_rate": 0.9, "stake": 100,
        })
    }

    #[tokio::test]
    async fn validate_card_accepts_well_formed() {
        let orch = PluginOrchestratorHandle::new(booted_host(), MarketActorHandle::spawn());
        let v = orch.validate_card(valid_card()).await.unwrap();
        assert_eq!(v["valid"], true);
    }

    #[tokio::test]
    async fn validate_card_rejects_bad_did() {
        let orch = PluginOrchestratorHandle::new(booted_host(), MarketActorHandle::spawn());
        let mut bad = valid_card();
        bad["agent_id"] = Value::from("not-a-did");
        let v = orch.validate_card(bad).await.unwrap();
        assert_eq!(v["valid"], false);
        assert!(v["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| { e.as_str().unwrap_or("").contains("DID") }));
    }

    #[tokio::test]
    async fn register_gated_registers_valid_card() {
        // P0-4：deposit 现为特权写，必须由治理集成员签名授信命令。
        // 这里构造一个 faucet 治理成员，注入治理集后签 credit 命令给 alice 注资。
        use crate::identity::{Did, Keypair};
        use crate::marketplace::{SignedGovernanceCommand, GOV_CAP_CREDIT};
        let mut seed = [0u8; 32];
        seed[0] = 42;
        let faucet_kp = Keypair::from_seed(&seed);
        let faucet_did = Did::from_public_key(faucet_kp.public_key()).to_string();
        let faucet_pk: [u8; 32] = faucet_kp.public_key().try_into().unwrap();
        let market = MarketActorHandle::spawn_with_governance_members(vec![(
            faucet_did.clone(),
            faucet_pk,
        )]);
        let orch = PluginOrchestratorHandle::new(booted_host(), market.clone());
        // 先由 faucet 签名授信 100 到注册方账户，满足注册质押锁定门槛。
        let now = 1_000_000u64;
        let credit = SignedGovernanceCommand::sign(
            "dep1",
            &faucet_did,
            &faucet_kp,
            GOV_CAP_CREDIT,
            "did:nau:alice",
            serde_json::json!({"amount": 100}),
            "n-dep-1",
            now - 10,
            300,
        );
        let d = market.deposit_signed(credit, now).await;
        assert!(matches!(d, MarketResponse::Ok(_)), "授信应成功: {d:?}");
        let resp = orch.register_agent_gated(valid_card()).await;
        match resp {
            MarketResponse::Ok(v) => assert_eq!(v["status"], "registered"),
            MarketResponse::Err(e) => panic!("应注册成功: {e}"),
        }
    }

    #[tokio::test]
    async fn register_gated_rejects_invalid_card() {
        let orch = PluginOrchestratorHandle::new(booted_host(), MarketActorHandle::spawn());
        let mut bad = valid_card();
        bad["agent_id"] = Value::from("not-a-did");
        let resp = orch.register_agent_gated(bad).await;
        match resp {
            MarketResponse::Ok(v) => panic!("不应注册: {v}"),
            MarketResponse::Err(e) => assert!(e.contains("BAD_REQUEST")),
        }
    }

    #[tokio::test]
    async fn match_gated_no_bids_conflicts() {
        let orch = PluginOrchestratorHandle::new(booted_host(), MarketActorHandle::spawn());
        let resp = orch.match_task_gated("t-missing".to_string()).await;
        match resp {
            MarketResponse::Ok(v) => panic!("无投标不应成功: {v}"),
            MarketResponse::Err(e) => assert!(e.contains("无投标") || e.contains("CONFLICT")),
        }
    }

    #[tokio::test]
    async fn audit_ledger_clean_passes() {
        let orch = PluginOrchestratorHandle::new(booted_host(), MarketActorHandle::spawn());
        // 空账本：无流水、零余额，重放一致 → passed。
        let ledger = serde_json::json!({"records": [], "balances": {}});
        let v = orch.audit_ledger(ledger).await.unwrap();
        assert_eq!(v["passed"], true);
    }

    #[tokio::test]
    async fn audit_ledger_detects_ghost() {
        let orch = PluginOrchestratorHandle::new(booted_host(), MarketActorHandle::spawn());
        // 无对应 Deposited 流水，但余额凭空 100 → ghost，审计失败。
        let ledger = serde_json::json!({
            "records": [],
            "balances": {"ghost": 100}
        });
        let v = orch.audit_ledger(ledger).await.unwrap();
        assert_eq!(v["passed"], false);
        assert!(v["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| { m["account"] == "ghost" }));
    }
}
