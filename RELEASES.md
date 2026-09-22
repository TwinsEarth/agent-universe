# Agent Universe Releases

19 个版本按顺序发布，每个版本对应一个 commit。

## 版本谱系

| # | 版本 | Commit | 核心特性 |
|---|------|--------|----------|
| 1 | v1.0.0 | `fce9960` | 基础分片索引 + DHT 聚合 + AgentCard/Task 模型 |
| 2 | v2.0.0 | `758aae1` | 多级分片 + 地理位置分片 + 分片合并 |
| 3 | v2.0.1 | `3b39512` | 区域桥接 + 动态区域 + 容量预测 |
| 4 | v2.0.2 | `aeb48d7` | predictor 七模块包 |
| 5 | v2.0.3 | `339f177` | shard_of 哈希均匀分布修复 |
| 6 | v2.0.4 | `d9230da` | ShardMetadata counts 统一写入 |
| 7 | v2.0.5 | `03e6856` | get_shard_stats() 查询接口 |
| 8 | v2.0.6 | `52682c4` | 分片过载自动翻倍分裂（上限16） |
| 9 | v2.1.0 | `cf1c47b` | 多链桥接（CCIP + LayerZero） |
| 10 | v2.1.1 | `d1356f3` | ReputationBridge + SettlementBridge |
| 11 | v2.1.2 | `920a4f4` | CrossChainRouter 路由层 |
| 12 | v2.1.3 | `47fc8ea` | BridgeInsurance 保险池 |
| 13 | v2.1.4 | `1243941` | GovernorToken ERC20+信誉 |
| 14 | v2.1.5 | `6d1c3a0` | CrossChainMessageBase 抽象基合约 |
| 15 | v2.1.6 | `553fd76` | syncReputation MAX_REPUTATION 校验 |
| 16 | v2.2.0 | `03c011a` | 对等网络 + 多模式终端 + 根种子 |
| 17 | v2.2.1 | `0fa3f53` | 全局代码审计修复 |
| 18 | v2.2.2 | `32ef645` | Foundry+Rust+Python 三栈重建 |
| 19 | v2.2.3 | `2f62d52` | 全局审计版（34/34 测试全绿） |

## 三栈测试

- Foundry: 17/17 passed
- Rust: 10/10 passed
- Python: 7/7 passed
- **Total: 34/34 passed**

## 核心架构

```
Agent Universe
├── Python SDK (aip-sdk-py)
│   ├── models: AgentCard, Task, TaskStatus
│   ├── index: ShardedIndex, ShardMetadata
│   └── dht_backend: MemoryDHT
├── Rust Core (gsn-core)
│   ├── net: libp2p, DHT, GossipSub
│   ├── identity: DID, Ed25519
│   └── agent: Task state machine
└── Contracts (Foundry)
    ├── ReputationBridge
    ├── SettlementBridge
    ├── CrossChainRouter
    ├── BridgeInsurance
    └── GovernorToken
```
