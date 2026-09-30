// Agent Universe v2.9.0 — 前端 API 封装
// 1) Tauri 原生命令（daemon 管理、配置、终端、文件、对话框）
// 2) gsn-daemon HTTP 客户端（agents / tasks / 沙箱 / 网络 / 市场）

import { invoke } from '@tauri-apps/api/core';

// ── Tauri 原生命令 ──────────────────────────────────────────
export const tauri = {
  appInfo: () => invoke('get_app_info'),
  daemonStatus: () => invoke('get_daemon_status'),
  startDaemon: () => invoke('start_daemon'),
  stopDaemon: () => invoke('stop_daemon'),
  restartDaemon: () => invoke('restart_daemon'),
  getConfig: () => invoke('get_config'),
  saveConfig: (cfg) => invoke('save_config', { cfg }),
  runShell: (cmd, cwd) => invoke('run_shell', { cmd, cwd }),
  listDir: (path) => invoke('list_dir', { path }),
  readText: (path) => invoke('read_text', { path }),
  readBinaryBase64: (path) => invoke('read_binary_base64', { path }),
  pickFolder: () => invoke('pick_folder'),
};

// ── Daemon HTTP 客户端 ─────────────────────────────────────
let apiBase = 'http://127.0.0.1:4002';
export function setApiPort(port) {
  apiBase = `http://127.0.0.1:${port}`;
}
export function getApiBase() {
  return apiBase;
}

async function http(method, path, body) {
  const init = { method };
  if (body !== undefined) {
    init.headers = { 'Content-Type': 'application/json' };
    init.body = JSON.stringify(body);
  }
  let res;
  try {
    res = await fetch(apiBase + path, init);
  } catch (e) {
    const err = new Error('无法连接 daemon（' + apiBase + '）：' + e.message);
    err.network = true;
    throw err;
  }
  const text = await res.text();
  let data = null;
  if (text) {
    try { data = JSON.parse(text); } catch { data = text; }
  }
  if (!res.ok) {
    const msg = data && typeof data === 'object' && (data.error || data.message)
      ? (data.error || data.message)
      : `HTTP ${res.status}`;
    const err = new Error(msg);
    err.status = res.status;
    err.data = data;
    throw err;
  }
  return data;
}

export const api = {
  get: (p) => http('GET', p),
  post: (p, b) => http('POST', p, b ?? {}),

  // 节点
  health: () => http('GET', '/health'),
  version: () => http('GET', '/version'),

  // Agents
  listAgents: (params) =>
    http('GET', '/api/v1/agents?' + (params ? new URLSearchParams(params) : 'all=1')),
  getAgent: (id) => http('GET', `/api/v1/agents/${encodeURIComponent(id)}`),
  registerAgent: (card) => http('POST', '/api/v1/agents', card),

  // Tasks
  listTasks: () => http('GET', '/api/v1/tasks?all=1'),
  getTask: (id) => http('GET', `/api/v1/tasks/${encodeURIComponent(id)}`),
  publishTask: (t) => http('POST', '/api/v1/tasks', t),
  taskAction: (id, action, body) =>
    http('POST', `/api/v1/tasks/${encodeURIComponent(id)}/${action}`, body ?? {}),

  // Accounts
  balance: (acct) => http('GET', `/api/v1/accounts/${encodeURIComponent(acct)}/balance`),
  deposit: (acct, amount) =>
    http('POST', `/api/v1/accounts/${encodeURIComponent(acct)}/deposit`, { amount }),

  // Disputes
  openDispute: (d) => http('POST', '/api/v1/disputes', d),
  arbitrate: (id, arbitrator, guilty) =>
    http('POST', `/api/v1/disputes/${encodeURIComponent(id)}/arbitrate`, { arbitrator, guilty }),

  // 市场审计 / 统计
  conservation: () => http('GET', '/api/v1/conservation'),
  audit: () => http('GET', '/api/v1/audit'),
  leaderboard: (limit = 10) => http('GET', `/api/v1/leaderboard?limit=${limit}`),
  stats: () => http('GET', '/api/v1/stats'),

  // 沙箱
  createSandbox: (cfg) => http('POST', '/api/v1/sandboxes', cfg ?? {}),
  sandboxExec: (id, language, code) =>
    http('POST', `/api/v1/sandboxes/${encodeURIComponent(id)}/exec`, { language, code }),
  sandboxLifecycle: (id, action) =>
    http('POST', `/api/v1/sandboxes/${encodeURIComponent(id)}/${action}`, {}),

  // 网络
  network: (path) => http('GET', '/api/v1/network' + path),
};

// ── 通用工具 ─────────────────────────────────────────────
export function el(html) {
  const t = document.createElement('template');
  t.innerHTML = html.trim();
  return t.content.firstElementChild;
}

export function clear(node) {
  while (node.firstChild) node.removeChild(node.firstChild);
  return node;
}

export function escapeHtml(s) {
  if (s === null || s === undefined) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
