# Agent Universe 部署与运维手册

本手册覆盖：从源码构建 → 单节点/多节点部署 → Docker/Compose/Kubernetes →
systemd 常驻 → 可观测性 → 链上信任锚配置。配套的回滚步骤见
[`ROLLBACK.md`](ROLLBACK.md)，快速试玩见仓库根 [`README.md`](../README.md)。

> 术语：**根种子节点（bootnode）**= 网络最初的稳定引导点（通常部署在 Mac mini /
> 公网服务器）；**全节点**= 参与 DHT/GossipSub/CRDT 与经济活动的常规节点；
> **轻节点**= 只发起查询、不服务 DHT 的低资源节点（Android/edge）。

---

## 1. 构建要求

### 1.1 工具链

- Rust：**MSRV 1.88.0**，推荐最新 stable（开发机实测 1.98.1）。安装：
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup default stable
  ```
- 系统依赖：
  - Debian/Ubuntu：`build-essential pkg-config libssl-dev`（默认用 rustls，
    实际可不需要 OpenSSL；保留以便其它工具）。
  - 编译期可能用到 `clang cmake`（rusqlite 已启用 `bundled`，一般不需要系统 sqlite）。
  - 无 `sudo`/容器环境时，用户态即可构建（见 README 快速开始）。

### 1.2 构建命令

```bash
cd gsn-core
cargo build --release                      # 仅核心库 + 两个二进制
cargo build --release --workspace          # 完整工作区
```

release profile 已设置（`gsn-core/Cargo.toml`）：`lto = "fat"`、
`codegen-units = 1`、`strip = "symbols"`，用于缩小体积、提升运行期性能。
产物：`gsn-core/target/release/gsn-daemon`（节点）、`gsn`（统一 CLI）。

质量关卡：
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --lib
cargo test --workspace
```

---

## 2. 节点角色与端口

每个节点使用：

| 端口 | 协议 | 用途 |
|---|---|---|
| `--port`（默认 4001） | TCP | libp2p P2P（Noise + Yamux） |
| `--port`（同号） | UDP | QUIC v1（与 TCP 同端口，支持 UDP 打洞） |
| `--api-port`（默认 4002） | TCP | REST/HTTP API（含 `/health`、`/metrics`） |

多节点同机时，端口按 **步长 10** 规划：
`n0=4001/4002`，`n1=4011/4012`，`n2=4021/4022`，`n_i=4001+10i / 4002+10i`。

### 2.1 根种子节点（Mac mini / 公网服务器）

- 身份：`--data-dir` 指向稳定持久卷（如 macOS `/Volumes/GSN/data`），**不带**
  `--bootstrap`，作为全网最初引导。
- 关键：监听 `0.0.0.0`，开放 4001/TCP、4001/UDP（QUIC）；必要时配置端口转发。
- 建议固定内网 IP / DDNS，便于 worker 引用。
- macOS 开机自启：`deploy/macos/com.gsn.node.plist`（`launchctl load`）。

```bash
gsn-daemon --data-dir /Volumes/GSN/data --listen 0.0.0.0 \
  --port 4001 --api-port 4002 --mode full
```

### 2.2 全节点（Windows / Linux）

```bash
gsn-daemon --data-dir ~/.gsn/data --listen 0.0.0.0 --port 4001 --api-port 4002 \
  --mode full --bootstrap /ip4/<boot-ip>/tcp/4001
```

- Windows 前台或 NSSM/计划任务常驻；Linux 用 systemd（见 §4）。
- 全节点默认以 **Kademlia Server 模式** 运行，参与 DHT 路由与 GossipSub 转发。

### 2.3 轻节点（Android / edge）

- 轻节点保持 Kademlia **Client 模式**（只查询、不服务 DHT），适合低资源/移动端。
- 启动参数与全节点相同，但 `--mode light`（或 `edge`）。

> 注：Android 端打包/桌面工作台属于客户端工程，不在本手册的核心服务部署范围；
> 节点二进制的跨平台行为在 Windows/Linux/macOS 上验证。

---

## 3. 多节点本地测试网（可复现）

仓库脚本一键完成（详见 README「多节点本地组网」）：

