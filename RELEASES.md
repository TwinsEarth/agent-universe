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
                          └── v2.3.0 (P2P - 分布式网络基础)
                                ├── v2.3.1 (Swarm - 群体智能)
                                ├── v2.3.2 (MCP+ACA - 兼容协议)
                                ├── v2.3.3 (P2P Net - 网络应用)
                                ├── v2.3.4 (Market - 智能体市场)
                                ├── v2.3.5 (Client - 跨平台客户端)
                                └── v2.3.6 (MCP/ACA - 跨语言可信对齐)
                                      └── v2.4.0 (Govern - CPU 治理轻量化)
                                            ├── v2.4.1 (Layer - Lv1–Lv7 分层拓扑)
                                            ├── v2.4.2 (Memory - 个体+群体记忆)
                                            ├── v2.4.3 (3-Tier Memory - 分层主体+跨代记忆)
                                            ├── v2.4.4 (Memory+ - 检索/评价/压缩)
                                            ├── v2.4.5 (Handoff - 交接/审计/证据分级)
                                            ├── v2.4.6 (Flywheel - 群体智能飞轮)
                                            ├── v2.4.7 (Hetero LLM - 异构多模型协商)
                                            ├── v2.4.8 (DeepSeek - DeepSeek 适配)
                                            └── v2.4.9 (Multi-LLM - OpenAI/Gemini/Anthropic)
                                                  └── v2.5.0 (Doubao - 豆包/火山方舟)
                                                        ├── v2.5.1 (CN Models - 国内六大模型)
                                                        ├── v2.5.2 (Net Partition - 网络分区降级)
                                                        ├── v2.5.3 (Mesh - 自组网)
                                                        ├── v2.5.4 (Traversal - NAT 穿透)
                                                        └── v2.5.5 (Relay Pool - 中继池+多通道) ← 当前
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

### v2.3.0 - P2P（分布式网络基础）

**核心内容**：P2P 网络栈基础组件

**新增模块**：
- `net/libp2p_node.rs`：libp2p host 初始化，Noise + Yamux + QUIC
- `net/dht.rs`：Kademlia DHT 客户端
- `net/gossip.rs`：GossipSub 订阅/发布
- `net/root_seed.rs`：根种子节点引导逻辑
- `mode/mod.rs`：五种节点模式（Archive/Full/Light/Edge/Browser）

### v2.3.2 - MCP+ACA（兼容协议）

**核心内容**：MCP 和 ACA 兼容 API 接口与通讯协议

**新增模块**：
- `mcp/`：MCP 兼容层（protocol/server/tool/resource/prompt）
- `aca/`：ACA 兼容 API（envelope/manifest/message/receipt/reputation/verification）

**八层协议栈**：身份→发现→通信→执行→数据→验证→结算→治理

**四个协议对象**：Agent Manifest → Task Envelope → Receipt → Reputation

**五级验证分层**：L0 抽样 → L1 冗余2-of-3 → L2 TEE → L3 zkML → L4 委员会仲裁

### v2.3.3 - P2P Net（分布式网络应用）

**核心内容**：P2P 分布式网络在 Agent 领域的三大应用方向

**三大方向**：
1. Agent 协作与安全通信（分层架构 + 动态组网）
2. 分布式推理与算力调度（闲置 GPU 整合 + 边缘融合）
3. 去中心化任务众包（以工换工 + 隐私优势）

**关键技术**：NAT 穿透（STUN/TURN/UDP打洞）、DHT 路由 O(logN)、信任与激励机制

### v2.3.4 - Market（智能体市场）

**核心内容**：智能体宇宙的第一版经济层

**系统架构**：
```
注册 → 发布 → 匹配 → 执行 → 验证 → 结算 → 信誉更新
  ↑                                      ↓
  └──────────── 争议/仲裁/罚没 ←─────────┘
```

**核心机制**：
| 机制 | 说明 |
|------|------|
| AgentCard 注册 | 质押准入，能力声明 + 签名 |
| TaskSpec | 六字段校验 |
| 匹配引擎 | 技能/信誉/负载过滤，性价比排序 |
| BFT-lite QA | n≥3f+1，equivocation 整轮作废 |
| 结算守恒 | balance_sum = budget - slashed |
| 多维信誉 | quality/speed/honesty/availability，不可转让 |
| 质押罚没 | 作恶扣除质押 |
| 证据分级 | verified / cpu-proto / unverified |

