# Agent Universe 回滚方案

本方案覆盖：代码回滚、二进制回滚、配置/治理回滚、链上链下不一致处理，以及
数据/状态迁移的回退。部署/升级前先确认本文件的对应步骤。

---

## 1. 回滚总则

- **先可逆，后不可逆**：升级前先备份（二进制、配置、数据目录），确认新版本
  健康后再清理旧版本。
- **优先回到最近一个已知良好版本**，而不是直接回退到很旧的版本（避免数据
  格式跨度太大）。
- 每一次升级都应记录：版本号、提交 SHA、时间、变更内容、回滚触发条件。

---

## 2. 代码/提交回滚

代码仓库为单一主线，本地提交均不强制立即推送。回滚分两种：

### 2.1 回退到指定版本（丢弃升级提交）

适用于新版本尚未外发、且确认不需要保留升级改动：

```bash
# 查看提交链，确定已知良好的提交 SHA
git log --oneline -20
# 硬回退（会丢弃目标之后的提交；不可对已推送的主线随意使用）
git reset --hard <KNOWN_GOOD_SHA>
```

当前加固序列的已知良好基线（第三轮开始前）：`0cf151640c695be712cb55f93870dbe57a246650`。
各加固提交的 SHA 见第三轮交付报告与 `git log`。

### 2.2 用 revert 做安全回退（保留历史）

适用于已推送/需要保留审计痕迹的主线：

```bash
git revert <UPGRADE_SHA>          # 生成反向提交，不改写历史
git revert <SHA_A> <SHA_B>        # 连续回退多个
```

> 若已对外发布（push/Release/npm），**必须用 revert / 新版本号**修复，
> 不得 force-push 改写已发布历史。

---

## 3. 二进制回滚

### 3.1 保留上一版本（prior-good）

升级安装时保留上一可用二进制：

```bash
# 升级前
cp /usr/local/bin/gsn-daemon /usr/local/bin/gsn-daemon.bak
# 部署新版本
install -m 0755 gsn-core/target/release/gsn-daemon /usr/local/bin/gsn-daemon
# 回滚
mv /usr/local/bin/gsn-daemon.bak /usr/local/bin/gsn-daemon
systemctl restart gsn-daemon
```

- systemd：回滚后 `sudo systemctl restart gsn-daemon`，并 `systemctl status`
  + `journalctl -u gsn-daemon -n 100` 确认恢复。
- Docker：回滚到上一镜像 tag（不要复用 `latest`，生产用固定版本号）：
  ```bash
  docker compose -f deploy/docker-compose.yml down
  # 把镜像 tag 改回上一版本（如 agent-universe/gsn-daemon:3.9.8）
  docker compose -f deploy/docker-compose.yml up -d
  ```

> 自动更新模块（`src/update/`）当前在 Linux 不保留旧版备份、无
> recover-update；如需自动通道的 prior-good 回滚，按上面的手动备份/回滚执行。
> 该能力是后续增强项。

---

## 4. 配置 / 治理集回滚

- **配置文件**：`--data-dir`、`GSN_GOVERNANCE_FILE`、`deploy/env.example`
  等配置改动前先备份原文件，回滚直接恢复原文件后重启。
- **治理集（官方多签）**：
  - 治理集是配置驱动、未配置 fail-closed。
  - 若新治理集有问题，恢复上一版 `governance.json`（成员 did→pubkey）即可；
    已进入节点的 `seen_nonces` 会在重启后重置（单节点内存态）。
  - 更换/新增治理成员属于治理动作，需多签/链上治理确认（后续演进为链上治理）。
- **REST 写闸门 token**：怀疑泄露时轮换 `REST_BEARER_TOKEN`，并重启节点使旧 token 失效。

---

## 5. 链上 / 链下不一致处理

> 链上交互为“待测试网凭据”阶段，真实交易未验证；以下为设计的恢复流程。

### 5.1 链上确认延迟（特殊检查点⑥）

- 场景：链下任务已完成/已授信，但链上结算交易迟迟未确认（甚至被回撤）。
- 处理：
  1. 授信/放款以“链上确认达到约定确认数”为前提；未确认前链下记为
     **pending（待结算）**，不进入最终可用余额。
  2. 若确认延迟，等待 `eth_getTransactionReceipt`；超时则按未完成处理。
  3. 若交易被回撤（重组/替换），链下对该笔 pending 做**回滚/补偿**（恢复
     到结算前状态），并重新发起结算。
- 账本内部守恒由独立的只追加流水 + 重放审计保证（`independent_audit`）；
  如需人工纠正，可基于流水重放恢复（`restore_ledger`）。

### 5.2 不一致窗口与告警

- 在 `/metrics` 中关注 pending 结算数量；通过 §7（DEPLOY）的告警在窗口
  超阈值时通知运维。
- 恢复后核对：链上事实（receipt）与链下账本逐笔一致，且守恒审计通过。

---

## 6. 数据 / 状态迁移回滚

- SQLite 存储位于 `--data-dir/gsn.db`。迁移**前必须离线备份**：
  ```bash
  cp -a <data-dir> <data-dir>.bak-$(date +%Y%m%d%H%M%S)
  ```
- 若迁移失败或新版本无法正确读取，停止节点、恢复备份目录、再启动旧二进制。
- 迁移逻辑必须向前兼容（旧版本数据可被新版本读取）；不支持跨版本“向前
  写入”时，旧版本回滚后应仍能读到未被新格式破坏的备份。
- CRDT 状态：周期快照（`GSN_CRDT_SNAPSHOT_INTERVAL_SECS`）提供反熵基线；
  回滚后重新加入网络即可经 Op/Snapshot 重新收敛。
- 纠删码分片：分片携带 blob 元数据（含 original_size）；回滚后从存活分片
  重建，缺失分片数 ≤ parity 时数据可恢复。

---

## 7. 回滚后验收清单

- [ ] 二进制/镜像已切回目标版本，节点启动正常（`/health` 返回 200）。
- [ ] DHT 路由表重新填充（`/api/v1/network/info`），peer 重新发现。
- [ ] CRDT 状态重新收敛（`/api/v1/crdt`，各节点一致）。
- [ ] 账本守恒审计通过，链上/链下 pending 已处理。
- [ ] 关键日志无持续错误（`journalctl` / out.err 日志）。
- [ ] 记录本次回滚原因，用于后续修复与发布。
