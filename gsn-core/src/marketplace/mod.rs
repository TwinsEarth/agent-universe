//! 智能体市场（Agent Market）v2.3.4
//!
//! 让智能体、技能、任务、算力、模型可以：
//! 注册 → 发现 → 匹配 → 执行 → 验证 → 结算 → 信誉更新
//!
//! 核心机制：
//! - BFT-lite QA 委员会（n≥3f+1）
//! - 守恒账本（balance_sum = paid - slashed）
//! - 多维信誉（不可转让）
//! - 质押罚没
//! - 证据分级

pub mod agent_card;
pub mod evidence;
pub mod governance;
pub mod money;
pub mod qa_committee;
pub mod reputation;
pub mod settlement;
pub mod task;

pub use agent_card::{
    AgentCategory, Currency, MarketAgentCard, Pricing, PricingModel, SchemaField, SkillManifest,
    Sla,
};
pub use evidence::EvidenceGrade;
pub use governance::{
    parse_arbitrate_claim, parse_credit_claim, Governance, GovernanceAction, GovernanceMemberEntry,
    SignedGovernanceCommand, GOV_CAP_ARBITRATE, GOV_CAP_CREDIT,
};
pub use money::Money;
pub use qa_committee::{QaCommittee, QaDecision, QaMember, QaVote, SignedQaVote};
pub use reputation::{MarketReputation, ReputationManager, StakeRecord, StakeStatus};
pub use settlement::{
    AccountMismatch, AuditReport, ConservationReport, SettlementEngine, SettlementReason,
    SettlementRecord,
};
pub use task::{ErrorType, ResultEnvelope, TaskSpec, TaskState, VerificationPolicy};

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// 质押锁定账户（注册时从自有余额转入）
pub fn stake_account(did: &str) -> String {
    format!("__stake__:{}", did)
}

/// 任务托管账户（发布任务时从需求方余额锁定）
pub fn escrow_account(task_id: &str) -> String {
    format!("__escrow__:{}", task_id)
}

/// 服务端罚没规则（v2.8.5，GAP §3.7/§3.8）：
///
/// 罚没金额**只由服务端规则决定，绝不接受请求体指定的金额**（根治
/// 「调用方决定罚多少」）。两条路径统一从同一规则函数出账，保证
/// 「质押记录」与「账本」口径一致：
/// - 仲裁判定作恶（guilty）：罚没**全部**质押（100%）；
/// - 验收终局拒绝 / 重复劳动（Rejected）：罚没 10% 质押。
pub const SLASH_RATE_ARBITRATION: i64 = 100;
pub const SLASH_RATE_REJECT: i64 = 10;

/// 认证式 QA 委员会人数下限（v3.5.1，AU-01/AU-05，QA 委员会策略常量）。
///
/// # BFT 依据
///
/// 通用 [`QaCommittee::with_fixed_members`] 令 `f = (n-1)/3`、法定人数
/// `quorum = 2f+1`。代入得：
///
/// | n  | f=(n-1)/3 | quorum=2f+1 | 单人能否自批 |
/// |----|-----------|-------------|--------------|
/// | 1  | 0         | 1           | ✅ 是（1 票即 Stop）|
/// | 2  | 0         | 1           | ✅ 是 |
/// | 3  | 0         | 1           | ✅ 是 |
/// | **4** | **1**   | **3**       | ❌ 需 3 张独立签名票，容忍 1 个恶意/宕机 |
///
/// 故取 `n >= 4` 才有 `f >= 1`。旧实现未设此下限，调用方带一把自造密钥、
/// `n=1` 即可凑成 Stop 自我批准——本常量在市场层闸门处杜绝该退化。
///
/// # 残留风险（如实声明）
///
/// 本闸门只锚定「独立、已足额质押、非任务执行者」的 DID 身份并设 BFT 下限；
/// 一个仍掌握 ≥4 个各自足额质押身份的策划者（Sybil）仍可凑齐 Stop。按质押
/// 加权的验证人集、作恶投票的 slashing 联动，属后续 minor（不在本补丁范围）。
pub const MIN_QA_COMMITTEE_SIZE: usize = 4;

/// 按质押总额与服务端百分比规则计算罚没金额。
fn slash_amount_by_rule(stake_total: Money, rate_pct: i64) -> Money {
    // 全额质押：直接返回总额；否则按比例（先乘后除，避免截断误差）。
    if rate_pct >= 100 {
        stake_total
    } else {
        Money::new(stake_total.as_i64().saturating_mul(rate_pct) / 100)
    }
}

/// 对字节求 SHA256，返回小写十六进制（重复劳动检测用）
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 投标
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Bid {
    pub agent_id: String,
    pub task_id: String,
    pub proposed_price: Money,
    pub estimated_latency_ms: u64,
    pub score: f64,
}

/// 争议记录
#[derive(Debug, Clone)]
pub struct DisputeCase {
    pub dispute_id: String,
    pub task_id: String,
    pub complainant: String,
    pub respondent: String,
    pub reason: String,
    pub resolved: bool,
    pub verdict: Option<String>,
    /// 仲裁者身份（v2.8.5，GAP §3.7）：仲裁时必须显式指定，拒绝匿名。
    pub arbitrator: Option<String>,
}

/// 智能体市场主管理器
pub struct AgentMarket {
    /// 注册的 Agent
    agents: HashMap<String, MarketAgentCard>,
    /// 技能索引：skill -> agent_ids
    skill_index: HashMap<String, Vec<String>>,
    /// 发布的任务
    tasks: HashMap<String, TaskSpec>,
    /// 投标：task_id -> bids
    bids: HashMap<String, Vec<Bid>>,
    /// 结果：task_id -> envelope
    results: HashMap<String, ResultEnvelope>,
    /// 已提交结果的内容哈希：task_id -> sha256(report)（重复劳动检测）
    result_hashes: HashMap<String, String>,
    /// 重复劳动标记：返工后提交与上次完全相同的结果
    duplicate_work: HashSet<String>,
    /// 争议
    disputes: Vec<DisputeCase>,
    /// 结算引擎
    settlement: SettlementEngine,
    /// 信誉管理器
    reputation_mgr: ReputationManager,
    /// 最低质押
    min_stake: Money,
    /// 已消费的认证 QA 投票 nonce（v3.5.2，AU-04，跨请求重放去重）。
    ///
    /// 键为 `(task_id, round, voter_did, nonce)`。通用 [`QaCommittee`] 实例内的
    /// `seen_nonces` 在每次 `verify_result_authenticated` 新建委员会时被清空，
    /// 无法跨请求去重；本字段把去重提升到市场层，使其在同一进程的多次验收调用间存活。
    ///
    /// # 残留边界（如实声明）
    ///
    /// 这是**运行期跨请求**去重；服务重启后本集合随内存重建而清空。跨重启窗口内的重放
    /// 由投票自身的服务端时间窗约束（`cast_signed_vote` 强制 `now ∈ [issued_at,
    /// expires_at]`，过期票一律拒绝）。要把已消费 nonce 落盘持久化需要既有 PersistentStore
    /// 的 schema 扩张，超出本补丁（patch）语义，列入后续 minor。
    seen_qa_nonces: HashSet<(String, u32, String, String)>,
    /// 受权治理集（P0-4）：仲裁罚没 / off-chain 授信两类特权写的签名校验者。
    ///
    /// 从 `GSN_GOVERNANCE_FILE`（JSON `[{did,pubkey_hex}]`）加载；未设置/空 =
    /// 治理集为空 = 特权路径 fail-closed（无人能经未认证命令罚没/铸币）。
    governance: Governance,
    /// 已消费的签名 PoCV nonce（P0-3，跨请求重放去重；重启后由时间窗兜底）。
    seen_pocv_nonces: HashSet<String>,
}

impl AgentMarket {
    pub fn new() -> Self {
        let min_stake = Money::MIN_STAKE;
        Self {
            agents: HashMap::new(),
            skill_index: HashMap::new(),
            tasks: HashMap::new(),
            bids: HashMap::new(),
            results: HashMap::new(),
            result_hashes: HashMap::new(),
            duplicate_work: HashSet::new(),
            disputes: Vec::new(),
            settlement: SettlementEngine::new(),
            reputation_mgr: ReputationManager::new(min_stake),
            min_stake,
            seen_qa_nonces: HashSet::new(),
            governance: Governance::from_env(),
            seen_pocv_nonces: HashSet::new(),
        }
    }

