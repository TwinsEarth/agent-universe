# Agent Universe

> **一句话简介**：群众化的 AGI 路线，去中心化的智能体共享 & 开源网络。

> **使命**：让科技造福全人类！

---

## 项目目的

当前 AI Agent 的痛点：
- **孤岛化**：每个 Agent 封闭在各自平台，无法跨平台发现和协作
- **中心化依赖**：依赖中心化服务器发现、调度和结算
- **信任缺失**：Agent 之间无法建立可验证的信誉和经济关系
- **重复造轮子**：每个项目都要重新实现 P2P 发现、身份、结算

Agent Universe 的目的：
1. **统一协议**：一套标准化的 Agent 身份、发现、任务、结算协议
2. **去中心化**：无中心服务器，P2P 网络自组织、自修复、自扩展
3. **经济激励**：质押、验证、信誉驱动的开放协作市场
4. **跨链互联**：通过多链桥接实现跨链信誉迁移和结算

## 项目意义

- **技术意义**：将 BitTorrent 的 P2P 分发思想扩展到智能体领域
- **经济意义**：构建 Agent 之间的开放市场，降低协作成本
- **治理意义**：链上治理 + 信誉系统，实现去中心化的 Agent 治理
- **生态意义**：为 AI Agent 提供标准协议层，类似 HTTP 之于 Web

---

## 系统架构

### 三栈总览

```
┌─────────────────────────────────────────────────────────────────┐
│                    Agent Universe v2.2.3                        │
├─────────────────┬─────────────────┬─────────────────────────────┤
│   contracts/   │    gsn-core/   │     aip-sdk-py/            │
│   (Foundry)    │     (Rust)     │      (Python)              │
├─────────────────┼─────────────────┼─────────────────────────────┤
│ ┌─────────────┐│ ┌─────────────┐│ ┌─────────────────────────┐ │
│ │ ReputationB ││ │ net/        ││ │ models.py               │ │
│ │ SettlementB ││ │  libp2p     ││ │  AgentCard              │ │
│ │ CrossChainR ││ │  DHT        ││ │  Task / TaskStatus      │ │
│ │ BridgeInsur ││ │  GossipSub  ││ │                         │ │
│ │ CrossChainMB││ │ transport   ││ │ index/                  │ │
│ │ GovernorTok ││ │             ││ │  ShardedIndex           │ │
│ └─────────────┘│ │ identity/   ││ │  ShardMetadata          │ │
│                 │ │  DID        ││ │                         │ │
│ 链上信任锚      │ │  Ed25519    ││ │ dht_backend.py          │ │
│ · 身份锚定      │ │  keyring    ││ │  MemoryDHT              │ │
│ · 质押罚没      │ │             ││ │                         │ │
│ · 结算分账      │ │ agent/      ││ │                         │ │
│ · 治理投票      │ │  card       ││ │                         │ │
│                 │ │  task FSM   ││ │                         │ │
│                 │ │             ││ │                         │ │
│                 │ │ chain/      ││ │                         │ │
│                 │ │  PoCV       ││ │                         │ │
│                 │ │  RPC        ││ │                         │ │
│                 │ │             ││ │                         │ │
│                 │ │ storage/    ││ │                         │ │
│                 │ │  SQLite     ││ │                         │ │
└─────────────────┴─────────────────┴─────────────────────────────┘
       链上层              P2P 网络层              SDK 应用层
```

### 五层技术栈

| 层 | 技术 | 功能 |
|------|------|------|
| 链上信任锚 | Solidity / Foundry | 身份、质押、结算、治理 |
| P2P 发现 | libp2p + Kademlia DHT | Agent 发现、路由表 |
| 消息广播 | GossipSub | AgentCard 广播、任务公告 |
| 状态同步 | CRDT | 任务状态、信誉更新 |
| 数据冗余 | Reed-Solomon 纠删码 | 3 副本跨区域冗余 |

---

## 运行逻辑

### Agent 发现流程

```
Agent A 发布 AgentCard
      │
      ▼
本地签名 + 存储
      │
      ▼
计算分片：shard_of(did, num_shards)
      │
      ▼
写入对应分片 DHT
      │
      ▼
GossipSub 广播摘要
      │
      ▼
链上锚定 CID 哈希

Agent B 查找 Agent
      │
      ▼
DHT 查询 capability
      │
      ▼
遍历该能力所有分片
      │
      ▼
获取 AgentCard 列表
      │
      ▼
验证链上锚定
      │
      ▼
返回可信 Agent 列表
```

### 任务执行流程

```
任务发起
      │
      ▼
DHT 查找符合能力的 Agent
      │
      ▼
任务广播（GossipSub）
      │
      ▼
Agent 接单 → 状态：ASSIGNED
      │
      ▼
Agent 执行 → 状态：RUNNING
      │
      ▼
Agent 提交结果 → 状态：COMPLETED
      │
      ▼
验证者验证 → 状态：VERIFIED
      │
      ▼
链上结算 → 状态：SETTLED
      │
      ▼
信誉更新（CRDT 同步）
```

