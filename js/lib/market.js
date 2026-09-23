// lib/market.js
// v2.3.4 Agent Market：注册 / 发布 / 投标 / 匹配 / 结算 / 罚没 / 守恒

const { Task, TaskStatus } = require('./models');

const MIN_STAKE = 100;

/** 投标 */
class Bid {
  constructor(agentId, price) {
    this.agentId = agentId;
    this.price = price;
  }
}

/** 信誉档案（简化四维） */
class Reputation {
  constructor() {
    this.quality = 0;
    this.speed = 0;
    this.honesty = 0;
    this.availability = 0;
  }

  overall() {
    return this.quality * 0.35 + this.speed * 0.20 +
      this.honesty * 0.30 + this.availability * 0.15;
  }
}

/**
 * 智能体市场
 */
class AgentMarket {
  constructor() {
    this.agents = new Map();      // agentId → { card, stake, reputation }
    this.tasks = new Map();       // taskId → Task
    this.bids = new Map();        // taskId → Bid[]
    this.balances = new Map();    // 账户 → 余额
    this.totalSlashed = 0;
    this.paidTasks = new Set();   // 已结算任务（防重复支付）
  }

  /** 充值 / 质押金进入账户 */
  deposit(account, amount) {
    if (amount < 0) throw new Error('金额不能为负');
    const cur = this.balances.get(account) || 0;
    this.balances.set(account, cur + amount);
  }

  balance(account) {
    return this.balances.get(account) || 0;
  }

  /** 质押锁定账户（仍计入系统总余额，只是不可流动） */
  _stakeAccount(agentId) {
    return `__stake__:${agentId}`;
  }

  /** 注册 Agent（需质押 ≥ MIN_STAKE） */
  registerAgent(card, stake) {
    if (!card || !card.did) throw new Error('agent_id 不能为空');
    if (!card.name) throw new Error('name 不能为空');
    if (this.agents.has(card.did)) throw new Error('Agent 已存在');
    if (stake < MIN_STAKE) {
      throw new Error(`质押不足，最低 ${MIN_STAKE}`);
    }
    // 质押金从发布者账户转入锁定账户（不退出系统，仍计入总余额）
    const owner = card.did;
    if (this.balance(owner) < stake) {
      throw new Error('质押账户余额不足，请先 deposit');
    }
    const stakeAcc = this._stakeAccount(owner);
    this.balances.set(owner, this.balance(owner) - stake);
    this.balances.set(stakeAcc, this.balance(stakeAcc) + stake);

    this.agents.set(card.did, {
      card,
      stake,
      reputation: new Reputation(),
      status: 'active',
    });
    return true;
  }

  /** 发布任务 */
  publishTask(taskId, goal, budget, requester) {
    if (!goal) throw new Error('goal 不能为空');
    if (budget <= 0) throw new Error('预算必须为正');
    if (this.balance(requester) < budget) {
      throw new Error('需求方余额不足以覆盖预算');
    }
    // 预算锁定
    this.balances.set(requester, this.balance(requester) - budget);

    const task = new Task(taskId, goal);
    task.transition(TaskStatus.OPEN);
    task.requester = requester;
    task.budget = budget;
    this.tasks.set(taskId, task);
    this.bids.set(taskId, []);
    return task;
  }

  /** 提交投标 */
  submitBid(taskId, agentId, price) {
    if (!this.agents.has(agentId)) throw new Error('Agent 未注册');
    const task = this.tasks.get(taskId);
    if (!task || task.status !== TaskStatus.OPEN) {
      throw new Error('任务不在开放状态');
    }
    this.bids.get(taskId).push(new Bid(agentId, price));
    return true;
  }

