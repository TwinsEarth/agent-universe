// Agent Universe v2.9.0 — 应用核心：初始化、路由、骨架、toast、首次引导、轮询

import { tauri, setApiPort, escapeHtml } from './api.js';

export class App {
  constructor(root) {
    this.root = root;
    this.info = null;
    this.config = null;
    this.daemon = { port: 4002, healthy: false, bin: '' };
    this.views = [];
    this.current = null;
    this.pollTimer = null;
    this.mainEl = null;
  }

  registerView(v) {
    this.views.push(v);
  }

  setBootSub(t) {
    const el = document.getElementById('boot-sub');
    if (el) el.textContent = t;
  }

  showBootError(msg) {
    const boot = document.getElementById('boot');
    if (boot) {
      boot.innerHTML =
        '<div class="boot-logo" style="color:var(--danger)">!</div>' +
        '<div class="boot-title">初始化失败</div>' +
        '<div class="boot-sub" style="max-width:460px;text-align:center">' +
        escapeHtml(msg) + '</div>' +
        '<button class="btn btn-primary" id="boot-retry" style="margin-top:10px">重试</button>';
      document.getElementById('boot-retry').onclick = () => location.reload();
    }
  }

  async init() {
    try {
      this.setBootSub('正在读取应用信息…');
      this.info = await tauri.appInfo();
      this.config = await tauri.getConfig();

      this.setBootSub('检查本地 daemon 状态…');
      let status = await tauri.daemonStatus();
      if (!status.healthy) {
        this.setBootSub('正在启动本地 gsn-daemon…');
        try {
          status = await tauri.startDaemon();
        } catch (e) {
          this.showBootError(e);
          return;
        }
      }
      this.daemon.port = status.port;
      this.daemon.healthy = status.healthy;
      this.daemon.bin = status.bin;
      setApiPort(status.port);

      this.renderShell();
      this.navigate('dashboard');
      this.startPolling();
      if (!this.config.onboarding_done) this.openOnboarding();
    } catch (e) {
      this.showBootError(e.message || String(e));
    }
  }

  /* ── 骨架 ── */
  renderShell() {
    const sections = [];
    for (const v of this.views) {
      let s = sections.find((x) => x.name === v.section);
      if (!s) {
        s = { name: v.section, items: [] };
        sections.push(s);
      }
      s.items.push(v);
    }

    const sideHtml = sections
      .map(
        (s) =>
          '<div class="nav-section">' + escapeHtml(s.name) + '</div>' +
          s.items
            .map(
              (v) =>
                '<div class="nav-item" data-nav="' + v.id + '">' +
                '<span class="ico">' + v.icon + '</span>' +
                '<span>' + escapeHtml(v.name) + '</span>' +
                (v.count ? '<span class="badge-count" data-count="' + v.id + '"></span>' : '') +
                '</div>'
            )
            .join('')
      )
      .join('');

    this.root.innerHTML =
      '<div class="workbench">' +
        '<div class="topbar">' +
          '<div class="brand"><span class="logo">◈</span> Agent Universe</div>' +
          '<div class="spacer"></div>' +
          '<div class="top-actions">' +
            '<div class="conn"><span class="dot" id="conn-dot"></span>' +
              '<span id="conn-text">—</span></div>' +
            '<button class="btn btn-sm" id="btn-restart-daemon" title="重启 daemon">重启服务</button>' +
          '</div>' +
        '</div>' +
        '<div class="body">' +
          '<div class="sidebar">' + sideHtml +
            '<div class="side-foot">v' + this.info.version + ' · ' +
              escapeHtml(this.info.platform) + '</div>' +
          '</div>' +
          '<div class="main" id="main"></div>' +
        '</div>' +
      '</div>' +
      '<div id="toast-host"></div>';

    this.mainEl = document.getElementById('main');
    this.root.querySelectorAll('.nav-item').forEach((n) => {
      n.onclick = () => this.navigate(n.dataset.nav);
    });
    document.getElementById('btn-restart-daemon').onclick = async () => {
      this.updateConn('busy', '重启中…');
      try {
        const st = await tauri.restartDaemon();
        this.daemon.port = st.port;
        this.daemon.healthy = st.healthy;
        setApiPort(st.port);
        this.updateConn(st.healthy ? 'ok' : 'down',
          st.healthy ? '已连接 :' + st.port : '未连接');
        this.navigate(this.current);
        this.toast('daemon 已重启', 'ok');
      } catch (e) {
        this.updateConn('down', '未连接');
        this.toast('重启失败：' + e.message, 'err');
      }
    };
    this.updateConn(this.daemon.healthy ? 'ok' : 'down',
      this.daemon.healthy ? '已连接 :' + this.daemon.port : '未连接');
  }

