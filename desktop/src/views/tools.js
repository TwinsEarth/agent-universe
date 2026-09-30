// Agent Universe v2.9.0 — 代码执行（沙箱）
// 选择语言 → 编写代码 → 运行（懒创建沙箱并绑定 owner）→ 查看 stdout/stderr/耗时

import { api, el, clear, escapeHtml } from '../api.js';

let sandboxId = null;
let language = 'python';

const SAMPLES = {
  python:
    'print("hello from sandbox")\n' +
    'total = sum(range(11))\n' +
    'print("sum(0..10) =", total)',
  javascript:
    'const total = Array.from({length: 11}, (_, i) => i).reduce((a, b) => a + b, 0);\n' +
    'console.log("hello from sandbox");\n' +
    'console.log("sum(0..10) =", total);',
};

export const toolsView = {
  id: 'tools',
  name: '代码执行',
  icon: '▷',
  section: '工具',
  async render(container, app) {
    clear(container);
    container.appendChild(renderTools(app));
  },
};

function renderTools(app) {
  const root = el('<div class="fade-in"></div>');
  const head = el(
    '<div class="view-head"><div><div class="view-title">代码执行</div>' +
      '<div class="view-sub" id="sb-info">在受控沙箱中运行 Python / JavaScript</div></div>' +
      '<div class="flex-center gap8">' +
      '<button class="btn" id="btn-reset">重置沙箱</button></div></div>'
  );
  const body = el(
    '<div class="grid-2">' +
      '<div class="card">' +
      '<div class="card-title flex-center" style="justify-content:space-between">' +
      '<div class="seg" id="lang-seg">' +
      '<button data-lang="python">Python</button>' +
      '<button data-lang="javascript">JavaScript</button></div>' +
      '<button class="btn btn-primary" id="btn-run">▶ 运行</button></div>' +
      '<textarea class="code-editor" id="code-editor" spellcheck="false"></textarea>' +
      '<div class="sample-row" id="sample-row"></div>' +
      '</div>' +
      '<div class="card"><div class="card-title">输出</div>' +
      '<div id="run-output" class="run-output"><div class="empty">点击「运行」执行代码</div></div>' +
      '</div></div>'
  );
  root.append(head, body);

  const editor = body.querySelector('#code-editor');
  editor.value = SAMPLES[language];

  const setLang = (l) => {
    language = l;
    if (editor.value === SAMPLES.python || editor.value === SAMPLES.javascript || !editor.value.trim()) {
      editor.value = SAMPLES[l];
    }
    body.querySelectorAll('#lang-seg button').forEach((b) => {
      b.classList.toggle('active', b.dataset.lang === l);
    });
  };
  body.querySelectorAll('#lang-seg button').forEach((b) => {
    b.onclick = () => setLang(b.dataset.lang);
  });
  setLang(language);

  const sampleRow = body.querySelector('#sample-row');
  sampleRow.innerHTML =
    '<button class="btn sm" data-sample="hello">示例：Hello</button>' +
    '<button class="btn sm" data-sample="compute">示例：计算</button>';
  sampleRow.querySelectorAll('[data-sample]').forEach((b) => {
    b.onclick = () => {
      if (b.dataset.sample === 'hello') {
        editor.value = language === 'python'
          ? 'print("hello")'
          : 'console.log("hello");';
      } else {
        editor.value = SAMPLES[language];
      }
    };
  });

  body.querySelector('#btn-run').onclick = () => run(app, body, editor);
  head.querySelector('#btn-reset').onclick = async () => {
    if (sandboxId) {
      try { await api.sandboxLifecycle(sandboxId, 'destroy'); } catch { /* ignore */ }
    }
    sandboxId = null;
    head.querySelector('#sb-info').textContent = '沙箱已重置（下次运行时创建）';
    app.toast('沙箱已重置');
  };

  return root;
}

async function run(app, body, editor) {
  const code = editor.value;
  const out = body.querySelector('#run-output');
  out.innerHTML = '<div class="loading">执行中…</div>';

  try {
    if (!sandboxId) {
      const created = await api.createSandbox({});
      sandboxId = created.id;
      app.toast('已创建沙箱：' + sandboxId);
    }
    const r = await api.sandboxExec(sandboxId, language, code);
    renderResult(out, r);
  } catch (e) {
    // 若沙箱已失效（id 不存在），清掉 id 并提示
    if (e && (e.status === 404 || e.status === 401)) {
      sandboxId = null;
    }
    out.innerHTML =
      '<div class="error-box"><div>执行失败：' + escapeHtml(String(e.message || e)) + '</div>' +
      '<div class="muted small">提示：在部分平台进程后端默认不可用（NullExecutor），' +
      '需要配置可执行的沙箱后端。</div></div>';
  }
}

function renderResult(out, r) {
  const ok = r.exit_code === 0;
  out.innerHTML =
    '<div class="result-status ' + (ok ? 'ok' : 'err') + '">' +
    (ok ? '退出码 0' : '退出码 ' + r.exit_code) +
    (r.wall_ms ? ' · ' + r.wall_ms + ' ms' : '') + '</div>' +
    (r.stdout ? '<pre class="code-result out-ok"></pre>' : '') +
    (r.stderr ? '<pre class="code-result out-err"></pre>' : '') +
    (!r.stdout && !r.stderr ? '<div class="muted">（无输出）</div>' : '');
  const outOk = out.querySelector('.out-ok');
  const outErr = out.querySelector('.out-err');
  if (outOk) outOk.textContent = r.stdout;
  if (outErr) outErr.textContent = r.stderr;
}
