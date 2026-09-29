//! 市场 Actor
//!
//! 独占 AgentMarket，HTTP/MCP/CLI handler 通过 mpsc 通道发命令，
//! 与 swarm actor 同样的 actor 模式，避免共享 Mutex 在异步事件循环中死锁。

use crate::marketplace::*;
use crate::storage::PersistentStore;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// 当前 Unix 毫秒
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 简化的智能体注册输入
///
/// 只暴露调用方关心的字段，其余（版本/时间戳/信誉/证据等级/SLA 等）
/// 在 actor 内部填充默认值，避免客户端构造庞大的 MarketAgentCard。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegisterAgentInput {
    /// 全局唯一 DID（必填）
    pub agent_id: String,
    /// 名称（必填）
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub modalities: Vec<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub owner: String,
    /// 质押金额（须 ≥ 市场最低质押，整数；注册前需先 deposit）
    #[serde(default)]
    pub stake: Money,
    /// 单次调用价格（整数）
    #[serde(default)]
    pub price: Money,
    /// 货币：credit / token / fiat（默认 credit）
    #[serde(default = "default_currency")]
    pub currency: String,
    /// 定价模式：per_call / per_token / per_hour / subscription（默认 per_call）
    #[serde(default = "default_pricing_model")]
    pub pricing_model: String,
}

fn default_currency() -> String { "credit".to_string() }
fn default_pricing_model() -> String { "per_call".to_string() }

impl RegisterAgentInput {
    /// 转换为完整 MarketAgentCard
    pub fn into_card(self) -> Result<MarketAgentCard, String> {
        let currency = match self.currency.to_lowercase().as_str() {
            "token" => Currency::Token,
            "fiat" => Currency::Fiat,
            _ => Currency::Credit,
        };
        let model = match self.pricing_model.to_lowercase().as_str() {
            "per_token" => PricingModel::PerToken,
            "per_hour" => PricingModel::PerHour,
            "subscription" => PricingModel::Subscription,
            _ => PricingModel::PerCall,
        };
        let now = now_ms() / 1000;
        Ok(MarketAgentCard {
            agent_id: self.agent_id,
            version: env!("CARGO_PKG_VERSION").to_string(),
            name: self.name,
            description: self.description,
            skills: self.skills,
            modalities: self.modalities,
            models: self.models,
            endpoint: self.endpoint,
            pricing: Pricing { model, price: self.price, currency },
            sla: Sla::default(),
            owner: self.owner,
            stake: self.stake,
            reputation_score: 0.0,
            total_calls: 0,
            success_rate: 0.0,
            evidence_grade: EvidenceGrade::Unverified,
            verified: false,
            created_at: now,
            updated_at: now,
        })
    }
}