    /// 带自定义最低质押创建
    pub fn with_min_stake(min_stake: Money) -> Self {
        Self {
            agents: HashMap::new(),
            skill_index: HashMap::new(),
            tasks: HashMap::new(),
            bids: HashMap::new(),
            results: HashMap::new(),
            result_hashes: HashMap::new(),
            duplicate_work: HashSet::new(),
            disputes: Vec::new(),
            settlement: SettlementEngine::new(),
            reputation_mgr: ReputationManager::new(min_stake),
            min_stake,
            seen_qa_nonces: HashSet::new(),
            governance: Governance::from_env(),
            seen_pocv_nonces: HashSet::new(),
        }
    }

    // ===== F1: Agent 注册 =====

    /// 注册 Agent
    pub fn register_agent(&mut self, card: MarketAgentCard) -> Result<String, String> {
        if card.agent_id.trim().is_empty() {
            return Err("agent_id 不能为空".to_string());
        }
        // 幂等：禁止重复注册（避免重复锁定质押、技能索引重复追加、记录被静默覆盖）
        if self.agents.contains_key(&card.agent_id) {
            return Err(format!(
                "Agent {} 已注册，禁止重复注册（如需更新资料请走专门的更新接口）",
                card.agent_id
            ));
        }
        if card.name.trim().is_empty() {
            return Err("name 不能为空".to_string());
        }
        if card.stake < self.min_stake {
            return Err(format!(
                "质押不足：需要至少 {}，当前 {}",
                self.min_stake, card.stake
            ));
        }

        // 资金校验：质押金必须来自 Agent 自有余额（不凭空铸造）
        if self.settlement.balance(&card.agent_id) < card.stake {
            return Err(format!(
                "Agent {} 余额不足以锁定质押 {}，请先 deposit",
                card.agent_id, card.stake
            ));
        }

        // 质押金从自有余额转入锁定账户（可被罚没），并进入只追加流水
        let stake_acct = stake_account(&card.agent_id);
        self.settlement.lock(
            &card.agent_id,
            &card.agent_id,
            &stake_acct,
            card.stake,
            SettlementReason::Staked,
        )?;

        // 注册质押记录
        self.reputation_mgr
            .register_stake(&card.agent_id, card.stake)?;

        // 建立技能索引
        for skill in &card.skills {
            self.skill_index
                .entry(skill.to_lowercase())
                .or_default()
                .push(card.agent_id.clone());
        }

        let id = card.agent_id.clone();
        self.agents.insert(id.clone(), card);
        Ok(id)
    }

    /// 获取 Agent
    pub fn get_agent(&self, agent_id: &str) -> Option<&MarketAgentCard> {
        self.agents.get(agent_id)
    }

    /// 列出全部已注册 Agent（工作台，按 agent_id 排序）
    pub fn list_all_agents(&self) -> Vec<MarketAgentCard> {
        let mut cards: Vec<MarketAgentCard> = self.agents.values().cloned().collect();
        cards.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        cards
    }

    // ===== F2: 任务发布 =====

    /// 发布任务
    pub fn publish_task(&mut self, mut task: TaskSpec) -> Result<String, Vec<String>> {
        task.validate()?;

        // 发布即托管：需求方必须有足额预算，并锁定到托管账户（不托管则拒绝）
        if self.settlement.balance(&task.requester) < task.budget {
            return Err(vec![format!(
                "需求方 {} 余额不足以覆盖预算 {}，请先 deposit",
                task.requester, task.budget
            )]);
        }
        let escrow = escrow_account(&task.task_id);
        self.settlement
            .lock(
                &task.task_id,
                &task.requester,
                &escrow,
                task.budget,
                SettlementReason::Escrowed,
            )
            .map_err(|e| vec![e])?;

        task.winner_price = None;
        task.state = TaskState::Open;

        let id = task.task_id.clone();
        self.tasks.insert(id.clone(), task);
        Ok(id)
    }

    /// 获取任务
    pub fn get_task(&self, task_id: &str) -> Option<&TaskSpec> {
        self.tasks.get(task_id)
    }

