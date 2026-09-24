// Agent Universe v2.3.4 — 桌面客户端前端
// 直接引入 AgentMarket（不触发 node:crypto 的 Keypair），用固定 DID 演示市场结算链路

import { AgentMarket, MIN_STAKE } from '@twinsearth/agent-universe/lib/market.js';

const logEl = document.getElementById('log');
const btn = document.getElementById('btn-run');
const versionEl = document.getElementById('sdk-version');
const didEl = document.getElementById('did');

// 固定 DID（WKWebView 无 node:crypto，市场结算链路本身不依赖 Ed25519 签名）
const DEMO_DID = 'did:au:desktop-demo-fixed-did-0001';

didEl.textContent = DEMO_DID.slice(0, 24) + '…';

function log(msg, cls = '') {
  const line = cls ? `<span class="${cls}">${msg}</span>` : msg;
  logEl.innerHTML += line + '\n';
  logEl.scrollTop = logEl.scrollHeight;
}

function sep(title) {
  log('');
  log(`── ${title} ${'─'.repeat(Math.max(0, 40 - title.length))}`, 'warn');
}

async function runDemo() {
  btn.disabled = true;
  logEl.innerHTML = '';

  try {
    sep('初始化市场');
    const market = new AgentMarket();
    log('AgentMarket 实例已创建', 'ok');

    // 1. 充值
    sep('① 充值');
    market.deposit('requester-001', 500);
    market.deposit('agent-001', 200);
    market.deposit('agent-002', 300);
    log(`requester-001 余额: ${market.balance('requester-001')}`);
    log(`agent-001 余额: ${market.balance('agent-001')}`);
    log(`agent-002 余额: ${market.balance('agent-002')}`, 'ok');

    // 2. 质押注册
    sep('② 质押注册 Agent');
    const card1 = { did: 'did:au:agent-001', name: 'Translator-Bot', skills: ['translation'] };
    const card2 = { did: 'did:au:agent-002', name: 'Summarizer-Bot', skills: ['summarization'] };
    market.registerAgent(card1, MIN_STAKE);
    market.registerAgent(card2, MIN_STAKE);
    log(`已注册: ${card1.name} (stake=${MIN_STAKE})`, 'ok');
    log(`已注册: ${card2.name} (stake=${MIN_STAKE})`, 'ok');

    // 3. 发布任务
    sep('③ 发布任务');
    market.publishTask('task-001', '翻译这段英文', 50, 'requester-001');
    log('任务 task-001 已发布, 预算=50', 'ok');

    // 4. 投标
    sep('④ 投标');
    market.submitBid('task-001', 'did:au:agent-001', 40);
    market.submitBid('task-001', 'did:au:agent-002', 45);
    log('agent-001 投标 40', 'ok');
    log('agent-002 投标 45', 'ok');

    // 5. 匹配
    sep('⑤ 匹配');
    const winner = market.matchTask('task-001');
    log(`匹配结果: ${winner}`, 'ok');

    // 6. 完成
    sep('⑥ 完成任务');
    market.completeTask('task-001', true);
    log('任务完成, 验收通过', 'ok');

    // 7. 结算
    sep('⑦ 结算');
    const settlement = market.settle('task-001');
    log(`结算: ${JSON.stringify(settlement)}`, 'ok');

    // 8. 重复防护
    sep('⑧ 重复防护');
    try {
      market.settle('task-001');
      log('⚠️ 重复结算未被拦截!', 'err');
    } catch (e) {
      log(`重复结算已拦截: ${e.message}`, 'ok');
    }

    // 9. 守恒校验
    sep('⑨ 守恒校验');
    const totalDeposits = 500 + 200 + 300;
    const conserved = market.conservationCheck(totalDeposits);
    log(`总充值: ${totalDeposits}`, '');
    log(`守恒: ${conserved.conserved ? '✓ 通过' : '✗ 失败'}`, conserved.conserved ? 'ok' : 'err');
    log(`已付: ${conserved.totalPaid}, 罚没: ${conserved.totalSlashed}`, '');

    // 10. 排行榜
    sep('⑩ Agent 排行榜');
    const board = market.leaderboard(5);
    board.forEach((a, i) => {
      log(`  ${i + 1}. ${a.name} — 信誉 ${a.reputation.overall().toFixed(3)}`);
    });

    sep('演示完成');
    log('市场端到端流程全部通过 ✓', 'ok');

  } catch (e) {
    log(`\n错误: ${e.message}`, 'err');
    console.error(e);
  } finally {
    btn.disabled = false;
  }
}

btn.addEventListener('click', runDemo);
