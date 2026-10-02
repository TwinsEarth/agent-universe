# gsn-daemon 端到端性能基准

本目录存放 `gsn-daemon` 的**真实端到端性能基准**。这是对审计项 **DOC-07** 的回应：
旧文档（v2.1.8「性能指标」）给出过冷启动 / 内存 / 包体积数字但**从未测量**；本目录的
数字来自对 **release 二进制经真实 HTTP/1.1 回环**的可复现测量，不是库内函数调用，
也不是 mock。

## 复现

```bash
# 1) 构建 release 守护进程
cd gsn-core && cargo build --release --bins -j2 && cd ..

# 2) 跑基准（自动拉起临时 data-dir 的 daemon，结束自动清理）
node scripts/bench-e2e.mjs \
  --bin gsn-core/target/release/gsn-daemon \
  --n 200 --warmup 20 \
  --port 4081 --p2p-port 4080 \
  --out result.json
```

脚本会在结束前请求 `/api/v1/audit` 与 `/api/v1/conservation`，**审计不通过或资金不守恒
即以退出码 2 失败**——防止「跑得快但账错了」的数字被采信。

## 工作负载（每轮闭环）

`POST tasks`（发布并托管 50）→ `tasks/{id}/bids`（投标 10）→ `tasks/{id}/match`
（匹配）→ `tasks/{id}/results`（提交结果，policy=None 直接验收）→
`tasks/{id}/settle`（按中标价 10 付款、余款 40 退回需求方、落 SQLite）。

## 测量口径（务必随数字一起读）

- **单机回环（127.0.0.1）、单连接、顺序请求**，包含真实 SQLite 落盘与真实 HTTP 解析。
- **不含**：跨网 libp2p 往返、LLM 推理、并发竞争、TLS。
- 因此结果是「守护进程本地市场/账本栈在顺序负载下的延迟/吞吐基线」，
  **不能外推为公网多节点 TPS**。
- 分阶段延迟含 Node fetch + daemon 处理；闭环吞吐 = 测量轮数 / 测量墙钟（预热不计入）。

## v3.5.8 实测：Linux x64（云沙箱 4 vCPU / 8 GB，rustc 1.98.1，Node v22.23.2）

被测二进制为 **gsn-core 0.3.58 release**（即 v3.5.8 本体）。快照：
[`v3.5.8-linux-x64.json`](v3.5.8-linux-x64.json)（n=200，warmup=20，墙钟 46.823s）。

| 指标 | 值 |
|---|---|
| 闭环吞吐 | **4.27 轮/秒** |
| 单轮平均 / p50 / p95 / p99 | 234.1 / 234.6 / 247.7 / 251.0 ms |

分阶段延迟（ms，n=200）：

| 阶段 | avg | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| publish | 1.686 | 1.485 | 2.487 | 7.114 | 7.873 |
| bid | 4.835 | 4.914 | 7.289 | 8.057 | 11.477 |
| **match** | **109.243** | 109.135 | 114.372 | 117.587 | 122.763 |
| result | 1.930 | 1.498 | 3.554 | 7.280 | 8.651 |
| **settle** | **116.331** | 115.939 | 123.414 | 126.897 | 144.499 |

正确性：`/api/v1/audit` `passed=true`（663 条流水独立重放，expected=actual=12100，
聚合无 mismatch），`/api/v1/conservation` `conserved=true`。

> 复核说明：同脚本在 v3.5.6（gsn-core 0.3.56）二进制上另跑过一轮（4.34 轮/秒、
> match 108.0 / settle 114.6 ms）；两版市场/结算路径无代码差异，结果在测量噪声内一致，
> 佐证瓶颈稳定来自 fsync 而非版本回归。

## 瓶颈分析（诚实归因）

`match` 与 `settle` 各占约 108–115 ms，且方差极小——这不是 CPU 计算或网络特征，而是
**SQLite 同步提交的 fsync 开销**：存储层未设置 `PRAGMA synchronous` / `journal_mode`，
采用默认的 rollback journal + `synchronous=FULL`，每次提交都刷盘；云沙箱 overlayfs 上
一次 fsync 约 100 ms。`publish/bid/result` 也写库但单轮中与上述提交合并，故表现为个位数
毫秒。

**本版（v3.5.8）刻意不修改该同步策略**：改成 WAL 或 `synchronous=NORMAL` 涉及
断电/崩溃一致性权衡（本项目账本的卖点正是可重放、防篡改），属于需要专门设计与
崩溃注入验证的变更，超出 DOC-07「补一次真实测量」的范围。该优化已登记为后续候选，
不得在未做崩溃语义论证前为追求吞吐而放松持久性。