    /// 列出全部任务（工作台，新创建的在前）
    pub fn list_all_tasks(&self) -> Vec<TaskSpec> {
        let mut v: Vec<TaskSpec> = self.tasks.values().cloned().collect();
        v.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then(a.task_id.cmp(&b.task_id))
        });
        v
    }

    /// v2.8.4: 读取任务结果信封（GAP §3.2，验证结果信封重启后存活）。
    pub fn get_result(&self, task_id: &str) -> Option<&ResultEnvelope> {
        self.results.get(task_id)
    }

    /// v2.8.4: 资格查询（GAP §3.2，验证质押恢复后仍满足资格）。
    pub fn is_eligible(&self, agent_id: &str, min_reputation: f64) -> bool {
        self.reputation_mgr.is_eligible(agent_id, min_reputation)
    }

    // ===== F3: 发现与匹配 =====

    /// 按技能发现 Agent
    pub fn discover_by_skill(&self, skill: &str) -> Vec<&MarketAgentCard> {
        self.skill_index
            .get(&skill.to_lowercase())
            .map(|ids| ids.iter().filter_map(|id| self.agents.get(id)).collect())
            .unwrap_or_default()
    }

    /// 搜索 Agent
    pub fn search_agents(&self, query: &str) -> Vec<&MarketAgentCard> {
        let q = query.to_lowercase();
        self.agents
            .values()
            .filter(|a| {
                a.name.to_lowercase().contains(&q)
                    || a.description.to_lowercase().contains(&q)
                    || a.skills.iter().any(|s| s.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// 提交投标
    pub fn submit_bid(&mut self, bid: Bid) -> Result<(), String> {
        if !self.agents.contains_key(&bid.agent_id) {
            return Err(format!("Agent {} 未注册", bid.agent_id));
        }
        let task = self
            .tasks
            .get(&bid.task_id)
            .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", bid.task_id))?;

        // 价格校验（GAP §2.7）：投标价必须为正且不超过预算
        if !bid.proposed_price.is_positive() {
            return Err("投标价必须为正（0 或负数投标被拒绝）".to_string());
        }
        if bid.proposed_price > task.budget {
            return Err(format!(
                "投标价 {} 超过任务预算 {}",
                bid.proposed_price, task.budget
            ));
        }
        // 状态前置：仅 OPEN 任务接受投标
        if task.state != TaskState::Open {
            return Err(format!("任务状态为 {}，已截止投标", task.state.label()));
        }

        // 检查资格
        if !self.reputation_mgr.is_eligible(&bid.agent_id, 0.3) {
            return Err(format!("Agent {} 不满足资格要求", bid.agent_id));
        }

        self.bids.entry(bid.task_id.clone()).or_default().push(bid);
        Ok(())
    }

    /// 匹配任务（选择性价比最高的投标）
    pub fn match_task(&mut self, task_id: &str) -> Result<String, String> {
        let bids = self
            .bids
            .get(task_id)
            .ok_or_else(|| format!("任务 {} 无投标", task_id))?;

        if bids.is_empty() {
            return Err(format!("任务 {} 无有效投标", task_id));
        }

        // 计算性价比分数并选择最高
        let mut best: Option<&Bid> = None;
        let mut best_score = f64::MIN;

        for bid in bids {
            let _agent = self
                .agents
                .get(&bid.agent_id)
                .ok_or_else(|| format!("Agent {} 信息缺失", bid.agent_id))?;

            let rep_score = self
                .reputation_mgr
                .reputation(&bid.agent_id)
                .map(|r| r.overall())
                .unwrap_or(0.5);

            // 性价比 = 信誉 / 价格（submit_bid 已保证价格为正）
            let price_f = bid.proposed_price.as_i64() as f64;
            let cost_score = rep_score / price_f;
            // 延迟惩罚
            let latency_penalty = 1.0 / (1.0 + bid.estimated_latency_ms as f64 / 1000.0);
            let score = cost_score * latency_penalty;

            if score > best_score {
                best_score = score;
                best = Some(bid);
            }
        }

        let best_bid = best.ok_or_else(|| format!("任务 {} 无有效中标候选", task_id))?;
        let winner = best_bid.agent_id.clone();
        let winner_price = best_bid.proposed_price;

        // 更新任务状态并记录中标价
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.state = task.state.transition(TaskState::Matched)?;
            task.owner = Some(winner.clone());
            task.winner_price = Some(winner_price);
        }

        Ok(winner)
    }

    // ===== v3.5.0 主数据面接管：插件喂数据 / 应用插件结果 =====

    /// 构造 market-match 插件期望的投标 payload（含每投标者信誉与延迟）。
    ///
    /// 供插件编排器在主数据面 match 前喂给 `market-match.match`；宿主不再
    /// 在主路由里自行计算中标，而是把真实投标交给插件决策。
    pub fn bids_for_plugin(&self, task_id: &str) -> Result<Value, String> {
        let bids = self
            .bids
            .get(task_id)
            .ok_or_else(|| format!("任务 {} 无投标", task_id))?;
        let arr: Vec<Value> = bids
            .iter()
            .map(|b| {
                let rep = self
                    .reputation_mgr
                    .reputation(&b.agent_id)
                    .map(|r| r.overall())
                    .unwrap_or(0.5);
                json!({
                    "agent_id": b.agent_id,
                    "price": b.proposed_price,
                    "reputation": rep,
                    "latency_ms": b.estimated_latency_ms,
                })
            })
            .collect();
        Ok(json!({ "bids": arr }))
    }

    /// 应用 market-match 插件算出的中标方（校验 winner 是有效投标者之一）。
    ///
    /// 与旧 [`match_task`](Self::match_task) 的区别：中标决策由插件产出，
    /// 这里只校验 winner 合法性并更新状态 / owner / 中标价（宿主应用）。
    pub fn match_task_with_winner(
        &mut self,
        task_id: &str,
        winner: &str,
    ) -> Result<String, String> {
        let winning_price = {
            let bids = self
                .bids
                .get(task_id)
                .ok_or_else(|| format!("任务 {} 无投标", task_id))?;
            bids.iter()
                .find(|b| b.agent_id == winner)
                .map(|b| b.proposed_price)
                .ok_or_else(|| format!("插件中标方 {winner} 不是任务 {task_id} 的有效投标者"))?
        };
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.state = task.state.transition(TaskState::Matched)?;
            task.owner = Some(winner.to_string());
            task.winner_price = Some(winning_price);
        }
        Ok(winner.to_string())
    }

    /// 取投标者四维信誉（喂给 economy-reputation 插件 `overall`）。
    /// 新注册、尚未产生信誉的 agent 由默认信誉（0.5）兜底。
    pub fn reputation_dimensions(&self, agent_id: &str) -> (f64, f64, f64, f64) {
        self.reputation_mgr
            .reputation(agent_id)
            .map(|r| (r.quality, r.speed, r.honesty, r.availability))
            .unwrap_or((0.5, 0.5, 0.5, 0.5))
    }

    /// 构造 scheduler-task 插件期望的候选节点 payload（发布前可路由性预检）。
    ///
    /// 已注册 agent 作为候选，`load=0`（发布时无在途负载），延迟给默认值；
    /// 让发布在有候选时经插件确认可路由。
    pub fn routing_candidates_for_plugin(&self) -> Value {
        let nodes: Vec<Value> = self
            .agents
            .values()
            .map(|c| {
                json!({
                    "did": c.agent_id,
                    "load": 0,
                    "latency_ms": 100,
                })
            })
            .collect();
        json!({ "nodes": nodes })
    }

    // ===== F4/F5: 执行与验证 =====

    /// 提交执行结果
    pub fn submit_result(&mut self, mut envelope: ResultEnvelope) -> Result<(), String> {
        let task = self
            .tasks
            .get(&envelope.task_id)
            .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", envelope.task_id))?;

        if task.state != TaskState::Matched && task.state != TaskState::Running {
            return Err(format!("任务状态为 {}，不能提交结果", task.state.label()));
        }

        // 重复劳动检测：对结果报告内容求 SHA256，与上次提交比对
        let content_hash = sha256_hex(envelope.report.as_bytes());
        let is_duplicate = self
            .result_hashes
            .get(&envelope.task_id)
            .map(|prev| prev == &content_hash)
            .unwrap_or(false);

        if let Some(task) = self.tasks.get_mut(&envelope.task_id) {
            if is_duplicate {
                // 返工后提交与上次完全相同的结果 = 重复劳动，终局拒绝
                task.state = task.state.transition(TaskState::Rejected)?;
            } else {
                // policy=None：无需 QA，提交结果直接验收；否则进入验收阶段
                let target = if matches!(task.verification_policy, VerificationPolicy::None) {
                    TaskState::Accepted
                } else {
                    TaskState::Verifying
                };
                task.state = task.state.transition(target)?;
            }
        }

        if is_duplicate {
            self.duplicate_work.insert(envelope.task_id.clone());
        } else {
            self.result_hashes
                .insert(envelope.task_id.clone(), content_hash);
        }

        // v3.5.1（AU-18）：信封自报的 evidence_grade 不可信，入库即强制降为
        // Unverified。旧实现原样落库，执行者自报 Verified 即可直接满足结算可信
        // 闸门。此后只有认证 QA Stop（`verify_result_authenticated`）或仲裁路径
        // 才能把证据提升回 Verified/CpuProto（服务端签发，执行者无法自报）。
        envelope.evidence_grade = EvidenceGrade::Unverified;

        // P0-3 接线点：随结果提交的签名 PoCV 在入库前校验一次。
        // 实际产出一致性绑定：actual_input = task_id（被处理对象），
        // actual_output = report（实际产出正文）。证明有效则留痕；无效/篡改直接拒绝
        // 整个结果（4xx 语义），不静默入库。注意诚实边界：有有效签名证明 ≠ 计算正确，
        // 故不提升 evidence_grade；BFT QA 仍是策略门。需外部审计。
        if let Some(proof) = envelope.pocv.take() {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            proof.verify(
                envelope.task_id.as_bytes(),
                envelope.report.as_bytes(),
                &mut self.seen_pocv_nonces,
                now_secs,
            )?;
            envelope.pocv = Some(proof);
        }

        self.results.insert(envelope.task_id.clone(), envelope);
        Ok(())
    }

    /// QA 验证
    pub fn verify_result(
        &mut self,
        task_id: &str,
        committee: &QaCommittee,
    ) -> Result<QaDecision, String> {
        let decision = committee.tally();

        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;

        let target = match decision {
            QaDecision::Stop => TaskState::Accepted,
            QaDecision::Continue => TaskState::Rework,
            QaDecision::NoQuorum => TaskState::NoQuorum,
        };
        task.state = task.state.transition(target)?;

        Ok(decision)
    }

    /// 认证式 QA 验证（v2.5.9，GAP §3.1）
    ///
    /// `members` 为固定委员集 `(did, 公钥)`，`signed_votes` 为委员用私钥签发的
    /// 真实投票。调用方可以指定委员集，但**无法伪造票**，从而根治「服务端按
    /// approvals 合成委员与票、可自我批准」。
    ///
    /// # v3.5.1（AU-01/AU-05）服务端资格闸门
    ///
    /// 在构造委员会 / 验票**之前**，先在服务端对委员身份做三道闸门（任一不满足
    /// 即拒绝，错误信息带稳定前缀 `COMMITTEE_TOO_SMALL` /
    /// `COMMITTEE_EXECUTOR_CONFLICT` / `COMMITTEE_NOT_STAKED`，可被上层断言）：
    ///
    /// 1. **人数下限**：`members.len() >= [`MIN_QA_COMMITTEE_SIZE`]`（n≥4 → f≥1），
    ///    杜绝 n=1/f=0 的单人自批；
    /// 2. **执行者回避**：任务必须存在，且其 `owner`（中标执行者）不得出现在
    ///    委员 DID 集合中（利益回避，执行者不能给自己打分）；
    /// 3. **服务端质押锚定**：每个委员 DID 都必须在本服务端
    ///    [`ReputationManager`] 中持有有效锁定质押（`status==Locked` 且
    ///    `amount>=min_stake`，见 [`ReputationManager::has_locked_stake`]）。
    ///
    /// 本闸门不修改通用 [`QaCommittee`] 库逻辑，也不改动密码学验签。
    ///
    /// ## 残留风险（Sybil）
    ///
    /// 本补丁锚定的是「独立、已质押、非执行者」身份并设 BFT 下限；一个仍掌握
    /// ≥4 个各自足额质押身份的策划型攻击者（Sybil）仍可凑齐 Stop 票。按质押
    /// 加权的验证人集与 slashing 联动属后续 minor，不在本补丁范围。
    pub fn verify_result_authenticated(
        &mut self,
        task_id: &str,
        round: u32,
        members: Vec<(String, [u8; 32])>,
        signed_votes: Vec<SignedQaVote>,
        now: u64,
    ) -> Result<QaDecision, String> {
        // ── 闸门 1：委员会人数下限（BFT：n≥4 → f≥1，quorum≥3）──
        if members.len() < MIN_QA_COMMITTEE_SIZE {
            return Err(format!(
                "COMMITTEE_TOO_SMALL: 委员数 {} 低于下限 {MIN_QA_COMMITTEE_SIZE}（n≥4 才能保证 f≥1、quorum≥3，杜绝单人自批）",
                members.len()
            ));
        }

        // ── 闸门 2：任务存在 + 执行者回避（先克隆 owner 出作用域，避免借用冲突）──
        let owner = self
            .tasks
            .get(task_id)
            .map(|t| t.owner.clone())
            .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;
        if let Some(owner) = &owner {
            if members.iter().any(|(did, _)| did == owner) {
                return Err(format!(
                    "COMMITTEE_EXECUTOR_CONFLICT: 任务执行者 {owner} 不得出任 QA 委员（利益回避）"
                ));
            }
        }

        // ── 闸门 3：每个委员 DID 必须在服务端持有有效锁定质押 ──
        for (did, _) in &members {
            if !self.reputation_mgr.has_locked_stake(did) {
                return Err(format!(
                    "COMMITTEE_NOT_STAKED: 委员 {did} 无有效锁定质押（需 status=Locked 且 amount≥min_stake），拒绝认证验收"
                ));
            }
        }

        // ── 闸门 4：跨请求 nonce 重放去重（v3.5.2，AU-04）──
        // 通用 QaCommittee 实例内的 seen_nonces 随每次新建委员会而清空，无法跨请求去重；
        // 此处把 (task_id, round, voter, nonce) 提升到市场层集合做只读预检。任一已被
        // 本进程历史消费过即拒绝（稳定前缀 QA_NONCE_REPLAY）。
        let vote_keys: Vec<(String, u32, String, String)> = signed_votes
            .iter()
            .map(|sv| {
                (
                    sv.task_id.clone(),
                    sv.round,
                    sv.voter.clone(),
                    sv.nonce.clone(),
                )
            })
            .collect();
        for k in &vote_keys {
            if self.seen_qa_nonces.contains(k) {
                return Err(format!(
                    "QA_NONCE_REPLAY: 委员 {} 在任务 {} 轮 {} 的投票 nonce {} 已被消费过（重放）",
                    k.2, k.0, k.1, k.3
                ));
            }
        }

        let mut committee = QaCommittee::with_fixed_members(task_id, round, members)?;
        for sv in signed_votes {
            committee.cast_signed_vote(sv, now)?;
        }
        let decision = committee.tally();

        let target = match decision {
            QaDecision::Stop => TaskState::Accepted,
            QaDecision::Continue => TaskState::Rework,
            QaDecision::NoQuorum => TaskState::NoQuorum,
        };

        // 状态转换必须先成功（v2.8.5，GAP §3.8）：转换失败时直接返回错误，
        // 绝不提前升级证据——旧实现先升级证据再转换，一旦转换失败就会留下
        // 「状态未变、证据却已被永久标记为可信」的不可回滚污染。
        {
            let task = self
                .tasks
                .get_mut(task_id)
                .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;
            task.state = task.state.transition(target)?;
        }

        // 状态转换成功后才升级证据（仅认证式 BFT 判定 Stop）：Unverified
        // 结果经固定委员集签名投票判定 Stop 后，将其证据等级提升为 Verified，
        // 使其满足 settle_task 的可信闸门；非认证 verify_result 路径不做此提升。
        if matches!(decision, QaDecision::Stop) {
            if let Some(env) = self.results.get_mut(task_id) {
                if !env.evidence_grade.is_trustworthy() {
                    env.evidence_grade = EvidenceGrade::Verified;
                }
            }
        }

        // 整条验收链成功落库后，才把本轮投票 nonce 记入市场级去重表（跨请求存活）。
        // 此前任一环节失败（验签失败/状态转换失败）都不消费 nonce，避免误烧合法票。
        for k in vote_keys {
            self.seen_qa_nonces.insert(k);
        }

        Ok(decision)
    }

    /// 恢复边：返工任务回到执行中（v2.6.0，GAP §3.4）。
    ///
    /// QA 判 Continue 后任务进入 Rework；执行者按意见修改后调用本方法，
    /// 任务回到 Running，随后可重新 `submit_result` 提交新一轮结果。
    pub fn resume_after_rework(&mut self, task_id: &str) -> Result<(), String> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;
        if task.state != TaskState::Rework {
            return Err(format!(
                "任务状态为 {}，非 REWORK，不能恢复执行",
                task.state.label()
            ));
        }
        task.state = task.state.transition(TaskState::Running)?;
        Ok(())
    }

    /// 恢复边：无共识任务重新开放（v2.6.0，GAP §3.4）。
    ///
    /// QA 本轮未达成共识（NoQuorum）时任务不再卡死；调用本方法把任务
    /// 重新置为 Open，可重新匹配或组织下一轮验收，消除 NoQuorum 吸收态。
    pub fn reopen_after_no_quorum(&mut self, task_id: &str) -> Result<(), String> {
        {
            let task = self
                .tasks
                .get_mut(task_id)
                .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;
            if task.state != TaskState::NoQuorum {
                return Err(format!(
                    "任务状态为 {}，非 NO_QUORUM，不能重新开放",
                    task.state.label()
                ));
            }
            task.state = task.state.transition(TaskState::Open)?;
        }
        // 流程重置（区别于返工）：清除上次结果哈希，允许重新提交相同结果，
        // 不被误判为重复劳动（重复劳动仅针对 QA 要求改进后的返工重提）
        self.result_hashes.remove(task_id);
        Ok(())
    }

    // ===== F6: 结算 =====

    /// 结算已验收任务
    /// 终局拒绝任务（GAP §2.8）：验收确认结果不通过，转 Rejected。
    ///
    /// 仅在 Verifying / Rework 状态可拒绝；若结果信封可信且成功则不允许拒绝
    /// （防止无依据拒付）。拒绝后调用 settle_task：付执行者 0、退预算、罚没。
    pub fn reject_task(&mut self, task_id: &str) -> Result<(), String> {
        let state = self
            .tasks
            .get(task_id)
            .map(|t| t.state)
            .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;
        if !matches!(state, TaskState::Verifying | TaskState::Rework) {
            return Err(format!(
                "任务状态为 {}，不能终局拒绝（仅验证中/返工可拒绝）",
                state.label()
            ));
        }
        if let Some(env) = self.results.get(task_id) {
            if env.evidence_grade.is_trustworthy() && env.is_success() {
                return Err("结果可信且成功，不能拒绝".to_string());
            }
        }
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;
        task.state = task.state.transition(TaskState::Rejected)?;
        Ok(())
    }

    pub fn settle_task(&mut self, task_id: &str) -> Result<Money, String> {
        let task = self
            .tasks
            .get(task_id)
            .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;

        let is_rejected = task.state == TaskState::Rejected;
        if task.state != TaskState::Accepted && !is_rejected {
            return Err(format!("任务状态为 {}，不能结算", task.state.label()));
        }

        let agent_id = task
            .owner
            .clone()
            .ok_or_else(|| "任务无执行 Agent".to_string())?;
        let requester = task.requester.clone();
        let budget = task.budget;
        let escrow = escrow_account(task_id);

        // ===== 拒绝 / 重复劳动分支（GAP §2.8）：付 0、退预算、罚没 =====
        if is_rejected {
            let reason = if self.duplicate_work.contains(task_id) {
                SettlementReason::DuplicateWork
            } else {
                SettlementReason::Rejected
            };
            // 付执行者 0（留痕并标记已付，防重复结算）
            let paid = self
                .settlement
                .settle(task_id, &escrow, &agent_id, Money::ZERO, reason)?;
            // 托管预算全额退回需求方
            self.settlement
                .refund(task_id, &escrow, &requester, budget)?;
            // 罚没执行者 10% 质押（服务端规则 SLASH_RATE_REJECT，资金退出
            // 系统，守恒仍成立）。统一走 slash_stake_synced：先更新质押记录
            // 再扣账本，根治旧逻辑「只扣账本、质押记录不变」的口径不一致。
            let stake_total = self
                .reputation_mgr
                .stake(&agent_id)
                .map(|s| s.amount)
                .unwrap_or(Money::ZERO);
            let slash_amt = slash_amount_by_rule(stake_total, SLASH_RATE_REJECT);
            if slash_amt.is_positive() {
                self.slash_stake_synced(&agent_id, slash_amt)?;
            }
            // 信誉记一次失败
            if let Some(rep) = self.reputation_mgr.reputation_mut(&agent_id) {
                rep.record_call(false, 1.0);
            }
            if let Some(task) = self.tasks.get_mut(task_id) {
                task.state = task.state.transition(TaskState::Settled)?;
            }
            return Ok(paid);
        }

        // 证据分级强制闸门（v2.6.0，GAP「证据谓词零调用点」）：
        // 需要验证的任务，结果信封必须存在且证据等级可信（Verified/CpuProto）；
        // Unverified 或缺失结果一律拒绝结算付款。verification_policy = None 时豁免。
        if !matches!(task.verification_policy, VerificationPolicy::None) {
            let envelope = self
                .results
                .get(task_id)
                .ok_or_else(|| "任务缺少结果信封，无法核验证据等级，不能结算".to_string())?;
            if !envelope.evidence_grade.is_trustworthy() {
                return Err(format!(
                    "结果证据等级为 {}（不可信），不能结算；请重新执行或升级证据等级",
                    envelope.evidence_grade.label()
                ));
            }
        }

        // 中标价：默认全额；实际支付 = min(中标价, 预算)
        let price = task.winner_price.unwrap_or(budget);
        let pay = price.min(budget);

        // 从托管账户支付执行者（发布即托管，资金已锁定；无需也绝不铸币）
        let paid = self.settlement.settle(
            task_id,
            &escrow,
            &agent_id,
            pay,
            SettlementReason::Completed,
        )?;

        // 预算余款退回需求方
        if budget > pay {
            let refund_amt = budget.checked_sub(pay)?;
            self.settlement
                .refund(task_id, &escrow, &requester, refund_amt)?;
        }

        // 更新信誉
        let envelope = self.results.get(task_id);
        let success = envelope.map(|e| e.is_success()).unwrap_or(true);
        let latency_ratio = envelope
            .map(|e| e.latency_ms as f64 / 2000.0)
            .unwrap_or(1.0);

        if let Some(rep) = self.reputation_mgr.reputation_mut(&agent_id) {
            rep.record_call(success, latency_ratio);
            if success {
                rep.reward_honesty();
            }
        }

        // 更新 Agent 卡片
        if let Some(agent) = self.agents.get_mut(&agent_id) {
            agent.total_calls += 1;
            if success {
                agent.success_rate = (agent.success_rate * (agent.total_calls - 1) as f64 + 1.0)
                    / agent.total_calls as f64;
            }
            agent.reputation_score = self
                .reputation_mgr
                .reputation(&agent_id)
                .map(|r| r.overall())
                .unwrap_or(agent.reputation_score);
        }

        // 更新任务状态
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.state = task.state.transition(TaskState::Settled)?;
        }

        Ok(paid)
    }

    // ===== F7: 争议与仲裁 =====

    /// 同步罚没质押（v2.8.5，GAP §3.8）：
    ///
    /// 先更新质押记录（`reputation_mgr.slash_stake`），再扣账本资金
    /// （`settlement.slash`），保证「质押记录」与「账本」口径一致，
    /// 杜绝旧实现 reject 路径「只扣账本、质押记录不变」的不一致。
    fn slash_stake_synced(&mut self, agent_id: &str, amount: Money) -> Result<Money, String> {
        let stake_acct = stake_account(agent_id);
        // v3.5.3（AU-19）：旧顺序「先 reputation_mgr.slash_stake 改质押记录，再 settlement.slash
        // 扣账本」——账本步失败时质押记录已被扣，口径不一致（账上扣了钱、记录没扣）。
        // 改为：先扣账本（资金侧），成功后再更新质押记录；若记录步失败则把账本扣的钱补回
        // （deposit 回滚），保证两视图要么同时扣、要么都不扣。
        self.settlement.slash(&stake_acct, amount)?;
        if let Err(e) = self.reputation_mgr.slash_stake(agent_id, amount) {
            // 回滚账本：把刚扣的金额补回，避免资金已出、记录未动。
            let _ = self.settlement.deposit(&stake_acct, amount);
            return Err(e);
        }
        Ok(amount)
    }

    /// 发起争议
    pub fn open_dispute(
        &mut self,
        dispute_id: &str,
        task_id: &str,
        complainant: &str,
        reason: &str,
    ) -> Result<(), String> {
        if !self.tasks.contains_key(task_id) {
            return Err(format!("NOT_FOUND: 任务 {} 不存在", task_id));
        }

        // 状态经转换表进入 Disputed（GAP §3.7）：只有 Accepted/Running/
        // Verifying/Rework 可发起争议，Open/Matched 与终态一律拒绝；
        // 同时在此读取 owner（respondent），避免后续 push 时的借用冲突。
        let respondent = {
            let task = self
                .tasks
                .get_mut(task_id)
                .ok_or_else(|| format!("NOT_FOUND: 任务 {} 不存在", task_id))?;
            task.state = task.state.transition(TaskState::Disputed)?;
            task.owner.clone().unwrap_or_default()
        };

        self.disputes.push(DisputeCase {
            dispute_id: dispute_id.to_string(),
            task_id: task_id.to_string(),
            complainant: complainant.to_string(),
            respondent,
            reason: reason.to_string(),
            resolved: false,
            verdict: None,
            arbitrator: None,
        });

        Ok(())
    }

    /// 仲裁争议（P0-4：特权写必须携带签名治理命令）。
    ///
    /// 旧实现仅要求 `arbitrator` 非空字符串（自报即可罚没任意人 100% 质押）。
    /// 现在改为：调用方必须提交 capability = `governance:arbitrate` 的
    /// [`SignedGovernanceCommand`]，其 `target` 绑定 dispute_id、`claim.guilty` 为裁决。
    /// `Governance::verify` 全通过（成员→capability→目标→时间窗→验签→nonce）后，
    /// 仲裁者身份一律取自信封 `sender_did`（禁止请求体自报），再执行既有罚没逻辑。
    /// 普通请求（无治理信封 / 治理集为空）一律拒绝；slash 只在本特权路径触发。
    ///
    /// 返回 `(verdict, slashed)`：裁决结论与服务端实际罚没金额。
    ///
    /// # 需外部审计
    pub fn arbitrate_signed(
        &mut self,
        cmd: &SignedGovernanceCommand,
        now: u64,
    ) -> Result<(String, Money), String> {
        // 1. 治理信封校验（fail-closed：治理集空 / 非成员 / 验签失败一律拒绝）
        let action = self.governance.verify(cmd, now)?;
        if action.capability != GOV_CAP_ARBITRATE {
            return Err(format!(
                "GOV_BAD_CAPABILITY: 仲裁命令需要 {GOV_CAP_ARBITRATE}，实际 {}",
                action.capability
            ));
        }
        let dispute_id = action.target;
        let guilty = parse_arbitrate_claim(&action.claim)?;
        // 仲裁者取自信封 sender，禁止自报
        let arbitrator = action.sender_did;

        // 2. 先取出仲裁所需信息（避免与后续 &mut self 操作产生借用冲突）。
        let (agent_id, task_id) = {
            let dispute = self
                .disputes
                .iter_mut()
                .find(|d| d.dispute_id == dispute_id)
                .ok_or_else(|| format!("NOT_FOUND: 争议 {dispute_id} 不存在"))?;
            if dispute.resolved {
                return Err(format!("争议 {dispute_id} 已仲裁，不能重复仲裁",));
            }
            (dispute.respondent.clone(), dispute.task_id.clone())
        };

        let (verdict, slashed) = if guilty {
            // 罚没金额由服务端规则定：仲裁作恶罚没全部质押。
            let stake_total = self
                .reputation_mgr
                .stake(&agent_id)
                .map(|s| s.amount)
                .unwrap_or(Money::ZERO);
            let slash_amount = slash_amount_by_rule(stake_total, SLASH_RATE_ARBITRATION);
            if slash_amount.is_positive() {
                self.slash_stake_synced(&agent_id, slash_amount)?;
            }
            // 任务托管预算全额退回需求方（作恶方不应获得报酬）。
            if let Some(t) = self.tasks.get(&task_id) {
                let budget = t.budget;
                let requester = t.requester.clone();
                let escrow = escrow_account(&task_id);
                self.settlement
                    .refund(&task_id, &escrow, &requester, budget)?;
            }
            // 状态经转换表进入 Slashed（Disputed → Slashed）。
            if let Some(task) = self.tasks.get_mut(&task_id) {
                task.state = task.state.transition(TaskState::Slashed)?;
            }
            ("guilty".to_string(), slash_amount)
        } else {
            // 状态经转换表进入 Accepted（Disputed → Accepted）。
            if let Some(task) = self.tasks.get_mut(&task_id) {
                task.state = task.state.transition(TaskState::Accepted)?;
            }
            ("not_guilty".to_string(), Money::ZERO)
        };

        // 回写争议结果与仲裁者。
        if let Some(dispute) = self
            .disputes
            .iter_mut()
            .find(|d| d.dispute_id == dispute_id)
        {
            dispute.resolved = true;
            dispute.verdict = Some(verdict.clone());
            dispute.arbitrator = Some(arbitrator);
        }
        Ok((verdict, slashed))
    }

    // ===== 查询接口 =====

    /// 替换受权治理集（P0-4；测试/dev 注入；生产由 `Governance::from_env` 加载）。
    pub fn set_governance(&mut self, g: Governance) {
        self.governance = g;
    }

    /// 当前治理集是否为空（空 = 特权经济写路径 fail-closed 不可用）。
    pub fn governance_is_empty(&self) -> bool {
        self.governance.is_empty()
    }
    ///
    /// 旧实现 `deposit(account, amount)` 无凭证即可对任意账户记账（本地铸币）。
    /// 现在改为：调用方必须提交 capability = `governance:credit` 的
    /// [`SignedGovernanceCommand`]，`target` = account、`claim.amount` 为非负整数。
    ///
    /// **生产口径（未验证 / 需外部审计）**：off-chain 授信在生产环境必须由链上支付
    /// 凭证支持；本入口仅供 dev/faucet 与受权治理签批发起。`SettlementEngine::deposit`
    /// 作为内部可信构建块保留（内部/测试/账本恢复可用），但外部 actor 入口不再接受
    /// 无签名的普通授信。
    pub fn deposit_signed(
        &mut self,
        cmd: &SignedGovernanceCommand,
        now: u64,
    ) -> Result<(), String> {
        let action = self.governance.verify(cmd, now)?;
        if action.capability != GOV_CAP_CREDIT {
            return Err(format!(
                "GOV_BAD_CAPABILITY: 授信命令需要 {GOV_CAP_CREDIT}，实际 {}",
                action.capability
            ));
        }
        let amount = parse_credit_claim(&action.claim)?;
        self.settlement.deposit(&action.target, amount)
    }

    /// 余额
    pub fn balance(&self, account: &str) -> Money {
        self.settlement.balance(account)
    }

    /// 守恒检查
    pub fn conservation_check(&self) -> ConservationReport {
        self.settlement.conservation_check()
    }

    /// 独立审计（从只追加流水独立重放，v2.5.9）
    pub fn independent_audit(&self) -> AuditReport {
        self.settlement.independent_audit()
    }

    /// 信誉排行榜
    pub fn leaderboard(&self, limit: usize) -> Vec<(String, f64)> {
        self.reputation_mgr.leaderboard(limit)
    }

    /// Agent 数量
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// 任务数量
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// 争议数量
    pub fn dispute_count(&self) -> usize {
        self.disputes.len()
    }

    /// 已结算任务数
    pub fn settled_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| t.state == TaskState::Settled)
            .count()
    }

    // ───── v2.6.1 持久化支持（GAP §6.1）─────

    /// 当前只追加结算流水（持久化增量来源）
    pub fn settlement_records(&self) -> &[SettlementRecord] {
        self.settlement.records()
    }

    /// 账本快照（流水 + 全部余额，v3.5.0：供插件编排器喂给独立审计插件）。
    ///
    /// 流水序列化为 `reason/amount/from_account/to_account` 形状（`SettlementReason`
    /// 默认 serde 即变体名，与 market-settle 插件 `audit` 期望的 `Deposited`/
    /// `Slashed` 等一致），余额为 `{账户: 整数}`。
    pub fn ledger_snapshot(&self) -> serde_json::Value {
        let records: Vec<serde_json::Value> = self
            .settlement
            .records()
            .iter()
            .map(|r| {
                serde_json::json!({
                    "task_id": r.task_id,
                    "from_account": r.from_account,
                    "to_account": r.to_account,
                    "amount": r.amount,
                    "reason": r.reason,
                    "timestamp": r.timestamp,
                })
            })
            .collect();
        let balances: serde_json::Map<String, serde_json::Value> = self
            .settlement
            .balances_snapshot()
            .into_iter()
            .map(|(k, v)| (k, serde_json::json!(v)))
            .collect();
        serde_json::json!({
            "records": records,
            "balances": serde_json::Value::Object(balances),
        })
    }

    /// 从持久化流水恢复账本（替换结算引擎）；恢复后应再跑独立审计确认
    pub fn restore_ledger(&mut self, records: Vec<SettlementRecord>) -> Result<(), String> {
        self.settlement = SettlementEngine::restore(records)?;
        Ok(())
    }

    /// 快照全部 Agent 为持久化记录
    pub fn snapshot_agents_for_store(&self) -> Vec<crate::storage::StoredAgent> {
        self.agents
            .values()
            .map(|c| crate::storage::StoredAgent {
                agent_id: c.agent_id.clone(),
                name: c.name.clone(),
                skills: c.skills.join(","),
                stake: c.stake.as_i64(),
                reputation: c.reputation_score,
                created_at: c.created_at.to_string(),
                // v3.5.4（W-05）：完整卡片 JSON（version/pricing/sla/modalities/models 等不再丢）
                card_json: serde_json::to_string(c).ok(),
            })
            .collect()
    }

    /// 快照全部 Task 为持久化记录
    pub fn snapshot_tasks_for_store(&self) -> Vec<crate::storage::StoredTask> {
        self.tasks
            .values()
            .map(|t| crate::storage::StoredTask {
                task_id: t.task_id.clone(),
                goal: t.goal.clone(),
                state: t.state.label().to_string(),
                owner: t.owner.clone(),
                budget: t.budget.as_i64(),
                created_at: t.created_at.to_string(),
                // v2.8.4: 证据闸门/结算字段（GAP §3.2，不再恢复为 None 豁免/满额）
                winner_price: t.winner_price.map(|m| m.as_i64()),
                verification_policy: serde_json::to_string(&t.verification_policy)
                    .unwrap_or_default(),
                requester: t.requester.clone(),
                deadline: t.deadline as i64,
                // v3.5.4（W-04）：完整规格 JSON（context/done/todo/trace/required_skills 不再丢）
                spec_json: serde_json::to_string(t).ok(),
            })
            .collect()
    }

    /// v2.8.4: 快照全部结果信封（GAP §3.2，已验收未结算任务重启后可结算）。
    pub fn snapshot_results_for_store(&self) -> Vec<(String, String)> {
        self.results
            .iter()
            .filter_map(|(id, env)| {
                serde_json::to_string(env)
                    .ok()
                    .map(|payload| (id.clone(), payload))
            })
            .collect()
    }

    /// v2.8.4: 快照全部信誉（GAP §3.2）。
    pub fn snapshot_reputations_for_store(&self) -> Vec<(String, String)> {
        self.reputation_mgr.export_reputations()
    }

    /// v2.8.4: 快照全部质押（GAP §3.2）。
    pub fn snapshot_stakes_for_store(&self) -> Vec<(String, String)> {
        self.reputation_mgr.export_stakes()
    }

    /// v2.7.4: 从磁盘快照恢复 agents 到内存 market（重启后 /agents、/stats 可见）。
    ///
    /// v3.5.4（W-05）：优先反序列化 `card_json` 完整卡片（version/pricing/sla/modalities/
    /// models/endpoint/description/total_calls/success_rate 等不再丢失），再用经济身份权威
    /// 扁平列（stake/reputation/created_at）覆盖 JSON 内同名字段——资金/信誉以列为准，防止
    /// 卡片 JSON 与权威列不一致。card_json 缺失/损坏时回退旧的安全默认构造（不 panic）。
    pub fn restore_agents_from_store(&mut self, agents: Vec<crate::storage::StoredAgent>) {
        for a in agents {
            let mut card = match a.card_json.as_deref() {
                Some(s) if !s.trim().is_empty() => {
                    match serde_json::from_str::<MarketAgentCard>(s) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!(
                                "⚠️ agent {} 卡片 JSON 损坏，回退扁平列默认卡片: {e}",
                                a.agent_id
                            );
                            Self::fallback_agent_card(&a)
                        }
                    }
                }
                _ => Self::fallback_agent_card(&a),
            };
            // 经济身份以扁平权威列覆盖（资金/信誉/创建时间不允许被 JSON 改写）。
            card.agent_id = a.agent_id.clone();
            card.name = a.name.clone();
            card.stake = Money::new(a.stake);
            card.reputation_score = a.reputation;
            card.created_at = a.created_at.parse().unwrap_or(0);
            card.updated_at = card.updated_at.max(card.created_at);
            // 若 JSON 内 skills 为空，用扁平 skills 列兜底。
            if card.skills.is_empty() {
                card.skills = a
                    .skills
                    .split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect();
            }
            for sk in &card.skills {
                self.skill_index
                    .entry(sk.clone())
                    .or_default()
                    .push(card.agent_id.clone());
            }
            self.agents.insert(a.agent_id, card);
        }
    }

    /// v3.5.4：card_json 缺失/损坏时的旧版安全默认卡片（不含丰富声明字段）。
    fn fallback_agent_card(a: &crate::storage::StoredAgent) -> MarketAgentCard {
        let skills: Vec<String> = a
            .skills
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        MarketAgentCard {
            agent_id: a.agent_id.clone(),
            version: "0.0.0-restored".to_string(),
            name: a.name.clone(),
            description: String::new(),
            skills,
            modalities: vec![],
            models: vec![],
            endpoint: String::new(),
            pricing: Pricing {
                model: PricingModel::Subscription,
                price: Money::ZERO,
                currency: Currency::Credit,
            },
            sla: Sla::default(),
            owner: a.agent_id.clone(),
            stake: Money::new(a.stake),
            reputation_score: a.reputation,
            total_calls: 0,
            success_rate: 1.0,
            evidence_grade: EvidenceGrade::Unverified,
            verified: false,
            created_at: a.created_at.parse().unwrap_or(0),
            updated_at: a.created_at.parse().unwrap_or(0),
        }
    }

    /// v2.7.4: 从磁盘快照恢复 tasks 到内存 market。
    ///
    /// v3.5.4（W-04）：优先反序列化 `spec_json` 完整规格（context/done/todo/trace/
    /// required_skills 不再丢失，旧实现把 context 清空、todo 退化为 "(restored from disk)"）。
    /// 随后用扁平权威列覆盖经济/生命周期字段（budget/winner_price/deadline/state/owner/goal/
    /// requester/created_at）与单独解析的 verification_policy（保留 v2.8.4 的损坏告警语义）。
    /// spec_json 缺失/损坏时回退旧的安全默认规格（不 panic）。
    pub fn restore_tasks_from_store(&mut self, tasks: Vec<crate::storage::StoredTask>) {
        for t in tasks {
            // v2.8.4: 恢复验证策略（GAP §3.2）。新持久化的 None 也会序列化为
            // "None" 并正确解析回 None（用户真实意图）；仅遗留空/坏值回 None 并显式警告。
            let policy = match serde_json::from_str::<VerificationPolicy>(&t.verification_policy) {
                Ok(p) => p,
                Err(_) => {
                    if !t.verification_policy.is_empty() {
                        eprintln!(
                            "⚠️ 任务 {} 验证策略持久化损坏，回退 None（豁免闸门，请人工核查）",
                            t.task_id
                        );
                    } else {
                        eprintln!(
                            "⚠️ 任务 {} 为 v2.8.4 之前遗留数据、未持久化验证策略，回退 None（请人工核查）",
                            t.task_id
                        );
                    }
                    VerificationPolicy::None
                }
            };

            // v3.5.4: 优先从完整 spec_json 恢复 context/done/todo/trace/required_skills。
            let mut spec = match t.spec_json.as_deref() {
                Some(s) if !s.trim().is_empty() => match serde_json::from_str::<TaskSpec>(s) {
                    Ok(sp) => sp,
                    Err(e) => {
                        eprintln!(
                            "⚠️ 任务 {} 规格 JSON 损坏，context/todo/skills 回退默认: {e}",
                            t.task_id
                        );
                        Self::fallback_task_spec(&t, policy.clone())
                    }
                },
                _ => {
                    eprintln!(
                        "⚠️ 任务 {} 为 v3.5.4 之前遗留数据、未持久化完整规格，context/todo/skills 回退默认",
                        t.task_id
                    );
                    Self::fallback_task_spec(&t, policy.clone())
                }
            };

            // 经济/生命周期字段一律以扁平权威列覆盖（不信任 JSON 内可能过期的副本）。
            spec.task_id = t.task_id;
            spec.goal = t.goal;
            spec.state = TaskState::from_label(&t.state);
            spec.owner = t.owner;
            spec.budget = Money::new(t.budget);
            spec.created_at = t.created_at.parse().unwrap_or(0);
            spec.winner_price = t.winner_price.map(Money::new);
            spec.deadline = t.deadline.max(0) as u64;
            spec.requester = t.requester;
            spec.verification_policy = policy;

            self.tasks.insert(spec.task_id.clone(), spec);
        }
    }

    /// v3.5.4：spec_json 缺失/损坏时的旧版安全默认规格（丰富字段用默认占位）。
    fn fallback_task_spec(t: &crate::storage::StoredTask, policy: VerificationPolicy) -> TaskSpec {
        TaskSpec {
            task_id: t.task_id.clone(),
            goal: t.goal.clone(),
            context: String::new(),
            done: vec![],
            todo: vec!["(restored from disk)".to_string()],
            trace: vec![],
            owner: t.owner.clone(),
            budget: Money::new(t.budget),
            winner_price: t.winner_price.map(Money::new),
            deadline: t.deadline.max(0) as u64,
            required_skills: vec![],
            verification_policy: policy,
            requester: t.requester.clone(),
            state: TaskState::from_label(&t.state),
            created_at: t.created_at.parse().unwrap_or(0),
        }
    }

    /// v2.8.4: 从磁盘恢复结果信封（GAP §3.2，已验收未结算任务重启后可结算）。
    pub fn restore_results_from_store(&mut self, rows: Vec<(String, String)>) {
        let mut n = 0usize;
        for (id, payload) in rows {
            if let Ok(env) = serde_json::from_str::<ResultEnvelope>(&payload) {
                self.results.insert(id, env);
                n += 1;
            } else {
                eprintln!("⚠️ 结果信封持久化损坏已跳过：{id}");
            }
        }
        if n > 0 {
            println!("✅ 结果信封已从磁盘恢复：{n} 个");
        }
    }

    /// v2.8.4: 从磁盘恢复信誉与质押（GAP §3.2，重启后仍可出价/罚没）。
    pub fn restore_reputation_state_from_store(
        &mut self,
        reputations: Vec<(String, String)>,
        stakes: Vec<(String, String)>,
    ) {
        self.reputation_mgr.import_reputations(reputations);
        self.reputation_mgr.import_stakes(stakes);
    }
}

