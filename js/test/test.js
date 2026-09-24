// test/test.js
// Agent Universe JS SDK 测试（8 项）

const assert = require('node:assert');
const {
  version,
  AgentUniverse,
  Keypair,
  AgentCard,
  Task,
  TaskStatus,
  MemoryDHT,
  ShardedIndex,
  shardOf,
  AgentMarket,
  MIN_STAKE,
} = require('../index');

let passed = 0;
function test(name, fn) {
  try {
    fn();
    passed++;
    console.log(`  ✓ ${name}`);
  } catch (e) {
    console.error(`  ✗ ${name}`);
    console.error(`    ${e.message}`);
    process.exitCode = 1;
  }
}

console.log('Agent Universe JS SDK 测试\n');

// 1. 版本号
test('版本号为 2.3.4', () => {
  assert.strictEqual(version, '2.3.4');
});

// 2. 密钥对 + DID + 签名验证
test('Keypair 生成 DID 且签名可验证', () => {
  const kp = Keypair.generate();
  assert.ok(kp.did.startsWith('did:au:'), 'DID 前缀');
  assert.strictEqual(kp.did.length, 'did:au:'.length + 32);
  const msg = 'hello agent universe';
  const sig = kp.sign(msg);
  assert.ok(kp.verify(msg, sig), '签名验证通过');
  assert.ok(!kp.verify('tampered', sig), '篡改消息验证失败');
});

// 3. AgentCard
test('AgentCard 能力与技能', () => {
  const card = AgentCard.new({ did: 'did:au:abc', name: 'writer' })
    .withCapability('writing')
    .withSkill('translation');
  assert.ok(card.hasCapability('writing'));
  assert.deepStrictEqual(card.toJSON().skills, ['translation']);
});

// 4. Task 状态机
test('Task 状态机合法流转且拒绝非法转移', () => {
  const t = new Task('t1', 'write doc');
  t.transition(TaskStatus.OPEN);
  t.assign('did:au:worker');
  t.start();
  t.complete();
  t.verify(true);
  t.settle();
  assert.strictEqual(t.status, TaskStatus.SETTLED);
  assert.ok(t.isTerminal);

  const t2 = new Task('t2', 'bad');
  assert.throws(() => t2.start(), /非法状态转移/);
});

// 5. MemoryDHT
test('MemoryDHT 存取与前缀查找', () => {
  const dht = new MemoryDHT();
  dht.put('/aip/agent/1', { name: 'a' });
  dht.put('/aip/agent/2', { name: 'b' });
  assert.strictEqual(dht.size, 2);
  assert.strictEqual(dht.get('/aip/agent/1').name, 'a');
  assert.strictEqual(dht.findByPrefix('/aip/agent').length, 2);
});

// 6. ShardedIndex 分片
test('ShardedIndex 分片落盘且分片号合法', () => {
  const idx = new ShardedIndex(16);
  for (let i = 0; i < 100; i++) {
    const s = idx.put(`key-${i}`, i);
    assert.ok(s >= 0 && s < 16);
  }
  assert.strictEqual(idx.size, 100);
  assert.strictEqual(shardOf('key-0', 16), shardOf('key-0', 16));
  // 分布：100 key 到 16 片，不应有大片为空（概率性，至少覆盖 8 片）
  const nonEmpty = idx.distribution().filter((n) => n > 0).length;
  assert.ok(nonEmpty >= 8, `非空分片 ${nonEmpty} >= 8`);
});

// 7. AgentMarket 端到端 + 守恒
test('AgentMarket 端到端：注册→发布→投标→匹配→完成→结算→守恒', () => {
  const market = new AgentMarket();
  const owner = 'did:au:owner';
  const requester = 'did:au:requester';

  // 充值（owner 质押 100，requester 预算 50）
  market.deposit(owner, 100);
  market.deposit(requester, 50);
  const totalDeposits = 150;

  const card = AgentCard.new({ did: owner, name: 'worker' })
    .withSkill('writing');
  market.registerAgent(card, MIN_STAKE);

  market.publishTask('task-1', 'write an article', 50, requester);
  market.submitBid('task-1', owner, 40);
  const winner = market.matchTask('task-1');
  assert.strictEqual(winner.agentId, owner);

  market.completeTask('task-1', true);
  const result = market.settle('task-1');
  assert.strictEqual(result.paid, 40);

  // 重复结算短路
  const again = market.settle('task-1');
  assert.strictEqual(again.reason, 'already_paid');

  // 守恒：无罚没，总余额 = 总充值
  const check = market.conservationCheck(totalDeposits);
  assert.ok(check.conserved, `守恒: ${JSON.stringify(check)}`);
});

// 8. 罚没守恒
test('罚没后守恒：总余额 = 总充值 − 罚没', () => {
  const market = new AgentMarket();
  const owner = 'did:au:bad';
  market.deposit(owner, 200);
  const card = AgentCard.new({ did: owner, name: 'bad' });
  market.registerAgent(card, MIN_STAKE); // 质押 100

  const slashResult = market.slash(owner, 60, 'fraud');
  assert.strictEqual(slashResult.slashed, 60);

  const check = market.conservationCheck(200);
  assert.ok(check.conserved, `罚没守恒: ${JSON.stringify(check)}`);
  assert.strictEqual(check.balanceSum, 140);
});

console.log(`\n${passed}/8 测试通过`);
if (process.exitCode === 1) {
  console.error('存在失败测试');
} else {
  console.log('全部通过 ✅');
}
