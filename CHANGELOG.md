# Changelog

本文件记录 Agent Universe 各版本的重要变更。

## [v2.3.5] - 2026-09-24

### 新增：跨平台客户端 + 三大连接层重构

**跨平台客户端（Tauri 2）**：macOS / Windows / Android / Linux 客户端源码与安装包（`client/`、`desktop/`）。

**重新梳理、重构、补全 CLI / REST API / MCP 三大连接层**：

- **架构**：市场层改为 actor + mpsc（`MarketActorHandle` 独占 `AgentMarket`），HTTP/MCP/CLI 经 oneshot 通道发命令，避免共享 `Mutex` 在异步事件循环中死锁；
- **CLI**：新增独立子命令式二进制 `gsn`（`version` / `identity` / `daemon` / `mcp` / `market`），`market` 子命令经 HTTP API 连接运行中的节点；守护进程启动逻辑提取为库函数 `node::run_daemon`，`gsn-daemon` 与 `gsn daemon` 共用；
- **REST API**：补全 `/api/v1/*` 完整市场端点（注册/发现/发布/投标/匹配/结果/验证/结算/争议/守恒/排行/统计），与传输解耦的 `route`，旧版 `/agents` `/tasks` 双兼容；业务错误 422、不存在 404、创建 201、OPTIONS 204；
- **MCP**：注册 18 个 `market_*` 工具，`tools/call` 路由到 actor 真实执行（替换原占位响应）；新增 stdio 传输（JSON-RPC 逐行读写、日志走 stderr、通知不响应）与 HTTP/SSE 传输（`POST` 无状态、`GET` event-stream）；
- **注册简化**：新增 `RegisterAgentInput`，调用方只需提供少数字段，actor 内填充版本/时间戳/SLA/证据等级等默认值转完整 `MarketAgentCard`；
- **测试**：新增三层专项集成测试 9 个，全量 **153 passed / 0 failed**，构建零警告；
- **修复**：`POST /api/v1/agents` 此前未区分 GET/POST 而误走统计分支，已修复。

## [v2.3.4] - 2026-09-23

### 新增：智能体市场 Agent Market

智能体宇宙的第一版经济层，让智能体、技能、任务、算力完成完整交易闭环：

- **AgentCard 注册**：质押准入（最低 100），能力声明 + Ed25519 签名
- **TaskSpec**：六字段校验（goal/context/done/todo/trace/owner）
- **匹配引擎**：技能/信誉/负载过滤，性价比排序
- **BFT-lite QA**：n≥3f+1，equivocation 整轮作废，view change
- **结算守恒**：`balance_sum = budget - slashed`，防重复支付
- **多维信誉**：quality/speed/honesty/availability，不可转让
- **质押罚没**：作恶扣除质押，信誉同步下降
- **证据分级**：verified / cpu-proto / unverified
- **争议仲裁**：基于 TraceLedger 哈希链的只读仲裁

### 测试

- 新增 61 个市场模块测试
- 全项目 144 测试全部通过
- 端到端演示：注册→匹配→BFT验证→结算→仲裁→罚没

## [v2.3.3] - 2026-09-23

### 新增：P2P 分布式网络应用

- Agent 协作与安全通信分层架构
- 分布式推理与算力调度
- 去中心化任务众包
- NAT 穿透（STUN/TURN/UDP打洞）
- DHT 路由优化
- 信任与激励机制

## [v2.3.2] - 2026-09-23

### 新增：MCP 和 ACA 兼容 API

- **MCP 兼容层**：`mcp/` 模块，支持工具调用、资源、提示词
- **ACA 兼容 API**：`aca/` 模块，信封/清单/消息/收据/信誉/验证
- 八层协议栈完整定义
- 四个协议对象：Agent Manifest / Task Envelope / Receipt / Reputation
- 五级验证分层（L0-L4）

## [v2.3.1] - 2026-09-23

### 新增：群体智能 Swarm

- **群体智能层**（`swarm/`）：涌现检测、轻量共识、集体决策
- **信誉经济系统**（`economy/`）：信誉分时间衰减、贡献证明、动态定价
- **任务调度器**（`scheduler/`）：智能路由、负载均衡
- **网络拓扑**（`topology/`）：k-bucket 邻居管理、拓扑图 BFS
- **贡献证明**（`proof/`）：Proof of Contribution

### 修复

- erasure decode：真正的 Reed-Solomon 解码
- pocv verify_proof：真实 Proof of Computation 验证
- reputation decay：时间衰减拼写修复

## [v2.3.0] - 2026-09-23

### 新增：P2P 分布式网络基础

- libp2p 节点初始化
- Kademlia DHT 客户端
- GossipSub 消息广播
- 根种子节点（root_seed）
- 节点模式管理（Archive/Full/Light/Edge/Browser）

## [v2.2.0] - 2026-09-22

### 新增：全对等网络

- 所有终端协议层完全对等
- DHT 路由表自愈
- CRDT 状态同步
- Reed-Solomon 纠删码冗余
- 跨区域副本策略

## [v2.1.0] - 2026-09-22

### 新增：跨链桥接与经济层

- 跨链桥接架构
- Base + Arbitrum 多链支持
- 信誉跨链移植
- PoCV 结算合约
- AgentCard 链上锚定

## [v2.0.0] - 2026-09-22

### 新增：分片存储与 DHT

- Kademlia DHT 分片路由
- 数据分片与冗余
- 存储层抽象
- SQLite 本地持久化

## [v1.0.0] - 2026-09-22

### 初始版本

- 项目初始化
- Rust 核心库骨架
- DID 身份系统
- AgentCard 身份卡
- 任务生命周期管理