impl Default for AgentMarket {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod phase2_security_tests {
    //! P0-3 / P0-4 市场层接线测试（同模块内可访问私有字段）。
    use super::*;
    use crate::chain::pocv::SignedProofOfComputation;
    use crate::identity::{Did, Keypair};
    use serde_json::json;

    const NOW: u64 = 1_000_000;

    fn member(seed: u8) -> (String, Keypair) {
        let mut s = [0u8; 32];
        s[0] = seed;
        let kp = Keypair::from_seed(&s);
        let did = Did::from_public_key(kp.public_key()).to_string();
        (did, kp)
    }

    fn seeded_market() -> (AgentMarket, String, Keypair) {
        let mut m = AgentMarket::new();
        let (did, kp) = member(1);
        let pk: [u8; 32] = kp.public_key().try_into().unwrap();
        m.set_governance(Governance::from_members(vec![(did.clone(), pk)]));
        (m, did, kp)
    }

    #[test]
    fn p04_deposit_signed_credits_and_rejects_stranger() {
        let (mut m, judge, kp) = seeded_market();
        let cmd = SignedGovernanceCommand::sign(
            "c1",
            &judge,
            &kp,
            GOV_CAP_CREDIT,
            "alice",
            json!({"amount": 50}),
            "n1",
            NOW - 10,
            300,
        );
        m.deposit_signed(&cmd, NOW).unwrap();
        assert_eq!(m.balance("alice"), Money::new(50));

        // 非治理成员签名 → 拒绝（fail-closed，不能本地铸币）
        let mut s = [0u8; 32];
        s[0] = 9;
        let kp2 = Keypair::from_seed(&s);
        let did2 = Did::from_public_key(kp2.public_key()).to_string();
        let cmd2 = SignedGovernanceCommand::sign(
            "c2",
            &did2,
            &kp2,
            GOV_CAP_CREDIT,
            "bob",
            json!({"amount": 5}),
            "n2",
            NOW - 10,
            300,
        );
        assert!(m.deposit_signed(&cmd2, NOW).is_err());
        assert_eq!(m.balance("bob"), Money::ZERO);
    }

