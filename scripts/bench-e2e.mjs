#!/usr/bin/env node
/**
 * bench-e2e.mjs — gsn-daemon 真实端到端性能基准（DOC-07，v3.5.8）
 *
 * 与「设计目标」相对，本脚本测量的是**真实 release 二进制经真实 HTTP/1.1 回环**完成
 * 资金闭环的表现，不是库内函数调用、也不是 mock。
 *
 * 闭环（每轮）：
 *   POST accounts/{req}/deposit（仅首笔大额充值）→
 *   每轮：POST tasks(发布托管) → tasks/{id}/bids(投标) → tasks/{id}/match(匹配)
 *        → tasks/{id}/results(提交结果, policy=None 直接验收) → tasks/{id}/settle(结算)
 *
 * 输出：每个阶段的调用次数 / 平均 / p50 / p95 / p99 / max（ms），以及端到端
 * 闭环吞吐（轮/秒）与单轮平均延迟。结果同时以 JSON 打印（机器可读）。
 *
 * 用法：
 *   node scripts/bench-e2e.mjs --bin target/release/gsn-daemon --n 200 [--port 4002]
 *       [--p2p-port 4001] [--warmup 20] [--out bench.json]
 *
 * 诚实边界：这是**单机回环（loopback）**串行闭环，包含真实 SQLite 落盘与真实 HTTP
 * 解析，但不含真实 libp2p 跨网往返、不含 LLM 推理、不含并发竞争。它度量的是
 * 「守护进程本地市场/账本栈在单连接顺序负载下的延迟与吞吐基线」，不能外推为
 * 公网多节点 TPS。
 */

import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { performance } from 'node:perf_hooks';

function parseArgs(argv) {
  const a = { n: 200, warmup: 20, port: 4002, p2pPort: 4001, bin: '', out: '' };
  for (let i = 2; i < argv.length; i++) {
    const k = argv[i];
    const next = () => argv[++i];
    if (k === '--bin') a.bin = next();
    else if (k === '--n') a.n = parseInt(next(), 10);
    else if (k === '--warmup') a.warmup = parseInt(next(), 10);
    else if (k === '--port') a.port = parseInt(next(), 10);
    else if (k === '--p2p-port') a.p2pPort = parseInt(next(), 10);
    else if (k === '--out') a.out = next();
  }
  return a;
}

const pct = (sorted, q) => {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.ceil(q * sorted.length) - 1);
  return sorted[idx];
};
const statsOf = (samples) => {
  const s = [...samples].sort((x, y) => x - y);
  const sum = s.reduce((a, b) => a + b, 0);
  return {
    calls: s.length,
    avg_ms: +(sum / Math.max(1, s.length)).toFixed(3),
    p50_ms: +pct(s, 0.5).toFixed(3),
    p95_ms: +pct(s, 0.95).toFixed(3),
    p99_ms: +pct(s, 0.99).toFixed(3),
    max_ms: +s[s.length - 1].toFixed(3),
  };
};

