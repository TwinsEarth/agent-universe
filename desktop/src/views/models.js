// Agent Universe v2.9.0 — 模型提供商统一入口
// 提供商增删改查、启用/禁用、设为当前；配置持久化到本地 config.json

import { tauri, el, clear, escapeHtml } from '../api.js';

let cfg = null;

export const modelsView = {
  id: 'models',
  name: '模型提供商',
  icon: '◐',
  section: '工具',
  async render(container, app) {
    clear(container);
    container.appendChild(renderModels(app));
  },
};

function renderModels(app) {
  const root = el('<div class="fade-in"></div>');
  const head = el(
    '<div class="view-head"><div><div class="view-title">模型提供商</div>' +
      '<div class="view-sub">统一管理模型、端点与 API Key</div></div>' +
      '<button class="btn btn-primary" id="btn-add">＋ 添加提供商</button></div>'
  );
  const body = el('<div id="models-body"></div>');
  body.innerHTML = '<div class="loading">加载配置…</div>';
  root.append(head, body);

  head.querySelector('#btn-add').onclick = () => editModal(app, null);

  tauri
    .getConfig()
    .then((c) => {
      cfg = c;
      draw(app, body);
    })
    .catch((e) => {
      body.innerHTML = '<div class="error-box">配置加载失败：' + escapeHtml(String(e)) + '</div>';
    });

  return root;
}

function draw(app, body) {
  const note =
    '<div class="info-note">配置已持久化到本地。当前核心默认使用 mock 适配器；' +
    '真实 HTTP 调用的注入在后续版本接入（见 GAP 分析 LLM 层）。</div>';
  const cards = (cfg.providers || [])
    .map((p) => {
      const isActive = cfg.active_provider === p.id;
      return (
        '<div class="provider-card' + (p.enabled ? '' : ' disabled') + '">' +
        '<div class="provider-head"><div class="provider-name">' +
        (isActive ? '<span class="badge ok">当前</span> ' : '') +
        escapeHtml(p.name) + ' <span class="badge">' + escapeHtml(p.kind) + '</span></div>' +
        '<div class="provider-actions">' +
        '<button class="btn sm" data-act="edit" data-id="' + escapeHtml(p.id) + '">编辑</button>' +
        '<button class="btn sm btn-danger" data-act="del" data-id="' + escapeHtml(p.id) + '">删除</button>' +
        '</div></div>' +
        '<table class="kv">' +
        '<tr><th>Base URL</th><td class="mono">' + escapeHtml(p.base_url) + '</td></tr>' +
        '<tr><th>模型</th><td class="mono">' + escapeHtml(p.model) + '</td></tr>' +
        '<tr><th>API Key</th><td>' + (p.api_key ? '<span class="muted">••••••••' + escapeHtml(p.api_key.slice(-4)) + '</span>' : '<span class="muted">未配置</span>') + '</td></tr>' +
        '</table>' +
        '<div class="provider-foot">' +
        '<label class="switch"><input type="checkbox" data-act="enable" data-id="' + escapeHtml(p.id) + '"' + (p.enabled ? ' checked' : '') + '><span>启用</span></label>' +
        (isActive ? '' : '<button class="btn sm" data-act="active" data-id="' + escapeHtml(p.id) + '">设为当前</button>') +
        '</div>' +
        '</div>'
      );
    })
    .join('');

  body.innerHTML =
    note +
    '<div class="card-grid">' + (cards || '<div class="empty">暂无提供商</div>') + '</div>';

  body.querySelectorAll('button[data-act]').forEach((b) => {
    b.onclick = () => {
      const id = b.dataset.id;
      if (b.dataset.act === 'edit') return editModal(app, id);
      if (b.dataset.act === 'del') return removeProvider(app, body, id);
      if (b.dataset.act === 'active') return setActive(app, body, id);
    };
  });
  body.querySelectorAll('input[data-act="enable"]').forEach((cb) => {
    cb.onchange = () => toggleEnable(app, body, cb.dataset.id, cb.checked);
  });
}