    #[test]
    fn p04_arbitrate_fail_closed_without_governance() {
        let mut m = AgentMarket::new(); // 空治理集
        let (did, kp) = member(1);
        let cmd = SignedGovernanceCommand::sign(
            "c",
            &did,
            &kp,
            GOV_CAP_ARBITRATE,
            "disp-1",
            json!({"guilty": true}),
            "n",
            NOW - 10,
            300,
        );
        let err = m.arbitrate_signed(&cmd, NOW).unwrap_err();
        assert!(err.contains("NO_MEMBERS"), "实际: {err}");
    }

    #[test]
    fn p04_arbitrate_signed_slash_through_privileged_path() {
        let (mut m, judge, kp) = seeded_market();
        m.disputes.push(DisputeCase {
            dispute_id: "disp-1".into(),
            task_id: "task-1".into(),
            complainant: "req".into(),
            respondent: "evil".into(),
            reason: "bad".into(),
            resolved: false,
            verdict: None,
            arbitrator: None,
        });
        m.reputation_mgr
            .register_stake("evil", Money::new(100))
            .unwrap();
        // 授信 100 并锁定为质押
        let credit = SignedGovernanceCommand::sign(
            "c0",
            &judge,
            &kp,
            GOV_CAP_CREDIT,
            "evil",
            json!({"amount": 100}),
            "n0",
            NOW - 10,
            300,
        );
        m.deposit_signed(&credit, NOW).unwrap();
        m.settlement
            .lock(
                "task-1",
                "evil",
                "__stake__:evil",
                Money::new(100),
                SettlementReason::Staked,
            )
            .unwrap();

        let cmd = SignedGovernanceCommand::sign(
            "c1",
            &judge,
            &kp,
            GOV_CAP_ARBITRATE,
            "disp-1",
            json!({"guilty": true}),
            "n1",
            NOW - 10,
            300,
        );
        let (verdict, slashed) = m.arbitrate_signed(&cmd, NOW).unwrap();
        assert_eq!(verdict, "guilty");
        assert_eq!(slashed, Money::new(100));
        assert!(m.disputes[0].resolved);
        // 仲裁者取自信封 sender，而非请求体自报
        assert_eq!(m.disputes[0].arbitrator.as_deref(), Some(judge.as_str()));
    }

