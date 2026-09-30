// Agent Universe v2.9.0 — 智能体工作台
// 卡片网格 / 注册（自动生成 DID + 充值托管）/ 详情 / Agent Team 团队面板

import { api, el, clear, escapeHtml } from '../api.js';
import { U } from './util.js';
import { makeAgentIdentity } from '../crypto.js';

let mode = 'cards'; // cards | team

export const agentsView = {
  id: 'agents',
  name: '智能体',
  icon: '❖',
  section: '工作区',
  async render(container, app) {
    clear(container);
    container.appendChild(renderAgents(app));
  },
  async poll(container, app) {
    const body = container.querySelector('#agents-body');
    if (body) await loadAgentsData(app, body);
  },
};

function renderAgents(app) {
  const root = el('<div class="fade-in"></div>');
  const head = el(
    '<div class="view-head"><div><div class="view-title">智能体</div>' +
      '<div class="view-sub">注册、身份、质押与团队协作</div></div>' +
      '<div class="flex-center gap8">' +
      '<div class="seg"><button data-mode="cards">卡片</button>' +
      '<button data-mode="team">Agent Team</button></div>' +
      '<button class="btn btn-primary" id="btn-register">＋ 注册智能体</button></div></div>'
  );
  const body = el('<div id="agents-body"></div>');
  body.innerHTML = U.loadingHtml('加载智能体…');
  root.append(head, body);

  head.querySelectorAll('.seg button').forEach((b) => {
    b.onclick = () => {
      mode = b.dataset.mode;
      app.rerender();
    };
  });
  head.querySelector('#btn-register').onclick = () => registerModal(app);
  head.querySelectorAll('.seg button').forEach((b) =>
    b.classList.toggle('active', b.dataset.mode === mode));

  loadAgentsData(app, body);

  return root;
}

async function loadAgentsData(app, body) {
  try {
    const [ad, td] = await Promise.all([api.listAgents(), api.listTasks()]);
    const agents = U.agents(ad);
    const tasks = U.tasks(td);
    body.innerHTML =
      mode === 'team'
        ? teamHtml(agents, tasks)
        : agents.length ? cardsHtml(agents) : U.emptyHtml('暂无智能体，点击「注册智能体」');
    body.querySelectorAll('[data-id]').forEach((node) => {
      node.onclick = () => detailModal(app, node.dataset.id, agents);
    });
  } catch (e) {
    body.innerHTML = '<div class="error-box">加载失败：' + escapeHtml(String(e)) + '</div>';
  }
}

function cardsHtml(agents) {
  return (
    '<div class="card-grid">' +
    agents
      .map((a) => {
        return (
          '<div class="agent-card" data-id="' + escapeHtml(a.id) + '">' +
          '<div class="agent-top"><div class="agent-avatar">' + escapeHtml(a.name.slice(0, 2).toUpperCase()) + '</div>' +
          '<div><div class="agent-name">' + escapeHtml(a.name) + '</div>' +
          '<div class="agent-did mono">' + U.shortId(a.id, 18) + '</div></div></div>' +
          '<div class="agent-skills">' +
          a.skills.slice(0, 4).map((s) => '<span class="badge">' + escapeHtml(s) + '</span>').join(' ') +
          '</div>' +
          '<div class="agent-meta"><span>质押 <b>' + U.fmtMoney(a.stake) + '</b></span>' +
          '<span class="muted">信誉 ' + Number(a.reputation || 0).toFixed(2) + '</span></div>' +
          '</div>'
        );
      })
      .join('') +
    '</div>'
  );
}

function teamHtml(agents, tasks) {
  const byOwner = {};
  tasks.forEach((t) => {
    if (t.owner) {
      (byOwner[t.owner] = byOwner[t.owner] || []).push(t);
    }
  });
  const rows = agents
    .map((a) => {
      const assigned = byOwner[a.id] || [];
      const active = assigned.filter((t) => !['Settled', 'Slashed', 'Rejected'].includes(t.state));
      return (
        '<tr class="team-row" data-id="' + escapeHtml(a.id) + '">' +
        '<td><div class="flex-center gap8"><div class="agent-avatar sm">' + escapeHtml(a.name.slice(0, 2).toUpperCase()) + '</div>' +
        '<div><div>' + escapeHtml(a.name) + '</div><div class="muted mono">' + U.shortId(a.id, 16) + '</div></div></div></td>' +
        '<td>' + U.taskStateBadge(active.length ? active[0].state : 'Idle') + '</td>' +
        '<td>' +
        (active.length
          ? active.map((t) => '<div class="team-task">' + escapeHtml(t.goal) + '</div>').join('')
          : '<span class="muted">空闲</span>') +
        '</td>' +
        '<td class="num">' + active.length + ' / ' + assigned.length + '</td>' +
        '</tr>'
      );
    })
    .join('');
  return (
    '<div class="card"><div class="card-title">Agent Team — 实时成员与任务</div>' +
    (agents.length
      ? '<table><thead><tr><th>成员</th><th>状态</th><th>当前任务</th><th class="num">活跃/总计</th></tr></thead><tbody>' +
        rows + '</tbody></table>'
      : U.emptyHtml('团队暂无成员')) +
    '</div>'
  );
}

// ───────────────────────── 注册 ─────────────────────────

