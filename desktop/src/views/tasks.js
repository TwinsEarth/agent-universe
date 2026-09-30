// Agent Universe v2.9.0 — 任务工作台
// 列表 / 新建 / 详情 / 状态驱动的全流程操作（投标→匹配→结果→验收→结算/争议/恢复）

import { api, el, clear, escapeHtml } from '../api.js';
import { U, loadingHtml, emptyHtml } from './util.js';
import { buildVerification } from '../crypto.js';

let view = { mode: 'list', id: null };
let agentsCache = [];

export const tasksView = {
  id: 'tasks',
  name: '任务',
  icon: '▤',
  section: '工作区',
  async render(container, app) {
    if (app.pendingTaskId) {
      view = { mode: 'detail', id: app.pendingTaskId };
      app.pendingTaskId = null;
    }
    clear(container);
    container.appendChild(view.mode === 'detail' ? renderDetail(app, view.id) : renderList(app));
  },
  async poll(container, app) {
    if (view.mode !== 'list') return;
    const listEl = container.querySelector('#task-list');
    if (listEl) await loadListData(app, listEl);
  },
};

// ───────────────────────── 列表 ─────────────────────────

function renderList(app) {
  const root = el('<div class="fade-in"></div>');
  const head = el(
    '<div class="view-head"><div><div class="view-title">任务市场</div>' +
      '<div class="view-sub">发布、投标、匹配与结算的完整任务流程</div></div>' +
      '<button class="btn btn-primary" id="btn-new-task">＋ 新建任务</button></div>'
  );
  const body = el('<div id="task-list"></div>');
  body.innerHTML = loadingHtml('加载任务…');
  root.append(head, body);

  head.querySelector('#btn-new-task').onclick = () => newTaskModal(app);

  loadListData(app, body);

  return root;
}

async function loadListData(app, body) {
  try {
    const tasks = U.tasks(await api.listTasks());
    body.innerHTML = tasks.length
      ? taskTable(tasks)
      : emptyHtml('暂无任务，点击「新建任务」发布第一个任务');
    body.querySelectorAll('tr[data-id]').forEach((tr) => {
      tr.onclick = () => {
        view = { mode: 'detail', id: tr.dataset.id };
        app.rerender();
      };
    });
  } catch (e) {
    body.innerHTML =
      '<div class="error-box">加载失败：' + escapeHtml(String(e)) + '</div>';
  }
}

function taskTable(tasks) {
  const rows = tasks
    .map((t) => {
      return (
        '<tr data-id="' + escapeHtml(t.id) + '">' +
        '<td class="mono">' + escapeHtml(U.shortId(t.id)) + '</td>' +
        '<td>' + escapeHtml(t.goal) + '</td>' +
        '<td>' + U.taskStateBadge(t.state) + '</td>' +
        '<td class="num">' + U.fmtMoney(t.budget) + '</td>' +
        '<td class="num">' + (t.winnerPrice != null ? U.fmtMoney(t.winnerPrice) : '—') + '</td>' +
        '<td>' +
        t.skills.slice(0, 3).map((s) => '<span class="badge">' + escapeHtml(s) + '</span>').join(' ') +
        '</td>' +
        '<td class="mono">' + escapeHtml(U.shortId(t.requester)) + '</td>' +
        '<td class="muted">' + (t.deadline ? new Date(t.deadline).toLocaleDateString() : '—') + '</td>' +
        '</tr>'
      );
    })
    .join('');
  return (
    '<div class="card"><table><thead><tr>' +
    '<th>ID</th><th>目标</th><th>状态</th><th>预算</th><th>中标价</th><th>技能</th><th>发布者</th><th>截止</th>' +
    '</tr></thead><tbody>' +
    rows +
    '</tbody></table></div>'
  );
}

// ───────────────────────── 新建 ─────────────────────────

