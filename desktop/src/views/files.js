// Agent Universe v2.9.0 — 文件工作台
// 目录浏览 / 文本预览 / CSV·TSV 表格预览 / Excel（xlsx）多工作表预览

import { tauri, el, clear, escapeHtml } from '../api.js';
import * as XLSX from 'xlsx';

let state = { mode: 'browse', path: '', file: null };

const TEXT_EXT = ['txt', 'md', 'markdown', 'json', 'log', 'js', 'mjs', 'ts', 'py', 'rs', 'sh', 'toml', 'yaml', 'yml', 'csv', 'tsv', 'ini', 'cfg'];
const SHEET_EXT = ['xlsx', 'xls', 'xlsb', 'ods'];

export const filesView = {
  id: 'files',
  name: '文件',
  icon: '▦',
  section: '工具',
  async render(container, app) {
    clear(container);
    container.appendChild(renderFiles(app));
  },
};

function renderFiles(app) {
  const root = el('<div class="fade-in"></div>');
  if (state.mode === 'preview') {
    root.appendChild(previewView(app, state.file));
  } else {
    root.appendChild(browseView(app));
  }
  return root;
}

// ───────────────────────── 浏览 ─────────────────────────

function browseView(app) {
  const head = el(
    '<div class="view-head"><div><div class="view-title">文件</div>' +
      '<div class="view-sub" id="crumb"></div></div>' +
      '<div class="flex-center gap8"><button class="btn" id="btn-up">↑ 上级</button>' +
      '<button class="btn btn-primary" id="btn-pick">选择文件夹</button></div></div>'
  );
  const body = el('<div id="dir-body"></div>');
  const wrap = el('<div></div>');
  wrap.append(head, body);

  const load = (path) => {
    body.innerHTML = '<div class="loading">读取目录…</div>';
    tauri
      .listDir(path)
      .then((entries) => {
        state.path = path;
        renderCrumb(head, path, app);
        body.innerHTML = dirHtml(entries);
        body.querySelectorAll('[data-path]').forEach((node) => {
          node.onclick = () => {
            if (node.dataset.dir === '1') load(node.dataset.path);
            else {
              state = { mode: 'preview', path: state.path, file: node.dataset.path };
              app.rerender();
            }
          };
        });
      })
      .catch((e) => {
        body.innerHTML = '<div class="error-box">读取失败：' + escapeHtml(String(e)) + '</div>';
      });
  };

  head.querySelector('#btn-up').onclick = () => {
    const parts = state.path.split(/[\\/]/).filter(Boolean);
    parts.pop();
    const up = state.path.startsWith('/') ? '/' + parts.join('/') : parts.join('\\');
    load(up || state.path);
  };
  head.querySelector('#btn-pick').onclick = async () => {
    const p = await tauri.pickFolder();
    if (p) load(p);
  };

  if (!state.path) {
    tauri.appInfo().then((info) => load(info.workspace));
  } else {
    load(state.path);
  }

  return wrap;
}

function renderCrumb(head, path, app) {
  const sep = path.includes('\\') ? '\\' : '/';
  const parts = path.split(/[\\/]/).filter(Boolean);
  const crumb = head.querySelector('#crumb');
  let acc = path.startsWith('/') ? '/' : '';
  crumb.innerHTML = parts
    .map((p, i) => {
      if (i === 0 && !path.startsWith('/')) acc = p;
      else acc = acc + (acc.endsWith(sep) ? '' : sep) + p;
      const cur = acc;
      return '<span class="crumb" data-crumb="' + escapeHtml(cur) + '">' + escapeHtml(p) + '</span>' + (i < parts.length - 1 ? ' › ' : '');
    })
    .join('');
  crumb.querySelectorAll('.crumb').forEach((c) => {
    c.onclick = () => {
      state = { mode: 'browse', path: c.dataset.crumb };
      app.rerender();
    };
  });
}

function dirHtml(entries) {
  if (!entries.length) return '<div class="empty">空文件夹</div>';
  return (
    '<div class="card"><table><tbody>' +
    entries
      .map((e) => {
        const ico = e.is_dir ? '📁' : fileIcon(e.name);
        return (
          '<tr data-path="' + escapeHtml(e.path) + '" data-dir="' + (e.is_dir ? 1 : 0) + '">' +
          '<td class="file-ico">' + ico + '</td>' +
          '<td>' + escapeHtml(e.name) + '</td>' +
          '<td class="num muted">' + (e.is_dir ? '' : fmtSize(e.size)) + '</td>' +
          '</tr>'
        );
      })
      .join('') +
    '</tbody></table></div>'
  );
}

// ───────────────────────── 预览 ─────────────────────────

