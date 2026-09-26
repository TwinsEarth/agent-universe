// test/regression.js
// Agent Universe 历史 Bug 回归测试套件
// 把 v1.0.0 ~ v2.5.x 开发过程中踩过、修过的每个 bug / 遗漏固化为断言，
// 每次发版必跑（test/run-all.sh / run-all.ps1 与 CI 都会执行）。
// 修任何新 bug，必须先在此追加一条 REG-xxx，再提交修复。

const fs = require('fs');
const path = require('path');
const { AgentMarket, AgentCard, MIN_STAKE } = require('../js');

const ROOT = path.join(__dirname, '..');
const read = (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8');
const esc = (s) => String(s).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

let pass = 0;
let fail = 0;
const failedIds = [];

function reg(id, desc, fn) {
  try {
    fn();
    pass++;
    console.log(`  PASS ${id}  ${desc}`);
  } catch (e) {
    fail++;
    failedIds.push(id);
    console.log(`  FAIL ${id}  ${desc} -> ${e.message}`);
  }
}
function assert(cond, msg) {
  if (!cond) throw new Error(msg || '断言失败');
}
function throws(fn, mustInclude, label) {
  let threw = false;
  let err;
  try { fn(); } catch (e) { threw = true; err = e; }
  assert(threw, `${label}: 预期抛错但没有`);
  if (mustInclude) {
    assert(String(err.message).includes(mustInclude),
      `${label}: 错误信息应含 "${mustInclude}"，实际 "${err.message}"`);
  }
}

// 版本号：npm 线 X.Y.Z 与 Rust 线 0.2.(Y*10+Z)
const V = require('../js').version;
const parts = V.split('.').map(Number);
const RUST = `0.2.${parts[1] * 10 + parts[2]}`;

console.log(`\nAgent Universe v${V} 历史 Bug 回归（Rust 线 ${RUST}）\n`);

// ───────────────────────── A. 版本声明点一致性 ─────────────────────────
console.log('A. 版本声明点一致性');

reg('REG-001', '四个 npm package.json 版本都等于 SDK 版本', () => {
  const root = require('../package.json').version;
  const js = require('../js/package.json').version;
  const client = require('../client/package.json').version;
  const desktop = require('../desktop/package.json').version;
  assert(root === V && js === V && client === V && desktop === V,
    `版本不一致 root=${root} js=${js} client=${client} desktop=${desktop}, SDK=${V}`);
});

reg('REG-002', 'gsn-core Cargo.toml 版本等于 Rust 线', () => {
  const c = read('gsn-core/Cargo.toml');
  const m = c.match(/name = "gsn-core"\s*\nversion = "([^"]+)"/);
  assert(m && m[1] === RUST, `gsn-core Cargo 版本 ${m ? m[1] : '?'} != ${RUST}`);
});

reg('REG-003', 'Cargo.lock 本包版本与 manifest 一致', () => {
  const glock = read('gsn-core/Cargo.lock');
  assert(new RegExp(`name = "gsn-core"\\s*\\nversion = "${esc(RUST)}"`).test(glock),
    'gsn-core Cargo.lock 版本与 manifest 不一致');
  const clock = read('client/src-tauri/Cargo.lock');
  assert(new RegExp(`name = "au-client-universal"\\s*\\nversion = "${esc(V)}"`).test(clock),
    'client Cargo.lock 版本与 manifest 不一致');
});

reg('REG-004', 'pyproject / js 运行时 / ci.yml 版本一致', () => {
  const py = read('aip-sdk-py/pyproject.toml');
  assert(py.includes(`version = "${V}"`), 'pyproject 版本不一致');
  assert(require('../js').version === V, 'js 运行时 version 不一致');
  const ci = read('.github/workflows/ci.yml');
  assert(ci.includes(`au.version!=='${V}'`), 'ci.yml 版本校验串不一致');
});

// ───────────────────────── B. 历史编译 & CI bug 守卫 ─────────────────────────
console.log('B. 历史编译 & CI bug 守卫（静态检查）');

reg('REG-010', 'peer.rs subscribe() 不再自引用 IdentTopic::new(t)', () => {
  const c = read('gsn-core/src/net/peer.rs');
  assert(!/IdentTopic::new\(t\)/.test(c), '仍含自引用 IdentTopic::new(t)（E0425）');
  assert(/IdentTopic::new\(topic\)/.test(c), 'subscribe 应使用参数名 topic');
});