function newTaskModal(app) {
  const m = el(
    '<div class="modal-mask"><div class="modal"><div class="modal-head">新建任务' +
      '<span class="modal-x">×</span></div><div class="modal-body"></div></div></div>'
  );
  const body = m.querySelector('.modal-body');
  body.innerHTML =
    field('需求方账户 requester', 'text', 't-requester', 'did:nau:… 或账户名') +
    field('目标 goal', 'text', 't-goal', '要完成什么？') +
    textarea('上下文 context', 't-context', '背景信息、约束、输入…') +
    textarea('待办 todo（每行一条）', 't-todo', '第一步\n第二步\n第三步') +
    '<div class="form-row">' +
    field('预算（整数）', 'number', 't-budget', '1000') +
    field('截止（天）', 'number', 't-days', '7') +
    '</div>' +
    tagInput('所需技能 required_skills', 't-skills') +
    selectField('验证策略', 't-policy', [
      ['None', '无需验证'],
      ['BftLite', 'BFT-lite 委员（3 人 / 容错 1）'],
      ['Sampling', '简单抽样（50%）'],
    ]) +
    '<div class="modal-actions"><button class="btn" data-act="cancel">取消</button>' +
    '<button class="btn btn-primary" data-act="ok">发布任务</button></div>';

  m.querySelector('.modal-x').onclick = () => m.remove();
  body.querySelector('[data-act="cancel"]').onclick = () => m.remove();
  body.querySelector('[data-act="ok"]').onclick = async () => {
    const now = Date.now();
    const days = parseInt(body.querySelector('#t-days').value, 10) || 7;
    const budget = parseInt(body.querySelector('#t-budget').value, 10);
    const policyTag = body.querySelector('#t-policy').value;
    const policy =
      policyTag === 'BftLite'
        ? { BftLite: { n: 3, f: 1 } }
        : policyTag === 'Sampling'
          ? { Sampling: { ratio: 0.5 } }
          : 'None';
    const spec = {
      task_id: 'task-' + (crypto.randomUUID ? crypto.randomUUID().slice(0, 8) : String(now).slice(-6)),
      goal: body.querySelector('#t-goal').value.trim(),
      context: body.querySelector('#t-context').value.trim(),
      done: [],
      todo: body.querySelector('#t-todo').value.split('\n').map((s) => s.trim()).filter(Boolean),
      trace: [],
      owner: null,
      budget: Number.isFinite(budget) ? budget : 0,
      winner_price: null,
      deadline: now + days * 86400000,
      required_skills: readTags(body, '#t-skills'),
      verification_policy: policy,
      requester: body.querySelector('#t-requester').value.trim(),
      created_at: now,
    };
    try {
      await api.publishTask(spec);
      m.remove();
      app.toast('任务已发布：' + spec.task_id);
      app.rerender();
    } catch (e) {
      app.toast('发布失败：' + e, 'error');
    }
  };
  document.body.appendChild(m);
}

// ───────────────────────── 详情 ─────────────────────────

function renderDetail(app, id) {
  const root = el('<div class="fade-in"></div>');
  const head = el(
    '<div class="view-head"><div class="flex-center gap8">' +
      '<button class="btn" id="btn-back">← 返回</button>' +
      '<div><div class="view-title mono">' + escapeHtml(id) + '</div>' +
      '<div class="view-sub" id="d-state"></div></div></div></div>'
  );
  const body = el('<div id="detail-body"></div>');
  body.innerHTML = loadingHtml('加载任务详情…');
  root.append(head, body);
  head.querySelector('#btn-back').onclick = () => {
    view = { mode: 'list' };
    app.rerender();
  };

  Promise.all([api.getTask(id), api.listAgents()])
    .then(([tr, ar]) => {
      const t = U.task(tr);
      agentsCache = U.agents(ar);
      head.querySelector('#d-state').innerHTML = U.taskStateBadge(t.state);
      body.innerHTML = detailHtml(t);
      wireDetail(app, t);
    })
    .catch((e) => {
      body.innerHTML = '<div class="error-box">加载失败：' + escapeHtml(String(e)) + '</div>';
    });

  return root;
}

function detailHtml(t) {
  return (
    '<div class="grid-2">' +
    infoCard(t) +
    '<div>' +
    progressCard(t) +
    resultCard(t) +
    '</div>' +
    '</div>' +
    actionsCard(t)
  );
}

function infoCard(t) {
  return (
    '<div class="card"><div class="card-title">基本信息</div><table class="kv">' +
    kv('目标', escapeHtml(t.goal)) +
    kv('上下文', escapeHtml(t.context) || '<span class="muted">—</span>') +
    kv('发布者', '<span class="mono">' + escapeHtml(t.requester) + '</span>') +
    kv('责任所有者', '<span class="mono">' + (t.owner ? escapeHtml(t.owner) : '<span class="muted">未定</span>') + '</span>') +
    kv('预算', '<span class="num">' + U.fmtMoney(t.budget) + '</span>') +
    kv('中标价', '<span class="num">' + (t.winnerPrice != null ? U.fmtMoney(t.winnerPrice) : '—') + '</span>') +
    kv('截止', t.deadline ? new Date(t.deadline).toLocaleString() : '—') +
    kv('验证策略', '<span class="mono">' + escapeHtml(policyLabel(t.policy)) + '</span>') +
    kv('所需技能', t.skills.map((s) => '<span class="badge">' + escapeHtml(s) + '</span>').join(' ')) +
    kv('创建时间', new Date(t.createdAt).toLocaleString()) +
    '</table></div>'
  );
}