function previewView(app, file) {
  const name = file.split(/[\\/]/).pop();
  const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : '';

  const head = el(
    '<div class="view-head"><div class="flex-center gap8">' +
      '<button class="btn" id="btn-back">← 返回</button>' +
      '<div><div class="view-title">' + escapeHtml(name) + '</div>' +
      '<div class="view-sub mono">' + escapeHtml(file) + '</div></div></div></div>'
  );
  const body = el('<div id="preview-body"></div>');
  const wrap = el('<div></div>');
  wrap.append(head, body);
  head.querySelector('#btn-back').onclick = () => {
    state = { mode: 'browse', path: state.path };
    app.rerender();
  };

  if (ext === 'csv' || ext === 'tsv') {
    body.innerHTML = '<div class="loading">解析表格…</div>';
    tauri.readText(file).then((text) => {
      const sep = ext === 'tsv' ? '\t' : ',';
      body.innerHTML = delimitedTable(text, sep);
    }).catch((e) => body.innerHTML = '<div class="error-box">' + escapeHtml(String(e)) + '</div>');
  } else if (SHEET_EXT.includes(ext)) {
    body.innerHTML = '<div class="loading">解析 Excel…</div>';
    tauri.readBinaryBase64(file).then((b64) => {
      body.innerHTML = excelHtml(b64);
      wireExcelTabs(body);
    }).catch((e) => body.innerHTML = '<div class="error-box">' + escapeHtml(String(e)) + '</div>');
  } else if (TEXT_EXT.includes(ext)) {
    body.innerHTML = '<div class="loading">读取文件…</div>';
    tauri.readText(file).then((text) => {
      const pre = el('<pre class="text-preview"></pre>');
      pre.textContent = text;
      clear(body).appendChild(pre);
    }).catch((e) => body.innerHTML = '<div class="error-box">' + escapeHtml(String(e)) + '</div>');
  } else {
    body.innerHTML = '<div class="empty">该文件类型暂不支持预览（支持文本、CSV/TSV、Excel）</div>';
  }

  return wrap;
}

function delimitedTable(text, sep) {
  const rows = text.split(/\r?\n/).filter((l, i) => i === 0 || l.length).map((l) => splitCsvLine(l, sep));
  if (!rows.length) return '<div class="empty">空表格</div>';
  const header = rows[0];
  const bodyRows = rows.slice(1);
  return (
    '<div class="card sheet-card"><table class="sheet"><thead><tr>' +
    header.map((h) => '<th>' + escapeHtml(h) + '</th>').join('') +
    '</tr></thead><tbody>' +
    bodyRows.map((r) => '<tr>' + r.map((c) => '<td>' + escapeHtml(c) + '</td>').join('') + '</tr>').join('') +
    '</tbody></table></div>'
  );
}

function excelHtml(b64) {
  let wb;
  try {
    wb = XLSX.read(b64, { type: 'base64' });
  } catch (e) {
    return '<div class="error-box">Excel 解析失败：' + escapeHtml(String(e)) + '</div>';
  }
  const tabs = wb.SheetNames
    .map((n, i) => '<button class="sheet-tab' + (i === 0 ? ' active' : '') + '" data-sheet="' + i + '">' + escapeHtml(n) + '</button>')
    .join('');
  const panes = wb.SheetNames
    .map((n, i) => {
      const arr = XLSX.utils.sheet_to_json(wb.Sheets[n], { header: 1, raw: false, defval: '' });
      return (
        '<div class="sheet-pane" id="sheet-pane-' + i + '" style="' + (i === 0 ? '' : 'display:none') + '">' +
        jsonRowsToTable(arr) + '</div>'
      );
    })
    .join('');
  return '<div class="card sheet-card"><div class="sheet-tabs">' + tabs + '</div>' + panes + '</div>';
}

function wireExcelTabs(root) {
  root.querySelectorAll('.sheet-tab').forEach((b) => {
    b.onclick = () => {
      root.querySelectorAll('.sheet-tab').forEach((x) => x.classList.remove('active'));
      root.querySelectorAll('.sheet-pane').forEach((p) => (p.style.display = 'none'));
      b.classList.add('active');
      const pane = root.querySelector('#sheet-pane-' + b.dataset.sheet);
      if (pane) pane.style.display = '';
    };
  });
}

function jsonRowsToTable(arr) {
  if (!arr.length) return '<div class="empty">空工作表</div>';
  return (
    '<table class="sheet"><tbody>' +
    arr
      .map((r, i) => {
        const tag = i === 0 ? 'th' : 'td';
        return '<tr>' + r.map((c) => '<' + tag + '>' + escapeHtml(String(c)) + '</' + tag + '>').join('') + '</tr>';
      })
      .join('') +
    '</tbody></table>'
  );
}

// ───────────────────────── 工具 ─────────────────────────

function fileIcon(name) {
  const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : '';
  if (ext === 'csv' || ext === 'tsv') return '▦';
  if (SHEET_EXT.includes(ext)) return '▦';
  if (TEXT_EXT.includes(ext)) return '📄';
  return '📎';
}

function fmtSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB';
  return (bytes / 1024 / 1024 / 1024).toFixed(1) + ' GB';
}

function splitCsvLine(line, sep) {
  const out = [];
  let cur = '';
  let inQ = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (inQ) {
      if (c === '"') {
        if (line[i + 1] === '"') { cur += '"'; i++; }
        else inQ = false;
      } else cur += c;
    } else if (c === '"') {
      inQ = true;
    } else if (c === sep) {
      out.push(cur); cur = '';
    } else cur += c;
  }
  out.push(cur);
  return out;
}
