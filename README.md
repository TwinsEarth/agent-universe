# Agent Universe（智能体宇宙）

> **使命：让科技造福全人类！**
> 
> **一句话简介：群众化的 AGI 路线，去中心化的智能体共享 & 开源网络。**

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

## 项目架构

### 核心模块（Rust）

| 模块 | 功能 |
|------|------|
| `identity/` | DID 身份、密钥对、签名、安全存储 |
| `agent/` | AgentCard 身份卡、任务生命周期 |
| `net/` | Kademlia DHT、GossipSub、根种子节点 |
| `chain/` | PoCV 可验证计算 |
| `storage/` | 本地存储 |
| `verifier/` | Verifier HTTP 客户端 |
| `swarm/` | **群体智能层**（Swarm、涌现检测、轻量共识） |
| `economy/` | **信誉经济系统**（信誉分、贡献证明、动态定价） |
| `scheduler/` | **任务调度器**（任务路由、负载均衡） |
| `topology/` | **网络拓扑**（邻居管理、拓扑图） |
| `proof/` | **贡献证明**（Proof of Contribution） |
| `mode/` | 节点模式管理（Archive/Full/Light/Edge/Browser） |
| `crdt/` | CRDT 无冲突复制数据类型 |
| `erasure/` | Reed-Solomon 纠删码 |

### 技术栈

- **语言**: Rust 2021 Edition
- **P2P**: libp2p（Kademlia DHT + GossipSub）
- **加密**: Ed25519 + SHA256
- **存储**: SQLite（本地）+ DHT（网络）
- **合约**: Solidity（Base 主网）
- **部署**: Mac mini M4 + launchd

## 版本谱系

| 版本 | 代号 | 核心特性 |
|------|------|----------|
| v1.0.0 | Genesis | 项目初始化 |
| v2.0.0 | Shard | 分片存储架构 |
| v2.0.1-v2.0.6 | - | 分片优化、DHT 改进 |
| v2.1.0 | Bridge | 跨链桥接 |
| v2.1.1-v2.1.6 | - | 桥接优化、节点管理 |
| v2.1.7 | Mainnet | Mac mini 主网上线部署 |
| v2.1.8 | Client | 轻客户端跨平台架构（Tauri + Flutter） |
| v2.1.9 | Mesh | macOS 全对等网络升级 |
| v2.2.0 | Mesh+ | 网状网络增强 |
| v2.2.1 | Audit | 全局审计 |
| v2.2.2 | Rebuild | 三栈重建 |
| v2.2.3 | Final | 最终版 |
| **v2.3.1** | **Swarm** | **群体智能：智能体宇宙** |

## 测试基线

- Rust: **40/40** 通过
  - lib_test: 10 个
  - audit_test: 8 个
  - integration_test: 6 个
  - v231_test: 16 个
- Python SDK: **6/6** 通过

## 快速开始

### 编译

```bash
cd gsn-core
cargo build --release
```

### 测试

```bash
cargo test
```

### 部署到 Mac mini

```bash
cd deploy
./deploy-mainnet.sh
```

## 开源协议

MIT License

---

**Agent Universe · 智能体宇宙**

*让科技造福全人类！*
