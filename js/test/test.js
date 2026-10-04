// test/test.js
// Agent Universe JS SDK 测试（21 项）

const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
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
  AipIdentity,
  buildManifest,
  buildEnvelope,
  proposalMessage,
  buildReceipt,
  verifyManifest,
  verifyMessage,
  verifyReceipt,
  verifyReceiptResult,
  McpHttpClient,
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
test('版本号为 3.9.2', () => {
  assert.strictEqual(version, '3.9.2');
});

// 2. 密钥对 + DID + 签名验证
test('Keypair 生成 DID 且签名可验证', () => {
  const kp = Keypair.generate();
  assert.ok(kp.did.startsWith('did:nau:'), 'DID 前缀');
  assert.strictEqual(kp.did.length, 'did:nau:'.length + 16);
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
  // v3.5.3（AU-39）：旧断言 shardOf('key-0',16) === shardOf('key-0',16) 恒真（自身比自身）。
  // 改为有意义的不变量：①同一 key 分片结果确定（两次调用一致）；②结果落在合法区间 [0,16)。
  const shard = shardOf('key-0', 16);
  assert.strictEqual(shardOf('key-0', 16), shard);
  assert.ok(Number.isInteger(shard) && shard >= 0 && shard < 16, `分片号合法: ${shard}`);
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

// 8b. 整数账本：拒绝浮点金额与 0/负出价（与 Rust Money(i64) 对齐）
test('整数账本：拒绝浮点金额与非正出价', () => {
  const market = new AgentMarket();
  const owner = 'did:au:int';
  assert.throws(() => market.deposit(owner, 10.5), /整数/);
  market.deposit(owner, 200);
  const card = AgentCard.new({ did: owner, name: 'int' });
  market.registerAgent(card, MIN_STAKE);
  market.publishTask('t-int', 'goal', 50, owner);
  assert.throws(() => market.submitBid('t-int', owner, 0), /正/);
  assert.throws(() => market.submitBid('t-int', owner, -5), /正/);
  assert.throws(() => market.submitBid('t-int', owner, 5.5), /整数/);
});

// 8c. 跨语言金额向量：逐账户余额命中 conformance/money-vectors.json（与 Rust 一致）
test('跨语言金额向量：逐账户余额命中 conformance 向量', () => {
  const vector = JSON.parse(
    fs.readFileSync(path.join(__dirname, '../../conformance/money-vectors.json'), 'utf8')
  );
  const market = new AgentMarket();
  market.deposit('agent-1', 100);
  const card = AgentCard.new({ did: 'agent-1', name: 'worker' }).withSkill('writing');
  market.registerAgent(card, MIN_STAKE);
  market.deposit('requester-1', 50);
  market.publishTask('task-1', 'translate', 50, 'requester-1');
  market.submitBid('task-1', 'agent-1', 10);
  assert.strictEqual(market.matchTask('task-1').agentId, 'agent-1');
  market.completeTask('task-1', true);
  const paid = market.settle('task-1');
  assert.strictEqual(paid.paid, vector.expected_report.total_paid);
  for (const [key, want] of Object.entries(vector.expected_balances)) {
    assert.strictEqual(market.balance(key), want, `账户 ${key} 跨语言不一致`);
  }
  const report = market.conservationCheck(vector.expected_report.total_deposits);
  assert.strictEqual(report.totalSlashed, vector.expected_report.total_slashed);
  assert.strictEqual(report.balanceSum, vector.expected_report.balance_sum);
  assert.strictEqual(report.conserved, vector.expected_report.conserved);
});

// 8d. 认证式 QA：固定委员集 + 签名票（v2.5.9，与 Rust 认证式 BFT 对齐）
test('认证式 QA：固定委员签名票通过、伪造与重放拒绝', () => {
  const market = new AgentMarket();
  const owner = 'did:au:qa-owner';
  const requester = 'did:au:qa-req';
  market.deposit(owner, 100);
  market.deposit(requester, 50);
  const card = AgentCard.new({ did: owner, name: 'worker' }).withSkill('writing');
  market.registerAgent(card, MIN_STAKE);
  market.publishTask('task-qa', 'goal', 50, requester);
  market.submitBid('task-qa', owner, 40);
  market.matchTask('task-qa');
  // 执行完成，停在 COMPLETED 等待认证式验收（不走本地 verify）
  const t = market.tasks.get('task-qa');
  t.start();
  t.complete();

  // 固定 4 委员（n=4, f=1, quorum=3）
  const now = Math.floor(Date.now() / 1000);
  const kps = [];
  const members = [];
  for (let i = 0; i < 4; i++) {
    const kp = Keypair.generate();
    kps.push(kp);
    members.push({ did: kp.did, public_key: kp.rawPublicKey().toString('hex') });
  }
  const makeVote = (kp, vote, nonce, round = 0) => {
    const sv = {
      task_id: 'task-qa', round, voter: kp.did, vote, nonce,
      issued_at: now - 60, expires_at: now + 3600, signature: '',
    };
    sv.signature = kp.sign(AgentMarket._voteSigningBytes(sv));
    return sv;
  };
  // 前 3 委员签 Stop
  const votes = [
    makeVote(kps[0], 'Stop', 'n1'),
    makeVote(kps[1], 'Stop', 'n2'),
    makeVote(kps[2], 'Stop', 'n3'),
  ];
  const r = market.verifyResultAuthenticated('task-qa', members, votes, 0, now);
  assert.strictEqual(r.decision, 'stop');
  assert.strictEqual(r.accepted, true);

  // 伪造签名：委员3 签的票冒充委员0（round 1）→ 验签失败
  const forged = makeVote(kps[3], 'Stop', 'nX', 1);
  forged.voter = kps[0].did;
  assert.throws(
    () => market.verifyResultAuthenticated('task-qa', members, [forged], 1, now),
    /签名无效/,
  );

  // 重放：委员3 在 round1 复用已用 nonce n1 → 拒绝
  const replay = makeVote(kps[3], 'Stop', 'n1', 1);
  assert.throws(
    () => market.verifyResultAuthenticated('task-qa', members, [replay], 1, now),
    /重放/,
  );
});

// 8e. 独立审计：完整生命周期通过、守恒无法发现的拆账篡改被捕获（v2.5.9）
test('独立审计：完整生命周期通过并捕获账实篡改', () => {
  const market = new AgentMarket();
  const owner = 'did:au:au-owner';
  const requester = 'did:au:au-req';
  market.deposit(owner, 100);
  market.deposit(requester, 50);
  const card = AgentCard.new({ did: owner, name: 'w' }).withSkill('writing');
  market.registerAgent(card, MIN_STAKE);
  market.publishTask('task-au', 'goal', 50, requester);
  market.submitBid('task-au', owner, 40);
  market.matchTask('task-au');
  market.completeTask('task-au', true);
  market.settle('task-au');

  const good = market.independentAudit();
  assert.ok(good.passed, `审计应通过: ${JSON.stringify(good.mismatches)}`);

  // 拆账篡改：质押账户 100 → 50，另开幽灵账户 50（总额不变）
  const stakeAcc = `__stake__:${owner}`;
  const before = market.balance(stakeAcc);
  market.balances.set(stakeAcc, before - 50);
  market.balances.set('__ghost__:split', 50);
  // 守恒检查被蒙蔽（余额总和不变）
  const cons = market.conservationCheck(150);
  assert.ok(cons.conserved, '拆账不改总额，守恒被蒙蔽');
  // 独立审计捕获
  const bad = market.independentAudit();
  assert.ok(!bad.passed, '独立审计应发现账实不符');
  assert.ok(
    bad.mismatches.some((m) => m.account === '__ghost__:split'),
    '应列出幽灵拆账账户',
  );
});

// 9. AipIdentity DID 与签名往返
test('AipIdentity 生成 did:aip 且签名可验证', () => {
  const id = AipIdentity.generate();
  assert.ok(id.did.startsWith('did:aip:'), 'DID 前缀');
  assert.strictEqual(id.publicKey.length, 32, '原始公钥 32 字节');
  const obj = { did: id.did, name: 't' };
  id.signInto(obj);
  assert.ok(AipIdentity.verifyObject(obj, id.publicKey), '验签通过');
  const tampered = { ...obj, name: 'x' };
  assert.ok(!AipIdentity.verifyObject(tampered, id.publicKey), '篡改拒绝');
});

// 10. 跨语言基准（与 Rust/Python 固定种子一致）
test('AipIdentity.fromSeed 复算 Rust/Python 基准', () => {
  const id = AipIdentity.fromSeed(Buffer.alloc(32, 1));
  assert.strictEqual(
    id.publicKey.toString('hex'),
    '8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c'
  );
  assert.strictEqual(id.did, 'did:aip:34750f98bd59fcfc');
  const obj = {
    did: id.did,
    name: 'CrossLang',
    capabilities: ['text-generation', 'mcp'],
    stake: 100,
  };
  const sig = id.signObject(obj);
  assert.strictEqual(
    sig,
    'e14d3f9e8204ea185ea4ba32a8117f262095ab9dac1352e1a7964ce36d3355c288dd6804ffa7daeb5f6eba9f30428f52702a75fd3efbb6a62555a7f24665da0e'
  );
});

// 11. ACA manifest/message 构造与验签
test('ACA manifest 与 proposal message 可验签、篡改拒绝', () => {
  const owner = AipIdentity.generate();
  const peer = AipIdentity.generate();
  const manifest = buildManifest(owner, 'Agent1', ['text-generation'], {
    stake: 100,
    mcpToolCount: 3,
  });
  assert.ok(verifyManifest(manifest, owner.publicKey));
  assert.ok(!verifyManifest({ ...manifest, name: 'Fake' }, owner.publicKey));

  const envelope = buildEnvelope(owner.did, 'text-generation', 'out', {
    budget: 10,
  });
  const message = proposalMessage(owner, peer.did, envelope);
  assert.ok(verifyMessage(message, owner.publicKey));
  assert.ok(!verifyMessage(message, peer.publicKey), '他人公钥拒绝');
});

// 12. Receipt 签名与结果哈希
test('ACA receipt 签名、结果哈希正反例', () => {
  const owner = AipIdentity.generate();
  const result = Buffer.from('hello result');
  const receipt = buildReceipt(owner, 'task-1', result);
  assert.ok(verifyReceipt(receipt, owner.publicKey));
  assert.ok(verifyReceiptResult(receipt, result));
  assert.ok(!verifyReceiptResult(receipt, Buffer.from('other')));
});

// 13. v2.6.0 状态机恢复边：rework→running、no_quorum→open
test('v2.6.0 状态机恢复边', () => {
  const market = new AgentMarket();
  market.deposit('did:au:req', 50);
  market.publishTask('task-1', 'goal', 50, 'did:au:req');
  const t = market.tasks.get('task-1');
  t.assign('did:au:worker'); // OPEN→MATCHED→ASSIGNED
  t.start();                 // ASSIGNED→RUNNING
  t.complete();              // RUNNING→COMPLETED

  // 返工 → 恢复执行
  t.transition(TaskStatus.REWORK);
  assert.strictEqual(t.status, TaskStatus.REWORK);
  market.resumeAfterRework('task-1');
  assert.strictEqual(t.status, TaskStatus.RUNNING);
  // 非 REWORK 不能恢复
  assert.throws(() => market.resumeAfterRework('task-1'), /REWORK/);

  // 无共识 → 重新开放
  t.complete();              // RUNNING→COMPLETED
  t.markNoQuorum();          // COMPLETED→NO_QUORUM
  assert.strictEqual(t.status, TaskStatus.NO_QUORUM);
  market.reopenTask('task-1'); // NO_QUORUM→OPEN
  assert.strictEqual(t.status, TaskStatus.OPEN);
  // 非 NO_QUORUM 不能 reopen
  assert.throws(() => market.reopenTask('task-1'), /NO_QUORUM/);
});

// 14. v2.6.0 Unverified 证据拒绝结算
test('v2.6.0 Unverified 证据拒绝结算', () => {
  const market = new AgentMarket();
  const owner = 'did:au:owner';
  market.deposit(owner, 100);
  market.deposit('did:au:req', 50);
  market.registerAgent(AgentCard.new({ did: owner, name: 'w' }), MIN_STAKE);
  market.publishTask('task-1', 'goal', 50, 'did:au:req');
  market.submitBid('task-1', owner, 40);
  market.matchTask('task-1');
  // 验收通过但证据 Unverified
  market.completeTask('task-1', true, 'Unverified');
  assert.strictEqual(market.tasks.get('task-1').status, TaskStatus.VERIFIED);
  const r = market.settle('task-1');
  assert.strictEqual(r.reason, 'untrusted_evidence');
  assert.strictEqual(r.paid, 0);
});

// 15. v2.6.0 policy=None 提交即验收、豁免证据门禁
test('v2.6.0 policy=None 提交即验收并豁免证据', () => {
  const market = new AgentMarket();
  const owner = 'did:au:owner';
  market.deposit(owner, 100);
  market.deposit('did:au:req', 50);
  market.registerAgent(AgentCard.new({ did: owner, name: 'w' }), MIN_STAKE);
  const t = market.publishTask('task-1', 'goal', 50, 'did:au:req');
  t.verificationPolicy = 'None';
  market.submitBid('task-1', owner, 40);
  market.matchTask('task-1');
  // 无 QA 投票，提交（证据 Unverified）即验收
  market.completeTask('task-1', true, 'Unverified');
  assert.strictEqual(t.status, TaskStatus.VERIFIED);
  const r = market.settle('task-1');
  assert.strictEqual(r.reason, 'settled');
  assert.strictEqual(r.paid, 40);
});

// 16. v2.6.4 canonical 键序按 Unicode 码点序（GAP §4.4）
test('v2.6.4 canonical 键序按码点序，BMP私用区与astral分歧被修复', () => {
  const { stableStringify } = require('../lib/aca');
  const bmp = ''; // U+E000，码点 0xE000
  const astral = '😀'; // U+1F600，码点 0x1F600
  // 码点序：0xE000 < 0x1F600 → bmp 在前。
  // JS 默认 UTF-16 码元序：astral 高半 0xD83D < bmp 0xE000 → 会给出相反顺序。
  const s1 = stableStringify({ [astral]: 1, [bmp]: 2 });
  const s2 = stableStringify({ [bmp]: 2, [astral]: 1 });
  assert.strictEqual(s1, s2, '不同插入顺序应产出同一紧凑 JSON');
  assert.strictEqual(s1, '{"":2,"😀":1}', '键序必须是码点序（bmp 在前）');
});

// 17. v2.7.1 stableStringify 拒绝非法/静默错误载荷（GAP §9.1）
test('v2.7.1 stableStringify：undefined 键省略、BigInt/Date 显式拒绝、数组空值转 null', () => {
  const { stableStringify } = require('../lib/aca');
  // 对象中的 undefined 键被省略，不再产出裸 undefined
  assert.strictEqual(stableStringify({ a: undefined, b: 1 }), '{"b":1}');
  // 数组空位/undefined 元素转 null，保持合法 JSON
  assert.strictEqual(stableStringify([1, undefined, 2]), '[1,null,2]');
  // BigInt 抛明确类型错误，而不是 Node 的晦涩异常
  assert.throws(() => stableStringify({ n: 1n }), /BigInt/);
  // Date 不再静默变成 {}
  assert.throws(() => stableStringify({ d: new Date(0) }), /Date/);
  // 顶层 undefined 拒绝
  assert.throws(() => stableStringify(undefined), /undefined/);
  // 正常路径不受影响
  assert.strictEqual(stableStringify({ b: 1, a: 2 }), '{"a":2,"b":1}');
});

// 18. MCP Bearer 注入（v3.5.3，AU-15）
test('McpHttpClient：带 token 发 Bearer 头、不带则不发', () => {
  // 不设置 token → 不出现 Authorization 头。
  const noAuth = new McpHttpClient();
  assert.strictEqual(noAuth._headers().Authorization, undefined);
  assert.strictEqual(noAuth._headers()['Content-Type'], 'application/json');
  // 设置 token → 带正确的 Bearer 头。
  const withAuth = new McpHttpClient('http://127.0.0.1:4002', '/api/v1/mcp', 10000, 'secret-xyz');
  assert.strictEqual(withAuth._headers().Authorization, 'Bearer secret-xyz');
});

console.log(`\n${passed} 项测试通过`);
if (process.exitCode === 1) {
  console.error('存在失败测试');
} else {
  console.log('全部通过');
}