/// 市场命令（覆盖 AgentMarket 全部公共操作）
#[derive(Debug)]
pub enum MarketCommand {
    /// 注册智能体（传入卡片 JSON）
    RegisterAgent { card: Value, reply: oneshot::Sender<MarketResponse> },
    /// 查询单个智能体
    GetAgent { agent_id: String, reply: oneshot::Sender<MarketResponse> },
    /// 按技能发现
    Discover { skill: String, reply: oneshot::Sender<MarketResponse> },
    /// 关键词搜索
    Search { query: String, reply: oneshot::Sender<MarketResponse> },
    /// 发布任务（传入任务 JSON）
    PublishTask { task: Value, reply: oneshot::Sender<MarketResponse> },
    /// 查询任务
    GetTask { task_id: String, reply: oneshot::Sender<MarketResponse> },
    /// 列出全部任务
    ListTasks { reply: oneshot::Sender<MarketResponse> },
    /// 投标
    SubmitBid { bid: Value, reply: oneshot::Sender<MarketResponse> },
    /// 匹配任务
    MatchTask { task_id: String, reply: oneshot::Sender<MarketResponse> },
    /// 提交结果
    SubmitResult { envelope: Value, reply: oneshot::Sender<MarketResponse> },
    /// 验证结果（认证式，v2.5.9）
    ///
    /// members: 固定委员集 (did, 公钥)；signed_votes: 委员私钥签发的真实投票。
    /// 服务端不再按 approvals 合成委员与票。
    VerifyResult {
        task_id: String,
        round: u32,
        members: Vec<(String, [u8; 32])>,
        signed_votes: Vec<SignedQaVote>,
        now: u64,
        reply: oneshot::Sender<MarketResponse>,
    },
    /// 结算任务
    SettleTask { task_id: String, reply: oneshot::Sender<MarketResponse> },
    /// 恢复边：返工任务回到执行中（v2.6.0）
    ResumeRework { task_id: String, reply: oneshot::Sender<MarketResponse> },
    /// 恢复边：无共识任务重新开放（v2.6.0）
    ReopenTask { task_id: String, reply: oneshot::Sender<MarketResponse> },
    /// 开启争议
    OpenDispute { dispute: Value, reply: oneshot::Sender<MarketResponse> },
    /// 仲裁
    Arbitrate { dispute_id: String, guilty: bool, slash: Money, reply: oneshot::Sender<MarketResponse> },
    /// 充值
    Deposit { account: String, amount: Money, reply: oneshot::Sender<MarketResponse> },
    /// 查询余额
    Balance { account: String, reply: oneshot::Sender<MarketResponse> },
    /// 守恒检查
    Conservation { reply: oneshot::Sender<MarketResponse> },
    /// 独立审计（从流水独立重放，v2.5.9）
    Audit { reply: oneshot::Sender<MarketResponse> },
    /// 信誉排行榜
    Leaderboard { limit: usize, reply: oneshot::Sender<MarketResponse> },
    /// 市场概览统计
    Stats { reply: oneshot::Sender<MarketResponse> },
}

/// 市场响应
#[derive(Debug, Clone)]
pub enum MarketResponse {
    /// 成功，携带 JSON
    Ok(Value),
    /// 业务错误（携带错误信息）
    Err(String),
}

impl MarketResponse {
    pub fn ok(v: Value) -> Self {
        MarketResponse::Ok(v)
    }
    pub fn err<M: Into<String>>(m: M) -> Self {
        MarketResponse::Err(m.into())
    }
}

/// 市场 Actor 句柄（克隆廉价，内部是 mpsc Sender）
#[derive(Clone)]
pub struct MarketActorHandle {
    tx: mpsc::Sender<MarketCommand>,
}

impl MarketActorHandle {
    /// 启动一个市场 Actor，返回句柄
    pub fn spawn() -> Self {
        let (tx, mut rx) = mpsc::channel::<MarketCommand>(256);
        tokio::spawn(async move {
            let mut market = AgentMarket::new();
            while let Some(cmd) = rx.recv().await {
                dispatch(&mut market, cmd);
            }
        });
        Self { tx }
    }

    /// 指定最低质押启动
    pub fn spawn_with_min_stake(min_stake: Money) -> Self {
        let (tx, mut rx) = mpsc::channel::<MarketCommand>(256);
        tokio::spawn(async move {
            let mut market = AgentMarket::with_min_stake(min_stake);
            while let Some(cmd) = rx.recv().await {
                dispatch(&mut market, cmd);
            }
        });
        Self { tx }
    }

