# Agent Universe 版本发布记录

## 版本谱系总览

```
v1.0.0 (Genesis)
  └── v2.0.0 (Shard)
        ├── v2.0.1 ~ v2.0.6 (6 个小版本)
        └── v2.1.0 (Bridge)
              ├── v2.1.1 ~ v2.1.6 (6 个小版本)
              ├── v2.1.7 (Mainnet - 主网上线)
              ├── v2.1.8 (Client - 轻客户端)
              └── v2.1.9 (Mesh - 全对等网络)
                    └── v2.2.0 (Mesh+)
                          ├── v2.2.1 (Audit - 全局审计)
                          ├── v2.2.2 (Rebuild - 三栈重建)
                          ├── v2.2.3 (Final - 最终版)
                          └── v2.3.1 (Swarm - 群体智能) ← 当前
```

## 大版本详情

### v1.0.0 - Genesis（创世纪）

- 项目初始化
- 基础架构搭建
- Rust 核心库骨架

### v2.0.0 - Shard（分片）

- 分片存储架构
- Kademlia DHT 分片路由
- 数据分片与冗余

### v2.1.0 - Bridge（桥接）

- 跨链桥接架构
- 多链支持（Base + Arbitrum）
- 信誉跨链移植

### v2.1.7 - Mainnet（主网上线）

**核心内容**：Mac mini M4 作为 GSN 主网家庭服务器

**运行逻辑**：
- Mac mini M4 闲置功耗 4W，月电费不到 ¥15
- Base 主网最低 gas 费 0.005 gwei，部署全部合约成本约 $0.06
- launchd 管理服务，KeepAlive 自动重启
- pmset 关闭睡眠，7×24 运行

**系统架构**：
```
Mac mini M4
├── gsn-daemon (AIP 节点)
│   ├── libp2p: 4001 (P2P)
│   ├── HTTP API: 4002
│   └── DHT 分片索引
├── verifier-service (HTTP: 8080)
├── chain-relayer (链上事件监听)
├── dashboard-api (HTTP: 8000)
└── observability
    ├── prometheus: 9090
    ├── alertmanager: 9093
    └── grafana: 3000
```

### v2.1.8 - Client（轻客户端）

**核心内容**：PC / 手机 / 服务器跨平台架构

**系统架构**：
- Rust 核心层（gsn-core）：单一真相源，所有平台共享
- Tauri 桌面壳：Windows / macOS / Linux
- Flutter 移动壳：Android / iOS
- headless daemon：Linux / Docker

**技术决策**：
- Tauri 2：桌面端不可替代，Rust 后端天然同构
- Flutter：移动端全平台覆盖最成熟
- UniFFI：自动生成 Swift/Kotlin 绑定

### v2.1.9 - Mesh（全对等网络）

**核心内容**：macOS 端升级为全对等网络架构

**运行逻辑**：
- Mac mini 是第一个节点，不是中心服务器
- 根种子启动后自动降级为普通节点
- 所有终端在协议层完全对等
- DHT 路由表自愈，无单点故障

**技术融合**：
- 区块链：链上信任锚（身份、质押、结算、治理）
- BT：文件与 Agent 的分发（做种、磁力链）
- DHT：统一的发现层（Kademlia）
- CRDT：状态同步
- 纠删码：数据冗余

**节点模式**：Archive / Full / Light / Edge / Browser

### v2.2.0-v2.2.3 - 网状网络与审计

- v2.2.0: 网状网络增强
- v2.2.1: 全局审计（查重、查错、查漏）
- v2.2.2: 三栈重建（Rust + Python + Solidity）
- v2.2.3: 最终版

### v2.3.1 - Swarm（群体智能）

**核心内容**：群体智能 = 网络结构的 Scaling Law

