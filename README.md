# Agent Universe（智能体宇宙）

> **使命：让科技造福全人类！**
>
> **一句话简介：群众化的 AGI 路线，去中心化的智能体共享 & 开源网络。**

[![CI](https://github.com/TwinsEarth/agent-universe/actions/workflows/ci.yml/badge.svg)](https://github.com/TwinsEarth/agent-universe/actions/workflows/ci.yml)
[![Client Build](https://img.shields.io/github/actions/workflow/status/TwinsEarth/agent-universe/client-build.yml?label=client%20build)](https://github.com/TwinsEarth/agent-universe/actions/workflows/client-build.yml)
[![Release Pipeline](https://img.shields.io/github/actions/workflow/status/TwinsEarth/agent-universe/release.yml?label=release%20pipeline)](https://github.com/TwinsEarth/agent-universe/actions/workflows/release.yml)
[![Publish Pipeline](https://img.shields.io/github/actions/workflow/status/TwinsEarth/agent-universe/publish.yml?label=publish%20pipeline)](https://github.com/TwinsEarth/agent-universe/actions/workflows/publish.yml)

[![Release](https://img.shields.io/github/v/release/TwinsEarth/agent-universe)](https://github.com/TwinsEarth/agent-universe/releases)
[![Release Date](https://img.shields.io/github/release-date/TwinsEarth/agent-universe)](https://github.com/TwinsEarth/agent-universe/releases)
[![npm](https://img.shields.io/npm/v/@twinsearth/agent-universe?color=red)](https://www.npmjs.com/package/@twinsearth/agent-universe)
[![npm downloads](https://img.shields.io/npm/dm/@twinsearth/agent-universe?color=red)](https://www.npmjs.com/package/@twinsearth/agent-universe)

[![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey)](https://github.com/TwinsEarth/agent-universe/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)
[![MSRV](https://img.shields.io/badge/MSRV-1.88%2B-orange)](https://github.com/TwinsEarth/agent-universe/actions/workflows/ci.yml)
[![Python](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/)
[![Node](https://img.shields.io/badge/node-18%2B-green.svg)](https://nodejs.org/)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Safety Gates](https://img.shields.io/badge/safety-no%20panic%20%E2%80%A2%20unsafe%20contained-success)](https://github.com/TwinsEarth/agent-universe/actions/workflows/ci.yml)
[![Stars](https://img.shields.io/github/stars/TwinsEarth/agent-universe)](https://github.com/TwinsEarth/agent-universe/stargazers)
[![Forks](https://img.shields.io/github/forks/TwinsEarth/agent-universe)](https://github.com/TwinsEarth/agent-universe/network/members)
[![Contributors](https://img.shields.io/github/contributors/TwinsEarth/agent-universe)](https://github.com/TwinsEarth/agent-universe/graphs/contributors)
[![Issues](https://img.shields.io/github/issues/TwinsEarth/agent-universe)](https://github.com/TwinsEarth/agent-universe/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

## 核心理念

### 群体智能 = 网络结构的 Scaling Law

大模型通过参数、数据、算力的 Scaling Law 已实现智能涌现，解决了智能的有无问题。

**智能体宇宙**是在其基础上，实现更多、更大、更强的智能与自我迭代——这是**网络结构的 Scaling Law**，从 token 网络结构向智能体网络结构演进。

### 群众路线

当前基础大模型军备竞赛已达千亿万亿元级门槛，普通人无法参与，智能即将被巨头垄断。

**急需第三方介入**，在闭源阵营与开源阵营之间探索一条普惠大众、全民参与的 AGI 甚至 ASI 之路——**群众路线**。

通过这个全民网络，让底层人民也能参与、贡献、分享这场智能科技革命。（类似于 Linux）

### 结构相变拐点

大模型（参数、数据、算力）的 Scaling Law 已预见单个大模型撞到了 Scaling Law 规模墙。

通过大力出奇迹的收益已经逼近"结构相变拐点"：**Scaling Law 即将失效**，需要从结构上突破。

## 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Agent Universe 网络                       │
│                                                               │
│   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐       │
│   │ Mac mini│  │ Windows │  │ Android │  │ 服务器   │       │
│   │ 根种子   │  │ 全节点   │  │ 轻节点   │  │ 全节点   │       │
│   └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘       │
│        └────────────┴────────────┴────────────┘             │
│                         │                                    │
│              ┌──────────┴──────────┐                        │
│              │  Kademlia DHT 路由表  │                        │
│              │  （每个节点持有切片）  │                        │
│              └──────────┬──────────┘                        │
│                         │                                    │
│         ┌───────────────┼───────────────┐                   │
│         ▼               ▼               ▼                   │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐               │
│   │ GossipSub │   │  CRDT    │   │ 纠删码    │               │
│   │ 消息广播  │   │ 状态同步  │   │ 数据冗余  │               │
│   └──────────┘   └──────────┘   └──────────┘               │
│                                                               │
│   链上信任锚（Base / Arbitrum）                                │
│   · 身份  · 质押  · 结算  · 治理                              │
└─────────────────────────────────────────────────────────────┘
```

## 核心模块

### Rust 核心库（gsn-core）

| 模块 | 功能 |
|------|------|
| `identity/` | DID 身份、Ed25519 密钥对、签名、平台安全存储 |
| `agent/` | AgentCard 身份卡、Skill 注册、任务生命周期 |
| `net/` | Kademlia DHT、GossipSub、libp2p 节点、根种子 |
| `chain/` | PoCV 可验证计算、EVM 轻客户端 |
| `storage/` | SQLite 本地存储 |
| `verifier/` | Verifier HTTP 客户端 |
| `swarm/` | **群体智能层**：涌现检测、轻量共识 |
| `economy/` | **信誉经济系统**：信誉分、贡献证明、动态定价 |
| `scheduler/` | **任务调度器**：任务路由、负载均衡 |
| `topology/` | **网络拓扑**：邻居管理、拓扑图 |
| `proof/` | **贡献证明**（Proof of Contribution） |
| `mode/` | 节点模式：Archive / Full / Light / Edge / Browser |
| `crdt/` | CRDT 无冲突复制数据类型 |
| `erasure/` | Reed-Solomon 纠删码 |
| `mcp/` | **MCP 兼容层**（v2.3.2） |
| `aca/` | **ACA 兼容 API**（v2.3.2） |
| `marketplace/` | **智能体市场 Agent Market**（v2.3.4）：注册/发现/匹配/BFT验证/结算/信誉 |
| `bin/` | **gsn-daemon** 守护进程 |

> **库面模块与运行面的区分（v3.5.9，DOC-05 / 自审 F-4）**：上表是 **gsn-core 代码库的
> 模块清单**，不等于「守护进程实际构造运行的能力」。其中 `topology/`、`scheduler/`、
> `memory/`、`swarm/`、`mesh/`、`nat/` 目前是 **library-only / 实验性模块**：它们的类型
> 被 `lib.rs` re-export、有单元测试，但**不在 `gsn-daemon` 的启动运行图中构造**；节点实际的
> 网络发现/广播/打洞走 **libp2p**（`net/` 的 Kademlia、GossipSub、AutoNAT、DCUtR、Relay），
> 调度与群体涌现的运行能力由官方插件以各自实现提供。各模块头注释有逐条标注，「接线启用
> 还是收敛删除」的待决记录与能力边界现状见
> [`docs/CAPABILITY-STATUS.md`](docs/CAPABILITY-STATUS.md)。

### Python SDK（aip-sdk-py）

- Agent Interop Protocol 客户端
- 任务提交与查询
- 本地开发与测试

### 智能体市场（Agent Market · v2.3.4）

智能体宇宙的第一版**经济层**，让智能体、技能、任务、算力可以完成完整交易闭环：

```
注册 → 发布 → 匹配 → 执行 → 验证 → 结算 → 信誉更新
  ↑                                      ↓
  └──────────── 争议/仲裁/罚没 ←─────────┘
```

| 机制 | 说明 |
|------|------|
| AgentCard 注册 | 质押准入（最低门槛），能力声明 + 签名 |
| TaskSpec | 六字段校验：goal/context/done/todo/trace/owner |
| 匹配引擎 | 技能/信誉/负载过滤，性价比排序 |
| BFT-lite QA | n≥3f+1，equivocation 整轮作废，view change |
| 结算守恒 | balance_sum = budget - slashed，防重复支付 |
| 多维信誉 | quality/speed/honesty/availability，**不可转让** |
| 质押罚没 | 作恶扣除质押，信誉同步下降 |
| 证据分级 | verified / cpu-proto / unverified |

### Smart Contracts

| 合约 | 功能 |
|------|------|
| `GovernorToken.sol` | 治理代币 |
| `AgentCardAnchor.sol` | AgentCard 链上锚定 |
| `PoCVSettlement.sol` | PoCV 结算 |
| `ReputationRegistry.sol` | 链下信誉的链上锚定 |

## 快速开始

### 编译 Rust 核心库

```bash
cd gsn-core
cargo build --release
```

### 运行测试

```bash
# Rust 测试
cd gsn-core && cargo test

# Python SDK 测试
cd aip-sdk-py && PYTHONPATH=. pytest tests/ -v
```

### 启动 gsn-daemon

```bash
cd gsn-core
# 前台运行（默认 P2P 4001 / HTTP API 4002）
cargo run --release --bin gsn-daemon
# 自定义端口与数据目录
cargo run --release --bin gsn-daemon -- \
  --port 4001 --api-port 4002 --data-dir ~/.gsn/data
# 连接已有引导节点（非根种子）
cargo run --release --bin gsn-daemon -- \
  --bootstrap /ip4/<bootstrap-ip>/tcp/4001
```

启动后可访问 HTTP API：

| 方法 & 路径 | 功能 |
|---|---|
| `GET /health` | 节点健康、版本、模式、连接数、运行时长 |
| `GET /version` | daemon 版本 |
| `GET /peers` | 本地 Peer ID、连接数、DHT 路由表条目 |
| `GET /agents` | 已注册智能体列表 |
| `GET /tasks` | 任务列表 |
| `POST /agents` | 注册 AgentCard（同时落 SQLite 与 DHT） |

> **当前实现状态**：gsn-daemon 已是**真实网络节点**——libp2p（Noise 加密 + Kademlia DHT + GossipSub）真实 bind P2P 端口，HTTP API 真实 bind API 端口，agents/tasks 通过 SQLite 真实落盘并在重启后恢复。已真机验证：两端口 `LISTEN`、各 API 端点返回正确、POST 注册可查、404 路径正确转义、kill 重启后数据仍在。核心业务逻辑（Agent Market 结算守恒、BFT-lite 验证、信誉）由 618 个 Rust 测试 + 19 个 Python 测试 + 22 个 JS 测试守护（随版本增长，以发布时 cargo test / pytest / node 实测为准），跨语言签名测试保证三端身份/签名互验。链上结算与跨主机多节点 DHT 联调为下一步目标。

### 自动更新（v3.6.1）

daemon 每次启动会在**后台**联网到 npm registry（`@twinsearth/agent-universe` 的完整版本表）
检查是否最新，检查失败只告警、不影响启动。更新对象同时包含 **daemon 二进制**与 **npm 包**。

- **默认 Minor 通道**：只自动跟进同一大版本最新「中版本基线」`x.Y.0`，不追补丁，**绝不跨大版本**。
- **Patch 通道（先锋队员/贡献者白名单）**：自动跟进最新「小版本」`x.Y.Z`。名单为仓库内置、
  编译期嵌入的 [`config/pioneers.txt`](config/pioneers.txt)，可用 `GSN_PIONEERS_FILE` 追加。
- **手动更新任意版本（含跨大版本）**：

```bash
gsn update                 # 按本机通道自动更新 daemon + npm
gsn update --check         # 只检查，不安装
gsn update 3.6.1           # 手动更新到指定版本（可跨大版本/降级）
gsn update --track patch   # 本次按 patch 通道
gsn version --check        # 只查询通道内目标与新大版本提示
```

环境变量：`GSN_AUTO_UPDATE=0`（只检查不自动安装）、`GSN_NO_UPDATE_CHECK=1`（关闭联网检查）、
`GSN_UPDATE_TRACK=minor|patch`、`GSN_UPDATE_NPM=0`（不更新 npm 包）。

安全：daemon 二进制安装前强制校验随 Release 发布的 `.sha256`（不符即拒绝），同目录原子替换；
安装后**下次启动生效**，不强制重启。平台支持矩阵与诚实边界（如 macOS 仅 aarch64 有资产、
Windows exe 占用需先停节点）见 [`releases/v3.6.1.md`](releases/v3.6.1.md)。

### 三大连接层：CLI · API · MCP（v2.3.5 引入，v2.3.6 深化）

v2.3.5 重新梳理并补全三种连接方式，v2.3.6 进一步把 MCP 真实化、把 ACA 身份与签名补全，并让 Rust/Python/JS 三端在身份与协议层跨语言对齐。三者分别服务于不同场景：

- **CLI** 让用户通过命令行直接操控节点；
- **REST API** 让不同软件按约定交换数据与能力；
- **MCP** 为 AI 提供统一工具连接标准，让大模型安全、规范地调用市场能力。

**① CLI（子命令式，二进制 `gsn`）**

```bash
cargo run --bin gsn -- version          # 版本
cargo run --bin gsn -- identity         # 生成 Ed25519 身份
cargo run --bin gsn -- daemon           # 启动节点（等价 gsn-daemon）
cargo run --bin gsn -- mcp              # 以 stdio 方式启动 MCP 服务

# market 子命令通过 HTTP API 连接运行中的节点
export GSN_API=http://127.0.0.1:4002    # 或用 --api 指定
gsn market deposit caller-1 1000        # 充值
gsn market balance caller-1             # 查询余额
gsn market register card.json           # 注册智能体（也支持内联 JSON）
gsn market get <agent_id>               # 查询智能体
gsn market discover translation         # 按技能发现
gsn market search 关键词                # 搜索
gsn market stats                        # 市场统计
```

**② REST API（`/api/v1/*`，同时兼容旧版 `/agents` `/tasks`）**

| 方法 & 路径 | 功能 |
|---|---|
| `POST /api/v1/accounts/:account/deposit` | 充值 |
| `GET /api/v1/accounts/:account/balance` | 余额 |
| `POST /api/v1/agents` | 注册智能体（201，需质押） |
| `GET /api/v1/agents?skill=` / `?q=` | 按技能发现 / 关键词搜索 |
| `GET /api/v1/agents/:id` | 智能体详情 |
| `POST /api/v1/tasks` | 发布任务（201） |
| `GET /api/v1/tasks` / `/:id` | 任务列表 / 详情 |
| `POST /api/v1/tasks/:id/bids` | 投标（201） |
| `POST /api/v1/tasks/:id/match` | 匹配最优智能体 |
| `POST /api/v1/tasks/:id/results` | 提交执行结果（201） |
| `POST /api/v1/tasks/:id/verify` | BFT-lite QA 验证 |
| `POST /api/v1/tasks/:id/settle` | 结算 |
| `POST /api/v1/disputes` / `:id/arbitrate` | 争议 / 仲裁 |
| `GET /api/v1/conservation` | 结算守恒检查 |
| `GET /api/v1/leaderboard?limit=` / `/stats` | 排行榜 / 统计 |
| `POST`·`GET /api/v1/mcp` | MCP（无状态 JSON-RPC / SSE） |

业务错误返回 `422`，资源不存在返回 `404`，注册类成功返回 `201`，`OPTIONS` 返回 `204`。

**③ MCP（20 个市场工具，支持 stdio 与 HTTP/SSE 两种传输）**

- stdio：`gsn mcp`，逐行读写 JSON-RPC（日志走 stderr），可直接接入 Claude Desktop / Cursor 等；
- HTTP：`POST /api/v1/mcp` 无状态 JSON-RPC，`GET /api/v1/mcp` 返回 `text/event-stream` 初始化帧。

工具命名 `market_*`，覆盖：`market_register_agent`、`market_discover_agents`、`market_publish_task`、`market_submit_bid`、`market_match_task`、`market_submit_result`、`market_verify_result`、`market_settle_task`、`market_open_dispute`、`market_arbitrate`、`market_deposit`、`market_balance`、`market_conservation`、`market_leaderboard`、`market_stats` 等共 20 个。`tools/call` 全部路由到市场 actor **真实执行**（非占位）。

> 三层均已真机验证：daemon 真实 bind P2P/API 端口，curl 走通「充值→注册→发布→投标→匹配→结果→验证→结算→守恒」完整闭环；MCP stdio 完成 `initialize` / `tools/list`（20 工具）/ `tools/call`；CLI 各子命令连接节点返回真实数据。由 618 个 Rust 测试守护，跨语言签名测试保证 Rust/Python/JS 三端身份、规范载荷与签名逐字节一致、可互验。

### Python SDK 使用

```python
from aip import AgentCard, Task, ShardedIndex

card = AgentCard.new(did="did:aip:demo", name="my-agent")
card.with_capability("text-generation")

index = ShardedIndex(num_shards=16)
index.put(card.did, card)
```

### JS SDK 安装

**方式一：公共 npmjs（零认证，推荐）**

包已发布到公共 npm registry，任何人无需 token 即可安装：

```bash
npm install @twinsearth/agent-universe
```

**方式二：jsDelivr 公开 CDN（无需登录 / 无需 token）**

浏览器或 Node 直接下载：

```bash
# 一键下载主入口与全部模块（零认证）
BASE="https://cdn.jsdelivr.net/gh/TwinsEarth/agent-universe@main/js"
curl -O "$BASE/index.js" --create-dirs
for f in keychain models dht market aca mcp; do
  curl -o "lib/$f.js" --create-dirs "$BASE/lib/$f.js"
done
```

也可在 HTML 中直接引用单文件：`https://cdn.jsdelivr.net/gh/TwinsEarth/agent-universe@main/js/index.js`

**方式三：GitHub Packages（需 GitHub token）**

```bash
# 包发布在 GitHub Packages registry，需先配置凭证
npm config set @twinsearth:registry https://npm.pkg.github.com
npm install @twinsearth/agent-universe
```

已发布版本：1.0.0 / 2.0.0 / 2.2.0 / 2.3.0 / 2.3.1 / 2.3.4 / 2.3.5 / 2.3.6 / 2.4.0 ~ 2.9.2（v2.8.8 跳过）/ 3.0.0 / 3.1.0 ~ 3.6.1，详见 [Releases](https://github.com/TwinsEarth/agent-universe/releases)。

## 版本谱系

| 版本 | 代号 | 核心特性 |
|------|------|----------|
| v1.0.0 | Genesis | 项目初始化 |
| v2.0.0 | Shard | 分片存储与 DHT |
| v2.1.0 | Bridge | 跨链桥接与经济层 |
| v2.2.0 | Mesh | 全对等网络 |
| v2.3.0 | P2P | P2P 分布式网络基础 |
| v2.3.1 | Swarm | 群体智能：网络结构的 Scaling Law |
| v2.3.2 | MCP+ACA | MCP 和 ACA 兼容 API |
| v2.3.3 | P2P Net | P2P 分布式网络应用 |
| v2.3.4 | Market | 智能体市场 Agent Market |
| v2.3.5 | **Client** | **跨平台客户端 + CLI/REST/MCP 重构** |
| v2.3.6 | **MCP/ACA** | **MCP/ACA 深化重构 + 三端跨语言可信对齐** |
| v2.4.0 | **Govern** | **CPU 治理轻量化：结算守恒 O(1) 增量维护 + Sandbox trait 接口（进程级为默认后端；Docker/Firecracker 仅做环境探测，未实现隔离）** |
| v2.4.1 | **Layer** | **Lv1–Lv7 分层拓扑：扇入有界、边数亚二次、路由≤7 跳 + TopologyRouter 降级** |
| v2.4.2 | **Memory** | **个体内部记忆库 + 群体外部共享记忆库** |
| v2.4.3 | **3-Tier Memory** | **分层主体记忆（Lv1–Lv7）+ 跨代记忆哈希链** |
| v2.4.4 | **Memory+** | **个体记忆增强：标签/关键词检索、记忆评价防污染、LRU 压缩遗忘** |
| v2.4.5 | **Handoff** | **TransferBundle 六字段交接 + TraceLedger 审计 + 证据三级标签** |
| v2.4.6 | **Flywheel** | **群体智能飞轮：协同→经验→优化结构→闭环** |
| v2.4.7 | **Hetero LLM** | **异构 LLM 多智能体：跨智能体协同、评分内部投票、跨模型记忆协作** |
| v2.4.8 | **DeepSeek** | **DeepSeek 模型适配层：提示词编码、协议互转、token 编解码** |
| v2.4.9 | **Multi-LLM** | **OpenAI / Gemini / Anthropic 三大后端适配** |
| v2.5.0 | **Doubao** | **豆包 / 火山引擎方舟适配（Seed 2.1，1024K）** |
| v2.5.1 | **CN Models** | **国内六大模型统一适配：Kimi / 千问 / 智谱 / MiniMax / 混元 / 小米** |
| v2.5.2 | **Net Partition** | **国内外模型网络分区感知 + 外网不可达自动降级** |
| v2.5.3 | **Mesh** | **Mesh 自组网：心跳 / 广播 / 嗅探 / 会话，临时 SN↔永久身份绑定** |
| v2.5.4 | **Traversal** | **NAT 穿透：Circuit Relay v2 + AutoNAT + DCUtR + Ping，跨网中继** |
| v2.5.5 | **Relay Pool** | **Relay 节点池管理 + 多通道智能切换（默认 3 条，掉线自动补）** |
| v2.5.6 | **Regression** | **历史 Bug 回归套件：test/ 14 条 + Rust 网络回归，CI 强制全量回归，修新 bug 先加用例** |
| v2.5.7 | **Identity** | **跨实现身份一致性：统一 DID 派生口径 + conformance 签名向量逐字节命中上游 + 客户端构建链路根治（file: 依赖）** |
| v2.5.8 | **Ledger** | **精确整数账本 Money(i64)：守恒精确相等无容差 + 发布即托管 __escrow__ 防铸币 + 三端共享金额向量逐值一致** |
| v2.5.9 | **Auth QA** | **认证式 BFT（固定委员集 Ed25519 签名票，替代调用方合成）+ 可失败独立审计（只信任流水独立重放）+ nonce/时间窗重放保护 + 只追加结算流水** |
| v2.6.0 | **State Machine** | **状态机恢复边（no_quorum→open、rework→running，消除 NoQuorum 吸收态）+ 证据分级作为结算强制闸门（is_trustworthy 此前零调用点）+ policy=None 提交即验收** |
| v2.6.1 | **Ledger Persistence** | **账本落盘 + 重启从只追加流水重放恢复（ledger_entries，根治只写不读/重启丢账）+ MCP 单一来源参数校验（缺必填/类型错误返回 -32602，杜绝静默降级）** |
| v2.6.2 | **Version Source** | **VERSION 唯一来源 + check-version 全仓一致性断言（CI 强制，漂移即红）+ 合约重写可部署（Hardhat 18 逐缺陷测试）+ 真实 Reed-Solomon 纠删码 + 网络替身 InMemory* 诚实化** |
| v2.6.3 | **Settlement Closeout** | **重复注册拒绝 + 投标报价与预算强校验（0/负/超预算/已关闭拒绝）+ Rejected / DuplicateWork 终局可达（返工重提相同结果自动拒绝，付0、退预算、罚没10%质押）** |
| v2.6.4 | **Consensus Hardening** | **BFT checked 算术（溢出安全）+ 规范签名 Result 化/递归剥离 signature + JS 码点序跨语言键序 + PROTOCOL_VERSION 契约 + 弱公钥拒绝 + Clock 端口（消除 1970 panic）** |
| v2.6.5 | **Topology Truth** | **route_hops 按真实 lca（去常量 7）+ 边数 N→2N 线性验证 + 房间 BTreeSet 确定性分桶（与插入顺序无关）+ fanin 补上行边 + 协议版本 env! 唯一来源** |
| v2.6.6 | **HTTP & Storage Hardening** | **persist 锁毒化根除（into_inner）+ accept 容错不退进程 + 写操作强制 POST（405）+ 错误码机器前缀替代中文子串 + url_decode off-by-one** |
| v2.6.7 | **Memory Hardening** | **真 SHA-256 链存载荷（+verify_chain 定位首个断裂）+ 真 LRU（last_used 逻辑时钟）+ 派生质量防污染（成功率×10000，删除自报 weight）+ 盲猜死循环 break** |
| v2.6.8 | **MCP Auth** | **Bearer 闸门（未配置默认拒绝全部动钱工具）+ MCP 握手状态机 + RequestId Null + 未知工具 isError + LLM 五适配器去 panic** |
| v2.6.9 | **Doc Honesty** | **文档/测试一致性：SSE 假流/假网络分区/回归套件误导标注诚实化，停止过度承诺** |
| v2.7.0 | **Client Release** | **首次全平台客户端分发大版本（macOS/Windows/Linux），每 9 小版本后的大版本才重建一次客户端** |
| v2.7.1 | **stableStringify** | **undefined 键省略/BigInt·Date 显式 TypeError，杜绝静默非法载荷签名** |
| v2.7.2 | **Nat Honesty** | **NAT 占位检测返 Unknown 不再猜 PortRestrictedCone，真实分类交 AutoNAT** |
| v2.7.3 | **Deploy Fixes** | **发布缺 state 字段 422（serde default）+ 认证验收后证据自动升 Verified（打通结算死路）** |
| v2.7.4 | **Restart Restore** | **重启后 agents/tasks 注入内存 market（此前只 load 不注入，HTTP 全空）** |
| v2.7.5 | **DCUtR + Relay Pool** | **relay 直连打洞升级跟踪 + 多 relay 多通道同时在线，失效自动切换** |
| v2.7.6 | **Sandbox Core** | **Agent Sandbox 核心架构：IsolationLevel/ResourceLimits/NetworkPolicy/FilesystemPolicy 类型与安全默认** |
| v2.7.7 | **Process Runtime** | **进程级隔离运行时（不依赖 Docker/KVM）：独立临时目录 + env_clear + ulimit + 超时强杀** |
| v2.7.8 | **Sandbox Lifecycle** | **SandboxManager：预热池 + 休眠唤醒 + Checkpoint/Fork/Reset + evict_idle** |
| v2.7.9 | **Sandbox Security** | **NetworkGuard 白名单 + AuditLog append-only + PermissionChecker scope 校验** |
| v2.8.0 | **Sandbox Integration** | **沙箱接入 daemon 主链路：REST/E2B 兼容 API + 7 个 MCP 工具 + K8s CRD 清单** |
| v2.8.1 | **Windows Exec** | **Windows 直接 spawn 解释器（去 bash/python3 硬编码），print(1+1) 在 Windows 真机 200** |
| v2.8.2 | **Rust-native Timeout** | **超时改 Rust 原生 spawn+读线程+try_wait kill（去外部 timeout/gtimeout），macOS CI 变绿** |
| v2.8.3 | **Ledger Hash Chain** | **账本日志加 SHA256 哈希链 + 锚定 head（tamper-evident），坏行显式报告、水位取逻辑记录数、append 失败不推进（GAP §3.1/§3.6）** |
| v2.8.4 | **Restart Gate** | **重启恢复证据闸门/结果信封/信誉/质押：验证策略不回 None、winner_price 不满额兜底、结果信封与质押重启后存活，根治“重启即绕过”（GAP §3.2）** |
| v2.8.5 | **REST Auth** | **REST 变更接口 fail-closed 认证（未配 token 默认 401）、CORS 白名单消除通配、请求体上限/读超时、状态转换表（Open/终态不能争议）、罚没金额服务端定、reject_task 经 REST/MCP 可达（GAP §3.5/§3.7/§3.8/§2.2.6）** |
| v2.8.6 | **MCP Validate** | **MCP 参数校验接入两个生产传输（sse/stdio），非法金额/类型返 -32602 不再静默存 0；金额入口只接受整数（get_money/REST/CLI 去浮点）；ACA Receipt 计量整数化，规范签名载荷无浮点、三端一致（GAP §3.5/§4.1）** |
| v2.8.7 | **Sandbox Auth** | **沙箱变更接口统一认证（401）+ 所有权绑定（非所有者 403）；随机 id（sb- + 16 hex）不可猜、重启不复用；输出截断 1 MiB；请求体 env/初始文件/超时/域名/资源真正生效；Windows Job Object 强制资源/进程树；启动清扫孤儿沙箱（GAP §3.3）** |
| v2.8.9 | **Claim Verification** | **弱公钥拒绝接入 register_peer/委员/贡献验证；贡献验证 DID↔公钥绑定（一把密钥不能伪造 DID 绕去重）；审计点名生产 panic 全部类型化；NAT 守卫锁值 Unknown、纠删码测试真丢数据片靠校验片重建；锁毒化 into_inner 恢复全部告警（GAP §2.2）。v2.8.8 按指示跳过** |
| v2.9.0 | **Workbench** | **桌面工作台大版本（继续 Tauri 2，参考 DeepSeek Harness）：9 大视图、默认工作区、过程展示分级、托盘常驻/单实例、后台任务、Excel/CSV/TSV 预览、模型提供商统一入口、终端、Subagent/团队、插件管理；`?all=1` 真实任务/智能体列表；client-build 三平台构建工作台、ci 增 workbench-check** |
| v2.9.1 | **Sandbox Capability** | **沙箱能力声明闸门：边界要么强制执行、要么显式 waiver（带理由），否则拒绝；默认不执行（NullExecutor/后端禁用）；`trusted_local` 平台感知 waiver（Linux 网络/FS/磁盘，macOS 额外内存）；内存/CPU 跨平台（Linux ulimit -v、macOS 无 RLIMIT_AS 诚实标 unenforced、Node --max-old-space-size 全平台）；生产代码 panic 归零（静态关卡验证，v2.9.2 接入 CI）；快照持久化错误改为告警；cargo fmt 关卡** |
| v2.9.2 | **CI Engineering** | **rust-test 矩阵接入 Windows（fmt/build/test/clippy，winjob 等专属代码首次有 CI）；新增 static-gates job：check-no-panics（生产 panic 站点必为 0）+ check-unsafe-containment（每个 unsafe 有 SAFETY 理由）；两个静态检查脚本正式入库** |
| v3.0.0 | **Plugin Kernel（大版本）** | **一切插件化架构重构：插件内核（注册中心/插件总线 PMB/权限仲裁/生命周期/能力模型/黑名单）、热更新·热插拔·热兼容（原子切换+失败回滚+ABI 协商）、五级插件体系（系统/官方/认证/第三方/黑名单）、T1/T2/T3 独立进程隔离 + 能力闸门、T0 系统插件真实运行、`/api/v1/plugins` REST API；crate 版本映射规则升级（0.X.YZ，3.0.0→0.3.0）；WASM 为可选 feature（当前类型化拒绝）。详见 [PLUGIN_GUIDE.md](docs/PLUGIN_GUIDE.md)** |
| v3.1.0 | **Business Plugins** | **业务插件化：官方插件真正承载业务逻辑（entry 模块 + 随内核自动装配）** |
| v3.2.0 | **More Business Plugins** | **业务插件化深化：再 3 个核心官方插件（shard/bridge/crdt 等）承载真实业务 entry** |
| v3.2.1 | **PMB Security** | **PMB 安全通信：消息 HMAC 签名 + nonce 防重放 + 总线端到端接线** |
| v3.2.2 | **Swarm Detect** | **业务化 swarm-emergence：群体智能涌现检测（detect）** |
| v3.2.3 | **Skill Discover** | **业务化 agent-skill：按技能发现智能体（discover）** |
| v3.3.0 | **Chain Anchor** | **业务化 chain-anchor：离线锚定构造与双校验（anchor/verify）** |
| v3.4.0 | **Chain Bridge** | **业务化 chain-bridge：跨链信誉桥接与中位数共识（9/9 官方插件全业务化）** |
| v3.4.2 | **Plugin Audit** | **全局审核版本：补齐插件生命周期/信任/黑名单接线 + 架构合规关卡** |
| v3.4.5 | **Grounding** | **A 类接地修复 + B1 主数据面编排接管 + B3 系统插件接线** |
| v3.5.0 | **Plugin Outbox** | **B2：进程插件 outbox 主动通信（新能力）；Mac/Windows 实机部署与跨平台验证** |
| v3.5.4 | **Persist Fidelity** | **重启持久化全字段保真（W-04 任务规格 / W-05 智能体卡片）** |
| v3.5.5 | **CLI Hardening** | **gsn CLI 鉴权/金额/仲裁契约加固（W-03）与 daemon 版本开关（W-02）** |
| v3.5.6 | **Audit Persist** | **中继池遥测落盘失败显式化 + 市场层结算独立审计/防重复结算回归（#74）** |
| v3.5.7 | **MSRV Honesty** | **MSRV 诚实化：显式声明并实测最低工具链 1.88.0（#75 / W-01）** |
| v3.5.8 | **Real Benchmark** | **守护进程真实端到端吞吐/延迟基准 + 历史无测量性能小节诚实化（#76 / DOC-07）** |
| v3.5.9 | **Capability Status** | **能力边界状态登记 + 库面模块诚实标注 + 历史条目勘误指针（#77 / DEV-01~05、DOC-05/F-4、DOC-08）** |
| v3.6.0 | **Ops CLI** | **新增运维只读 CLI：`gsn ledger verify`（账本哈希链/坏行/重放守恒离线核验）与 `gsn doctor`（一键诊断）** |
| v3.6.1 | **Auto Update** | **daemon 自动更新：npm registry 权威源（不盲信 dist-tags）+ 默认 minor/白名单 patch 双通道 + 手动可跨大版本；daemon 二进制 sha256 fail-closed 原子替换 + npm 包同步；release.yml 三平台增原始二进制+.sha256 资产** |

详见 [RELEASES.md](RELEASES.md) 和 [releases/](releases/) 目录。

## 技术栈

- **语言**: Rust 2021 Edition + Python 3.10+
- **P2P**: libp2p（Kademlia DHT + GossipSub + QUIC）
- **加密**: Ed25519 + SHA256 + X25519
- **存储**: SQLite + DHT + IPFS Bitswap
- **合约**: Solidity（Base / Arbitrum 主网）
- **部署**: Mac mini M4 + launchd / Docker / systemd
- **CI/CD**: GitHub Actions（矩阵构建 + 自动测试）

## 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解开发流程。

## 安全

报告安全漏洞请参考 [SECURITY.md](SECURITY.md)。

## 开源协议

[MIT License](LICENSE)

---

**Agent Universe · 智能体宇宙**

*让科技造福全人类！*

## v2.3.5 跨平台客户端

v2.3.5 新增基于 Tauri 2 的跨平台客户端。客户端**源码**在仓库内，**安装包为构建产物、不入库**：在推送版本 tag 时由 [`.github/workflows/client-build.yml`](.github/workflows/client-build.yml) 在对应系统的 runner 上自动构建，并发布到 GitHub Release 下载。

**源码位置**

- 客户端工程：`client/`（前端 Vite + HTML/JS 在 `client/src/`，Rust 壳 `client/src-tauri/`，Android 工程 `client/src-tauri/gen/android`）
- 桌面端源码目录：`desktop/`
- 各平台本地构建说明：`client/platforms/`

**安装包（在 GitHub Release 下载，非仓库内路径）**

| 平台 | 产物 | 说明 |
|------|------|------|
| macOS | `.dmg` / `.app` | Apple Silicon（Intel 可本地自行构建） |
| Windows | `.msi` / `-setup.exe`（NSIS） | x64 |
| Linux | `.deb` / `.AppImage` | x64 |
| Android | `.apk` | universal / 分 ABI |
| iOS | 见 `client/ios/README.md` | 需 Apple Developer 证书与描述文件，开源仓库不内置签名成品 |

Release 下载：https://github.com/TwinsEarth/agent-universe/releases/tag/v2.3.5

**本地手动构建**

```bash
cd client
npm install
npm run build        # 前端
npx tauri build      # 桌面安装包（需在对应系统上，并装好平台依赖）
npx tauri android build --apk   # Android（需 JDK + Android SDK/NDK）
```

## v2.9.0 桌面工作台（Workbench）

v2.9.0 把客户端从极简演示升级为真正「工作台」，深度参考 DeepSeek Harness 桌面版；**继续用 Tauri 2，不迁移 Electron**。

- **工作台工程**：`desktop/`（前端 Vite + 9 视图在 `desktop/src/`，Rust 壳 `desktop/src-tauri/`）；旧精简客户端 `client/` 保留。
- **9 大视图**：工作区总览、任务、智能体（卡片/团队）、终端、文件（Excel/CSV/TSV 预览）、工具（沙箱执行）、模型提供商、插件、设置。
- **产品形态**：默认工作区（免选文件夹，`~/.agent-universe/workspaces/default`）、过程展示分级（results/steps/full）、托盘常驻 + 单实例、关闭隐藏、后台任务、模型提供商统一入口。
- **daemon 托管**：桌面端查找/启动/健康检查/停止/重启 gsn-daemon，仅绑 loopback，不暴露局域网。
- **构建节奏**：客户端不随每个版本构建，仅在大版本（约每 9 个小版本）手动 `workflow_dispatch` 构建一次（见 `client-build.yml`）；Android 仍由 client 产出。
- **边界**：LLM 仍为 mock、插件为实验开关、沙箱依赖可执行后端、实时性为轮询。

```bash
cd desktop
npm install
npm run build
npx tauri build      # 工作台桌面安装包（需在对应系统并装好平台依赖）
```

## v2.3.6 MCP/ACA 深化与跨语言可信对齐

v2.3.6 把 MCP 从占位门面重写为真实工具协议，把 ACA 的身份与规范签名补全，并让 Rust/Python/JavaScript 三端在身份、规范载荷与签名层面逐字节对齐、可互验。

**统一身份与规范签名**

三端身份口径一致：Ed25519 原始 32 字节公钥 → SHA256 前 8 字节 → `did:aip:<16hex>`；规范载荷为移除 `signature` 键后紧凑、键按字典序、非 ASCII 不转义的 JSON 字节。

```js
const { AipIdentity, buildManifest } = require('@twinsearth/agent-universe');
const id = AipIdentity.generate();
const manifest = buildManifest(id, 'MyAgent', ['text-generation'], { stake: 100 });
AipIdentity.verifyObject(manifest, id.publicKey);   // true
```

**MCP 客户端连接节点**

```js
const { McpHttpClient } = require('@twinsearth/agent-universe');
const mcp = new McpHttpClient('http://127.0.0.1:4002');
await mcp.initialize();
const tools = await mcp.listTools();        // 20 个工具，字段为规范 inputSchema
await mcp.callTool('market_stats', {});    // 经 daemon 真实路由执行
```

Python 侧对应 `aip.AipIdentity`、`aip.build_manifest`、`aip.McpHttpClient`、`aip.MarketClient`，与 JS/Rust 同口径。固定种子（32 字节 `0x01`）下三端公钥、DID、签名逐字节一致，篡改载荷或使用他人公钥即被拒绝。

Release 说明：https://github.com/TwinsEarth/agent-universe/releases/tag/v2.3.6

## 文档

- **版本号登记表（发版必读）**：[docs/version-checklist.md](docs/version-checklist.md) — 全仓所有版本声明点清单 + 发版 SOP；一键改版本见 `scripts/bump-version.sh`。
- **架构文档 v2.5.5**：[docs/architecture-v2.5.5.md](docs/architecture-v2.5.5.md) — v2.5.x 全量：Lv1–Lv7 分层拓扑、三层记忆共享、异构 LLM 适配层、网络分区降级、Mesh 自组网、Relay 池与多通道、三端跨网真机实测。
- **架构文档 v2.3.6**：[docs/architecture-v2.3.6.md](docs/architecture-v2.3.6.md) — 技术架构与系统框架、网络结构与安全机制、功能模块与产品功能（含分层图）。
- **设计文档 v2.4.0–v2.4.1**：[docs/design-v2.4.0-v2.4.1.md](docs/design-v2.4.0-v2.4.1.md) — CPU 治理轻量化与 Lv1–Lv7 分层拓扑设计。
- 深度分析 v2.3.4：[docs/v2.3.4-deep-analysis.md](docs/v2.3.4-deep-analysis.md)
- 部署与验证报告：[docs/部署与验证报告-2026-09-23.md](docs/%E9%83%A8%E7%BD%B2%E4%B8%8E%E9%AA%8C%E8%AF%81%E6%8A%A5%E5%91%8A-2026-09-23.md)
- 旗舰论文归档：[docs/papers/](docs/papers/) — 46 篇 × 三语言（zh / zhen / en）共 138 篇 PDF，含 P0d 群体智能旗舰。