  updateConn(kind, text) {
    const dot = document.getElementById('conn-dot');
    const tx = document.getElementById('conn-text');
    if (dot) dot.className = 'dot ' + kind;
    if (tx) tx.textContent = text;
  }

  /* ── 路由 ── */
  rerender() {
    if (this.current) this.navigate(this.current);
  }

  navigate(id) {
    const v = this.views.find((x) => x.id === id);
    if (!v) return;
    this.current = id;
    this.root.querySelectorAll('.nav-item').forEach((n) =>
      n.classList.toggle('active', n.dataset.nav === id));
    this.mainEl.innerHTML = '';
    v.render(this.mainEl, this).catch((e) => {
      this.mainEl.innerHTML = '<div class="error-box">视图渲染失败：' + escapeHtml(e.message || String(e)) + '</div>';
    });
  }

  /* ── 轮询 ── */
  startPolling() {
    if (this.pollTimer) clearInterval(this.pollTimer);
    this.pollTimer = setInterval(async () => {
      const v = this.views.find((x) => x.id === this.current);
      if (v && v.poll) {
        try {
          await v.poll(this.mainEl, this);
        } catch { /* 轮询失败静默 */ }
      }
      // 连接状态
      try {
        const st = await tauri.daemonStatus();
        const kind = st.healthy ? 'ok' : 'down';
        const text = st.healthy ? '已连接 :' + st.port : '未连接';
        if (this.daemon.healthy !== st.healthy) {
          this.daemon.healthy = st.healthy;
          this.daemon.port = st.port;
          if (st.healthy) setApiPort(st.port);
          this.updateConn(kind, text);
        }
      } catch { /* 忽略 */ }
    }, 4000);
  }

  /* ── Toast ── */
  toast(msg, kind = 'info') {
    const host = document.getElementById('toast-host');
    if (!host) return;
    const colors = { ok: '#16a34a', err: '#dc2626', info: '#2563eb', warn: '#d97706' };
    const t = document.createElement('div');
    t.textContent = msg;
    t.style.cssText =
      'background:' + (colors[kind] || colors.info) + ';color:#fff;padding:9px 16px;' +
      'border-radius:9px;font-size:13px;margin-top:8px;box-shadow:0 6px 20px rgba(0,0,0,.18);' +
      'opacity:0;transition:opacity .2s,transform .2s;transform:translateY(6px)';
    host.style.cssText = 'position:fixed;right:18px;bottom:18px;z-index:200;display:flex;' +
      'flex-direction:column;align-items:flex-end';
    host.appendChild(t);
    requestAnimationFrame(() => { t.style.opacity = '1'; t.style.transform = 'translateY(0)'; });
    setTimeout(() => {
      t.style.opacity = '0';
      setTimeout(() => t.remove(), 250);
    }, 2800);
  }

