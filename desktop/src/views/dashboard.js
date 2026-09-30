// Agent Universe v2.9.0 — 工作台总览

import { api, escapeHtml } from '../api.js';
import {
  normalizeAgents, normalizeTasks, getField, getTaskId,
  taskStateBadge, fmtMoney, shortId, safe, emptyHtml,
} from './util.js';

export const dashboardView = {
  id: 'dashboard',
  name: '工作台',
  icon: '◈',
  section: '工作区',

  async render(container, app) {
    container.innerHTML =
      '<div class="view-head"><h2>工作台</h2>' +
        '<span class="muted small" id="dash-env"></span>' +
        '<div class="actions"><button class="btn btn-primary btn-sm" id="new-task">+ 新建任务</button>' +
        '<button class="btn btn-sm" id="new-agent">注册智能体</button></div></div>' +
      '<div class="grid cols-4" style="margin-bottom:16px">' +
        '<div class="card stat-card"><div class="stat-label">智能体</div>' +
          '<div class="stat-value" id="s-agents">–</div><div class="stat-sub">已注册</div></div>' +
        '<div class="card stat-card"><div class="stat-label">进行中任务</div>' +
          '<div class="stat-value" id="s-tasks">–</div><div class="stat-sub" id="s-tasks-sub">总任务 0</div></div>' +
        '<div class="card stat-card"><div class="stat-label">网络节点</div>' +
          '<div class="stat-value" id="s-peers">–</div><div class="stat-sub">P2P 连接</div></div>' +
        '<div class="card stat-card"><div class="stat-label">守恒</div>' +
          '<div class="stat-value" id="s-cons">–</div><div class="stat-sub" id="s-cons-sub">账本审计</div></div>' +
      '</div>' +
      '<div class="grid cols-3">' +
        '<div class="card" style="grid-column: span 2;"><h3>最近任务</h3>' +
          '<div id="recent-tasks">' + emptyHtml('▤', '暂无任务，点击右上角「新建任务」') + '</div></div>' +
        '<div class="card"><h3>系统状态</h3><div id="sys-status"></div></div>' +
      '</div>';

    const env = container.querySelector('#dash-env');
    if (env && app.info) env.textContent = app.info.workspace;
    container.querySelector('#new-task').onclick = () => app.navigate('tasks');
    container.querySelector('#new-agent').onclick = () => app.navigate('agents');
    await load(container, app);
  },

  async poll(container, app) {
    await load(container, app);
  },
};

async function load(container, app) {
  const [agentsD, tasksD, statsD, versionD, consD, peersD] = await Promise.all([
    safe(api.listAgents()),
    safe(api.listTasks()),
    safe(api.stats()),
    safe(api.version()),
    safe(api.conservation()),
    safe(api.get('/peers')),
  ]);

  const agents = normalizeAgents(agentsD);
  const tasks = normalizeTasks(tasksD);
  const terminalStates = ['settled', 'cancelled', 'rejected'];
  const active = tasks.filter((t) => {
    const s = String(getField(t, ['state', 'status'], '')).toLowerCase();
    return !terminalStates.includes(s);
  });

  // 智能体
  const agentCount =
    agents.length ||
    (statsD && !statsD.__error
      ? getField(statsD, ['agent_count', 'total_agents', 'agents'], 0)
      : 0);
  setText(container, '#s-agents', agentCount);

  // 任务
  setText(container, '#s-tasks', active.length);
  setText(container, '#s-tasks-sub', '总任务 ' + tasks.length);

  // 网络
  let peers = 0;
  if (Array.isArray(peersD)) peers = peersD.length;
  else if (peersD && !peersD.__error) {
    peers = getField(peersD, ['peer_count', 'count'],
      Array.isArray(peersD.peers) ? peersD.peers.length : 0);
  }
  if ((!peers) && statsD && !statsD.__error) {
    peers = getField(statsD, ['peer_count', 'peers', 'connected_peers'], peers);
  }
  setText(container, '#s-peers', peers);

  // 守恒
  if (consD && !consD.__error) {
    const conserved = getField(consD, ['conserved', 'balanced', 'is_conserved', 'ok'], undefined);
    const el = container.querySelector('#s-cons');
    if (el) {
      if (conserved === true) { el.textContent = '✓'; el.style.color = 'var(--ok)'; }
      else if (conserved === false) { el.textContent = '✗'; el.style.color = 'var(--danger)'; }
      else { el.textContent = 'OK'; el.style.color = 'var(--ok)'; }
    }
    setText(container, '#s-cons-sub',
      getField(consD, ['message', 'note'], '账本守恒'));
  } else {
    setText(container, '#s-cons', '—');
  }

  // 最近任务
  const recent = tasks.slice(0, 6);
  const recentEl = container.querySelector('#recent-tasks');
  if (recentEl) {
    if (!recent.length) {
      recentEl.innerHTML = emptyHtml('▤', '暂无任务');
    } else {
      recentEl.innerHTML =
        '<table class="table"><thead><tr><th>ID</th><th>名称</th><th>状态</th>' +
        '<th class="num">预算</th></tr></thead><tbody>' +
        recent.map((t) => {
          const id = getTaskId(t);
          const name = getField(t, ['title', 'name', 'description'], id);
          const state = getField(t, ['state', 'status'], '—');
          const budget = getField(t, ['budget', 'budget_amount'], 0);
          return (
            '<tr class="clickable" data-detail="' + escapeHtml(id) + '">' +
            '<td><code class="inline">' + shortId(id) + '</code></td>' +
            '<td>' + escapeHtml(String(name).slice(0, 42)) + '</td>' +
            '<td>' + taskStateBadge(state) + '</td>' +
            '<td class="num">' + fmtMoney(budget) + '</td></tr>'
          );
        }).join('') +
        '</tbody></table>';
      recentEl.querySelectorAll('[data-detail]').forEach((r) => {
        r.onclick = () => {
          app.pendingTaskId = r.dataset.detail;
          app.navigate('tasks');
        };
      });
    }
  }

  // 系统状态
  const sysEl = container.querySelector('#sys-status');
  if (sysEl) {
    const dv =
      versionD && !versionD.__error
        ? typeof versionD === 'string'
          ? versionD
          : getField(versionD, ['version', 'core'], '—')
        : '—';
    sysEl.innerHTML =
      '<dl class="kv">' +
      '<dt>客户端</dt><dd>v' + escapeHtml(app.info?.version || '—') +
        ' · ' + escapeHtml(app.info?.platform || '') + '</dd>' +
      '<dt>daemon</dt><dd>' + escapeHtml(dv) + '</dd>' +
      '<dt>API 端口</dt><dd>' + app.daemon.port + '</dd>' +
      '<dt>数据目录</dt><dd>' + escapeHtml(app.info?.daemon_data_dir || '') + '</dd>' +
      '<dt>工作区</dt><dd>' + escapeHtml(app.info?.workspace || '') + '</dd>' +
      '</dl>';
  }
}

function setText(container, sel, val) {
  const el = container.querySelector(sel);
  if (el) el.textContent = val;
}
