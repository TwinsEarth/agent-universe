// lib/models.js
// AgentCard / Task / TaskStatus 状态机

/** 任务状态枚举 */
const TaskStatus = {
  PENDING: 'pending',
  OPEN: 'open',
  MATCHED: 'matched',
  ASSIGNED: 'assigned',
  RUNNING: 'running',
  VERIFYING: 'verifying',
  COMPLETED: 'completed',
  VERIFIED: 'verified',
  SETTLED: 'settled',
  REWORK: 'rework',
  NO_QUORUM: 'no_quorum',
  DISPUTED: 'disputed',
  REJECTED: 'rejected',
};

// 合法状态转移表
const TRANSITIONS = {
  [TaskStatus.PENDING]: [TaskStatus.OPEN],
  [TaskStatus.OPEN]: [TaskStatus.MATCHED, TaskStatus.ASSIGNED],
  [TaskStatus.MATCHED]: [TaskStatus.ASSIGNED],
  [TaskStatus.ASSIGNED]: [TaskStatus.RUNNING],
  [TaskStatus.RUNNING]: [TaskStatus.COMPLETED, TaskStatus.REWORK],
  [TaskStatus.COMPLETED]: [TaskStatus.VERIFIED, TaskStatus.DISPUTED, TaskStatus.REWORK, TaskStatus.NO_QUORUM],
  [TaskStatus.VERIFIED]: [TaskStatus.SETTLED],
  [TaskStatus.REWORK]: [TaskStatus.RUNNING], // 恢复边：返工 → 继续执行
  [TaskStatus.NO_QUORUM]: [TaskStatus.OPEN], // 恢复边：无共识 → 重新开放
  [TaskStatus.DISPUTED]: [TaskStatus.SETTLED, TaskStatus.REJECTED],
  [TaskStatus.SETTLED]: [],
  [TaskStatus.REJECTED]: [],
};

/** 智能体卡片 */
class AgentCard {
  constructor(did, name) {
    this.did = did;
    this.name = name;
    this.capabilities = [];
    this.skills = [];
    this.version = '1.0.0';
    this.stake = 0;
    this.reputation = 0;
  }

  static new({ did, name }) {
    return new AgentCard(did, name);
  }

  withCapability(cap) {
    if (!this.capabilities.includes(cap)) this.capabilities.push(cap);
    return this;
  }

  withSkill(skill) {
    if (!this.skills.includes(skill)) this.skills.push(skill);
    return this;
  }

  hasCapability(cap) {
    return this.capabilities.includes(cap);
  }

  toJSON() {
    return {
      did: this.did, name: this.name,
      capabilities: this.capabilities, skills: this.skills,
      version: this.version, stake: this.stake, reputation: this.reputation,
    };
  }
}

/** 任务与状态机 */
class Task {
  constructor(taskId, goal) {
    this.taskId = taskId;
    this.goal = goal;
    this.status = TaskStatus.PENDING;
    this.assignee = null;
    this.verificationPolicy = 'BftLite'; // 验证策略；'None' 时提交即验收、豁免证据门禁
    this.evidenceGrade = null;           // 结果信封证据等级
    this.history = [TaskStatus.PENDING];
  }

  transition(next) {
    const allowed = TRANSITIONS[this.status] || [];
    if (!allowed.includes(next)) {
      throw new Error(`非法状态转移: ${this.status} → ${next}`);
    }
    this.status = next;
    this.history.push(next);
    return this;
  }

  assign(did) {
    if (this.status === TaskStatus.OPEN) this.transition(TaskStatus.MATCHED);
    if (this.status === TaskStatus.MATCHED) this.transition(TaskStatus.ASSIGNED);
    this.assignee = did;
    return this;
  }

  start() {
    return this.transition(TaskStatus.RUNNING);
  }

  complete() {
    return this.transition(TaskStatus.COMPLETED);
  }

  verify(passed) {
    if (passed) return this.transition(TaskStatus.VERIFIED);
    return this.transition(TaskStatus.DISPUTED);
  }

  settle() {
    if (this.status === TaskStatus.VERIFIED) return this.transition(TaskStatus.SETTLED);
    throw new Error('只有验证通过的任务才能结算');
  }

  /** 本轮验收无共识（v2.6.0） */
  markNoQuorum() {
    return this.transition(TaskStatus.NO_QUORUM);
  }

  /** 恢复边：无共识任务重新开放（v2.6.0，消除吸收态） */
  reopenAfterNoQuorum() {
    if (this.status !== TaskStatus.NO_QUORUM) {
      throw new Error(`只有 NO_QUORUM 状态才能重新开放，当前 ${this.status}`);
    }
    return this.transition(TaskStatus.OPEN);
  }

  /** 恢复边：返工任务回到执行中（v2.6.0） */
  resumeAfterRework() {
    if (this.status !== TaskStatus.REWORK) {
      throw new Error(`只有 REWORK 状态才能恢复执行，当前 ${this.status}`);
    }
    return this.transition(TaskStatus.RUNNING);
  }

  get isTerminal() {
    return this.status === TaskStatus.SETTLED || this.status === TaskStatus.REJECTED;
  }
}

module.exports = { AgentCard, Task, TaskStatus, TRANSITIONS };