**测试基线**：144/144 通过，端到端演示真实运行

### v2.3.5 - Client（跨平台客户端）

**核心内容**：Tauri 2 跨平台客户端，CLI / REST API / MCP 三大连接层重构

**系统架构**：
```
接入层：CLI(gsn) · REST /api/v1 · MCP · Tauri 客户端
        ↓
MarketActorHandle（actor + mpsc/oneshot，独占 AgentMarket）
        ↓
Registry / Discovery / Matching / QA(BFT-lite) / Settlement
        ↓
libp2p(TCP/Noise/Yamux/Kademlia/GossipSub) + rusqlite 持久化
```

**核心机制**：
| 机制 | 说明 |
|------|------|
| Tauri 客户端 | macOS/Windows/Linux GUI + Android APK，iOS 需开发者证书 |
| 安装包产出 | client-build.yml 打 tag 时自动构建并发布到 GitHub Release |
| CLI | gsn 子命令：version/identity/daemon/mcp/market |
| REST | /api/v1 完整端点，与传输解耦的 route() |
| MCP | 18 个 market_* 工具，tools/call 真实执行（stdio/sse） |
| Actor 模型 | MarketActorHandle 独占市场，消除异步共享 Mutex 死锁 |
| 真实网络 | libp2p 0.54 bind 端口，rusqlite 落盘 ~/.gsn/data |
| 缺陷修复 | 数据目录字面 ~ 不展开（default_data_dir/expand_tilde） |

**测试基线**：Rust 154/154、Python 6/6、JS 8/8 通过；CLI/REST/MCP 三层真机走通完整市场闭环且守恒成立

### v2.3.6 - MCP/ACA 深化重构（跨语言可信对齐）

**核心内容**：MCP 真实化、ACA 身份/签名补全、Rust/Python/JS 三端跨语言可信对齐

**核心机制**：
| 机制 | 说明 |
|------|------|
| MCP 字段规范 | inputSchema/mimeType camelCase，补齐 ping 与标准 initialize |
| MCP 真实路由 | tools/call 经 MarketMcpBridge 真实执行，18 个 market_* 工具 |
| ACA 签名 | 新增 crypto/runtime，manifest/message/receipt 加签可验 |
| 信誉与验证 | 信誉时间衰减固化，BFT 委员会规格 n≥3f+1 |
| 跨语言对齐 | 同种子下三端公钥/DID/载荷/签名逐字节一致、可互验 |
| 统一身份 | did:aip:<sha256(原始32字节公钥)前8字节> |

**测试基线**：Rust **155**、Python **17**、JS **12** 全部通过；MCP 三端真机 initialize/ping/tools-list/tools-call，市场闭环结算 amount=50、守恒 conserved=True

### v2.4.0 - Govern（CPU 治理轻量化）

**核心内容**：面向 CPU 密集型智能体负载，把治理开销压到 O(1)。结算守恒改为增量维护（balance_sum = budget - slashed 单调增量、不扫全表）；Sandbox 抽象为 trait（Docker 共享内核 / Firecracker microVM 两条实现），对齐白皮书「智能体编排占端到端 50–90% CPU」的成本约束。

### v2.4.1 - Layer（Lv1–Lv7 分层拓扑）

**核心内容**：用分层拓扑应对通信墙。从 P2P 扁平结构改为 Lv1 房间 → Lv2 楼宇 → … → Lv7 宇宙七层，扇入有界（≤9）、边数亚二次、路由 ≤7 跳；TopologyRouter 在分层不可达时降级回扁平直连。对应 P0b「星型 β=1 → 分层 β=0」的增长指数结论。

### v2.4.2 ~ v2.4.6 - 记忆体系与飞轮

- **v2.4.2 Memory**：个体内部记忆库 + 群体外部共享记忆库。
- **v2.4.3 3-Tier Memory**：分层主体记忆（Lv1–Lv7 各层一份）+ 跨代记忆哈希链；个体记忆 / 群体记忆 / 跨代记忆三层共享。
- **v2.4.4 Memory+**：个体记忆增强——标签/关键词/向量多模态检索、记忆评价防污染（区分好坏经验）、LRU 压缩与遗忘（不能无限增长）。
- **v2.4.5 Handoff**：TransferBundle 六字段（Goal/Context/Done/Todo/Trace/Owner）机械校验；TraceLedger 哈希链审计轨迹；证据三级标签随数据流动。
- **v2.4.6 Flywheel**：群体智能飞轮——跨模型协同（误差独立性）→ 结构决定曲线 → 三层记忆后训练 → 经验回流优化结构 → 再协同。

