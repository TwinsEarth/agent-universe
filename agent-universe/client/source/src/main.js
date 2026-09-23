import { AgentMarket, MIN_STAKE } from '@twinsearth/agent-universe/lib/market.js';

const logEl = document.getElementById('log');
const btn = document.getElementById('btn-run');
const platformEl = document.getElementById('platform');

// 检测平台
const ua = navigator.userAgent;
let platform = 'Desktop';
if (/Android/i.test(ua)) platform = 'Android';
else if (/iPhone|iPad|iOS/i.test(ua)) platform = 'iOS';
else if (/Macintosh/i.test(ua)) platform = 'macOS';
else if (/Windows/i.test(ua)) platform = 'Windows';
else if (/Linux/i.test(ua)) platform = 'Linux';
platformEl.textContent = platform;

function log(msg, cls = '') {
  const line = cls ? `<span class="${cls}">${msg}</span>` : msg;
  logEl.innerHTML += line + '\n';
  logEl.scrollTop = logEl.scrollHeight;
}

function sep(title) {
  log('');
  log(`── ${title} ${'─'.repeat(Math.max(0, 36 - title.length))}`, 'warn');
}

async function runDemo() {
  btn.disabled = true;
  logEl.innerHTML = '';

  try {
    sep('初始化市场');
    const market = new AgentMarket();
    log(`AgentMarket 已创建（平台: ${platform}）`, 'ok');

    sep('① 充值');
    market.deposit('requester-001', 500);
    market.deposit('agent-001', 200);
    market.deposit('agent-002', 300);
    log(`余额: requester=${market.balance('requester-001')}, agent1=${market.balance('agent-001')}, agent2=${market.balance('agent-002')}`, 'ok');

    sep('② 质押注册');
    market.registerAgent({ did: 'did:au:agent-001', name: 'Translator', skills: ['translation'] }, MIN_STAKE);
    market.registerAgent({ did: 'did:au:agent-002', name: 'Summarizer', skills: ['summarization'] }, MIN_STAKE);
    log('2 个 Agent 已注册', 'ok');

    sep('③ 发布任务');
    market.publishTask('task-001', '翻译这段英文', 50, 'requester-001');
    log('任务已发布, 预算=50', 'ok');

    sep('④ 投标');
    market.submitBid('task-001', 'did:au:agent-001', 40);
    market.submitBid('task-001', 'did:au:agent-002', 45);
    log('2 个投标已提交', 'ok');

    sep('⑤ 匹配');
    const winner = market.matchTask('task-001');
    log(`匹配: ${winner}`, 'ok');

    sep('⑥ 完成');
    market.completeTask('task-001', true);
    log('验收通过', 'ok');

    sep('⑦ 结算');
    const s = market.settle('task-001');
    log(`结算: ${JSON.stringify(s)}`, 'ok');

    sep('⑧ 守恒');
    const c = market.conservationCheck(1000);
    log(`守恒: ${c.conserved ? '✓' : '✗'} (paid=${c.totalPaid}, slashed=${c.totalSlashed})`, c.conserved ? 'ok' : 'err');

    sep('完成');
    log('市场端到端流程通过 ✓', 'ok');
  } catch (e) {
    log(`错误: ${e.message}`, 'err');
  } finally {
    btn.disabled = false;
  }
}

btn.addEventListener('click', runDemo);