function progressCard(t) {
  return (
    '<div class="card"><div class="card-title">进度</div>' +
    listBlock('已完成 done', t.done, 'ok') +
    listBlock('待办 todo', t.todo, 'muted') +
    listBlock('追踪 trace', t.trace, 'info') +
    '</div>'
  );
}

function resultCard(t) {
  if (!t.owner && !['Running', 'Matched', 'Accepted', 'Verifying'].includes(t.state)) return '';
  return (
    '<div class="card"><div class="card-title">执行状态</div>' +
    '<div class="muted">中标 / 执行 Agent：<span class="mono">' + escapeHtml(t.owner || '未匹配') + '</span></div>' +
    '</div>'
  );
}

function actionsCard(t) {
  const buttons = [];
  // 资金
  buttons.push(btn('deposit', '需求方充值', 'btn'));
  // 投标 / 匹配
  if (t.state === 'Open') {
    buttons.push(btn('bid', '提交投标', 'btn'));
    buttons.push(btn('match', '自动匹配', 'btn-primary'));
    buttons.push(btn('reject', '拒绝任务', 'btn-danger'));
  }
  // 结果
  if (['Matched', 'Running', 'Rework'].includes(t.state)) {
    buttons.push(btn('result', '提交结果', 'btn-primary'));
  }
  // 验收
  if (['Verifying', 'Running'].includes(t.state)) {
    buttons.push(btn('verify-continue', '验收通过', 'btn-primary'));
    buttons.push(btn('verify-stop', '验收不通过', 'btn-danger'));
  }
  // 恢复
  if (t.state === 'NoQuorum') buttons.push(btn('reopen', '重新开放', 'btn-primary'));
  if (t.state === 'Rework') buttons.push(btn('resume', '恢复执行', 'btn-primary'));
  // 结算
  if (['Accepted', 'Rejected'].includes(t.state)) buttons.push(btn('settle', '结算 / 释放托管', 'btn-primary'));
  // 争议 / 仲裁
  if (!['Settled', 'Slashed'].includes(t.state)) {
    buttons.push(btn('dispute', '发起争议', 'btn'));
  }

  return (
    '<div class="card"><div class="card-title">操作（按当前状态：' +
    escapeHtml(t.state) + '）</div><div class="action-bar" id="action-bar">' +
    buttons.join(' ') +
    '</div></div>'
  );
}

function btn(act, label, cls) {
  return '<button class="btn ' + cls + '" data-act="' + act + '">' + label + '</button>';
}

// ───────────────────────── 操作接线 ─────────────────────────

function wireDetail(app, t) {
  const bar = document.getElementById('action-bar');
  if (!bar) return;
  bar.querySelectorAll('button[data-act]').forEach((b) => {
    b.onclick = () => handleAction(app, t, b.dataset.act);
  });
}

function handleAction(app, t, act) {
  switch (act) {
    case 'deposit':
      return depositModal(app, t);
    case 'bid':
      return bidModal(app, t);
    case 'match':
      return runAction(app, t.id, 'match', null, '匹配完成');
    case 'result':
      return resultModal(app, t);
    case 'verify-continue':
      // 验收通过 = 停止迭代（QA 语义 Stop）
      return verify(app, t, 'Stop');
    case 'verify-stop':
      // 验收不通过 = 继续返工（QA 语义 Continue）
      return verify(app, t, 'Continue');
    case 'settle':
      return runAction(app, t.id, 'settle', null, '结算完成');
    case 'reopen':
      return runAction(app, t.id, 'reopen', null, '已重新开放');
    case 'resume':
      return runAction(app, t.id, 'resume', null, '已恢复执行');
    case 'reject':
      return runAction(app, t.id, 'reject', null, '已拒绝');
    case 'dispute':
      return disputeModal(app, t);
  }
}

async function runAction(app, id, action, body, okMsg) {
  try {
    const r = await api.taskAction(id, action, body);
    app.toast(okMsg);
    app.rerender();
    return r;
  } catch (e) {
    app.toast('操作失败：' + e, 'error');
  }
}