**核心理念**：
- 大模型 Scaling Law 已实现智能涌现（解决智能有无问题）
- 智能体宇宙 = 网络结构的 Scaling Law
- 从 token 网络结构向智能体网络结构演进
- 群众路线：让底层人民参与、贡献、分享智能科技革命
- 单个大模型已撞 Scaling Law 规模墙，需要结构相变

**新增模块**：

1. **swarm/** - 群体智能层
   - `collective.rs`: Swarm / AgentNode / CollectiveDecision
   - `emergence.rs`: EmergenceDetector 涌现检测
   - `consensus.rs`: LightweightConsensus 轻量级共识

2. **economy/** - 信誉经济系统
   - `reputation.rs`: ReputationSystem 信誉分 + 时间衰减
   - `contribution.rs`: ContributionProof 贡献证明
   - `pricing.rs`: TaskPricing 动态定价

3. **scheduler/** - 任务调度器
   - `router.rs`: TaskRouter 智能路由
   - `load_balancer.rs`: LoadBalancer 负载均衡

4. **topology/** - 网络拓扑
   - `neighbor.rs`: NeighborManager k-bucket
   - `graph.rs`: TopologyGraph 拓扑图 + BFS

5. **proof/** - 贡献证明
   - `poc.rs`: ProofOfContribution 贡献证明

**修复清单**：
- erasure decode：真正的 Reed-Solomon 解码
- pocv verify_proof：真实的 ProofOfComputation 验证
- reputation decay：拼写错误修复
- load_balancer：类型不匹配修复

**测试基线**：40/40 通过

## 小版本更新日志

### v2.0.1-v2.0.6

| 版本 | 更新内容 |
|------|----------|
| v2.0.1 | DHT 分片优化 |
| v2.0.2 | Kademlia 路由表改进 |
| v2.0.3 | 数据冗余策略优化 |
| v2.0.4 | 存储层性能提升 |
| v2.0.5 | 纠删码参数调优 |
| v2.0.6 | 分片均衡算法改进 |

### v2.1.1-v2.1.6

| 版本 | 更新内容 |
|------|----------|
| v2.1.1 | 跨链消息格式标准化 |
| v2.1.2 | 桥接状态同步优化 |
| v2.1.3 | 节点发现协议改进 |
| v2.1.4 | 信誉跨链移植机制 |
| v2.1.5 | 桥接故障恢复 |
| v2.1.6 | 多链配置管理 |

## 项目实现

### 核心能力

- ✅ DID 身份系统
- ✅ Kademlia DHT 分片路由
- ✅ GossipSub 消息广播
- ✅ AgentCard 身份卡
- ✅ 任务生命周期管理
- ✅ PoCV 可验证计算
- ✅ CRDT 状态同步
- ✅ Reed-Solomon 纠删码
- ✅ 节点模式管理
- ✅ **群体智能层**（v2.3.1）
- ✅ **信誉经济系统**（v2.3.1）
- ✅ **任务调度器**（v2.3.1）
- ✅ **网络拓扑**（v2.3.1）
- ✅ **贡献证明**（v2.3.1）

### 技术栈

- Rust 2021 Edition
- libp2p（Kademlia + GossipSub）
- Ed25519 + SHA256
- SQLite + DHT
- Solidity（Base 主网）
- Mac mini M4 部署

### 部署架构

- 主网服务器：Mac mini M4（6W 功耗，月电费 ¥8-15）
- 轻客户端：Tauri 桌面 + Flutter 移动
- 浏览器：WASM + WebSocket
- 节点模式：Archive / Full / Light / Edge / Browser

## 项目目的

让智能不再被巨头垄断，让普通人也能参与、贡献、分享这场智能科技革命。

## 项目意义

- **打破垄断**：在闭源与开源之间探索第三条路
- **群众路线**：类似于 Linux，让全民参与 AGI
- **结构突破**：从 token 网络到智能体网络的结构相变
- **普惠智能**：让底层人民也能享受智能科技红利

---

**Agent Universe · 智能体宇宙**

*让科技造福全人类！*
