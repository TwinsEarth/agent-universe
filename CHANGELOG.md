# Changelog

本文件记录 Agent Universe 各版本的重要变更。

## [v2.5.5] - 2026-09-26

### 新增：Relay 节点池管理 + 多通道智能切换

- **Relay 节点池**：新增 `relay_pool/mod.rs`，容量策略 1 万起步、每月 +1 万、硬上限=运行年限×10 万；四类分类（专用/自有/第三方/通用）优先级选择；每小时巡检、DHT 发现、失败 3 次标记 dead 并清理。
- **多通道**：`net/peer.rs` 多 relay 通道，`DEFAULT_PARALLEL_RELAYS = 3`；QUIC/UDP 与 TCP 共存；`ensure_channels` 掉线秒级自动补全。
- **网络 API**：新增 `/api/v1/network/relays*`（get/add/remove/expand/discover/ensure）。
- **真机验证**：Mac↔Windows 经公共 relay 互见；remove 一条后 ensure 自动补新 relay、互见维持；lib 单测 61、集成 10 通过。

## [v2.5.4] - 2026-09-26

### 新增：NAT 穿透增强（Traversal）

真实 libp2p 层加入 Circuit Relay v2、AutoNAT、DCUtR 打洞、Ping 保活，跨不同 NAT 节点经公共中继互连。

## [v2.5.3] - 2026-09-26

### 新增：Mesh 自组网

新增 `mesh/` 模块：心跳（Online/Suspicious/Offline 三态）、广播、嗅探、会话；自组网分配临时 SN 并绑定永久身份识别码。

## [v2.5.2] - 2026-09-26

### 新增：网络分区感知与自动降级

国内/国外模型分两个独立网络组进程隔离运行；NetworkRouter 按网络可达性路由；外网不可达时自动降级为仅国内模型协商投票。

## [v2.5.1] - 2026-09-26

### 新增：国内六大模型统一适配

Kimi（月之暗面）、通义千问（阿里）、智谱 GLM、MiniMax、腾讯混元（元宝）、小米 MiLM，统一接入 `llm/domestic.rs` 与异构协商系统。

## [v2.5.0] - 2026-09-26

### 新增：豆包 / 火山引擎方舟适配

新增 `llm/doubao.rs`，支持 doubao-seed-2-1-pro/turbo 深度思考模型、最高 1024K 上下文。

## [v2.4.9] - 2026-09-26

### 新增：OpenAI / Gemini / Anthropic 适配

新增 `llm/openai.rs`、`llm/gemini.rs`、`llm/anthropic.rs`，各自精确复刻官方 API 格式，统一接入异构 LLM 协商。

## [v2.4.8] - 2026-09-26

### 新增：DeepSeek 模型适配层

新增 `deepseek/`（adapter/protocol/recipe/tokenizer），精确复刻官方提示词编码、多协议格式互转、token 级编解码与上下文预算。

## [v2.4.7] - 2026-09-26

### 新增：异构 LLM 多智能体

新增 `collaboration/hetero_llm.rs`：跨智能体协同、独立推理后评估/评分/内部投票、跨模型记忆协作。

## [v2.4.6] - 2026-09-26

### 新增：群体智能飞轮

`memory/flywheel.rs`：跨模型协同（误差独立性）→ 结构决定增长曲线 → 三层记忆后训练 → 经验回流优化结构 → 再协同闭环。

## [v2.4.5] - 2026-09-26

### 新增：交接协议 / 审计 / 可信度

`memory/handoff.rs` TransferBundle 六字段机械校验；TraceLedger 哈希链审计轨迹；证据三级标签随数据流动。

## [v2.4.4] - 2026-09-26

### 新增：个体记忆增强

`memory/enhanced.rs`：多模态标签/关键词/向量检索、记忆评价防污染、LRU 压缩与遗忘。

## [v2.4.3] - 2026-09-26

### 新增：三层记忆共享

`memory/layered.rs`、`memory/shared_memory.rs`：个体/群体/跨代记忆；分层主体记忆（Lv1–Lv7）+ 跨代记忆哈希链。

## [v2.4.2] - 2026-09-26

### 新增：个体 + 群体记忆

`memory/agent_memory.rs`：个体内部记忆库 + 外部共享记忆库。

## [v2.4.1] - 2026-09-26

### 新增：Lv1–Lv7 分层拓扑

`topology/layer.rs`、`topology/router.rs`：分层拓扑应对通信墙，扇入有界 ≤9、边数亚二次、路由 ≤7 跳；不可达降级回扁平。

## [v2.4.0] - 2026-09-26

### 新增：CPU 治理轻量化

结算守恒改 O(1) 增量维护；Sandbox 抽象为 trait（Docker / Firecracker microVM）。面向智能体 CPU 密集负载，治理开销 O(1)、不扫全表。

## [v2.3.6] - 2026-09-24

### 新增：MCP/ACA 深化重构 + 三端跨语言可信对齐

**MCP 真实化**：工具字段改为规范 camelCase（`inputSchema`、`mimeType`），补齐 `ping` 与标准 `initialize`/`notifications/initialized`，`tools/call` 经 `MarketMcpBridge` 真实路由（替换占位门面），注册 18 个 `market_*` 工具；stdio 与 HTTP/SSE 双传输真机走通。

**ACA 补全**：新增 `crypto`（Ed25519 + 规范签名）与 `runtime`，manifest/message/receipt 统一加签，信誉时间衰减固化，verification 增加 BFT 委员会规格（`n ≥ 3f + 1`）。

**跨语言可信对齐**：新增 Rust 测试 `cross_lang_signature.rs`，固定种子下 Rust/Python/JS 的公钥、DID、规范载荷、签名逐字节一致、可互验、篡改即拒；身份统一为 `did:aip:<sha256(原始公钥)前8字节>`。Python 新增 `crypto/aca/market_client/mcp_client`，JS 新增 `lib/aca.js`、`lib/mcp.js`。

**真机验证**：daemon 真实打开 SQLite、绑定 P2P/API 端口；市场端到端闭环结算 amount=50、守恒 conserved=True；MCP 三端 initialize/ping/tools-list/tools-call 真机通过；CLI version/identity/market stats/mcp stdio 真机通过。

**测试**：Rust **155** / Python **17** / JS **12**，全部通过。

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
