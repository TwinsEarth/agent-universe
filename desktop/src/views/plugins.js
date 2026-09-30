// Agent Universe v2.9.0 — 插件与扩展
// 内置能力目录 + 实验性扩展开关（持久化到 feature_flags）
// 诚实标注：当前版本没有前端动态插件运行时（安装/运行时卸载），实验扩展仅做接入开关。

import { tauri, el, clear, escapeHtml } from '../api.js';

let cfg = null;

const BUILTIN = [
  { id: 'mcp', name: 'MCP 协议', desc: 'Model Context Protocol 服务与工具', icon: '⧉' },
  { id: 'sandbox', name: '智能体沙箱', desc: '受控代码执行（Python / JavaScript）', icon: '▷' },
  { id: 'p2p', name: 'P2P 网络', desc: 'libp2p mesh、relay、DHT', icon: '❖' },
  { id: 'did', name: 'DID 身份', desc: 'Ed25519 去中心化身份与签名', icon: '✎' },
  { id: 'terminal', name: '本地终端', desc: '侧边栏 shell 命令执行', icon: '▌_' },
  { id: 'preview', name: '文件预览', desc: '文本 / CSV / TSV / Excel', icon: '▦' },
];

const EXPERIMENTAL = [
  { id: 'browser', name: '浏览器自动化', desc: 'Playwright / Chrome DevTools MCP / Stagestage 后端（实验性，运行时未接入）' },
  { id: 'computer_use', name: 'Computer Use', desc: '驱动本机桌面并获取截图（实验性，未接入）' },
  { id: 'tee', name: 'TEE 可信执行', desc: '硬件可信执行环境（未实现，含证明系统）' },
  { id: 'erasure', name: '纠删码', desc: 'Reed-Solomon 分片与恢复（核心已实现，应用接入实验性）' },
  { id: 'ssh', name: 'SSH 远程工作区', desc: '通过 SSH 操作远端工作区（实验性，未接入）' },
];

export const pluginsView = {
  id: 'plugins',
  name: '插件',
  icon: '⊞',
  section: '系统',
  async render(container, app) {
    clear(container);
    container.appendChild(renderPlugins(app));
  },
};

function renderPlugins(app) {
  const root = el('<div class="fade-in"></div>');
  const head = el(
    '<div class="view-head"><div><div class="view-title">插件与扩展</div>' +
      '<div class="view-sub">内置能力与实验性扩展</div></div></div>'
  );
  const body = el('<div id="plugins-body"></div>');
  body.innerHTML = '<div class="loading">加载…</div>';
  root.append(head, body);

  tauri
    .getConfig()
    .then((c) => {
      cfg = c;
      draw(app, body);
    })
    .catch((e) => {
      body.innerHTML = '<div class="error-box">' + escapeHtml(String(e)) + '</div>';
    });

  return root;
}

function draw(app, body) {
  const builtinHtml =
    '<div class="card"><div class="card-title">内置能力</div>' +
    '<div class="card-grid">' +
    BUILTIN.map((p) =>
      '<div class="plugin-card"><div class="plugin-icon">' + p.icon + '</div>' +
      '<div class="plugin-name">' + escapeHtml(p.name) + '</div>' +
      '<div class="plugin-desc">' + escapeHtml(p.desc) + '</div>' +
      '<span class="badge ok">已启用</span></div>'
    ).join('') +
    '</div></div>';

  const flags = cfg.feature_flags || {};
  const experimentalHtml =
    '<div class="card"><div class="card-title">实验性扩展</div>' +
    '<div class="info-note">实验扩展当前仅提供开关与接入点；动态插件运行时（安装、配置、运行时卸载）' +
    '尚未实现，开启后不代表该能力已可用。</div>' +
    '<div class="card-grid">' +
    EXPERIMENTAL.map((p) =>
      '<div class="plugin-card"><div class="plugin-name">' + escapeHtml(p.name) + '</div>' +
      '<div class="plugin-desc">' + escapeHtml(p.desc) + '</div>' +
      '<label class="switch"><input type="checkbox" data-flag="' + p.id + '"' + (flags[p.id] ? ' checked' : '') + '>' +
      '<span>' + (flags[p.id] ? '已开启' : '开启') + '</span></label></div>'
    ).join('') +
    '</div></div>';

  body.innerHTML = builtinHtml + experimentalHtml;

  body.querySelectorAll('input[data-flag]').forEach((cb) => {
    cb.onchange = async () => {
      cfg.feature_flags[cb.dataset.flag] = cb.checked;
      cb.nextElementSibling.textContent = cb.checked ? '已开启' : '开启';
      try {
        await tauri.saveConfig(cfg);
        app.toast((cb.checked ? '已开启：' : '已关闭：') + cb.dataset.flag);
      } catch (e) {
        app.toast('保存失败：' + e, 'error');
      }
    };
  });
}
