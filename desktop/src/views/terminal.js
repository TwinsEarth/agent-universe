// Agent Universe v2.9.0 — 侧边栏终端
// 命令 → 输出（本地 shell，经 Tauri run_shell），维护工作目录与命令历史

import { tauri, el, clear, escapeHtml } from '../api.js';

let cwd = '';
let isWin = false;
let history = [];
let histIdx = -1;
let busy = false;

export const terminalView = {
  id: 'terminal',
  name: '终端',
  icon: '▌_',
  section: '工具',
  async render(container, app) {
    clear(container);
    container.appendChild(renderTerminal(app));
  },
};

let cachedRoot = null;

function renderTerminal(app) {
  if (cachedRoot) return cachedRoot;
  const root = el('<div class="fade-in terminal-page"></div>');
  const head = el(
    '<div class="view-head"><div><div class="view-title">终端</div>' +
      '<div class="view-sub mono" id="term-cwd">本地 shell…</div></div>' +
      '<div class="flex-center gap8">' +
      '<button class="btn" id="btn-clear">清空</button></div></div>'
  );
  const term = el(
    '<div class="terminal" id="terminal"><div class="term-out" id="term-out"></div>' +
      '<div class="term-line"><span class="term-prompt" id="term-prompt">$</span>' +
      '<input class="term-input" id="term-input" spellcheck="false" autocomplete="off"></div></div>'
  );
  root.append(head, term);

  const out = term.querySelector('#term-out');
  const input = term.querySelector('#term-input');

  head.querySelector('#btn-clear').onclick = () => {
    out.innerHTML = '';
    input.focus();
  };

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      const cmd = input.value;
      input.value = '';
      runCommand(app, cmd, out);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (history.length) {
        histIdx = histIdx < 0 ? history.length - 1 : Math.max(0, histIdx - 1);
        input.value = history[histIdx] || '';
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (histIdx >= 0) {
        histIdx++;
        if (histIdx >= history.length) { histIdx = -1; input.value = ''; }
        else input.value = history[histIdx] || '';
      }
    }
  });

  out.addEventListener('click', () => input.focus());

  if (!cwd) {
    initShell(head, term);
  } else {
    updatePrompt(head, term);
  }

  setTimeout(() => input.focus(), 50);
  cachedRoot = root;
  return root;
}

async function initShell(head, term) {
  // 探测平台与工作目录
  let r;
  try {
    r = await tauri.runShell('pwd');
    if (!r.stdout || !r.stdout.trim() || (r.code && r.code !== 0)) throw new Error('pwd failed');
    isWin = false;
  } catch {
    isWin = true;
    r = await tauri.runShell('cd');
  }
  cwd = (r.stdout || '').trim() || cwd;
  updatePrompt(head, term);
}

function updatePrompt(head, term) {
  head.querySelector('#term-cwd').textContent = cwd || '本地 shell';
  const prompt = term.querySelector('#term-prompt');
  if (prompt) prompt.textContent = isWin ? '>' : '$';
}

function appendCommand(out, cmd) {
  const node = el(
    '<div class="term-entry"><div class="term-cmd"><span class="term-prompt">' +
      (isWin ? '>' : '$') + '</span> ' + escapeHtml(cmd) + '</div></div>'
  );
  out.appendChild(node);
  return node;
}

function appendOutput(node, text, cls) {
  if (!text) return;
  const pre = el('<pre class="term-result ' + cls + '"></pre>');
  pre.textContent = text;
  node.appendChild(pre);
}

async function runCommand(app, raw, out) {
  const cmd = raw.trim();
  if (!cmd) return;
  if (busy) return;
  busy = true;

  const entry = appendCommand(out, cmd);
  history.push(cmd);
  histIdx = -1;

  // cd 特殊处理（维护前端 cwd）
  const cdMatch = cmd.match(/^cd\s+(.+)$/);
  let actualCmd = cmd;
  let useCwd = cwd;
  if (cdMatch) {
    const target = cdMatch[1].trim().replace(/^["']|["']$/g, '');
    const next = resolveDir(target);
    // 用真实 shell 验证目录可进入
    const probe = isWin ? 'cd /d "' + next + '" && cd' : 'cd "' + next + '" && pwd';
    try {
      const r = await tauri.runShell(probe, cwd);
      if (r.code === 0 || r.code === null) {
        cwd = (r.stdout || '').trim().split(/\r?\n/).pop() || next;
        appendOutput(entry, '', 'ok');
      } else {
        appendOutput(entry, r.stderr || 'cd 失败', 'err');
      }
    } catch (e) {
      appendOutput(entry, String(e), 'err');
    }
    busy = false;
    scrollOut(out);
    return;
  }

  try {
    const r = await tauri.runShell(actualCmd, useCwd);
    appendOutput(entry, r.stdout, r.code === 0 || r.code === null ? 'ok' : 'err');
    appendOutput(entry, r.stderr, 'err');
  } catch (e) {
    appendOutput(entry, String(e), 'err');
  } finally {
    busy = false;
    scrollOut(out);
  }
}

function resolveDir(target) {
  if (target === '~') return homeHint();
  if (/^([/\\])|([A-Za-z]:[\\/])/.test(target)) return target; // 绝对路径
  // 相对路径
  const sep = isWin ? '\\' : '/';
  const base = cwd.endsWith(sep) ? cwd.slice(0, -1) : cwd;
  return base + sep + target;
}

function homeHint() {
  return cwd; // 简化：~ 退回当前目录（一次性 shell 无 HOME 解析）
}

function scrollOut(out) {
  out.scrollTop = out.scrollHeight;
}
