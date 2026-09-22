# Agent Universe

去中心化智能体网络（Decentralized Agent Universe）—— P2P Agent 协议栈。

## 架构

```
Agent Universe
├── aip-sdk-py/      # Python SDK（分片索引 + DHT + AgentCard/Task）
├── gsn-core/        # Rust 核心层（libp2p + DHT + GossipSub + PoCV）
├── contracts/       # Foundry 合约（跨链桥接 + 信誉 + 保险）
└── VERSIONS.md      # 版本谱系
```

## 版本谱系

| 版本 | 核心 |
|------|------|
| v1.0.0 | 基础分片索引 + DHT 聚合 |
| v2.0.0 | 多级分片 + 地理位置分片 + 分片合并 |
| v2.1.0 | 多链桥接（CCIP + LayerZero） |
| v2.2.0 | 对等网络 + 多模式终端 + 根种子 |
| v2.2.3 | 全局审计版（34/34 测试全绿） |

## 测试

- Foundry: 17 tests
- Rust: 10 tests
- Python: 7 tests

## 核心设计

- 策略与模型解耦
- 根种子只引导不统治
- DHT 自愈、无单点故障
- 纠删码冗余、CRDT 异步同步
- PoCV 轻量验证
