# Changelog

本文件记录 Agent Universe 各版本的重要变更。

## [v2.5.8] - 2026-09-27

### 修复：精确整数账本（Money），根治 f64 跨语言守恒失效（GAP §2.1–2.3）

- **Rust 引入 `Money(i64)` 整数金额**：新增 `marketplace/money.rs`（newtype，`#[serde(transparent)]`；刻意不实现 `Add/Sub`，强制走 `checked_add/checked_sub` 防溢出）；账本、质押、托管、结算、罚没全程 `Money`，`conservation_check` 改为**精确相等、无任何容差**（旧版 f64：Rust 容差 0.001、JS 容差 1e-9，相差六个数量级，跨语言必然失效）。
- **发布即托管，杜绝凭空铸币**：发布任务时预算从需求方转入托管账户 `__escrow__:{task}`（钱仍在系统内，任意时刻守恒）；结算时从托管账户支付给执行者、余款退回需求方；付款方余额不足一律 `Err`，不再像旧版那样在余额不足时凭空铸币完成支付。
- **JS SDK 整数化并对齐托管模型**：`market.js` 新增 `_assertMoney`（拒绝非整数、非安全整数），在充值/质押/发布/投标/罚没各入口校验，投标与罚没额外拒绝 `<= 0`（GAP §2.7 出价 0/负反而最优）；新增 `__escrow__` 托管账户（旧版 JS 发布时预算只从需求方扣除、未进任何账户，中途不守恒），守恒改为 `===` 精确相等。
- **Python SDK 类型对齐**：`market_client.py` 的 `budget` / `deposit amount` / `slash_amount` 由 `float` 改为 `int`。
- **跨语言金额向量**：新增 `conformance/money-vectors.json`（权威单一来源），Rust 测试 `test_money_vector_matches_conformance` 与 JS 测试读取同一向量，对同一市场场景的**逐账户余额与聚合守恒报告逐值一致**。

### 验证

- Rust：lib 125、v234 66、v235 10，0 failed；`market_demo` 端到端两场景守恒成立（场景一 充值600/支付8/罚0/余额600；场景二 充值600/支付8/罚80/余额520，精确）。JS SDK 14 项通过（含跨语言金额向量）。

## [v2.5.7] - 2026-09-27

### 新增：跨实现身份一致性（Cross-Implementation Identity）

- **统一三端 DID 派生口径**：`fingerprint = hex(SHA256(原始 32 字节公钥)[..8])`（16 hex）。Rust（`identity/did.rs`）、JS（`lib/keychain.js`）此前口径不一致（Rust 对原始公钥取 8 字节、前缀 aip；JS 对 SPKI DER 取 16 字节、前缀 au），现统一；新身份用 `did:nau:` 前缀以示区分。
- **跨实现签名向量**：新增 `conformance/generate.mjs`（Node/OpenSSL 独立生成，与 Rust 零共享代码）+ `conformance/vectors.json`；规范载荷签名**逐字节命中上游 gsn-core 测试向量** `e14d3f9e…`，构成真正跨实现校验（非"自己验自己"）。
- **上游身份向后兼容**：`Did::parse` 同时接受 `did:aip:` / `did:nau:`、拒绝其他方法；ACA `register_peer` 改为只比对指纹（与方法无关），声明 aip 的上游对端可正常注册。

### 修复：客户端构建链路（client/desktop）

- **根包补 `/lib/*` 子路径**：新增根 `lib/` 6 个 re-export，根 package.json files 加 `lib`；此前根发布包只含 `js/`，客户端 import `/lib/market.js` 落空（GAP §9.5）。
- **依赖改 `file:..`**：client/desktop 不再从 registry 拉 SDK，直接打包仓库根源码，根治"tag 触发构建时本版本 npm 包尚未发布"的时序竞争；新增 `.npmrc` `install-links=true`（打包成 node_modules 真实拷贝而非 symlink）。
- **import 改 default + 解构**：规避 rollup 对 re-export CJS 命名导出的静态识别失败；client/desktop `vite build` 均验证通过。
- 全仓 bump 到 npm 2.5.7 / gsn-core 0.2.57；补登记 Python SDK 三处版本常量、客户端 index.html 等长期遗漏的版本点。

### 验证

- Rust cargo test 0 failed（v2.5.7 独立快照）；JS SDK 12 项通过；client/desktop Vite build 通过；跨实现签名逐字节命中上游向量。

## [v2.5.6] - 2026-09-27

### 新增：历史 Bug 回归套件（Regression Suite）

- **test/regression.js**：14 条历史 bug 回归（A 版本一致性 4 / B 编译CI守卫 4 / C 市场重复防护 4 / D 资金守恒 2），本地 14/14 通过。
- **gsn-core/tests/regression_net.rs**：随机临时身份构造节点，subscribe→publish 不 panic；InsufficientPeers 判为正常。
- **一键脚本**：test/run-all.sh（Linux/macOS）、test/run-all.ps1（Windows）；test/README.md 用例索引与易踩坑清单。
- **CI 接入**：ci.yml JS job 新增回归步骤，每次 push/PR 强制跑、失败禁止发版。

### 修复

- **bump-version.sh 自身 5 处缺陷**：`$V_` 变量名、sed 正则 `\+` 转义、gsn-core 版本四段误拼、ci.yml sed 空格/转义、新增 client/desktop lib.rs 与 Identify 两条规则。
- 全仓 bump 到 npm 2.5.6 / gsn-core 0.2.56。

### 验证

- JS SDK 12 项、历史回归 14/14、Rust cargo test 0 failed。

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
- 五级验证分层：L0-L4

## [v2.3.1] - 2026-09-23

### 新增：群体智能 Swarm

- **群体智能层**（`swarm/`）：涌现检测、轻量共识、集体决策
- **信誉经济系统**（`economy/`）：信誉分时间衰减、贡献证明、动态定价
- **任务调度器**（`scheduler/`）：智能路由、负载均衡
- **网络拓扑**（`topology/`）：k-bucket 邻居管理、拓扑图 BFS
- **贡献证明**（`proof/`）：Proof of Contribution

### 修复

- erasure decode：真正的 Reed-Solomon 解码
- pocv verify_proof：真实的 Proof of Computation 验证
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
