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

pub mod evidence;
pub mod money;
pub mod agent_card;
pub mod task;
pub mod qa_committee;
pub mod settlement;
pub mod reputation;

pub use evidence::EvidenceGrade;
pub use money::Money;
pub use agent_card::{
    MarketAgentCard, SkillManifest, Pricing, PricingModel, Currency, Sla,
    SchemaField, AgentCategory,
};
pub use task::{
    TaskSpec, TaskState, ResultEnvelope, ErrorType, VerificationPolicy,
};
pub use qa_committee::{QaCommittee, QaMember, QaVote, QaDecision, SignedQaVote};
pub use settlement::{
    SettlementEngine, SettlementRecord, SettlementReason, ConservationReport, AuditReport,
    AccountMismatch,
};
pub use reputation::{
    ReputationManager, MarketReputation, StakeRecord, StakeStatus,
};

use std::collections::HashMap;

/// 质押锁定账户（注册时从自有余额转入）
pub fn stake_account(did: &str) -> String {
    format!("__stake__:{}", did)
}

/// 任务托管账户（发布任务时从需求方余额锁定）
pub fn escrow_account(task_id: &str) -> String {
    format!("__escrow__:{}", task_id)
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
    /// 争议
    disputes: Vec<DisputeCase>,
    /// 结算引擎
    settlement: SettlementEngine,
    /// 信誉管理器
    reputation_mgr: ReputationManager,
    /// 最低质押
    min_stake: Money,
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
            disputes: Vec::new(),
            settlement: SettlementEngine::new(),
            reputation_mgr: ReputationManager::new(min_stake),
            min_stake,
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
            disputes: Vec::new(),
            settlement: SettlementEngine::new(),
            reputation_mgr: ReputationManager::new(min_stake),
            min_stake,
        }
    }

    // ===== F1: Agent 注册 =====

    /// 注册 Agent
    pub fn register_agent(&mut self, card: MarketAgentCard) -> Result<String, String> {
        if card.agent_id.trim().is_empty() {
            return Err("agent_id 不能为空".to_string());
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
                .or_insert_with(Vec::new)
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

    // ===== F3: 发现与匹配 =====

    /// 按技能发现 Agent
    pub fn discover_by_skill(&self, skill: &str) -> Vec<&MarketAgentCard> {
        self.skill_index
            .get(&skill.to_lowercase())
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.agents.get(id))
                    .collect()
            })
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
        if !self.tasks.contains_key(&bid.task_id) {
            return Err(format!("任务 {} 不存在", bid.task_id));
        }

        // 检查资格
        if !self
            .reputation_mgr
            .is_eligible(&bid.agent_id, 0.3)
        {
            return Err(format!("Agent {} 不满足资格要求", bid.agent_id));
        }

        self.bids
            .entry(bid.task_id.clone())
            .or_insert_with(Vec::new)
            .push(bid);
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

            // 性价比 = 信誉 / 价格（价格为整数金额，转 f64 参与打分）
            let price_f = bid.proposed_price.as_i64() as f64;
            let cost_score = if price_f > 0.0 {
                rep_score / price_f
            } else {
                rep_score
            };
            // 延迟惩罚
            let latency_penalty = 1.0 / (1.0 + bid.estimated_latency_ms as f64 / 1000.0);
            let score = cost_score * latency_penalty;

            if score > best_score {
                best_score = score;
                best = Some(bid);
            }
        }

        let best_bid = best.unwrap();
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

    // ===== F4/F5: 执行与验证 =====

    /// 提交执行结果
    pub fn submit_result(&mut self, envelope: ResultEnvelope) -> Result<(), String> {
        let task = self
            .tasks
            .get(&envelope.task_id)
            .ok_or_else(|| format!("任务 {} 不存在", envelope.task_id))?;

        if task.state != TaskState::Matched && task.state != TaskState::Running {
            return Err(format!(
                "任务状态为 {}，不能提交结果",
                task.state.label()
            ));
        }

        if let Some(task) = self.tasks.get_mut(&envelope.task_id) {
            // policy=None：无需 QA，提交结果直接验收；否则进入验收阶段
            let target = if matches!(task.verification_policy, VerificationPolicy::None)
            {
                TaskState::Accepted
            } else {
                TaskState::Verifying
            };
            task.state = task.state.transition(target)?;
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
            .ok_or_else(|| format!("任务 {} 不存在", task_id))?;

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
    pub fn verify_result_authenticated(
        &mut self,
        task_id: &str,
        round: u32,
        members: Vec<(String, [u8; 32])>,
        signed_votes: Vec<SignedQaVote>,
        now: u64,
    ) -> Result<QaDecision, String> {
        let mut committee = QaCommittee::with_fixed_members(task_id, round, members)?;
        for sv in signed_votes {
            committee.cast_signed_vote(sv, now)?;
        }
        let decision = committee.tally();

        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| format!("任务 {} 不存在", task_id))?;
        let target = match decision {
            QaDecision::Stop => TaskState::Accepted,
            QaDecision::Continue => TaskState::Rework,
            QaDecision::NoQuorum => TaskState::NoQuorum,
        };
        task.state = task.state.transition(target)?;
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
            .ok_or_else(|| format!("任务 {} 不存在", task_id))?;
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
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| format!("任务 {} 不存在", task_id))?;
        if task.state != TaskState::NoQuorum {
            return Err(format!(
                "任务状态为 {}，非 NO_QUORUM，不能重新开放",
                task.state.label()
            ));
        }
        task.state = task.state.transition(TaskState::Open)?;
        Ok(())
    }

    // ===== F6: 结算 =====

    /// 结算已验收任务
    pub fn settle_task(&mut self, task_id: &str) -> Result<Money, String> {
        let task = self
            .tasks
            .get(task_id)
            .ok_or_else(|| format!("任务 {} 不存在", task_id))?;

        if task.state != TaskState::Accepted {
            return Err(format!(
                "任务状态为 {}，不能结算",
                task.state.label()
            ));
        }

        // 证据分级强制闸门（v2.6.0，GAP「证据谓词零调用点」）：
        // 需要验证的任务，结果信封必须存在且证据等级可信（Verified/CpuProto）；
        // Unverified 或缺失结果一律拒绝结算付款。verification_policy = None 时豁免。
        if !matches!(task.verification_policy, VerificationPolicy::None) {
            let envelope = self.results.get(task_id).ok_or_else(|| {
                "任务缺少结果信封，无法核验证据等级，不能结算".to_string()
            })?;
            if !envelope.evidence_grade.is_trustworthy() {
                return Err(format!(
                    "结果证据等级为 {}（不可信），不能结算；请重新执行或升级证据等级",
                    envelope.evidence_grade.label()
                ));
            }
        }

        let agent_id = task
            .owner
            .clone()
            .ok_or_else(|| "任务无执行 Agent".to_string())?;
        let requester = task.requester.clone();
        let budget = task.budget;
        // 中标价：默认全额；实际支付 = min(中标价, 预算)
        let price = task.winner_price.unwrap_or(budget);
        let pay = price.min(budget);
        let escrow = escrow_account(task_id);

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
                agent.success_rate =
                    (agent.success_rate * (agent.total_calls - 1) as f64 + 1.0)
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

    /// 发起争议
    pub fn open_dispute(
        &mut self,
        dispute_id: &str,
        task_id: &str,
        complainant: &str,
        reason: &str,
    ) -> Result<(), String> {
        if !self.tasks.contains_key(task_id) {
            return Err(format!("任务 {} 不存在", task_id));
        }

        if let Some(task) = self.tasks.get_mut(task_id) {
            task.state = TaskState::Disputed;
        }

        self.disputes.push(DisputeCase {
            dispute_id: dispute_id.to_string(),
            task_id: task_id.to_string(),
            complainant: complainant.to_string(),
            respondent: self
                .tasks
                .get(task_id)
                .and_then(|t| t.owner.clone())
                .unwrap_or_default(),
            reason: reason.to_string(),
            resolved: false,
            verdict: None,
        });

        Ok(())
    }

    /// 仲裁争议
    pub fn arbitrate(
        &mut self,
        dispute_id: &str,
        guilty: bool,
        slash_amount: Money,
    ) -> Result<String, String> {
        let dispute = self
            .disputes
            .iter_mut()
            .find(|d| d.dispute_id == dispute_id)
            .ok_or_else(|| format!("争议 {} 不存在", dispute_id))?;

        let verdict = if guilty {
            let agent_id = dispute.respondent.clone();
            let task_id = dispute.task_id.clone();
            if slash_amount.is_positive() {
                // 罚没质押：资金从质押锁定账户退出系统
                let stake_acct = stake_account(&agent_id);
                self.reputation_mgr
                    .slash_stake(&agent_id, slash_amount)?;
                self.settlement.slash(&stake_acct, slash_amount)?;
            }
            // 任务托管预算退回需求方（作恶方不应获得报酬）
            if let Some(t) = self.tasks.get(&task_id) {
                let budget = t.budget;
                let requester = t.requester.clone();
                let escrow = escrow_account(&task_id);
                self.settlement
                    .refund(&task_id, &escrow, &requester, budget)?;
            }
            if let Some(task) = self.tasks.get_mut(&task_id) {
                task.state = TaskState::Slashed;
            }
            "guilty".to_string()
        } else {
            if let Some(task) = self.tasks.get_mut(&dispute.task_id) {
                task.state = TaskState::Accepted;
            }
            "not_guilty".to_string()
        };

        dispute.resolved = true;
        dispute.verdict = Some(verdict.clone());
        Ok(verdict)
    }

    // ===== 查询接口 =====

    /// 充值（唯一资金入口）
    pub fn deposit(&mut self, account: &str, amount: Money) -> Result<(), String> {
        self.settlement.deposit(account, amount)
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
            })
            .collect()
    }
}

impl Default for AgentMarket {
    fn default() -> Self {
        Self::new()
    }
}
