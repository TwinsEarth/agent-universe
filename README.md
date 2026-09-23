# Agent Universe（智能体宇宙）

> **使命：让科技造福全人类！**
>
> **一句话简介：群众化的 AGI 路线，去中心化的智能体共享 & 开源网络。**

[![CI](https://github.com/TwinsEarth/agent-universe/actions/workflows/ci.yml/badge.svg)](https://github.com/TwinsEarth/agent-universe/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/TwinsEarth/agent-universe)](https://github.com/TwinsEarth/agent-universe/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/)
[![npm](https://img.shields.io/badge/npm-%40twinsearth%2Fagent--universe-red.svg)](https://github.com/TwinsEarth/agent-universe/packages)

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
| `ReputationBridge.sol` | 跨链信誉桥接 |

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

> **当前实现状态**：gsn-daemon 已是**真实网络节点**——libp2p（Noise 加密 + Kademlia DHT + GossipSub）真实 bind P2P 端口，HTTP API 真实 bind API 端口，agents/tasks 通过 SQLite 真实落盘并在重启后恢复。已真机验证：两端口 `LISTEN`、各 API 端点返回正确、POST 注册可查、404 路径正确转义、kill 重启后数据仍在。核心业务逻辑（Agent Market 结算守恒、BFT-lite 验证、信誉）由 144 个 Rust 测试 + 8 个 JS 测试守护。链上结算与跨主机多节点 DHT 联调为下一步目标。

### Python SDK 使用

```python
from aip import AgentCard, Task, ShardedIndex

card = AgentCard.new(did="did:aip:demo", name="my-agent")
card.with_capability("text-generation")

index = ShardedIndex(num_shards=16)
index.put(card.did, card)
```

### JS SDK 安装

**方式一：jsDelivr 公开 CDN（无需登录 / 无需 token，推荐）**

任何人无需认证即可获取完整包，浏览器或 Node 直接下载：

```bash
# 一键下载主入口与全部模块（零认证）
BASE="https://cdn.jsdelivr.net/gh/TwinsEarth/agent-universe@main/js"
curl -O "$BASE/index.js" --create-dirs
for f in keychain models dht market; do
  curl -o "lib/$f.js" --create-dirs "$BASE/lib/$f.js"
done
```

也可在 HTML 中直接引用单文件：`https://cdn.jsdelivr.net/gh/TwinsEarth/agent-universe@main/js/index.js`

**方式二：GitHub Packages（需 GitHub token）**

```bash
# 包发布在 GitHub Packages registry，需先配置凭证
npm config set @twinsearth:registry https://npm.pkg.github.com
npm install @twinsearth/agent-universe
```

已发布版本：1.0.0 / 2.0.0 / 2.2.0 / 2.3.0 / 2.3.1 / 2.3.4，详见 [Packages](https://github.com/TwinsEarth/agent-universe/packages)。

> 公共 npmjs（`npm install @twinsearth/agent-universe` 零认证）正在筹备中。

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
| v2.3.4 | **Market** | **智能体市场 Agent Market** |

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