```bash
cargo build --release --workspace
scripts/regression-3and5.sh          # 3 节点 + 5 节点
scripts/regression-partition.sh      # 网络分区 1+2，恢复后 30s 内收敛
scripts/regression-malicious.sh      # 10 节点 / 2 恶意，到达率 > 95%
scripts/regression-scale.sh          # 50 节点（GSN_N=50 K=20）
scripts/regression-sybil-100.sh      # 100 新节点冷启动刷信誉压力测试
scripts/crdt-24h-growth.sh           # 24h CRDT 状态增长（CSV）
```

### 3.1 手动起 3 节点

```bash
BIN=gsn-core/target/release/gsn-daemon
GSN_DISABLE_PUBLIC_RELAY=1 $BIN --listen 127.0.0.1 --port 4001 --api-port 4002 --data-dir /tmp/au/n0
# 读 boot 日志 “Peer ID: ...” 后：
GSN_DISABLE_PUBLIC_RELAY=1 $BIN --listen 127.0.0.1 --port 4011 --api-port 4012 \
  --data-dir /tmp/au/n1 --bootstrap /ip4/127.0.0.1/tcp/4001
GSN_DISABLE_PUBLIC_RELAY=1 $BIN --listen 127.0.0.1 --port 4021 --api-port 4022 \
  --data-dir /tmp/au/n2 --bootstrap /ip4/127.0.0.1/tcp/4001
```

- `GSN_DISABLE_PUBLIC_RELAY=1`：纯本地隔离，不连公共社区 relay（生产无需设置）。
- 身份密钥按 **data-dir** 首次生成并持久化（`identity.key`，权限 0600），
  因此每个节点 PeerId 互不相同。
- 冷启动 GossipSub mesh 需 1–2 秒；未就绪的消息自动排队重试（每 2s，最多 6 次）。

### 3.2 观察关键状态

```bash
curl -s 127.0.0.1:4002/peers | jq .                     # 连接数
curl -s 127.0.0.1:4002/api/v1/network/info | jq .        # DHT 路由表条目
curl -s "127.0.0.1:4002/api/v1/network/find?key=abc" | jq .  # 跨节点查找
curl -s 127.0.0.1:4002/api/v1/crdt | jq .               # CRDT 状态
```

---

## 4. Linux systemd 常驻

已提供 unit 模板：`deploy/systemd/gsn-daemon.service`。

```bash
sudo useradd --system --create-home --home-dir /var/lib/gsn gsn
sudo install -m 0755 gsn-core/target/release/gsn-daemon /usr/local/bin/gsn-daemon
sudo cp deploy/systemd/gsn-daemon.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now gsn-daemon
systemctl status gsn-daemon
journalctl -u gsn-daemon -f          # 日志（等价 macOS 的 out/err.log）
```

unit 已设置 `Restart=always`（对应 macOS plist 的 `KeepAlive`）、
`NoNewPrivileges`、`ProtectSystem=strict`、`ReadWritePaths=/var/lib/gsn`、
`LimitNOFILE=65536`。非根种子节点在 `ExecStart` 追加 `--bootstrap`。

---

## 5. Docker / docker-compose

```bash
docker build -t agent-universe/gsn-daemon:local .
docker compose -f deploy/docker-compose.yml up          # 3 节点
docker compose -f deploy/docker-compose.5nodes.yml up   # 5 节点
```

- 多阶段：`rust:1.88-alpine`（musl 静态构建）→ `gcr.io/distroless/static-debian12:nonroot`。
- 容器内 `HOME=/data`，身份密钥落挂载卷，每个容器 PeerId 独立；非 root（65532）。
- 暴露 `4001/tcp`、`4001/udp`、`4002/tcp`；HEALTHCHECK 调
  `gsn-daemon --healthcheck`。
- 环境变量参考 `deploy/env.example`，治理集模板 `deploy/governance.example.json`。

> 云开发机无 docker，镜像构建/组网由 CI 与部署环境验证；本地文件已通过
> YAML 语法校验。

---

## 6. Kubernetes（可选）

要点：StatefulSet + headless Service（含 UDP）+ 每 Pod 独立 PVC；
`gsn-0` 作为种子，其余 Pod 以 `gsn-0` 的 DNS 引导。