  /* ── 模态 ── */
  modal({ title, sub, body, okText = '确定', cancelText = '取消', onOk, hideFooter }) {
    const mask = document.createElement('div');
    mask.className = 'modal-mask';
    mask.innerHTML =
      '<div class="modal">' +
        '<h3>' + escapeHtml(title) + '</h3>' +
        (sub ? '<div class="modal-sub">' + escapeHtml(sub) + '</div>' : '') +
        '<div class="modal-body"></div>' +
        (hideFooter ? '' :
          '<div class="modal-foot"><button class="btn" data-cancel>' + escapeHtml(cancelText) +
          '</button><button class="btn btn-primary" data-ok>' + escapeHtml(okText) + '</button></div>') +
      '</div>';
    const bodyEl = mask.querySelector('.modal-body');
    if (typeof body === 'string') bodyEl.innerHTML = body;
    else if (body) bodyEl.appendChild(body);
    const close = () => mask.remove();
    mask.querySelector('[data-cancel]')?.addEventListener('click', close);
    mask.querySelector('[data-ok]')?.addEventListener('click', async () => {
      if (onOk) {
        const r = await onOk(bodyEl, close);
        if (r === false) return;
      }
      close();
    });
    this.root.appendChild(mask);
    return { mask, bodyEl, close };
  }

  /* ── 首次引导 ── */
  openOnboarding() {
    const cfg = JSON.parse(JSON.stringify(this.config));
    let picked = cfg.active_provider || 'deepseek';
    let apiKey = '';
    const active = cfg.providers.find((p) => p.id === picked);
    if (active) apiKey = active.api_key;

    const mask = document.createElement('div');
    mask.className = 'modal-mask';
    mask.innerHTML =
      '<div class="modal onboarding">' +
        '<div class="hero"><div class="logo">◈</div>' +
          '<h2>欢迎使用 Agent Universe</h2>' +
          '<p>默认工作区已就绪。选择一个模型提供商并配置 API Key，即可开始派发任务。</p></div>' +
        '<div class="provider-pick" id="pick"></div>' +
        '<div class="field"><label>API Key <span class="hint">（仅保存在本机配置）</span></label>' +
          '<input id="pk" type="password" placeholder="sk-..." /></div>' +
        '<div class="field"><label>默认模型</label><input id="pm" /></div>' +
        '<div class="modal-foot">' +
          '<button class="btn" id="later">稍后配置</button>' +
          '<button class="btn btn-primary" id="save">保存并开始</button>' +
        '</div>' +
      '</div>';
    this.root.appendChild(mask);

    const pickEl = mask.querySelector('#pick');
    const pkEl = mask.querySelector('#pk');
    const pmEl = mask.querySelector('#pm');

    function renderPick() {
      pickEl.innerHTML = cfg.providers
        .map(
          (p) =>
            '<div class="provider-chip ' + (p.id === picked ? 'active' : '') +
            '" data-p="' + p.id + '">' + escapeHtml(p.name) + '</div>'
        )
        .join('');
      pickEl.querySelectorAll('.provider-chip').forEach((c) =>
        c.onclick = () => {
          picked = c.dataset.p;
          const p = cfg.providers.find((x) => x.id === picked);
          pkEl.value = p ? p.api_key : '';
          pmEl.value = p ? p.model : '';
          renderPick();
        });
      const p = cfg.providers.find((x) => x.id === picked);
      pkEl.value = p ? p.api_key : '';
      pmEl.value = p ? p.model : '';
    }
    renderPick();

    const finish = async (save) => {
      if (save) {
        cfg.active_provider = picked;
        const p = cfg.providers.find((x) => x.id === picked);
        if (p) {
          p.api_key = pkEl.value.trim();
          p.model = pmEl.value.trim() || p.model;
          p.enabled = true;
        }
      }
      cfg.onboarding_done = true;
      try {
        await tauri.saveConfig(cfg);
        this.config = cfg;
      } catch (e) {
        this.toast('配置保存失败：' + e.message, 'err');
        return;
      }
      mask.remove();
      this.toast('工作台已就绪', 'ok');
    };
    mask.querySelector('#save').onclick = () => finish(true);
    mask.querySelector('#later').onclick = () => finish(false);
  }

  /* ── 保存配置（视图用） ── */
  async updateConfig(mutator) {
    const cfg = JSON.parse(JSON.stringify(this.config));
    mutator(cfg);
    await tauri.saveConfig(cfg);
    this.config = cfg;
  }
}