  /** 匹配：选性价比最高（信誉/价格）的投标，平局选价格低者 */
  matchTask(taskId) {
    const bids = this.bids.get(taskId) || [];
    if (bids.length === 0) throw new Error('无投标，无法匹配');

    let best = bids[0];
    let bestScore = this._bidScore(best);
    for (let i = 1; i < bids.length; i++) {
      const score = this._bidScore(bids[i]);
      if (score > bestScore ||
        (score === bestScore && bids[i].price < best.price)) {
        best = bids[i];
        bestScore = score;
      }
    }

    const task = this.tasks.get(taskId);
    task.assign(best.agentId);
    task.winnerPrice = best.price;
    return best;
  }

  _bidScore(bid) {
    const agent = this.agents.get(bid.agentId);
    const rep = agent.reputation.overall();
    // 性价比 = 信誉 / 价格（价格为 0 时退化为只看信誉）
    return bid.price > 0 ? rep / bid.price : rep;
  }

  /** 执行 → 完成 → 验证 */
  completeTask(taskId, passed = true) {
    const task = this.tasks.get(taskId);
    task.start();
    task.complete();
    task.verify(passed);
    return task;
  }

  /** 结算（验收通过才支付） */
  settle(taskId) {
    const task = this.tasks.get(taskId);

    // 结算优先级短路
    if (this.paidTasks.has(taskId)) {
      return { paid: 0, reason: 'already_paid' };
    }
    if (task.status !== TaskStatus.VERIFIED) {
      return { paid: 0, reason: 'rejected' };
    }

    const price = task.winnerPrice || task.budget;
    const pay = Math.min(price, task.budget);

    // 从任务锁定预算中支付给执行者（账户间转账，系统总余额不变）
    const assignee = task.assignee;
    this.balances.set(assignee, this.balance(assignee) + pay);
    // 未用完的预算退还需求方
    const refund = task.budget - pay;
    if (refund > 0) {
      this.balances.set(task.requester, this.balance(task.requester) + refund);
    }

    task.settle();
    this.paidTasks.add(taskId);
    return { paid: pay, reason: 'settled' };
  }

  /** 罚没（作恶）：从质押锁定账户扣除，罚没部分退出系统（总余额减少） */
  slash(agentId, amount, reason) {
    const entry = this.agents.get(agentId);
    if (!entry) throw new Error('Agent 不存在');
    const actual = Math.min(amount, entry.stake);
    entry.stake -= actual;
    // 从锁定账户实际扣除（退出系统）
    const stakeAcc = this._stakeAccount(agentId);
    this.balances.set(stakeAcc, this.balance(stakeAcc) - actual);
    this.totalSlashed += actual;
    entry.status = entry.stake < MIN_STAKE ? 'suspended' : entry.status;
    return { slashed: actual, reason, remainingStake: entry.stake };
  }

  /** 按技能发现 Agent */
  discoverBySkill(skill) {
    const target = String(skill).toLowerCase();
    const result = [];
    for (const [id, entry] of this.agents) {
      const skills = (entry.card.skills || [])
        .map((s) => String(s).toLowerCase());
      const caps = (entry.card.capabilities || [])
        .map((s) => String(s).toLowerCase());
      if (skills.includes(target) || caps.includes(target)) {
        result.push(entry.card);
      }
    }
    return result;
  }

  /** 排行榜（按信誉综合分） */
  leaderboard(limit = 10) {
    const entries = [...this.agents.values()]
      .filter((e) => e.status === 'active')
      .map((e) => ({ agentId: e.card.did, score: e.reputation.overall() }))
      .sort((a, b) => b.score - a.score);
    return entries.slice(0, limit);
  }

  /**
   * 守恒检查
   * 支付是账户间转账（系统总余额不变），只有罚没减少系统总余额。
   * 总充值（预算/质押）− 总罚没 = 当前全部账户余额之和
   */
  conservationCheck(totalDeposits) {
    let balanceSum = 0;
    for (const v of this.balances.values()) balanceSum += v;
    const expected = totalDeposits - this.totalSlashed;
    return {
      conserved: Math.abs(balanceSum - expected) < 1e-9,
      balanceSum,
      totalSlashed: this.totalSlashed,
      expected,
    };
  }
}

module.exports = { AgentMarket, Bid, Reputation, MIN_STAKE };
