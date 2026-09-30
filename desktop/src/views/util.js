// Agent Universe v2.9.0 — 视图共享工具

import { escapeHtml } from '../api.js';

// ── 原始数组提取 ─────────────────────────────────────

export function normalizeAgents(d) {
  if (Array.isArray(d)) return d;
  if (d && typeof d === 'object') {
    for (const k of ['agents', 'items', 'registered', 'list']) {
      if (Array.isArray(d[k])) return d[k];
    }
  }
  return [];
}

export function normalizeTasks(d) {
  if (Array.isArray(d)) return d;
  if (d && typeof d === 'object') {
    for (const k of ['tasks', 'items', 'list']) {
      if (Array.isArray(d[k])) return d[k];
    }
  }
  return [];
}

export function getField(obj, keys, def) {
  if (!obj || typeof obj !== 'object') return def;
  for (const k of keys) {
    if (obj[k] !== undefined && obj[k] !== null && obj[k] !== '') return obj[k];
  }
  for (const k of keys) {
    const camel = k.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
    if (obj[camel] !== undefined && obj[camel] !== null && obj[camel] !== '') return obj[camel];
  }
  return def;
}

function asArr(v) {
  if (Array.isArray(v)) return v;
  return v ? [v] : [];
}

// ── 规范化对象 ─────────────────────────────────────

export function agent(raw) {
  return {
    raw,
    id: getField(raw, ['agent_id', 'agentId', 'id', 'did'], ''),
    name: getField(raw, ['name', 'agent_name', 'agentName'], '未命名'),
    stake: getField(raw, ['stake', 'stake_amount', 'stakeAmount'], 0),
    skills: asArr(getField(raw, ['skills', 'required_skills', 'capabilities'], [])),
    reputation: getField(raw, ['reputation', 'reputation_score'], 0),
  };
}

export function agents(d) {
  return normalizeAgents(d).map(agent);
}

export function task(raw) {
  return {
    raw,
    id: getField(raw, ['task_id', 'taskId', 'id'], ''),
    goal: getField(raw, ['goal', 'title', 'objective'], ''),
    context: getField(raw, ['context', 'description', 'desc'], ''),
    done: asArr(getField(raw, ['done', 'completed'], [])),
    todo: asArr(getField(raw, ['todo', 'pending'], [])),
    trace: asArr(getField(raw, ['trace', 'logs'], [])),
    owner: getField(raw, ['owner', 'assignee'], null),
    budget: getField(raw, ['budget'], 0),
    winnerPrice: getField(raw, ['winner_price', 'winnerPrice'], null),
    deadline: getField(raw, ['deadline'], 0),
    skills: asArr(getField(raw, ['required_skills', 'requiredSkills', 'skills'], [])),
    policy: getField(raw, ['verification_policy', 'verificationPolicy'], 'None'),
    requester: getField(raw, ['requester', 'client'], ''),
    state: getField(raw, ['state'], 'Open'),
    createdAt: getField(raw, ['created_at', 'createdAt'], 0),
  };
}

export function tasks(d) {
  return normalizeTasks(d).map(task);
}

// ── 展示 ─────────────────────────────────────

export function getAgentId(a) {
  return getField(a, ['agent_id', 'agentId', 'id', 'did'], '—');
}

export function getTaskId(t) {
  return getField(t, ['task_id', 'taskId', 'id'], '—');
}

export function taskStateBadge(state) {
  const s = String(state || '').toLowerCase();
  const map = {
    draft: '', open: 'info', bidding: 'info', bid: 'info', matched: 'info',
    running: 'warn', in_progress: 'warn', inprogress: 'warn',
    pending_review: 'warn', pendingreview: 'warn', review: 'warn',
    verifying: 'warn', verified: 'ok', completed: 'ok', accepted: 'ok',
    disputed: 'danger', arbitration: 'danger', settled: 'ok',
    cancelled: 'danger', rejected: 'danger', rework: 'warn',
    no_quorum: 'warn', slashed: 'danger',
  };
  return '<span class="badge ' + (map[s] || '') + '">' + escapeHtml(state || '—') + '</span>';
}

export function evidenceBadge(grade) {
  const s = String(grade || '').toLowerCase();
  if (s.includes('verified')) return '<span class="badge ok">' + escapeHtml(grade) + '</span>';
  if (s.includes('proto') || s.includes('cpu')) return '<span class="badge warn">' + escapeHtml(grade) + '</span>';
  return '<span class="badge">' + escapeHtml(grade || 'Unverified') + '</span>';
}

export function fmtMoney(v) {
  if (v === null || v === undefined || v === '') return '0';
  if (typeof v === 'object') {
    if (v.amount !== undefined) v = v.amount;
    else if (v.value !== undefined) v = v.value;
  }
  const n = Number(v);
  return Number.isFinite(n) ? n.toLocaleString('en-US') : '0';
}

export function shortId(id, n = 14) {
  const s = String(id || '');
  return s.length > n ? s.slice(0, n) + '…' : s;
}

export function loadingHtml(msg = '加载中…') {
  return '<div class="empty"><div class="ico">◌</div>' + escapeHtml(msg) + '</div>';
}

export function emptyHtml(icon, msg) {
  if (msg === undefined) { msg = icon; icon = '◇'; }
  return '<div class="empty"><div class="ico">' + icon + '</div>' + escapeHtml(msg) + '</div>';
}

export function errorHtml(msg) {
  return '<div class="error-box">' + escapeHtml(msg) + '</div>';
}

export async function safe(p) {
  try {
    return await p;
  } catch (e) {
    return { __error: e.message || String(e) };
  }
}

// 聚合对象（视图里 `U.xxx`）
export const U = {
  agents, agent, tasks, task,
  normalizeAgents, normalizeTasks, getField,
  getAgentId, getTaskId,
  taskStateBadge, evidenceBadge, fmtMoney, shortId,
  loadingHtml, emptyHtml, errorHtml, safe,
};