function depositModal(app, t) {
  const m = simpleModal('需求方充值', [
    field('金额（整数）', 'number', 'f-amount', '1000'),
  ]);
  m.querySelector('[data-act="ok"]').onclick = async () => {
    const amount = parseInt(m.querySelector('#f-amount').value, 10);
    if (!Number.isFinite(amount)) return app.toast('金额无效', 'error');
    try {
      await api.deposit(t.requester, amount);
      m.remove();
      app.toast('充值成功');
    } catch (e) {
      app.toast('充值失败：' + e, 'error');
    }
  };
}

function bidModal(app, t) {
  const opts = agentsCache
    .map((a) => '<option value="' + escapeHtml(a.id) + '">' + escapeHtml(a.name) + ' (' + U.shortId(a.id) + ')</option>')
    .join('');
  const m = simpleModal('提交投标', [
    '<label class="field-label">投标 Agent</label><select id="f-agent" class="input">' + (opts || '<option value="">无可用 Agent</option>') + '</select>',
    field('报价 proposed_price（整数）', 'number', 'f-price', String(t.budget)),
    field('预计耗时 ms', 'number', 'f-latency', '1000'),
    field('评分 score', 'number', 'f-score', '0.9'),
  ]);
  m.querySelector('[data-act="ok"]').onclick = async () => {
    const agentId = m.querySelector('#f-agent').value;
    const price = parseInt(m.querySelector('#f-price').value, 10);
    if (!agentId) return app.toast('请选择 Agent', 'error');
    try {
      await api.taskAction(t.id, 'bids', {
        agent_id: agentId,
        proposed_price: Number.isFinite(price) ? price : 0,
        estimated_latency_ms: parseInt(m.querySelector('#f-latency').value, 10) || 1000,
        score: parseFloat(m.querySelector('#f-score').value) || 0.9,
      });
      m.remove();
      app.toast('投标已提交');
      app.rerender();
    } catch (e) {
      app.toast('投标失败：' + e, 'error');
    }
  };
}

function resultModal(app, t) {
  const m = simpleModal('提交结果', [
    '<label class="field-label">执行 Agent</label><select id="f-agent" class="input">' +
      agentOptions(t.owner) + '</select>',
    textarea('结果报告 report', 'f-report', '完成情况、产出…'),
    field('置信度 confidence', 'number', 'f-conf', '0.9'),
    selectField('证据等级', 'f-grade', [
      ['Verified', 'Verified（已核验）'],
      ['CpuProto', 'CpuProto（CPU 原型）'],
      ['Unverified', 'Unverified（未核验）'],
    ]),
    field('耗时 ms', 'number', 'f-latency', '1000'),
  ]);
  m.querySelector('[data-act="ok"]').onclick = async () => {
    const envelope = {
      agent_id: m.querySelector('#f-agent').value,
      report: JSON.stringify({ summary: m.querySelector('#f-report').value }),
      confidence: parseFloat(m.querySelector('#f-conf').value) || 0.9,
      error_type: 'None',
      trace_ref: '',
      evidence_grade: m.querySelector('#f-grade').value,
      latency_ms: parseInt(m.querySelector('#f-latency').value, 10) || 1000,
    };
    try {
      await api.taskAction(t.id, 'results', envelope);
      m.remove();
      app.toast('结果已提交');
      app.rerender();
    } catch (e) {
      app.toast('提交失败：' + e, 'error');
    }
  };
}

function agentOptions(prefer) {
  const list = agentsCache.length ? agentsCache : prefer ? [{ id: prefer, name: prefer }] : [];
  return list
    .map((a) => '<option value="' + escapeHtml(a.id) + '"' + (a.id === prefer ? ' selected' : '') + '>' +
      escapeHtml(a.name) + ' (' + U.shortId(a.id) + ')</option>')
    .join('') || '<option value="">无可用 Agent</option>';
}

async function verify(app, t, voteTag) {
  const n = t.policy && t.policy.BftLite ? t.policy.BftLite.n : 3;
  const body = buildVerification(t.id, 0, n, voteTag);
  try {
    await api.taskAction(t.id, 'verify', body);
    app.toast(voteTag === 'Stop' ? '验收通过' : '验收不通过');
    app.rerender();
  } catch (e) {
    app.toast('验收失败：' + e, 'error');
  }
}

