// Agent Universe v2.9.0 — 设置
// 过程展示分级、后台运行、daemon 生命周期、数据与配置路径、版本

import { tauri, el, clear, escapeHtml } from '../api.js';

let cfg = null;
let info = null;

export const settingsView = {
  id: 'settings',
  name: '设置',
  icon: '⚙',
  section: '系统',
  async render(container, app) {
    clear(container);
    container.appendChild(renderSettings(app));
  },
};

function renderSettings(app) {
  const root = el('<div class="fade-in"></div>');
  const head = el(
    '<div class="view-head"><div><div class="view-title">设置</div>' +
      '<div class="view-sub">工作台偏好、守护进程与数据</div></div></div>'
  );
  const body = el('<div id="settings-body"></div>');
  body.innerHTML = '<div class="loading">加载…</div>';
  root.append(head, body);

  Promise.all([tauri.getConfig(), tauri.appInfo(), tauri.daemonStatus()])
    .then(([c, i, d]) => {
      cfg = c; info = i;
      draw(app, body, d);
    })
    .catch((e) => {
      body.innerHTML = '<div class="error-box">' + escapeHtml(String(e)) + '</div>';
    });

  return root;
}

function draw(app, body, daemonStatus) {
  const vis = cfg.process_visibility || 'steps';
  const preferences =
    '<div class="card"><div class="card-title">工作台偏好</div>' +
    '<div class="field"><label class="field-label">Agent 过程展示</label>' +
    '<div class="seg" id="vis-seg">' +
    '<button data-vis="results">只看结果</button>' +
    '<button data-vis="steps">关键步骤</button>' +
    '<button data-vis="full">完整过程</button></div>' +
    '<div class="muted small">控制任务执行过程中展示的详细程度</div></div>' +
    '<label class="switch setting-switch"><input type="checkbox" id="bg-toggle"' + (cfg.run_in_background ? ' checked' : '') + '>' +
    '<span>长任务在后台继续运行</span></label>' +
    '</div>';

  const running = daemonStatus && daemonStatus.running;
  const healthy = daemonStatus && daemonStatus.healthy;
  const daemon =
    '<div class="card"><div class="card-title">守护进程（gsn-daemon）</div>' +
    '<div class="daemon-status-line">' +
    '<span class="dot ' + (healthy ? 'dot-ok' : running ? 'dot-warn' : 'dot-off') + '"></span>' +
    '<span>' + (healthy ? '运行中（健康）' : running ? '运行中（未就绪）' : '已停止') + '</span>' +
    (daemonStatus && daemonStatus.port ? '<span class="muted">端口 ' + daemonStatus.port + '</span>' : '') +
    '</div>' +
    '<div class="btn-row">' +
    '<button class="btn" data-daemon="start">启动</button>' +
    '<button class="btn" data-daemon="stop">停止</button>' +
    '<button class="btn btn-primary" data-daemon="restart">重启</button>' +
    '</div>' +
    (daemonStatus && daemonStatus.bin ? '<div class="muted mono small break-all">' + escapeHtml(daemonStatus.bin) + '</div>' : '') +
    '</div>';

  const paths =
    '<div class="card"><div class="card-title">数据与路径</div><table class="kv">' +
    row('工作区', info.workspace) +
    row('Daemon 数据', info.daemon_data_dir) +
    row('配置文件', info.config_path) +
    row('版本', info.version) +
    row('平台', info.platform) +
    '</table></div>';

  body.innerHTML =
    '<div class="settings-grid">' + preferences + daemon + paths + '</div>';

  const visSeg = body.querySelector('#vis-seg');
  visSeg.querySelectorAll('button').forEach((b) => {
    b.classList.toggle('active', b.dataset.vis === vis);
    b.onclick = async () => {
      cfg.process_visibility = b.dataset.vis;
      visSeg.querySelectorAll('button').forEach((x) => x.classList.remove('active'));
      b.classList.add('active');
      await save(app);
    };
  });

  const bgToggle = body.querySelector('#bg-toggle');
  bgToggle.onchange = async () => {
    cfg.run_in_background = bgToggle.checked;
    await save(app);
  };

  body.querySelectorAll('button[data-daemon]').forEach((b) => {
    b.onclick = async () => {
      const act = b.dataset.daemon;
      app.toast(act === 'start' ? '启动中…' : act === 'stop' ? '停止中…' : '重启中…');
      try {
        if (act === 'start') await tauri.startDaemon();
        else if (act === 'stop') await tauri.stopDaemon();
        else await tauri.restartDaemon();
        app.toast('已执行：' + act);
        app.rerender();
      } catch (e) {
        app.toast('操作失败：' + e, 'error');
      }
    };
  });
}

async function save(app) {
  try {
    await tauri.saveConfig(cfg);
  } catch (e) {
    app.toast('保存失败：' + e, 'error');
  }
}

function row(k, v) {
  return '<tr><th>' + k + '</th><td class="mono break-all">' + escapeHtml(v || '—') + '</td></tr>';
}
