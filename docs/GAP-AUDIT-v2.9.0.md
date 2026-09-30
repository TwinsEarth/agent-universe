# v2.9.0 增量审计 / Gap Audit — 参考 NewAgentUniverseByDeepSeek（只读，不改其线上）

本文用参考项目 NewAgentUniverseByDeepSeek（V1.2.3 + 本地 v2.9.1）已经验证的工程标准，
对照 `agent-universe` **v2.9.0**（gsn-core 0.2.90），列出参考项目已解决、
但 agent-universe 尚未解决的差距。遵循两条铁律：

- **没有调用点的修复不算修复**；
- **在修复前代码上也能通过的测试，什么也没证明**。

## 0. 基线（已实测）

| 项 | 值 |
|---|---|
| Rust 测试 | **448 passed / 0 failed / 0 ignored** |
| Clippy | **0 warning**（`-D warnings`） |
| `cargo fmt --check` | **失败（exit 1）**，约 9263 行 diff，涉及约 115 个文件 |
| CI（main 最新 run） | 7 job 全绿，但**无 fmt gate、无 Windows Rust job** |

## 1. 已修好、必须保住、不许回退

| 能力 | 证据 |
|---|---|
| 账本哈希链（tamper-evident） | `storage/persist.rs:149-154` ledger_entries 加 prev_hash/record_hash；`:558-561` SHA256(prev‖payload)；head 锚 `LEDGER_HEAD_KEY` |
| 重启恢复证据闸门 | `marketplace/mod.rs:1042` restore_tasks_from_store 恢复 verification_policy（v2.8.4）与 winner_price；损坏/遗留显式警告 |
| 结果信封/信誉/质押落盘与恢复 | `api/market_actor.rs:281-291` 启动恢复；`:318-327` 快照 |
| sandbox 认证 + 所有权 + 随机 ID + 孤儿清扫 | `sandbox/api.rs:57-60`（变更类无 caller 401）、`:105` owner 校验、`manager.rs:74-80`（sb-随机，替代 sb-N） |
| Windows Job Object | `sandbox/runtime/winjob.rs`（整树 kill / 进程数 / 内存上限） |
| 账本水位逻辑 | `market_actor.rs:293-310` 水位=成功恢复逻辑记录数；append 失败不推进水位 |
| 无 `.lock().unwrap()` | grep 零命中 |

## 2. 待修复差距（按优先级）

### 【critical / 安全 → v2.9.1】

**G1. sandbox 无 OS 级隔离，子进程可读写整个宿主文件系统。**
- 现状：`runtime/process.rs:162` `Command::new("bash")` / `:186` spawn 解释器；
  全 src 无 `unshare/chroot/landlock/seccomp/pivot_root/CLONE_NEW`；
  `safe_join`（process.rs:404-408）只约束 Rust 侧 read_file/write_file，**不约束子进程**；
  子进程以 daemon 同一 OS 用户运行。
- 参考做法（NewAgentUniverse nau-sandbox）：
  - 默认后端 **NullExecutor**：什么也不执行；
  - 唯一执行后端必须**声明它实际能强制的能力**；
  - 请求了无法强制的策略（FS 隔离、网络隔离、磁盘配额）→ **具名拒绝**
    （PolicyNotEnforceable，说明是哪条边界），绝不静默不受限地执行；
  - 跨平台：Windows 用 Job Object（已有 winjob）。

**G2. 20 处生产 panic 路径（unwrap/expect）。**
实测（剥离注释/字符串、cfg(test) 切分后），包括：
- `api/market_actor.rs:462,507` `serde_json::to_value(..).unwrap()`
- `erasure/mod.rs:28,60` `.expect("reed-solomon 参数/encode")`
- `llm/network.rs:304,314` `endpoint(k).unwrap()`
- `marketplace/mod.rs:398,603` `best.unwrap()` / `get_mut().unwrap()`
- `mcp/protocol.rs:164,265`、`mcp/stdio.rs:144`
- `net/peer.rs:149,154,285` `.expect("valid gossipsub/swarm")`
- `relay_pool/mod.rs:144,148,165,166` `class.parse().unwrap()`
- `scheduler/load_balancer.rs:85` `partial_cmp().unwrap()`（NaN 即 panic）
- `swarm/collective.rs:142` `partial_cmp().unwrap()`（NaN 即 panic）

**G3. 快照持久化错误被静默丢弃。**
- `api/market_actor.rs:313,316,320,323,326`：`let _ = store.upsert_agent/upsert_task/
  put_json_row(...)`，失败无任何日志。
- 参考做法：失败至少 `eprintln!` 记录；或映射为错误/可观测降级。

### 【工程化 / CI → v2.9.1/v2.9.2】

**G4. CI 无 fmt gate，且 115 个文件未格式化。**
- ci.yml rust-test 无 `cargo fmt --check`。
- 修复：先 `cargo fmt` 统一格式化，再在 CI 加 fmt --check（置于 build 前）。

**G5. CI 无 Windows Rust job。**
- rust-test 矩阵 `[ubuntu-latest, macos-latest]` 无 Windows；v2.8.7 起的 Windows
  代码（winjob、process Windows 分支）不在 CI 验证。
- 修复：矩阵加 `windows-latest`（注意 Windows 无 bash ulimit，需保证步骤可运行）。

**G6. winjob 的 3 个 unsafe 块无 SAFETY 注释。**
- `sandbox/runtime/winjob.rs:29,59,70`。

**G7. 缺少静态检查关卡。**
- 参考项目有 `check-no-panics.mjs`、`check-unsafe-containment.mjs`；
  agent-universe 无。移植并在 CI 加 job，让「文档里写了但没人执行」的规则变红。

## 3. 版本规划

- **v2.9.1**：G1（NullExecutor + 能力声明/拒绝）、G2（panic 路径）、G3（持久化错误）、
  G4（fmt + gate）、G6（SAFETY）；每条配能在旧代码上失败的回归测试。
- **v2.9.2**：G5（Windows Rust CI）、G7（静态检查关卡），架构与工程化收尾。