- Service：一个 headless service（clusterIP: None），端口 4001-tcp / 4001-udp / 4002-tcp。
- StatefulSet：`replicas`、`volumeClaimTemplates` 提供 `/data`；
  命令参数（容器内 HOME=/data）：
  ```
  args: ["--listen","0.0.0.0","--port","4001","--api-port","4002",
         "--data-dir","/data","--mode","full",
         "--bootstrap","/dns4/gsn-0.gsn-headless/udp/4001/quic-v1"]
  ```
  注意 index 0 不带 bootstrap；可在启动脚本里按 Pod 序号裁剪。
- liveness/readiness：httpGet port 4002 path `/health`；readiness 未通过不接流量。
- 指标：ServiceMonitor 抓取各 Pod `/metrics`（见 §7）。

> 具体 manifest 可按部署方约定（Helm/kustomize）生成；本仓库提供完整的
> Docker/compose 与 systemd 路径，K8s 复用同一镜像与配置契约。

---

## 7. 可观测性

- 健康检查：`GET /health` 为**真实依赖检查**——硬检查 storage（失败 → 503），
  软检查连接数 / DHT 路由条目 / CRDT 键数与字节。
- 指标：`GET /metrics`（Prometheus 文本格式 0.0.4），导出：
  - `gsn_dht_routing_entries`（DHT 路由表大小）
  - `gsn_connected_peers`（已连接 peer 数）
  - `gsn_gossip...`（GossipSub 吞吐）
  - `gsn_crdt_keys` / `gsn_crdt_applied_ops` / `gsn_crdt_size_bytes`
  - `gsn_reputation...`（信誉分布）
- 进程级探针：`gsn-daemon --healthcheck`（读 `GSN_HEALTHCHECK_URL`，
  默认 `http://127.0.0.1:4002/health`，2xx→exit 0，否则 1；当前仅支持 http）。
- 日志：结构化（`tracing`）；Linux 走 `journalctl` → 可接 Loki；
  macOS 走 launchd 重定向的 out/err.log。

Grafana 建议面板：DHT 路由表分布（min/median/max）、GossipSub 消息速率与
重复丢弃率、CRDT 状态大小曲线、连接数/churn、信誉分布直方图。

---

## 8. 链上信任锚配置（Base / Arbitrum）

> 真实链上交易需要部署方提供 RPC 与凭据；本代码库的签名/载荷/门禁在离线
> 测试全绿，但**真实广播为“未验证-待测试网凭据”**。

- 网络选择：`GSEN_NETWORK`（`base-sepolia` / `arbitrum-sepolia` / `base` /
  `arbitrum`；默认测试网）。
- RPC：`GSEN_RPC_URL`（覆盖内置公共 RPC）。
- 私钥：仅从 `GSEN_PRIVATE_KEY`（hex）或 `GSEN_KEY_FILE`（本地文件）读取；
  绝不入库、不进日志、不进提交、不进错误信息。
- 治理集：`GSN_GOVERNANCE_FILE`（JSON，`deploy/governance.example.json`
  模板，部署时填入官方多签公钥）；未配置则特权路径 fail-closed。
- **主网 fail-closed**：必须同时显式设置主网确认开关（如
  `GSEN_CONFIRM_MAINNET=1`）与有效凭据，才允许构造/广播；否则返回 403。

部署环境变量占位模板见 `deploy/env.example`。`.env.example`（若使用）只放
占位符、不含真实值，且不提交真实密钥。

---

## 9. 验收对照

| 验收项 | 验证方式 |
|---|---|
| `cargo build --release --workspace` 无错误 | 实跑 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 实跑 |
| `cargo test --lib` / `cargo test --workspace` 全绿 | 实跑 |
| 3 节点发现/DHT 填充/CRDT 同步/重启恢复 | `regression-3and5.sh` |
| 5 节点 mesh / 广播延迟 | `ONLY=5 regression-3and5.sh` |
| 分区恢复后 30s 内 CRDT 收敛 | `regression-partition.sh` |
| 20% 恶意节点到达率 > 95% | `regression-malicious.sh` |
| 50 节点每节点路由表 ≥ K | `regression-scale.sh` |
| 健康检查真实依赖 / metrics 可导出 | `/health`、`/metrics` |
| Docker 镜像可构建可运行 | 部署机（开发机无 docker） |
| 链上真实交易 | 待测试网凭据（当前未验证） |