    /// 启动带持久化的市场 Actor（v2.6.1，GAP §6.1）。
    ///
    /// 启动时从只追加流水完整恢复账本（余额 / 充值 / 罚没 / 已结算任务），
    /// 每条写命令处理后增量 append 新流水并 upsert agents / tasks，
    /// 使重启后守恒检查与独立审计连续、不铸币、不丢账。
    pub fn spawn_with_store(store: Arc<PersistentStore>) -> Self {
        let (tx, mut rx) = mpsc::channel::<MarketCommand>(256);
        tokio::spawn(async move {
            let mut market = AgentMarket::new();

            // ── v2.8.3: 启动先校验账本哈希链（GAP §3.1，tamper-evident）──
            match store.verify_ledger_chain() {
                Ok(head) => {
                    if head.is_empty() {
                        println!("🔗 账本哈希链：空（全新库）");
                    } else {
                        println!("🔗 账本哈希链校验通过，head={}", &head[..head.len().min(12)]);
                    }
                }
                Err(seq) => {
                    if seq == u64::MAX {
                        eprintln!("🚨 CRITICAL: 账本链 head 锚定不一致（kv_meta 与链末不符），疑似持久化被篡改");
                    } else {
                        eprintln!("🚨 CRITICAL: 账本哈希链在 seq={seq} 处断裂，疑似持久化被篡改（当前以容错模式加载，请人工核查）");
                    }
                }
            }

            // ── 启动恢复：账本（权威，资金安全）。v2.8.3: 坏行显式报告（GAP §3.6）──
            let restored = match store.load_ledger_records_checked() {
                Ok(load) => {
                    for (seq, _payload, reason) in &load.corrupt {
                        eprintln!("⚠️ 账本流水 seq={seq} 损坏已跳过：{reason}");
                    }
                    let n = load.records.len();
                    match market.restore_ledger(load.records) {
                        Ok(()) => println!("✅ 账本已从磁盘恢复：{n} 条流水"),
                        Err(e) => eprintln!("⚠️ 账本恢复失败: {e}"),
                    }
                    n
                }
                Err(e) => {
                    eprintln!("⚠️ 账本读取失败: {e}（持久化降级，避免水位错位）");
                    0
                }
            };
            // v2.7.4: agents / tasks 真正注入内存 market（此前只取 .len() 打日志，
            // 导致重启后账本/余额恢复但 /agents、/tasks、/stats 全空）。
            match store.load_agents() {
                Ok(list) => {
                    let n = list.len();
                    market.restore_agents_from_store(list);
                    println!("✅ agents 已从磁盘恢复：{n} 个");
                }
                Err(e) => eprintln!("⚠️ agents 读取失败: {e}"),
            }
            match store.load_tasks() {
                Ok(list) => {
                    let n = list.len();
                    market.restore_tasks_from_store(list);
                    println!("✅ tasks 已从磁盘恢复：{n} 个");
                }
                Err(e) => eprintln!("⚠️ tasks 读取失败: {e}"),
            }

            // v2.8.3: 水位 = 成功恢复的逻辑记录数（GAP §3.6，不再用物理 COUNT，
            // 物理行数含损坏行会导致水位超前、账本空洞）。
            let mut ledger_water = restored;

            while let Some(cmd) = rx.recv().await {
                dispatch(&mut market, cmd);

                // ── 写后增量持久化（v2.8.3：逐条 append，失败不推进水位，GAP §3.6）──
                let records = market.settlement_records();
                while records.len() > ledger_water {
                    match store.append_ledger_record(&records[ledger_water]) {
                        Ok(()) => ledger_water += 1,
                        Err(e) => {
                            eprintln!("⚠️ 账本流水 append 失败（停在水位 {ledger_water}，下轮重试）: {e}");
                            break;
                        }
                    }
                }
                // agents / tasks 快照 upsert（幂等，覆盖最新状态）
                for a in market.snapshot_agents_for_store() {
                    let _ = store.upsert_agent(&a);
                }
                for t in market.snapshot_tasks_for_store() {
                    let _ = store.upsert_task(&t);
                }
            }
        });
        Self { tx }
    }

    /// 发送命令并等待响应
    async fn call(&self, make: impl FnOnce(oneshot::Sender<MarketResponse>) -> MarketCommand) -> MarketResponse {
        let (reply, rx) = oneshot::channel();
        let cmd = make(reply);
        if self.tx.send(cmd).await.is_err() {
            return MarketResponse::err("market actor 已关闭");
        }
        rx.await.unwrap_or_else(|_| MarketResponse::err("market actor 无响应"))
    }