async function persist(app, body) {
  try {
    await tauri.saveConfig(cfg);
    draw(app, body);
  } catch (e) {
    app.toast('保存失败：' + e, 'error');
  }
}

function toggleEnable(app, body, id, enabled) {
  const p = cfg.providers.find((x) => x.id === id);
  if (p) {
    p.enabled = enabled;
    if (enabled) cfg.active_provider = id;
    persist(app, body);
  }
}

function setActive(app, body, id) {
  const p = cfg.providers.find((x) => x.id === id);
  if (p) {
    cfg.active_provider = id;
    p.enabled = true;
    persist(app, body);
  }
}

function removeProvider(app, body, id) {
  cfg.providers = cfg.providers.filter((x) => x.id !== id);
  if (cfg.active_provider === id) {
    cfg.active_provider = cfg.providers[0] ? cfg.providers[0].id : '';
  }
  persist(app, body);
}

function editModal(app, id) {
  const existing = id ? cfg.providers.find((x) => x.id === id) : null;
  const p = existing || {
    id: 'custom-' + String(Date.now()).slice(-5),
    name: '',
    kind: 'custom',
    base_url: '',
    api_key: '',
    model: '',
    enabled: true,
  };
  const m = el(
    '<div class="modal-mask"><div class="modal"><div class="modal-head">' +
      (existing ? '编辑提供商' : '添加提供商') +
      '<span class="modal-x">×</span></div><div class="modal-body"></div></div></div>'
  );
  const body = m.querySelector('.modal-body');
  body.innerHTML =
    inField('名称', 'text', 'f-name', '例如：MyProvider') +
    selectKind('类型', 'f-kind', p.kind) +
    inField('Base URL', 'text', 'f-url', 'https://…') +
    inField('API Key', 'password', 'f-key', 'sk-…') +
    inField('默认模型', 'text', 'f-model', 'model-name') +
    '<label class="switch"><input type="checkbox" id="f-enabled"' + (p.enabled ? ' checked' : '') + '><span>启用</span></label>' +
    '<div class="modal-actions"><button class="btn" data-act="cancel">取消</button>' +
    '<button class="btn btn-primary" data-act="ok">保存</button></div>';

  body.querySelector('#f-name').value = p.name;
  body.querySelector('#f-url').value = p.base_url;
  body.querySelector('#f-key').value = p.api_key;
  body.querySelector('#f-model').value = p.model;

  m.querySelector('.modal-x').onclick = () => m.remove();
  body.querySelector('[data-act="cancel"]').onclick = () => m.remove();
  body.querySelector('[data-act="ok"]').onclick = async () => {
    p.name = body.querySelector('#f-name').value.trim();
    p.kind = body.querySelector('#f-kind').value;
    p.base_url = body.querySelector('#f-url').value.trim();
    const key = body.querySelector('#f-key').value;
    // 编辑时若 key 留空则保留原值
    if (key) p.api_key = key;
    p.model = body.querySelector('#f-model').value.trim();
    p.enabled = body.querySelector('#f-enabled').checked;
    if (!p.name) return app.toast('名称必填', 'error');
    if (!existing) cfg.providers.push(p);
    if (p.enabled) cfg.active_provider = p.id;
    try {
      await tauri.saveConfig(cfg);
      m.remove();
      app.toast('已保存');
      app.rerender();
    } catch (e) {
      app.toast('保存失败：' + e, 'error');
    }
  };
  document.body.appendChild(m);
}

function inField(label, type, id, ph) {
  return (
    '<div class="field"><label class="field-label">' + label + '</label>' +
    '<input class="input" type="' + type + '" id="' + id + '" placeholder="' + escapeHtml(ph || '') + '"></div>'
  );
}

function selectKind(label, id, current) {
  const kinds = ['openai', 'anthropic', 'deepseek', 'ollama', 'custom'];
  return (
    '<div class="field"><label class="field-label">' + label + '</label><select class="input" id="' + id + '">' +
    kinds.map((k) => '<option value="' + k + '"' + (k === current ? ' selected' : '') + '>' + k + '</option>').join('') +
    '</select></div>'
  );
}