### 对等网络自愈

```
节点离线
      │
      ▼
心跳超时（30s）
      │
      ▼
DHT 路由表标记不可达
      │
      ▼
邻居节点接管其分片
      │
      ▼
任务队列自动重派
      │
      ▼
节点上线后 CRDT 合并
      │
      ▼
自动恢复为正常节点
```

---

## 版本谱系

### 大版本概览

| 大版本 | 代号 | 核心主题 | 内容 |
|--------|------|----------|------|
| **v1.0.0** | Genesis | 基础协议 | AgentCard/Task 数据模型 + 内存 DHT + 分片索引 |
| **v2.0.0** | Shard | 分片扩展 | 多级分片 + 地理位置 + 容量预测 + 自动分裂 |
| **v2.1.0** | Bridge | 跨链桥接 | CCIP + LayerZero + 信誉同步 + 结算分账 + 保险池 |
| **v2.2.0** | Mesh | 对等网络 | Rust 核心层 + libp2p + 多模式终端 + 根种子 |

### 完整版本列表（19 个）

| # | 版本 | 类型 | 核心特性 | 开发日志 |
|---|------|------|----------|----------|
| 1 | v1.0.0 | 🟢 大版本 | 基础分片索引 + DHT 聚合 + AgentCard/Task 模型 | [详情](releases/v1.0.0.md) |
| 2 | v2.0.0 | 🟢 大版本 | 多级分片 + 地理位置分片 + 分片合并 | [详情](releases/v2.0.0.md) |
| 3 | v2.0.1 | 🔵 小版本 | 区域桥接 + 动态区域 + 容量预测 | [详情](releases/v2.0.1.md) |
| 4 | v2.0.2 | 🔵 小版本 | predictor 七模块包（趋势/变点/季节/残差/置信/融合/报告） | [详情](releases/v2.0.2.md) |
| 5 | v2.0.3 | 🟡 修复 | shard_of 哈希均匀分布修复 + 分布测试 | [详情](releases/v2.0.3.md) |
| 6 | v2.0.4 | 🟡 修复 | ShardMetadata counts 统一写入，消除不一致 | [详情](releases/v2.0.4.md) |
| 7 | v2.0.5 | 🔵 小版本 | get_shard_stats() 运维查询接口 | [详情](releases/v2.0.5.md) |
| 8 | v2.0.6 | 🔵 小版本 | 分片过载自动翻倍分裂（80% 阈值，上限 16） | [详情](releases/v2.0.6.md) |
| 9 | v2.1.0 | 🟢 大版本 | 多链桥接：ReputationBridge + SettlementBridge + Router + Insurance + GovernorToken | [详情](releases/v2.1.0.md) |
| 10 | v2.1.1 | 🔵 小版本 | ReputationBridge/SettlementBridge 完善 + Foundry 测试 | [详情](releases/v2.1.1.md) |
| 11 | v2.1.2 | 🔵 小版本 | CrossChainRouter：速率限制 + 熔断器模式 | [详情](releases/v2.1.2.md) |
| 12 | v2.1.3 | 🔵 小版本 | BridgeInsurance：质押保险池 + 赔付机制 | [详情](releases/v2.1.3.md) |
| 13 | v2.1.4 | 🔵 小版本 | GovernorToken：ERC20 + 信誉注册表 + 治理投票 | [详情](releases/v2.1.4.md) |
| 14 | v2.1.5 | 🟣 重构 | CrossChainMessageBase 抽象基合约，消除重复代码 | [详情](releases/v2.1.5.md) |
| 15 | v2.1.6 | 🟡 修复 | syncReputation 补 MAX_REPUTATION = 10000 校验 | [详情](releases/v2.1.6.md) |
| 16 | v2.2.0 | 🟢 大版本 | Rust 核心层：libp2p + DHT + GossipSub + 多模式终端 + 根种子 | [详情](releases/v2.2.0.md) |
| 17 | v2.2.1 | 🟡 审计 | 全局代码审计：查重/查错/查漏，34/34 测试全绿 | [详情](releases/v2.2.1.md) |
| 18 | v2.2.2 | 🟣 集成 | Foundry + Rust + Python 三栈重建，端到端验证 | [详情](releases/v2.2.2.md) |
| 19 | **v2.2.3** | 🏁 最终版 | **全局审计最终版，34/34 测试全绿** | [详情](releases/v2.2.3.md) |

---

## 测试

| 栈 | 测试数 | 状态 |
|------|--------|------|
| Foundry (Solidity) | 17 | ✅ passed |
| Rust (gsn-core) | 10 | ✅ passed |
| Python (aip-sdk-py) | 7 | ✅ passed |
| **Total** | **34** | **✅ all green** |

## 技术栈

- **Rust**: libp2p 0.54, tokio, ed25519-dalek, rusqlite, uniffi
- **Python**: asyncio, dataclasses, pytest
- **Solidity**: Foundry, OpenZeppelin, CCIP, LayerZero
- **协议**: Kademlia DHT, GossipSub, Ed25519, DID, CRDT

## 许可证

MIT License