    pub async fn register_agent(&self, card: Value) -> MarketResponse {
        self.call(|reply| MarketCommand::RegisterAgent { card, reply }).await
    }
    pub async fn get_agent(&self, agent_id: String) -> MarketResponse {
        self.call(|reply| MarketCommand::GetAgent { agent_id, reply }).await
    }
    pub async fn discover(&self, skill: String) -> MarketResponse {
        self.call(|reply| MarketCommand::Discover { skill, reply }).await
    }
    pub async fn search(&self, query: String) -> MarketResponse {
        self.call(|reply| MarketCommand::Search { query, reply }).await
    }
    pub async fn publish_task(&self, task: Value) -> MarketResponse {
        self.call(|reply| MarketCommand::PublishTask { task, reply }).await
    }
    pub async fn get_task(&self, task_id: String) -> MarketResponse {
        self.call(|reply| MarketCommand::GetTask { task_id, reply }).await
    }
    pub async fn list_tasks(&self) -> MarketResponse {
        self.call(|reply| MarketCommand::ListTasks { reply }).await
    }
    pub async fn submit_bid(&self, bid: Value) -> MarketResponse {
        self.call(|reply| MarketCommand::SubmitBid { bid, reply }).await
    }
    pub async fn match_task(&self, task_id: String) -> MarketResponse {
        self.call(|reply| MarketCommand::MatchTask { task_id, reply }).await
    }
    pub async fn submit_result(&self, envelope: Value) -> MarketResponse {
        self.call(|reply| MarketCommand::SubmitResult { envelope, reply }).await
    }
    /// 认证式验证：固定委员集 + 委员签名票（v2.5.9）
    pub async fn verify_result(
        &self,
        task_id: String,
        round: u32,
        members: Vec<(String, [u8; 32])>,
        signed_votes: Vec<SignedQaVote>,
        now: u64,
    ) -> MarketResponse {
        self.call(move |reply| MarketCommand::VerifyResult {
            task_id,
            round,
            members,
            signed_votes,
            now,
            reply,
        })
        .await
    }
    pub async fn settle_task(&self, task_id: String) -> MarketResponse {
        self.call(|reply| MarketCommand::SettleTask { task_id, reply }).await
    }
    /// 恢复边：返工任务回到执行中（v2.6.0）
    pub async fn resume_rework(&self, task_id: String) -> MarketResponse {
        self.call(|reply| MarketCommand::ResumeRework { task_id, reply }).await
    }
    /// 恢复边：无共识任务重新开放（v2.6.0）
    pub async fn reopen_task(&self, task_id: String) -> MarketResponse {
        self.call(|reply| MarketCommand::ReopenTask { task_id, reply }).await
    }
    pub async fn open_dispute(&self, dispute: Value) -> MarketResponse {
        self.call(|reply| MarketCommand::OpenDispute { dispute, reply }).await
    }
    pub async fn arbitrate(&self, dispute_id: String, guilty: bool, slash: Money) -> MarketResponse {
        self.call(|reply| MarketCommand::Arbitrate { dispute_id, guilty, slash, reply }).await
    }
    pub async fn deposit(&self, account: String, amount: Money) -> MarketResponse {
        self.call(|reply| MarketCommand::Deposit { account, amount, reply }).await
    }
    pub async fn balance(&self, account: String) -> MarketResponse {
        self.call(|reply| MarketCommand::Balance { account, reply }).await
    }
    pub async fn conservation(&self) -> MarketResponse {
        self.call(|reply| MarketCommand::Conservation { reply }).await
    }
    /// 独立审计（v2.5.9）
    pub async fn audit(&self) -> MarketResponse {
        self.call(|reply| MarketCommand::Audit { reply }).await
    }
    pub async fn leaderboard(&self, limit: usize) -> MarketResponse {
        self.call(|reply| MarketCommand::Leaderboard { limit, reply }).await
    }
    pub async fn stats(&self) -> MarketResponse {
        self.call(|reply| MarketCommand::Stats { reply }).await
    }
}

