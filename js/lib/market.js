// lib/market.js
// v2.3.5 Agent Market：注册 / 发布 / 投标 / 匹配 / 结算 / 罚没 / 守恒

const { Task, TaskStatus } = require('./models');
// Ed25519 同步验签：Node 走 node:crypto，浏览器经 package.json "browser"
// 字段替换为 fail-closed 占位。该模块是本文件唯一的平台相关依赖，
// market.js 本体保持纯逻辑、不直接 require node:crypto，可安全被浏览器引用。
const { verifyEd25519 } = require('./edverify');

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
    this.totalDeposits = 0;
    this.paidTasks = new Set();   // 已结算任务（防重复支付）
    this.records = [];            // 只追加结算流水（独立审计的唯一信任源）
    this._seenNonces = new Set(); // 已用投票 nonce（防重放）
  }

  /** 追加一条结算流水（与 Rust SettlementRecord 对齐） */
  _record(from, to, amount, reason) {
    this.records.push({ from, to, amount, reason });
  }

  /** 金额必须是安全整数（与 Rust Money(i64) 对齐，杜绝浮点导致的跨语言守恒失效） */
  _assertMoney(v, label) {
    if (typeof v !== 'number' || !Number.isInteger(v) || !Number.isSafeInteger(v)) {
      throw new Error(`${label || '金额'}必须是整数`);
    }
    return v;
  }

  /** 充值 / 质押金进入账户 */
  deposit(account, amount) {
    this._assertMoney(amount, '金额');
    if (amount < 0) throw new Error('金额不能为负');
    const cur = this.balances.get(account) || 0;
    this.balances.set(account, cur + amount);
    this.totalDeposits += amount;
    this._record('', account, amount, 'Deposited');
  }

  balance(account) {
    return this.balances.get(account) || 0;
  }

  /** 质押锁定账户（仍计入系统总余额，只是不可流动） */
  _stakeAccount(agentId) {
    return `__stake__:${agentId}`;
  }

  /** 任务预算托管账户（发布即锁定，结算时从此支付 / 退款），与 Rust 命名一致 */
  _escrowAccount(taskId) {
    return `__escrow__:${taskId}`;
  }

  /** 注册 Agent（需质押 ≥ MIN_STAKE） */
  registerAgent(card, stake) {
    if (!card || !card.did) throw new Error('agent_id 不能为空');
    if (!card.name) throw new Error('name 不能为空');
    if (this.agents.has(card.did)) throw new Error('Agent 已存在');
    this._assertMoney(stake, '质押');
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
    this._record(owner, stakeAcc, stake, 'Staked');

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
    this._assertMoney(budget, '预算');
    if (budget <= 0) throw new Error('预算必须为正');
    if (this.balance(requester) < budget) {
      throw new Error('需求方余额不足以覆盖预算');
    }
    // 发布即托管：预算从需求方转入任务托管账户（钱仍在系统内，任意时刻守恒）
    this.balances.set(requester, this.balance(requester) - budget);
    const escrowAcc = this._escrowAccount(taskId);
    this.balances.set(escrowAcc, this.balance(escrowAcc) + budget);
    this._record(requester, escrowAcc, budget, 'Escrowed');

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
    this._assertMoney(price, '出价');
    if (price <= 0) throw new Error('出价必须为正');
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
  completeTask(taskId, passed = true, evidenceGrade = 'CpuProto') {
    const task = this.tasks.get(taskId);
    task.start();
    task.complete();
    task.evidenceGrade = evidenceGrade;
    if (task.verificationPolicy === 'None') {
      // 无需 QA：提交结果即验收（证据门禁亦豁免）
      task.transition(TaskStatus.VERIFIED);
    } else {
      task.verify(passed);
    }
    return task;
  }

  /** 恢复边：返工任务回到执行中（v2.6.0） */
  resumeAfterRework(taskId) {
    const task = this.tasks.get(taskId);
    if (!task) throw new Error('任务不存在');
    task.resumeAfterRework();
    return task;
  }

  /** 恢复边：无共识任务重新开放（v2.6.0，消除吸收态） */
  reopenTask(taskId) {
    const task = this.tasks.get(taskId);
    if (!task) throw new Error('任务不存在');
    task.reopenAfterNoQuorum();
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

    // 证据分级强制闸门（v2.6.0）：需要验证的任务，结果证据必须可信；
    // policy='None' 时豁免。
    if (task.verificationPolicy !== 'None') {
      if (task.evidenceGrade !== 'Verified' && task.evidenceGrade !== 'CpuProto') {
        return { paid: 0, reason: 'untrusted_evidence' };
      }
    }

    const price = task.winnerPrice || task.budget;
    const pay = Math.min(price, task.budget);

    // 从托管账户支付给执行者、余款退还需求方（账户间转账，总余额不变）
    const escrowAcc = this._escrowAccount(taskId);
    const assignee = task.assignee;
    this.balances.set(escrowAcc, this.balance(escrowAcc) - pay);
    this.balances.set(assignee, this.balance(assignee) + pay);
    this._record(escrowAcc, assignee, pay, 'Completed');
    // 未用完的预算退还需求方
    const refund = task.budget - pay;
    if (refund > 0) {
      this.balances.set(escrowAcc, this.balance(escrowAcc) - refund);
      this.balances.set(task.requester, this.balance(task.requester) + refund);
      this._record(escrowAcc, task.requester, refund, 'Refunded');
    }

    task.settle();
    this.paidTasks.add(taskId);
    return { paid: pay, reason: 'settled' };
  }

  /** 罚没（作恶）：从质押锁定账户扣除，罚没部分退出系统（总余额减少） */
  slash(agentId, amount, reason) {
    const entry = this.agents.get(agentId);
    if (!entry) throw new Error('Agent 不存在');
    this._assertMoney(amount, '罚没金额');
    if (amount <= 0) throw new Error('罚没金额必须为正');
    const actual = Math.min(amount, entry.stake);
    entry.stake -= actual;
    // 从锁定账户实际扣除（退出系统）
    const stakeAcc = this._stakeAccount(agentId);
    this.balances.set(stakeAcc, this.balance(stakeAcc) - actual);
    this.totalSlashed += actual;
    this._record(stakeAcc, '', actual, 'Slashed');
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
      conserved: balanceSum === expected,
      balanceSum,
      totalSlashed: this.totalSlashed,
      expected,
    };
  }

  /**
   * 认证式 QA 验证（v2.5.9，与 Rust verify_result_authenticated 对齐）
   * members: [{did, public_key(hex32)}] 固定委员集
   * signedVotes: [SignedQaVote]，vote 为 Stop/Continue
   */
  verifyResultAuthenticated(
    taskId,
    members,
    signedVotes,
    round = 0,
    now = Math.floor(Date.now() / 1000),
  ) {
    if (!Array.isArray(members) || members.length === 0) {
      throw new Error('members 必须为非空固定委员集');
    }
    const n = members.length;
    const f = Math.floor((n - 1) / 3);
    const pubkeys = new Map();
    for (const m of members) {
      const pk = m.public_key || m.publicKey;
      if (!m.did || !pk) throw new Error('委员需含 did 与 public_key');
      if (!/^[0-9a-fA-F]{64}$/.test(pk)) throw new Error('委员公钥须为 32 字节 hex');
      pubkeys.set(m.did, pk.toLowerCase());
    }

    // 逐票校验；voterVotes 记录每个委员最终立场（equivocated 票作废）
    const voterVotes = new Map();
    for (const sv of signedVotes || []) {
      const pk = pubkeys.get(sv.voter);
      if (!pk) throw new Error(`投票者 ${sv.voter} 不在固定委员集`);
      if (sv.task_id !== taskId) throw new Error('投票任务不匹配');
      if (Number(sv.round) !== round) throw new Error('投票轮次不匹配');
      const issued = Number(sv.issued_at);
      const expires = Number(sv.expires_at);
      if (!(expires > issued) || now < issued || now > expires) {
        throw new Error(`投票 ${sv.nonce} 不在有效时间窗`);
      }
      const voteTag = String(sv.vote).toLowerCase();
      if (voteTag === 'silent') throw new Error('Silent 票不计入');
      // 先验签，避免无效票污染 nonce
      if (!AgentMarket._verifyEd25519(pk, AgentMarket._voteSigningBytes(sv), sv.signature)) {
        throw new Error(`投票 ${sv.nonce} 签名无效`);
      }
      if (!sv.nonce) throw new Error('nonce 不能为空');
      if (this._seenNonces.has(sv.nonce)) throw new Error(`投票 ${sv.nonce} 重放`);
      this._seenNonces.add(sv.nonce);
      const prev = voterVotes.get(sv.voter);
      if (prev && prev !== voteTag) voterVotes.set(sv.voter, 'equivocated');
      else if (!prev) voterVotes.set(sv.voter, voteTag);
    }

    let stops = 0;
    let continues = 0;
    for (const v of voterVotes.values()) {
      if (v === 'stop') stops += 1;
      else if (v === 'continue') continues += 1;
    }
    const quorum = 2 * f + 1;
    let decision;
    if (stops >= quorum) decision = 'stop';
    else if (continues >= quorum) decision = 'continue';
    else decision = 'no_quorum';
    const accepted = decision === 'stop';
    const task = this.tasks.get(taskId);
    if (task && decision === 'stop') task.verify(true);
    return { decision, accepted, stops, continues, quorum, n, f };
  }

  /**
   * 独立审计（v2.5.9，与 Rust independent_audit 对齐）
   * 只信任 records，逐笔重放出期望余额，再与当前 balances 逐户比对；
   * 能发现守恒检查无法察觉的账实不符（如拆账、幽灵账户）。
   */
  independentAudit() {
    const expected = new Map();
    const add = (acct, delta) => {
      const cur = expected.get(acct) || 0;
      const v = cur + delta;
      if (!Number.isSafeInteger(v)) throw new Error('重放溢出');
      expected.set(acct, v);
    };
    let replayedDeposits = 0;
    let replayedSlashed = 0;
    for (const r of this.records) {
      switch (r.reason) {
        case 'Deposited':
          add(r.to, r.amount);
          replayedDeposits += r.amount;
          break;
        case 'Slashed':
          add(r.from, -r.amount);
          replayedSlashed += r.amount;
          break;
        default:
          if (r.amount > 0) {
            add(r.from, -r.amount);
            add(r.to, r.amount);
          }
      }
    }

    const accounts = new Set([...expected.keys(), ...this.balances.keys()]);
    const mismatches = [];
    for (const a of accounts) {
      const exp = expected.get(a) || 0;
      const act = this.balances.get(a) || 0;
      if (exp !== act) mismatches.push({ account: a, expected: exp, actual: act });
    }
    let balanceSum = 0;
    for (const v of this.balances.values()) balanceSum += v;
    const aggregateMatches =
      replayedDeposits === this.totalDeposits &&
      replayedSlashed === this.totalSlashed;
    const expectedTotal = replayedDeposits - replayedSlashed;
    return {
      passed:
        mismatches.length === 0 &&
        aggregateMatches &&
        expectedTotal === balanceSum,
      replayedRecords: this.records.length,
      replayedDeposits,
      replayedSlashed,
      expectedTotal,
      actualTotal: balanceSum,
      aggregateMatches,
      mismatches,
    };
  }

  /** 投票待签名规范字节（与 Rust SignedQaVote::signing_bytes 逐字一致） */
  static _voteSigningBytes(sv) {
    const tag = String(sv.vote).toUpperCase();
    return (
      `AU-QA-VOTE\ntask_id=${sv.task_id}\nround=${sv.round}\nvoter=${sv.voter}` +
      `\nvote=${tag}\nnonce=${sv.nonce}\nissued_at=${sv.issued_at}` +
      `\nexpires_at=${sv.expires_at}`
    );
  }

  /**
   * Ed25519 验签；委托给平台适配模块。
   * Node 下真实验签；无该能力的环境（浏览器/WKWebView）由
   * edverify.browser.js fail-closed 抛出明确错误，而非静默通过。
   * 密码学原语，算法安全性需外部审计。
   */
  static _verifyEd25519(pubkeyHex, message, signatureHex) {
    return verifyEd25519(pubkeyHex, message, signatureHex);
  }
}

module.exports = { AgentMarket, Bid, Reputation, MIN_STAKE };
