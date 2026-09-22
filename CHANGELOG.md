# v2.2.3 全局审计版

## 修复（15项）

### 查重
- 提取 CrossChainMessageBase 抽象基合约
- 消除 ReputationBridge/SettlementBridge 重复代码

### 查错
- Python Task.status 裸字符串改 TaskStatus 枚举
- sharded.py counts 统一写入 ShardMetadata
- ReputationBridge.syncReputation 补 MAX_REPUTATION 校验
- Rust unused warnings 清理

### 查漏
- 新增 gossip.rs/transport.rs 桩文件
- 新增 get_shard_stats()
- 分片过载自动翻倍分裂（上限16）
- 签名负例测试

## 测试结果

```
Foundry:  17/17 passed
Rust:     10/10 passed
Python:    7/7 passed
Total:    34/34 passed
```