/// 在 Actor 内部分发命令到 AgentMarket
fn dispatch(market: &mut AgentMarket, cmd: MarketCommand) {
    match cmd {
        MarketCommand::RegisterAgent { card, reply } => {
            match serde_json::from_value::<RegisterAgentInput>(card) {
                Ok(input) => {
                    match input.into_card() {
                        Ok(c) => match market.register_agent(c) {
                            Ok(id) => {
                                let _ = reply.send(MarketResponse::ok(serde_json::json!({
                                    "status": "registered", "agent_id": id
                                })));
                            }
                            Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
                        },
                        Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
                    }
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(format!("卡片格式错误: {e}"))); }
            }
        }
        MarketCommand::GetAgent { agent_id, reply } => {
            match market.get_agent(&agent_id) {
                Some(a) => { let _ = reply.send(MarketResponse::ok(serde_json::to_value(a).unwrap())); }
                None => { let _ = reply.send(MarketResponse::err(format!("NOT_FOUND: agent 不存在: {agent_id}"))); }
            }
        }
        MarketCommand::Discover { skill, reply } => {
            let agents: Vec<&MarketAgentCard> = market.discover_by_skill(&skill);
            let _ = reply.send(MarketResponse::ok(serde_json::json!({
                "skill": skill, "count": agents.len(),
                "agents": agents,
            })));
        }
        MarketCommand::Search { query, reply } => {
            let agents: Vec<&MarketAgentCard> = market.search_agents(&query);
            let _ = reply.send(MarketResponse::ok(serde_json::json!({
                "query": query, "count": agents.len(),
                "agents": agents,
            })));
        }
        MarketCommand::PublishTask { task, reply } => {
            match serde_json::from_value::<TaskSpec>(task) {
                Ok(mut t) => {
                    t.state = TaskState::Open;
                    match market.publish_task(t) {
                        Ok(id) => {
                            let _ = reply.send(MarketResponse::ok(serde_json::json!({
                                "status": "published", "task_id": id
                            })));
                        }
                        Err(gaps) => {
                            let _ = reply.send(MarketResponse::err(format!("任务校验失败: {}", gaps.join("; "))));
                        }
                    }
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(format!("任务格式错误: {e}"))); }
            }
        }
        MarketCommand::GetTask { task_id, reply } => {
            match market.get_task(&task_id) {
                Some(t) => { let _ = reply.send(MarketResponse::ok(serde_json::to_value(t).unwrap())); }
                None => { let _ = reply.send(MarketResponse::err(format!("NOT_FOUND: task 不存在: {task_id}"))); }
            }
        }
        MarketCommand::ListTasks { reply } => {
            // 通过搜索无法直接列出，用统计 + 空查询；这里复用 get_task 不可行，
            // 改为遍历：AgentMarket 未暴露迭代器，用 leaderboard 之外的方式。
            // 简化：返回统计信息。
            let _ = reply.send(MarketResponse::ok(serde_json::json!({
                "task_count": market.task_count(),
                "settled_count": market.settled_count(),
            })));
        }
        MarketCommand::SubmitBid { bid, reply } => {
            match serde_json::from_value::<Bid>(bid) {
                Ok(b) => match market.submit_bid(b) {
                    Ok(_) => { let _ = reply.send(MarketResponse::ok(serde_json::json!({"status": "bid_submitted"}))); }
                    Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
                },
                Err(e) => { let _ = reply.send(MarketResponse::err(format!("投标格式错误: {e}"))); }
            }
        }
        MarketCommand::MatchTask { task_id, reply } => {
            match market.match_task(&task_id) {
                Ok(agent_id) => {
                    let _ = reply.send(MarketResponse::ok(serde_json::json!({
                        "status": "matched", "task_id": task_id, "agent_id": agent_id
                    })));
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
            }
        }
        MarketCommand::SubmitResult { envelope, reply } => {
            match serde_json::from_value::<ResultEnvelope>(envelope) {
                Ok(e) => match market.submit_result(e) {
                    Ok(_) => { let _ = reply.send(MarketResponse::ok(serde_json::json!({"status": "result_submitted"}))); }
                    Err(err) => { let _ = reply.send(MarketResponse::err(err)); }
                },
                Err(e) => { let _ = reply.send(MarketResponse::err(format!("结果格式错误: {e}"))); }
            }
        }
        MarketCommand::VerifyResult {
            task_id, round, members, signed_votes, now, reply,
        } => {
            // v2.5.9：只接受固定委员集成员的有效签名票，不再合成 qa-N 委员与票
            match market.verify_result_authenticated(
                &task_id, round, members, signed_votes, now,
            ) {
                Ok(decision) => {
                    let tag = match decision {
                        QaDecision::Stop => "stop",
                        QaDecision::Continue => "continue",
                        QaDecision::NoQuorum => "no_quorum",
                    };
                    let _ = reply.send(MarketResponse::ok(serde_json::json!({
                        "task_id": task_id,
                        "decision": tag,
                        "accepted": decision == QaDecision::Stop,
                    })));
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
            }
        }
        MarketCommand::SettleTask { task_id, reply } => {
            match market.settle_task(&task_id) {
                Ok(amount) => {
                    let _ = reply.send(MarketResponse::ok(serde_json::json!({
                        "status": "settled", "task_id": task_id, "amount": amount
                    })));
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
            }
        }
        MarketCommand::ResumeRework { task_id, reply } => {
            match market.resume_after_rework(&task_id) {
                Ok(()) => {
                    let _ = reply.send(MarketResponse::ok(serde_json::json!({
                        "status": "running", "task_id": task_id
                    })));
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
            }
        }
        MarketCommand::ReopenTask { task_id, reply } => {
            match market.reopen_after_no_quorum(&task_id) {
                Ok(()) => {
                    let _ = reply.send(MarketResponse::ok(serde_json::json!({
                        "status": "open", "task_id": task_id
                    })));
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
            }
        }
        MarketCommand::OpenDispute { dispute, reply } => {
            let dispute_id = dispute.get("dispute_id").and_then(|v| v.as_str())
                .unwrap_or(&format!("dispute-{}", uuid::Uuid::new_v4())).to_string();
            let task_id = dispute.get("task_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let complainant = dispute.get("complainant").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let reason = dispute.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();
            match market.open_dispute(&dispute_id, &task_id, &complainant, &reason) {
                Ok(_) => {
                    let _ = reply.send(MarketResponse::ok(serde_json::json!({
                        "status": "dispute_opened", "dispute_id": dispute_id
                    })));
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
            }
        }
        MarketCommand::Arbitrate { dispute_id, guilty, slash, reply } => {
            match market.arbitrate(&dispute_id, guilty, slash) {
                Ok(verdict) => {
                    let _ = reply.send(MarketResponse::ok(serde_json::json!({
                        "status": "arbitrated",
                        "dispute_id": dispute_id,
                        "verdict": verdict,
                        "slash_amount": if guilty { slash } else { Money::ZERO },
                    })));
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
            }
        }
        MarketCommand::Deposit { account, amount, reply } => {
            match market.deposit(&account, amount) {
                Ok(_) => {
                    let _ = reply.send(MarketResponse::ok(serde_json::json!({
                        "status": "deposited", "account": account,
                        "amount": amount, "balance": market.balance(&account),
                    })));
                }
                Err(e) => { let _ = reply.send(MarketResponse::err(e)); }
            }
        }
        MarketCommand::Balance { account, reply } => {
            let _ = reply.send(MarketResponse::ok(serde_json::json!({
                "account": account, "balance": market.balance(&account),
            })));
        }
        MarketCommand::Conservation { reply } => {
            let report = market.conservation_check();
            let _ = reply.send(MarketResponse::ok(serde_json::json!({
                "conserved": report.conserved,
                "total_deposits": report.total_deposits,
                "total_paid": report.total_paid,
                "total_slashed": report.total_slashed,
                "balance_sum": report.balance_sum,
            })));
        }
        MarketCommand::Audit { reply } => {
            let report = market.independent_audit();
            let _ = reply.send(
                MarketResponse::ok(serde_json::to_value(&report).unwrap_or_else(|e| {
                    serde_json::json!({"passed": false, "error": format!("审计序列化失败: {e}")})
                })),
            );
        }
        MarketCommand::Leaderboard { limit, reply } => {
            let board: Vec<Value> = market
                .leaderboard(limit)
                .into_iter()
                .map(|(id, score)| serde_json::json!({"agent_id": id, "reputation": score}))
                .collect();
            let _ = reply.send(MarketResponse::ok(serde_json::json!({"leaderboard": board})));
        }
        MarketCommand::Stats { reply } => {
            let _ = reply.send(MarketResponse::ok(serde_json::json!({
                "agent_count": market.agent_count(),
                "task_count": market.task_count(),
                "dispute_count": market.dispute_count(),
                "settled_count": market.settled_count(),
            })));
        }
    }
}