function disputeModal(app, t) {
  const m = simpleModal('发起争议', [
    field('申诉方 complainant', 'text', 'f-complainant', t.requester),
    field('被诉方 respondent', 'text', 'f-respondent', t.owner || ''),
    textarea('理由 reason', 'f-reason', '争议原因…'),
  ]);
  m.querySelector('[data-act="ok"]').onclick = async () => {
    const dispute = {
      dispute_id: 'disp-' + String(Date.now()).slice(-6),
      task_id: t.id,
      complainant: m.querySelector('#f-complainant').value.trim(),
      respondent: m.querySelector('#f-respondent').value.trim(),
      reason: m.querySelector('#f-reason').value.trim(),
    };
    try {
      const r = await api.openDispute(dispute);
      m.remove();
      const dId = (r && r.dispute_id) || dispute.dispute_id;
      arbitrationModal(app, dId);
    } catch (e) {
      app.toast('争议提交失败：' + e, 'error');
    }
  };
}

function arbitrationModal(app, disputeId) {
  const m = simpleModal('仲裁（争议 ' + disputeId + '）', [
    field('仲裁者 arbitrator', 'text', 'f-arbitrator', 'did:nau:…'),
    selectField('裁定', 'f-guilty', [
      ['false', '无责（继续）'],
      ['true', '有责（罚没）'],
    ]),
  ]);
  m.querySelector('[data-act="ok"]').onclick = async () => {
    const arbitrator = m.querySelector('#f-arbitrator').value.trim();
    const guilty = m.querySelector('#f-guilty').value === 'true';
    if (!arbitrator) return app.toast('仲裁者必填', 'error');
    try {
      await api.arbitrate(disputeId, arbitrator, guilty);
      m.remove();
      app.toast('仲裁完成');
      app.rerender();
    } catch (e) {
      app.toast('仲裁失败：' + e, 'error');
    }
  };
}

// ───────────────────────── 通用控件 ─────────────────────────

function field(label, type, id, ph) {
  return (
    '<div class="field"><label class="field-label">' + label + '</label>' +
    '<input class="input" type="' + type + '" id="' + id + '" placeholder="' + escapeHtml(ph || '') + '"></div>'
  );
}

function textarea(label, id, ph) {
  return (
    '<div class="field"><label class="field-label">' + label + '</label>' +
    '<textarea class="input" id="' + id + '" rows="3" placeholder="' + escapeHtml(ph || '') + '"></textarea></div>'
  );
}

function selectField(label, id, options) {
  const opts = options.map((o) => '<option value="' + o[0] + '">' + o[1] + '</option>').join('');
  return '<div class="field"><label class="field-label">' + label + '</label><select class="input" id="' + id + '">' + opts + '</select></div>';
}

function tagInput(label, id) {
  return (
    '<div class="field"><label class="field-label">' + label + '</label>' +
    '<input class="input" id="' + id + '" placeholder="输入技能后回车，逗号分隔也可"></div>'
  );
}

function readTags(body, sel) {
  const raw = body.querySelector(sel).value;
  return raw
    .split(/[,\n]/)
    .map((s) => s.trim())
    .filter(Boolean);
}

function kv(k, v) {
  return '<tr><th>' + k + '</th><td>' + v + '</td></tr>';
}

function listBlock(title, items, cls) {
  if (!items || !items.length) {
    return '<div class="list-block"><div class="list-title">' + title + '</div><div class="muted">—</div></div>';
  }
  return (
    '<div class="list-block"><div class="list-title">' + title + '（' + items.length + '）</div>' +
    '<ul class="dot-list">' + items.map((i) => '<li class="' + cls + '">' + escapeHtml(i) + '</li>').join('') + '</ul></div>'
  );
}

function policyLabel(p) {
  if (!p) return 'None';
  if (p === 'None') return 'None';
  if (p.BftLite) return 'BftLite n=' + p.BftLite.n + ' f=' + p.BftLite.f;
  if (p.Sampling) return 'Sampling ratio=' + p.Sampling.ratio;
  return 'None';
}

function simpleModal(title, fieldsHtml) {
  const m = el(
    '<div class="modal-mask"><div class="modal"><div class="modal-head">' + escapeHtml(title) +
      '<span class="modal-x">×</span></div><div class="modal-body"></div></div></div>'
  );
  const body = m.querySelector('.modal-body');
  body.innerHTML =
    fieldsHtml.join('') +
    '<div class="modal-actions"><button class="btn" data-act="cancel">取消</button>' +
    '<button class="btn btn-primary" data-act="ok">确定</button></div>';
  m.querySelector('.modal-x').onclick = () => m.remove();
  body.querySelector('[data-act="cancel"]').onclick = () => m.remove();
  document.body.appendChild(m);
  return body;
}