function registerModal(app) {
  const m = el(
    '<div class="modal-mask"><div class="modal modal-lg"><div class="modal-head">注册智能体' +
      '<span class="modal-x">×</span></div><div class="modal-body"></div></div></div>'
  );
  const body = m.querySelector('.modal-body');
  body.innerHTML =
    '<div class="flex-center gap8">' +
    '<button class="btn" id="btn-gen-id">⟳ 生成去中心化身份</button>' +
    '<span class="muted mono" id="id-preview">尚未生成身份</span></div>' +
    inField('名称 name', 'text', 'f-name', '例如：ResearchAgent') +
    inField('描述 description', 'text', 'f-desc', '能力简介') +
    tagBox('技能 skills（逗号分隔）', 'f-skills', 'research, code, mcp') +
    '<div class="form-row">' +
    inField('质押 stake（整数，注册即锁定）', 'number', 'f-stake', '100') +
    inField('单次报价 price（整数）', 'number', 'f-price', '10') +
    '</div>' +
    '<div class="modal-actions"><button class="btn" data-act="cancel">取消</button>' +
    '<button class="btn btn-primary" data-act="ok">充值并注册</button></div>';

  let identity = null;
  m.querySelector('.modal-x').onclick = () => m.remove();
  body.querySelector('[data-act="cancel"]').onclick = () => m.remove();
  body.querySelector('#btn-gen-id').onclick = () => {
    identity = makeAgentIdentity();
    body.querySelector('#id-preview').textContent = identity.did;
  };

  body.querySelector('[data-act="ok"]').onclick = async () => {
    if (!identity) identity = makeAgentIdentity();
    const name = body.querySelector('#f-name').value.trim();
    const stake = parseInt(body.querySelector('#f-stake').value, 10);
    const price = parseInt(body.querySelector('#f-price').value, 10);
    if (!name) return app.toast('名称必填', 'error');
    if (!(stake > 0)) return app.toast('质押必须为正整数', 'error');
    const now = Date.now();
    const card = {
      agent_id: identity.did,
      version: '2.9.0',
      name,
      description: body.querySelector('#f-desc').value.trim(),
      skills: body.querySelector('#f-skills').value.split(',').map((s) => s.trim()).filter(Boolean),
      modalities: ['text'],
      models: [],
      endpoint: '',
      pricing: { model: 'PerCall', price: Number.isFinite(price) ? price : 0, currency: 'Credit' },
      sla: { latency_p95_ms: 2000, availability: 0.95, max_concurrency: 10 },
      owner: identity.did,
      stake,
      reputation_score: 0,
      total_calls: 0,
      success_rate: 0,
      evidence_grade: 'Unverified',
      verified: false,
      created_at: now,
      updated_at: now,
    };
    try {
      // 防铸币：先向该 DID 账户充值（覆盖质押），再注册锁定
      await api.deposit(identity.did, stake);
      await api.registerAgent(card);
      m.remove();
      app.toast('已注册：' + name);
      app.rerender();
    } catch (e) {
      app.toast('注册失败：' + e, 'error');
    }
  };
  document.body.appendChild(m);
}

// ───────────────────────── 详情 ─────────────────────────

function detailModal(app, id, agents) {
  const a = agents.find((x) => x.id === id);
  if (!a) return;
  const m = el(
    '<div class="modal-mask"><div class="modal"><div class="modal-head">智能体详情' +
      '<span class="modal-x">×</span></div><div class="modal-body"></div></div></div>'
  );
  const body = m.querySelector('.modal-body');
  m.querySelector('.modal-x').onclick = () => m.remove();
  body.innerHTML = U.loadingHtml('加载余额…');
  api
    .balance(id)
    .then((bal) => {
      const b = bal && bal.balance !== undefined ? bal.balance : bal;
      body.innerHTML =
        '<table class="kv">' +
        row('DID', '<span class="mono">' + escapeHtml(a.id) + '</span>') +
        row('名称', escapeHtml(a.name)) +
        row('描述', escapeHtml((a.raw && a.raw.description) || '') || '—') +
        row('技能', a.skills.map((s) => '<span class="badge">' + escapeHtml(s) + '</span>').join(' ')) +
        row('质押', '<span class="num">' + U.fmtMoney(a.stake) + '</span>') +
        row('账户余额', '<span class="num">' + U.fmtMoney(b) + '</span>') +
        row('信誉', Number(a.reputation || 0).toFixed(2)) +
        row('已验证', a.raw && a.raw.verified ? '是' : '否') +
        '</table>' +
        '<div class="modal-actions"><button class="btn btn-primary" data-act="close">关闭</button></div>';
      body.querySelector('[data-act="close"]').onclick = () => m.remove();
    })
    .catch((e) => {
      body.innerHTML = '<div class="error-box">' + escapeHtml(String(e)) + '</div>';
    });
  document.body.appendChild(m);
}

function inField(label, type, id, ph) {
  return (
    '<div class="field"><label class="field-label">' + label + '</label>' +
    '<input class="input" type="' + type + '" id="' + id + '" placeholder="' + escapeHtml(ph || '') + '"></div>'
  );
}

function tagBox(label, id, ph) {
  return (
    '<div class="field"><label class="field-label">' + label + '</label>' +
    '<input class="input" id="' + id + '" placeholder="' + escapeHtml(ph || '') + '"></div>'
  );
}

function row(k, v) {
  return '<tr><th>' + k + '</th><td>' + v + '</td></tr>';
}