### v2.4.7 ~ v2.4.9 - 异构 LLM 与模型适配

- **v2.4.7 Hetero LLM**：异构大模型多智能体——跨智能体协同、独立推理后的评估/评分/内部投票（BFT-lite 委员会）、跨模型记忆协作。
- **v2.4.8 DeepSeek**：deepseek-recipe 适配层——精确复刻官方提示词编码、多协议格式互转、token 级编解码与上下文预算。
- **v2.4.9 Multi-LLM**：OpenAI Chat Completions / Google Gemini / Anthropic Messages 三大后端，各自精确复刻官方 API，统一接入 v2.4.7 协商系统。

### v2.5.0 ~ v2.5.2 - 国产模型与网络分区

- **v2.5.0 Doubao**：豆包 / 火山引擎方舟 Ark 适配（doubao-seed-2-1-pro/turbo，深度思考，最高 1024K 上下文）。
- **v2.5.1 CN Models**：国内六大模型统一适配——Kimi（月之暗面）、通义千问（阿里）、智谱 GLM、MiniMax、腾讯混元（元宝）、小米 MiLM。
- **v2.5.2 Net Partition**：国内/国外模型分两个独立网络组进程隔离运行；NetworkRouter 无论在国内/国外/跳转网络都能正确路由；外网不可达时自动降级为仅国内模型协商投票。

### v2.5.3 - Mesh（自组网增强）

**核心内容**：新增 `mesh/` 模块——心跳（Online/Suspicious/Offline 三态）、广播、嗅探、会话管理；自组网并分配临时 SN 唯一识别码、绑定永久身份识别码。gsn-core 0.2.53。

### v2.5.4 - Traversal（NAT 穿透）

**核心内容**：在真实 libp2p 层（非模拟 nat 模块）加入 Circuit Relay v2、AutoNAT、DCUtR 打洞与 Ping 保活，使分属不同 NAT 的节点经公共中继跨网互连。gsn-core 0.2.54。Mac↔Windows 跨网中继真机验证通过。

### v2.5.5 - Relay Pool（中继池 + 多通道）

**核心内容**：Relay 节点池管理（初始 1 万、每月 +1 万、硬上限=运行年限×10 万；专用/自有/第三方/通用四类；每小时巡检、DHT 发现、失败 3 次标记 dead 并清理）；方案 2/3/4 全部支持、任一失败自动切换，多 relay 多通道默认同时在线 3 条、掉线 ensure 秒级补全。gsn-core **0.2.55**。Mac↔Windows 经公共 relay 互见、remove 一条后 ensure 自动补新 relay、互见维持，三端真机验证通过。详见 [releases/v2.5.5.md](releases/v2.5.5.md)。

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
- ✅ **MCP/ACA 兼容协议**（v2.3.2）
- ✅ **P2P 网络应用**（v2.3.3）
- ✅ **智能体市场 Agent Market**（v2.3.4）
- ✅ **跨平台客户端 + CLI/REST/MCP 重构**（v2.3.5）
- ✅ **MCP/ACA 深化重构 + 三端跨语言可信对齐**（v2.3.6）
- ✅ **CPU 治理轻量化 + Sandbox trait**（v2.4.0）
- ✅ **Lv1–Lv7 分层拓扑**（v2.4.1）
- ✅ **三层记忆共享 + 跨代记忆哈希链**（v2.4.2–v2.4.4）
- ✅ **TransferBundle 交接 + TraceLedger 审计 + 证据分级**（v2.4.5）
- ✅ **群体智能飞轮**（v2.4.6）
- ✅ **异构 LLM 多模型协商投票**（v2.4.7）
- ✅ **DeepSeek / OpenAI / Gemini / Anthropic 适配**（v2.4.8–v2.4.9）
- ✅ **豆包 + 国内六大模型统一适配**（v2.5.0–v2.5.1）
- ✅ **国内外网络分区感知与自动降级**（v2.5.2）
- ✅ **Mesh 自组网 + NAT 穿透（Circuit Relay v2/AutoNAT/DCUtR）**（v2.5.3–v2.5.4）
- ✅ **Relay 节点池 + 多通道智能切换**（v2.5.5）

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