    #[test]
    fn p03_submit_result_accepts_valid_pocv_and_rejects_tampered() {
        let mut m = AgentMarket::new();
        // 直接塞一个 Matched 任务（私有字段，同模块测试可构造）
        let task = TaskSpec {
            task_id: "task-x".into(),
            goal: "do".into(),
            context: "ctx".into(),
            done: vec![],
            todo: vec!["t".into()],
            trace: vec![],
            owner: None,
            budget: Money::new(10),
            winner_price: None,
            deadline: NOW,
            required_skills: vec!["s".into()],
            verification_policy: VerificationPolicy::None,
            requester: "req".into(),
            state: TaskState::Matched,
            created_at: NOW,
        };
        m.tasks.insert("task-x".into(), task);

        let (worker, kp) = member(5);
        let report = "the report body";
        // submit_result 内部取真实时钟，故证明 issued_at 用真实时间
        let real_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // 有效证明：对 (task_id, report) 签名
        let proof = SignedProofOfComputation::sign(
            &worker,
            &kp,
            b"task-x",
            report.as_bytes(),
            1,
            "pocv-n1",
            real_now - 10,
            600,
        );
        let env = ResultEnvelope {
            task_id: "task-x".into(),
            agent_id: worker.clone(),
            report: report.into(),
            confidence: 0.9,
            error_type: ErrorType::None,
            trace_ref: String::new(),
            evidence_grade: EvidenceGrade::Unverified,
            latency_ms: 1,
            pocv: Some(proof),
        };
        m.submit_result(env).unwrap();

        // 篡改报告的假证明：签名时 output 是别的内容，与实际 report 不一致 → 拒绝
        let task2 = TaskSpec {
            state: TaskState::Matched,
            ..task_clone_helper()
        };
        m.tasks.insert("task-y".into(), task2);
        let bad_proof = SignedProofOfComputation::sign(
            &worker,
            &kp,
            b"task-y",
            b"something-else",
            1,
            "pocv-n2",
            real_now - 10,
            600,
        );
        let env2 = ResultEnvelope {
            task_id: "task-y".into(),
            agent_id: worker,
            report: "changed report".into(),
            confidence: 0.9,
            error_type: ErrorType::None,
            trace_ref: String::new(),
            evidence_grade: EvidenceGrade::Unverified,
            latency_ms: 1,
            pocv: Some(bad_proof),
        };
        assert!(m.submit_result(env2).is_err());
    }

    fn task_clone_helper() -> TaskSpec {
        TaskSpec {
            task_id: "task-y".into(),
            goal: "do".into(),
            context: "ctx".into(),
            done: vec![],
            todo: vec!["t".into()],
            trace: vec![],
            owner: None,
            budget: Money::new(10),
            winner_price: None,
            deadline: NOW,
            required_skills: vec!["s".into()],
            verification_policy: VerificationPolicy::None,
            requester: "req".into(),
            state: TaskState::Matched,
            created_at: NOW,
        }
    }
}
