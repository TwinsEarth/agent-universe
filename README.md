# Agent Universe

> 去中心化智能体网络 —— P2P Agent 协议栈

Agent Universe（GSN, Global Agent Network）是一个去中心化的智能体网络协议栈，实现了 Agent 的发现、验证、结算和跨链协作。基于 libp2p + DHT + 区块链，构建"人人为我，我为人人"的智能体经济。

## 架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Agent Universe v2.2.3                    │
├─────────────────┬─────────────────┬─────────────────────────┤
│   contracts/   │    gsn-core/    │     aip-sdk-py/         │
│   (Foundry)    │     (Rust)     │      (Python)           │
├─────────────────┼─────────────────┼─────────────────────────┤
│ ReputationBridge│ net/           │ models.py               │
│ SettlementBridge│   libp2p       │   AgentCard             │
│ CrossChainRouter│   DHT          │   Task                  │
│ BridgeInsurance │   GossipSub    │   TaskStatus            │
│ CrossChainMsgBase│ identity/     │ index/                  │
│ GovernorToken   │   DID          │   ShardedIndex          │
│                 │   Ed25519      │   ShardMetadata         │
│                 │ agent/         │ dht_backend.py          │
│                 │   Task FSM     │   MemoryDHT             │
│                 │ chain/        │                         │
│                 │   PoCV         │                         │
└─────────────────┴─────────────────┴─────────────────────────┘
```

## 核心特性

- **P2P 发现**：libp2p + Kademlia DHT，分片索引自动分裂
- **跨链桥接**：CCIP + LayerZero，信誉跨链同步
- **对等网络**：根种子只引导不统治，无单点故障
- **多模式终端**：Full / Light / Mobile / Browser
- **PoCV 验证**：轻量可验证计算
- **34/34 测试全绿**：Foundry 17 + Rust 10 + Python 7

## 快速开始

### Python SDK

```bash
cd aip-sdk-py
pip install -e .
```

```python
from aip.models import AgentCard, Task, TaskStatus
from aip.dht_backend import MemoryDHT
from aip.index.sharded import ShardedIndex

# 创建 Agent
card = AgentCard.new("did:aip:agent-001", "my-agent")
card.with_capability("text.translate")

# 发布到 DHT
dht = MemoryDHT()
index = ShardedIndex(dht, peer_id="peer-a")
await index.publish_card(card)

# 查找 Agent
agents = await index.find_by_capability("text.translate")
print(agents)  # ['peer-a']
```

### Rust Core

```bash
cd gsn-core
cargo build --release
cargo test
```

### Foundry Contracts

```bash
cd contracts
forge build
forge test
```

## 版本谱系（19 个 Release）

| # | 版本 | 类型 | 核心特性 | 开发日志 |
|---|------|------|----------|----------|
| 1 | v1.0.0 | 大版本 | 基础分片索引 + DHT 聚合 | [详情](releases/v1.0.0.md) |
| 2 | v2.0.0 | 大版本 | 多级分片 + 地理位置分片 + 分片合并 | [详情](releases/v2.0.0.md) |
| 3 | v2.0.1 | 小版本 | 区域桥接 + 动态区域 + 容量预测 | [详情](releases/v2.0.1.md) |
| 4 | v2.0.2 | 小版本 | predictor 七模块包 | [详情](releases/v2.0.2.md) |
| 5 | v2.0.3 | 小版本 | shard_of 哈希均匀分布修复 | [详情](releases/v2.0.3.md) |
| 6 | v2.0.4 | 小版本 | ShardMetadata counts 统一写入 | [详情](releases/v2.0.4.md) |
| 7 | v2.0.5 | 小版本 | get_shard_stats() 查询接口 | [详情](releases/v2.0.5.md) |
| 8 | v2.0.6 | 小版本 | 分片过载自动翻倍分裂（上限16） | [详情](releases/v2.0.6.md) |
| 9 | v2.1.0 | 大版本 | 多链桥接（CCIP + LayerZero） | [详情](releases/v2.1.0.md) |
| 10 | v2.1.1 | 小版本 | ReputationBridge + SettlementBridge | [详情](releases/v2.1.1.md) |
| 11 | v2.1.2 | 小版本 | CrossChainRouter 路由层 | [详情](releases/v2.1.2.md) |
| 12 | v2.1.3 | 小版本 | BridgeInsurance 保险池 | [详情](releases/v2.1.3.md) |
| 13 | v2.1.4 | 小版本 | GovernorToken ERC20+信誉 | [详情](releases/v2.1.4.md) |
| 14 | v2.1.5 | 小版本 | CrossChainMessageBase 抽象基合约 | [详情](releases/v2.1.5.md) |
| 15 | v2.1.6 | 小版本 | syncReputation MAX_REPUTATION 校验 | [详情](releases/v2.1.6.md) |
| 16 | v2.2.0 | 大版本 | 对等网络 + 多模式终端 + 根种子 | [详情](releases/v2.2.0.md) |
| 17 | v2.2.1 | 小版本 | 全局代码审计修复 | [详情](releases/v2.2.1.md) |
| 18 | v2.2.2 | 小版本 | Foundry+Rust+Python 三栈重建 | [详情](releases/v2.2.2.md) |
| 19 | **v2.2.3** | **最终版** | **全局审计版，34/34 测试全绿** | [详情](releases/v2.2.3.md) |

## 测试

| 栈 | 测试数 | 状态 |
|------|--------|------|
| Foundry | 17 | ✅ passed |
| Rust | 10 | ✅ passed |
| Python | 7 | ✅ passed |
| **Total** | **34** | **✅ all green** |

## 技术栈

- **Rust**: libp2p 0.54, tokio, ed25519-dalek, rusqlite
- **Python**: asyncio, dataclasses
- **Solidity**: Foundry, OpenZeppelin, CCIP, LayerZero
- **协议**: Kademlia DHT, GossipSub, Ed25519, DID

## 许可证

MIT License
