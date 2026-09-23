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
pub mod agent_card;
pub mod task;
pub mod qa_committee;
pub mod settlement;
pub mod reputation;

pub use evidence::EvidenceGrade;
pub use agent_card::{
    MarketAgentCard, SkillManifest, Pricing, PricingModel, Currency, Sla,
    SchemaField, AgentCategory,
};
pub use task::{
    TaskSpec, TaskState, ResultEnvelope, ErrorType, VerificationPolicy,
};
pub use qa_committee::{QaCommittee, QaMember, QaVote, QaDecision};
pub use settlement::{
    SettlementEngine, SettlementRecord, SettlementReason, ConservationReport,
};
pub use reputation::{
    ReputationManager, MarketReputation, StakeRecord, StakeStatus,
};

use std::collections::HashMap;

/// 投标
#[derive(Debug, Clone)]
pub struct Bid {
    pub agent_id: String,
    pub task_id: String,
    pub proposed_price: f64,
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
    min_stake: f64,
}

impl AgentMarket {
    pub fn new() -> Self {
        let min_stake = 100.0;
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
    pub fn with_min_stake(min_stake: f64) -> Self {
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

        // 注册质押
        self.reputation_mgr
            .register_stake(&card.agent_id, card.stake)?;

        // 质押资金同步存入结算引擎（锁定，用于罚没）
        self.settlement.deposit(&card.agent_id, card.stake);

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

            // 性价比 = 信誉 / 价格
            let cost_score = if bid.proposed_price > 0.0 {
                rep_score / bid.proposed_price
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

        let winner = best.unwrap().agent_id.clone();

        // 更新任务状态
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.state = TaskState::Matched;
            task.owner = Some(winner.clone());
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
            task.state = TaskState::Verifying;
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

        match decision {
            QaDecision::Stop => {
                task.state = TaskState::Accepted;
            }
            QaDecision::Continue => {
                task.state = TaskState::Rework;
            }
            QaDecision::NoQuorum => {
                task.state = TaskState::NoQuorum;
            }
        }

        Ok(decision)
    }

    // ===== F6: 结算 =====

    /// 结算已验收任务
    pub fn settle_task(&mut self, task_id: &str) -> Result<f64, String> {
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

        let agent_id = task
            .owner
            .clone()
            .ok_or_else(|| "任务无执行 Agent".to_string())?;
        let payer = task.requester.clone();
        let amount = task.budget;

        // 确保付款方有余额
        if self.settlement.balance(&payer) < amount {
            self.settlement.deposit(&payer, amount);
        }

        let paid = self.settlement.settle(
            task_id,
            &payer,
            &agent_id,
            amount,
            SettlementReason::Completed,
        )?;

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
            task.state = TaskState::Settled;
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
        slash_amount: f64,
    ) -> Result<String, String> {
        let dispute = self
            .disputes
            .iter_mut()
            .find(|d| d.dispute_id == dispute_id)
            .ok_or_else(|| format!("争议 {} 不存在", dispute_id))?;

        let verdict = if guilty {
            // 罚没
            let agent_id = dispute.respondent.clone();
            if slash_amount > 0.0 {
                self.reputation_mgr
                    .slash_stake(&agent_id, slash_amount)?;
                self.settlement.slash(&agent_id, slash_amount)?;
            }
            if let Some(task) = self.tasks.get_mut(&dispute.task_id) {
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

    /// 充值
    pub fn deposit(&mut self, account: &str, amount: f64) {
        self.settlement.deposit(account, amount);
    }

    /// 余额
    pub fn balance(&self, account: &str) -> f64 {
        self.settlement.balance(account)
    }

    /// 守恒检查
    pub fn conservation_check(&self) -> ConservationReport {
        self.settlement.conservation_check()
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
}

impl Default for AgentMarket {
    fn default() -> Self {
        Self::new()
    }
}
