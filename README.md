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
cargo run --bin gsn-daemon -- --help
```

### Python SDK 使用

```python
from aip import AgentClient

client = AgentClient()
card = client.register_agent(
    name="my-agent",
    capabilities=["text-generation"]
)
print(f"Agent DID: {card.did}")
```

### npm 包

```bash
npm install @twinsearth/agent-universe
```

详见 [Packages](https://github.com/TwinsEarth/agent-universe/packages)。

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