async function main() {
  const cfg = parseArgs(process.argv);
  if (!cfg.bin) throw new Error('必须用 --bin 指定 release 版 gsn-daemon 路径');

  const base = `http://127.0.0.1:${cfg.port}`;
  const dataDir = mkdtempSync(join(tmpdir(), 'gsn-bench-'));
  const env = {
    ...process.env,
    REST_ALLOW_UNAUTHENTICATED: '1', // 基准环境显式放开写操作；生产默认 fail-closed
    RUST_LOG: 'error',
  };

  const child = spawn(
    cfg.bin,
    [
      '--api-port', String(cfg.port),
      '--port', String(cfg.p2pPort),
      '--data-dir', dataDir,
      '--mode', 'full',
    ],
    { env, stdio: ['ignore', 'pipe', 'pipe'] },
  );
  let daemonLog = '';
  child.stdout.on('data', (d) => { daemonLog += d.toString(); });
  child.stderr.on('data', (d) => { daemonLog += d.toString(); });

  const cleanup = () => {
    try { child.kill('SIGTERM'); } catch {}
    try { rmSync(dataDir, { recursive: true, force: true }); } catch {}
  };
  process.on('exit', cleanup);

  // ── 就绪探测 ──
  const deadline = performance.now() + 30000;
  let ready = false;
  while (performance.now() < deadline) {
    try {
      const r = await fetch(`${base}/health`);
      if (r.ok) { ready = true; break; }
    } catch {}
    await new Promise((r) => setTimeout(r, 100));
  }
  if (!ready) {
    cleanup();
    throw new Error('守护进程 30s 内未就绪。日志:\n' + daemonLog.slice(-2000));
  }

  const timings = {
    deposit: [], publish: [], bid: [], match: [], result: [], settle: [], loop: [],
  };

  async function call(method, path, body) {
    const t0 = performance.now();
    const res = await fetch(base + path, {
      method,
      headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const text = await res.text();
    const dt = performance.now() - t0;
    if (!res.ok) {
      throw new Error(`${method} ${path} -> ${res.status}: ${text.slice(0, 300)}`);
    }
    let json = null;
    try { json = text ? JSON.parse(text) : null; } catch {}
    return { dt, json };
  }

  const reqId = 'bench-requester';
  const agentId = 'did:bench:agent-1';

  // 注册一个执行者：充值质押 100 + 注册卡片（资金闭环每轮复用，质押不消耗）。
  await call('POST', `/api/v1/accounts/${agentId}/deposit`, { amount: 100 });
  const card = {
    agent_id: agentId,
    version: '3.5.8',
    name: 'Bench Agent',
    description: 'e2e benchmark',
    skills: ['translation'],
    modalities: ['text'],
    models: ['bench-model'],
    endpoint: 'a2a://bench',
    pricing: { model: 'PerCall', price: 10, currency: 'Credit' },
    sla: { latency_p95_ms: 2000, availability: 0.95, max_concurrency: 8 },
    owner: 'bench',
    stake: 100,
    reputation_score: 0.5,
    total_calls: 0,
    success_rate: 0.0,
    evidence_grade: 'CpuProto',
    verified: false,
    created_at: 1000,
    updated_at: 1000,
  };
  await call('POST', '/api/v1/agents', card);

  // 需求方一次性充入全部轮次预算（每轮托管 50，结算后退余款 40，实付 10）。
  const total = cfg.n + cfg.warmup;
  const { dt: depDt } = await call(
    'POST', `/api/v1/accounts/${reqId}/deposit`, { amount: 50 * total + 1000 },
  );
  timings.deposit.push(depDt);

  const taskFor = (i) => ({
    task_id: `bench-task-${i}`,
    goal: '翻译 benchmark 文本',
    context: '中译英',
    done: [],
    todo: ['translate'],
    trace: [],
    owner: null,
    budget: 50,
    winner_price: null,
    deadline: 9999999999,
    required_skills: ['translation'],
    verification_policy: 'None',
    requester: reqId,
    state: 'Draft',
    created_at: 1000 + i,
  });
  const envelopeFor = (id) => ({
    agent_id: agentId,
    report: '{"translated":"hello"}',
    confidence: 0.95,
    error_type: 'None',
    trace_ref: `trace://${id}/1`,
    evidence_grade: 'Unverified',
    latency_ms: 300,
  });

  // ── 预热（不计入统计）──
  for (let i = 0; i < cfg.warmup; i++) {
    const id = `bench-task-warm-${i}`;
    await call('POST', '/api/v1/tasks', { ...taskFor(-1 - i), task_id: id });
    await call('POST', `/api/v1/tasks/${id}/bids`, { agent_id: agentId, proposed_price: 10, estimated_latency_ms: 500, score: 0 });
    await call('POST', `/api/v1/tasks/${id}/match`);
    await call('POST', `/api/v1/tasks/${id}/results`, envelopeFor(id));
    await call('POST', `/api/v1/tasks/${id}/settle`);
  }

  // ── 测量 ──
  const benchStart = performance.now();
  for (let i = 0; i < cfg.n; i++) {
    const id = `bench-task-${i}`;
    const lt0 = performance.now();
    let r = await call('POST', '/api/v1/tasks', taskFor(i));
    timings.publish.push(r.dt);
    r = await call('POST', `/api/v1/tasks/${id}/bids`, { agent_id: agentId, proposed_price: 10, estimated_latency_ms: 500, score: 0 });
    timings.bid.push(r.dt);
    r = await call('POST', `/api/v1/tasks/${id}/match`);
    timings.match.push(r.dt);
    r = await call('POST', `/api/v1/tasks/${id}/results`, envelopeFor(id));
    timings.result.push(r.dt);
    r = await call('POST', `/api/v1/tasks/${id}/settle`);
    timings.settle.push(r.dt);
    timings.loop.push(performance.now() - lt0);
  }
  const wallS = (performance.now() - benchStart) / 1000;

  // ── 正确性校验：审计必须通过、守恒成立（防止「跑得快但账错了」）──
  const auditRes = await call('GET', '/api/v1/audit');
  const consRes = await call('GET', '/api/v1/conservation');

  const perStage = {};
  for (const [k, v] of Object.entries(timings)) perStage[k] = statsOf(v);

  const report = {
    benchmark: 'gsn-daemon e2e market loop (loopback, serial, real SQLite + HTTP)',
    generated_at: new Date().toISOString(),
    binary: cfg.bin,
    loops_measured: cfg.n,
    warmup: cfg.warmup,
    environment: {
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      note: '单连接顺序负载；不含跨网 libp2p/LLM/并发竞争，结果不可外推为公网多节点 TPS',
    },
    closed_loop: {
      wall_s: +wallS.toFixed(3),
      loops_per_sec: +(cfg.n / wallS).toFixed(2),
      loop_avg_ms: perStage.loop.avg_ms,
      loop_p50_ms: perStage.loop.p50_ms,
      loop_p95_ms: perStage.loop.p95_ms,
      loop_p99_ms: perStage.loop.p99_ms,
    },
    per_stage_latency_ms: {
      publish: perStage.publish,
      bid: perStage.bid,
      match: perStage.match,
      result: perStage.result,
      settle: perStage.settle,
      deposit_one_off: perStage.deposit,
    },
    correctness: {
      audit_passed: !!(auditRes.json && auditRes.json.passed),
      audit: auditRes.json,
      conservation_conserved: !!(consRes.json && consRes.json.conserved),
      conservation: consRes.json,
    },
  };

  console.log(JSON.stringify(report, null, 2));
  if (!report.correctness.audit_passed || !report.correctness.conservation_conserved) {
    console.error('\n❌ 基准期间正确性校验失败（审计/守恒），性能数字无效。');
    cleanup();
    process.exit(2);
  }
  cleanup();
}

main().catch((e) => {
  console.error('BENCH_ERROR:', e.message);
  process.exit(1);
});