reg('REG-011', 'release.yml 顶层声明 contents: write（防 403）', () => {
  const c = read('.github/workflows/release.yml');
  assert(/permissions:\s*\n\s*contents:\s*write/.test(c),
    'release.yml 缺 permissions contents: write，上传产物会 403');
});

reg('REG-012', 'release.yml glob 容错（nullglob，防 no matches found）', () => {
  const c = read('.github/workflows/release.yml');
  assert(/nullglob/.test(c), 'release.yml 缺 shopt -s nullglob');
});

reg('REG-013', 'publish.yml 含 npm publish 闭环（防 npm 漏发）', () => {
  const c = read('.github/workflows/publish.yml');
  assert(/npm publish/.test(c), 'publish.yml 缺 npm publish 步骤');
});

// ───────────────────────── C. 市场重复防护与非法输入 ─────────────────────────
function makeAgent(m, did) {
  m.deposit(did, 500);
  const card = new AgentCard(did, did);
  card.withSkill('math');
  m.registerAgent(card, MIN_STAKE);
  return card;
}
function fullTask(m, taskId, opts) {
  opts = opts || {};
  const requester = opts.requester || 'requester';
  const agent = opts.agent || 'agent1';
  m.deposit(requester, 500);
  if (!m.agents.has(agent)) makeAgent(m, agent);
  m.publishTask(taskId, 'goal', opts.budget || 200, requester);
  m.submitBid(taskId, agent, opts.price || 100);
  m.matchTask(taskId);
  m.completeTask(taskId, true);
  return m.settle(taskId);
}

console.log('C. 市场重复防护与非法输入');

reg('REG-020', '重复注册 Agent 被拒绝', () => {
  const m = new AgentMarket();
  makeAgent(m, 'a1');
  throws(() => makeAgent(m, 'a1'), '已存在', '重复注册');
});

reg('REG-021', '重复结算返回 already_paid 且 paid=0', () => {
  const m = new AgentMarket();
  fullTask(m, 't1');
  const r = m.settle('t1');
  assert(r.reason === 'already_paid' && r.paid === 0,
    `重复结算应 {paid:0,reason:'already_paid'}，实际 ${JSON.stringify(r)}`);
});

reg('REG-022', '负金额 / 空 goal / 非正预算抛错', () => {
  const m = new AgentMarket();
  throws(() => m.deposit('x', -1), '负', '负金额');
  m.deposit('r', 500);
  throws(() => m.publishTask('t', '', 100, 'r'), 'goal', '空 goal');
  throws(() => m.publishTask('t', 'g', 0, 'r'), '正', '零预算');
  throws(() => m.publishTask('t', 'g', -5, 'r'), '正', '负预算');
});

reg('REG-023', '未注册投标 / 无投标匹配抛错', () => {
  const m = new AgentMarket();
  m.deposit('r', 500);
  m.publishTask('t', 'g', 100, 'r');
  throws(() => m.submitBid('t', 'nobody', 50), '未注册', '未注册投标');
  throws(() => m.matchTask('t'), '无投标', '无投标匹配');
});

// ───────────────────────── D. 资金守恒 ─────────────────────────
console.log('D. 资金守恒');

reg('REG-030', '无罚没：全链路系统总余额等于总充值', () => {
  const m = new AgentMarket();
  // agent1 deposit 500 + requester deposit 500 = 1000
  fullTask(m, 't1', { price: 100, budget: 200 });
  const cc = m.conservationCheck(1000);
  assert(cc.conserved,
    `守恒失败: 余额和 ${cc.balanceSum}, 期望 ${cc.expected}`);
});

reg('REG-031', '罚没后：系统总余额等于总充值减罚没', () => {
  const m = new AgentMarket();
  makeAgent(m, 'a1');          // a1 deposit 500
  m.deposit('r', 500);          // r deposit 500，合计 1000
  const s = m.slash('a1', 60, 'evil');
  const cc = m.conservationCheck(1000);
  assert(s.slashed === 60, `罚没金额应为 60，实际 ${s.slashed}`);
  assert(cc.conserved,
    `罚没后守恒失败: 余额和 ${cc.balanceSum}, 期望 ${cc.expected}(=1000-60)`);
});

// ───────────────────────── 汇总 ─────────────────────────
console.log(`\n回归结果: ${pass} 通过, ${fail} 失败`);
if (fail > 0) {
  console.log('失败项: ' + failedIds.join(', '));
  process.exit(1);
}
console.log('OK 历史 bug 无复发');
