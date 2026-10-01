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
                                                        ├── v2.5.5 (Relay Pool - 中继池+多通道)
                                                        ├── v2.5.6 (Regression - 历史 Bug 回归套件)
                                                        ├── v2.5.7 (Identity - 跨实现身份一致性)
                                                        ├── v2.5.8 (Ledger - 精确整数账本)
                                                        ├── v2.5.9 (Auth QA - 认证式 BFT + 独立审计 + 重放保护)
                                                        ├── v2.6.0 (State Machine - 状态机恢复边 + 证据结算闸门)
                                                        ├── v2.6.1 (Ledger Persistence - 账本落盘重放 + MCP 参数校验)
                                                        ├── v2.6.2 (Version Source - 版本唯一来源 + 合约可部署 + 纠删码真修 + 网络替身诚实化)
                                                        ├── v2.6.3 (Settlement Closeout - 重复注册拒绝 + 出价脱钩校验 + Rejected/DuplicateWork 终局可达)
                                                        ├── v2.6.4 (Consensus Hardening - BFT checked 算术 + 规范签名 Result 化 + 时钟端口 + 弱公钥拒绝)
                                                        ├── v2.6.5 (Topology Truth - 分层拓扑确定性重写：真实 lca 跳数 + 边数线性验证 + 房间哈希分桶)
                                                        ├── v2.6.6 (HTTP & Storage Hardening - 锁毒化根除 + accept 容错 + 写操作 405 + 错误码前缀)
                                                        ├── v2.6.7 (Memory Hardening - 真 SHA-256 链存载荷+verify + 真 LRU + 派生质量防污染 + 盲猜死循环)
                                                        └── v2.6.8 (MCP Auth - Bearer 闸门/默认拒绝动钱 + 握手状态机 -32002 + RequestId Null + 未知工具 isError + LLM 去 panic)
                                                              └── v2.6.9 (Doc Honesty - SSE 单帧/probe 静态查表/回归跑 JS 全部诚实标注)
                                                                    └── v2.7.0 (Client Release - 首次全平台客户端分发大版本)
                                                                          └── v2.7.1 (stableStringify 拒非法/静默载荷 - GAP §9.1)
                                                                                └── v2.7.2 (NAT 占位检测返 Unknown 不猜测 - GAP §5.6)
                                                                                      └── v2.7.3 (实机部署修复 - 发布缺字段/认证验收证据升级)
                                                                                            └── v2.7.4 (重启恢复 - agents/tasks 注入内存 market)
                                                                                                  └── v2.7.5 (DCUtR 直连升级 + 多 relay 多通道同时在线)
                                                                                                        └── v2.7.6 (Agent Sandbox 核心架构)
                                                                                                              └── v2.7.7 (进程级隔离运行时)
                                                                                                                    └── v2.7.8 (生命周期管理与弹性供给)
                                                                                                                          └── v2.7.9 (安全边界)
                                                                                                                                └── v2.8.0 (沙箱集成核心链路 - E2B/K8s API + 7 个 MCP 工具)
                                                                                                                                      ├── v2.8.1 (Windows exec 修复 - 去 bash/python3 硬编码，直接 spawn)
                                                                                                                                      └── v2.8.2 (macOS CI 修复 - 去外部 timeout/gtimeout，Rust 原生超时)
                                                                                                                                            └── v2.8.3 (Ledger Hash Chain - 账本日志哈希链 tamper-evident + 水位/吞错/恢复失败修复 - GAP §3.1/§3.6)
                                                                                                                                                  └── v2.8.4 (Restart Gate - 重启恢复证据闸门/结果信封/信誉/质押，根治“重启即绕过” - GAP §3.2)
                                                                                                                                                        └── v2.8.5 (REST Auth - REST 认证闸门/CORS 白名单/请求体上限/状态转换表/罚没服务端定/终局拒绝可达 - GAP §3.5/§3.7/§3.8/§2.2.6)
                                                                                                                                                              └── v2.8.6 (MCP Validate - MCP 校验接入生产传输 sse/stdio/金额入口拒绝浮点/规范签名载荷整数化 - GAP §3.5/§4.1)
                                                                                                                                                                    └── v2.8.7 (Sandbox Auth - 沙箱认证/所有权/资源限制/随机 id/孤儿清扫/Win Job Object - GAP §3.3)
                                                                                                                                                                          ├── v2.8.9 (Claim Verification - 弱公钥拒绝接入/贡献验证 DID 绑定/生产 panic 类型化/NAT 守卫锁值/纠删码真恢复/毒化告警 - GAP §2.2)
                                                                                                                                                                          │     （v2.8.8 按用户指示跳过）
                                                                                                                                                                          ├── v2.9.0 (Workbench - 桌面工作台大版本：默认工作区/过程展示分级/后台任务/Office预览/模型提供商/终端/Subagent团队/插件管理 - 继续 Tauri 2)
                                                                                                                                                                          ├── v2.9.1 (Sandbox Capability - 沙箱能力声明闸门/默认不执行/trusted_local 显式 waiver/生产 panic 归零/快照持久化告警/cargo fmt 关卡)
                                                                                                                                                                          ├── v2.9.2 (CI Engineering - Windows 矩阵接入/静态关卡 static-gates 接入/panic 与 unsafe 机械检查/两脚本正式提交)
│     └── v3.0.0 (Plugin Kernel - 一切插件化架构/插件内核/热更新·热插拔·热兼容/五级插件体系/进程隔离/T0系统插件/REST 插件 API - gsn-core 0.3.0)
│           └── v3.1.0 (Business Plugins - 业务插件化/官方插件 entry 模块/信誉overall+市场match 真实算法/boot_official 随内核自动装配 9 个 T1 - gsn-core 0.3.10)
│                 └── v3.2.0 (Business Plugins II - 业务插件化深化/结算audit 独立审计+任务route 路由+卡片validate 校验/5 个官方插件承载真实业务/除零隐患类型化 - gsn-core 0.3.20)
│                       └── v3.2.1 (PMB Secure Messaging - PMB 消息 HMAC-SHA256 签名+nonce 防重放+常量时间比较/validate_sender 七道/host send_to·publish·open_inbox 端到端接线/静态关卡 stripLiterals 三 bug 修复/7 处生产 panic 类型化 - gsn-core 0.3.21) ← 当前
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

### v2.5.6 - Regression（历史 Bug 回归套件）

**核心内容**：把 v1.0.0~v2.5.5 开发中踩过、修过的全部 bug / 遗漏 / 注意事项固化为每次必跑的回归测试。新增 `test/regression.js`（14 条：版本一致性 / 编译CI守卫 / 市场重复防护 / 资金守恒）、`gsn-core/tests/regression_net.rs`（subscribe→publish 不 panic）、`test/run-all.sh` / `run-all.ps1` 一键全量、`test/README.md` 用例索引与易踩坑清单；CI 强制跑回归、失败禁止发版；修复 `bump-version.sh` 自身 5 处缺陷。gsn-core **0.2.56**。验证 JS 12 项、回归 14/14、Rust 0 failed。详见 [releases/v2.5.6.md](releases/v2.5.6.md)。

### v2.5.7 - Identity（跨实现身份一致性）

**核心内容**：统一 Rust / JS 两端（及上游 gsn-core）的 DID 派生口径为 `hex(SHA256(原始 32B 公钥)[..8])`（16 hex），新身份用 `did:nau:` 前缀；新增 `conformance/generate.mjs`（Node/OpenSSL 独立实现、与 Rust 零共享代码）+ `vectors.json`，规范载荷签名**逐字节命中上游测试向量** `e14d3f9e…`，构成真正跨实现校验（非"自己验自己"）；`Did::parse` 同时接受 aip/nau、ACA register_peer 只比指纹，上游身份向后兼容。根治客户端构建链路：根包补 `/lib/*` 子路径、client/desktop 依赖改 `file:..` + `.npmrc install-links=true`、import 改 default + 解构，Vite build 双双通过。gsn-core **0.2.57**。验证 Rust 0 failed、JS 12 项、conformance 签名逐字节命中、client/desktop build 通过。详见 [releases/v2.5.7.md](releases/v2.5.7.md)。

### v2.5.8 - Ledger（精确整数账本）

**核心内容**：把全链路金额从 f64 改为精确整数——Rust 新增 `marketplace/money.rs` 的 `Money(i64)` newtype（`#[serde(transparent)]`，刻意不实现 `Add/Sub`、强制走 `checked_add/checked_sub` 防溢出）；守恒检查改为**精确相等、无容差**（旧 Rust 容差 0.001、JS 1e-9 差六个数量级，跨语言失效）。发布任务即把预算锁定到托管账户 `__escrow__:{task}`、注册即锁定质押到 `__stake__:`，付款方余额不足一律拒绝，杜绝凭空铸币（GAP §2.1–2.3）。JS `market.js` 新增 `_assertMoney`（拒绝非整数/非安全整数）、补齐 `__escrow__` 托管（旧版预算发布后"消失"、中途不守恒）、投标与罚没拒绝 `<= 0`（GAP §2.7）；Python `market_client.py` 金额注解 float→int。新增 `conformance/money-vectors.json` 权威向量，Rust 测试 `test_money_vector_matches_conformance` 与 JS 测试读取同一向量、逐账户逐聚合值一致。gsn-core **0.2.58**。验证 Rust lib 125 / v234 66 / v235 10、JS 14 项、market_demo 两场景精确守恒。详见 [releases/v2.5.8.md](releases/v2.5.8.md)。

### v2.5.9 - Auth QA（认证式 BFT + 独立审计 + 重放保护）

**核心内容**：把 QA 验收闸门从"调用方传 approvals、服务端合成 qa-0..n 委员与赞成票"改为**固定委员集的 Ed25519 签名投票**——新增 `SignedQaVote`（task_id/round/voter/vote/nonce/issued_at/expires_at/signature，对固定格式 signing_bytes 签名），`with_fixed_members` 绑定委员公钥集（n、f=(n-1)/3），`cast_signed_vote` 依次校验委员身份 / 任务 / 轮次 / 时间窗 / 拒绝 Silent / 验签 / nonce 去重，equivocation 整轮作废、`advance_view` 换轮恢复（GAP §3.1）。新增**可失败的独立审计一等 API** `independent_audit()`：只信任只追加流水、独立逐笔重放出期望余额再与当前余额取并集逐户比对（覆盖幽灵 / 缺失 / 篡改 / 拆账），并交叉核对充值 / 罚没聚合与总额，返回 `AuditReport`；旧 `audit_full_scan` 与守恒同算法同数据源且零调用点（GAP §2.4）。**重放保护**：每票一次性 nonce + 签发 / 过期时间窗（GAP §2.5/§4.7）。充值 / 质押 / 托管 / 支付 / 退款 / 罚没全部写入**只追加流水**，作为审计唯一信任源。REST（`/audit`、认证式 verify）、MCP（market_audit、认证参数）、CLI（`gsn audit`、`verify @file`）、JS（verifyResultAuthenticated / independentAudit / Ed25519 验签，WKWebView 无 crypto 抛错不静默）、Python（认证式 verify_result、audit()）同步。gsn-core **0.2.59**。验证 Rust lib **135** / 集成全绿（v235 认证式 10、v234 66、cross_lang 2）、JS **16** 项；关键反例：拆账 100→两户 50 时守恒被蒙蔽、独立审计判失败，伪造签名与 nonce 重放被拒。详见 [releases/v2.5.9.md](releases/v2.5.9.md)。

### v2.6.0 - State Machine（状态机恢复边 + 证据结算闸门）

**核心内容**：为任务生命周期建立集中的合法状态转移表，补上两条此前缺失的恢复边——**NoQuorum→Open**（无共识后重新开放）、**Rework→Running**（返工后恢复执行），消除 NoQuorum 吸收态（GAP §3.4）。`TaskState` 新增 `can_transition_to`（集中合法边表）与 `transition`（非法转移报错）；`AgentMarket` 新增 `resume_after_rework` / `reopen_after_no_quorum`（显式校验前置态），全生命周期状态赋值统一走 transition。把 `EvidenceGrade::is_trustworthy()` 从"有谓词、零调用点"升级为结算付款前的**强制闸门**：policy 非 None 时结果信封必须存在且证据可信（Verified/CpuProto），否则拒付、状态保持 Accepted（旧 `settle_task` 信封缺失时 `unwrap_or(true)` 放行）。**policy=None 提交即验收**：提交结果直接转 Accepted、豁免证据门禁。REST 新增 `/tasks/{id}/resume`、`/reopen`；JS（models.js / market.js）、Python（market_client.py）三端同步。gsn-core **0.2.60**。验证 Rust v234 **71**（新增 5 用例）、JS **19** 项；关键反例：NoQuorum/Rework 恢复全链路、Unverified 拒付、policy=None 豁免、非法跨级转移被拒。详见 [releases/v2.6.0.md](releases/v2.6.0.md)。

### v2.6.1 - Ledger Persistence（账本落盘重放 + MCP 参数校验）

**核心内容**：让只追加结算流水真正落盘并在重启后完整重放恢复（GAP §6.1）。新增 `replay_records`（只信任流水、有符号增量逐笔重放、checked 防溢出）与 `SettlementEngine::restore`（重建余额 / 充值 / 罚没 / 已结算任务）；`PersistentStore` 新建 `ledger_entries` 表（JSON 列 + SQLite 事务，无半行）与 `append/load/count`（损坏行容错跳过）；actor 新增 `spawn_with_store`（启动恢复账本、写后增量 append + 快照 upsert），daemon 改用之，根治"只写不读、重启丢账"。**MCP 参数校验**（GAP §8.1）：新增 `validate_arguments`（缺必填 / null 占必填 / 类型错误指名参数），tools/call 在执行器前用工具自身 inputSchema 校验、失败返回 **-32602**，schema 与 tools/list 同源，杜绝 `unwrap_or(0.0)/unwrap_or("")` 静默降级。`independent_audit` 重构复用 `replay_records`。gsn-core **0.2.61**。验证 Rust v261 **5**、lib **138**（含 storage 持久化 3 用例：drop/reopen 往返、同 id 覆盖、损坏行容错），全量 0 failed；关键反例：账本恢复逐账户连续、MCP 错误参数被 -32602 拒。详见 [releases/v2.6.1.md](releases/v2.6.1.md)。

### v2.6.2 - Version Source（版本唯一来源 + 合约可部署 + 纠删码真修 + 网络替身诚实化）

**核心内容**：把版本号收敛为根 `VERSION` 唯一权威 + `scripts/check-version.sh` 全仓一致性断言（ci / publish / release 的 gate 强制，漂移即红），并修复表格 awk 把 markdown 转义竖线 `\|` 误当字段分隔致“当前值”不刷新（GAP §9.3）。重写此前 4 个中 3 个不可部署的 Solidity 合约（GovernorToken 自委托计票 / AgentCardAnchor 不可改锚定 / PoCVSettlement 改 pull 领取与验证者多数 / ReputationRegistry 中位数与法定人数），接入 Hardhat 真实编译、18 个逐缺陷测试，ci 新增 contracts-check（GAP §9.4）。进程内网络替身从 libp2p 命名剥离：`GsnNode / KademliaClient / GossipSub` → `InMemoryNode / InMemoryKademlia / InMemoryGossip`（GAP §5）。纠删码去假修：引入 reed-solomon-erasure v6.0.0 真实 RS，数据片丢失可靠校验片重建、超额丢失报错；NAT / TEE 标志位诚实标注（不做真实打洞、不实例化 enclave / 不做远程证明）。gsn-core **0.2.62**。验证 Rust 全量约 **325**（lib 138）0 failed / 0 ignored、JS **19**、历史回归 **14**、Hardhat **18**，clippy 清零。详见 [releases/v2.6.2.md](releases/v2.6.2.md)。

### v2.6.3 - Settlement Closeout（经济结算收尾：重复注册拒绝 + 出价脱钩校验 + Rejected/DuplicateWork 终局可达）

**核心内容**：补齐经济结算链路三处 GAP 缺陷。**重复注册**（GAP §2.6）：`register_agent` 新增 `contains_key` 检查，同一 agent_id 再注册即报错，不再重复锁定质押 / 重复技能索引 / 静默覆盖，与 JS `market.js` 和 REG-020 对齐。**出价脱钩**（GAP §2.7）：`submit_bid` 校验报价必须为正、不超任务预算、任务处于 Open，0 / 负 / 超预算 / 已关闭一律拒绝；`match_task` 成本打分简化为 `rep_score / price`，删除 price<=0 不惩罚分支。**结算原因不可达**（GAP §2.8）：`TaskState` 新增 `Rejected` 终态与 `Verifying/Running/Rework→Rejected`、`Rejected→Settled` 边；`SettlementReason` 新增 `DuplicateWork`；新增结果内容 SHA256 哈希记录，返工后提交完全相同结果自动转 Rejected 并标记 DuplicateWork；新增 `reject_task`（可信成功结果不可拒）；拒绝结算付执行者 0、托管预算全额退回、罚没 10% 质押、记信誉失败；NoQuorum 流程重开清除哈希以豁免合法重试。gsn-core **0.2.63**。验证 v234 套件 **74**（新增价格校验 / Rejected 端到端 / DuplicateWork 端到端 3 用例），全量 0 failed / 0 ignored，clippy 清零。详见 [releases/v2.6.3.md](releases/v2.6.3.md)。

### v2.6.4 - Consensus Hardening（共识 / 身份签名加固：BFT checked 算术 + 规范签名 Result 化 + 时钟端口 + 弱公钥拒绝）

**核心内容**：按 GAP 集中加固共识与身份九处缺陷。**BFT 算术安全**（GAP §3.3）：委员会 `min_n=3f+1` 与 `quorum` 改 checked，溢出即 Err/安全降级。**贡献验证带签名身份**（§3.6）：新增 `verify_contribution_signed`，验签覆盖 `hash||verifier_did`、禁自验、去重、饱和计数，杜绝重复投票凑数。**规范签名 Result 化**（§4.2/§4.3）：`canonical_object` 要求根为对象、`strip_signatures` 任意深度递归剥离、`sign_hex/verify_hex/canonical_payload` 全改 `Result`，不可序列化对象不再静默签空字节。**跨语言键序码点化**（§4.4）：JS 排序器改 Unicode 码点序，新增 astral 平面键序向量钉住三端一致。**规范字节契约**（§4.5）：`PROTOCOL_VERSION="nau/1"`。**弱公钥拒绝**（§4.6）：`is_weak_pubkey` 拒长度错/全零/全 0xFF/全相同，保留 8 字节指纹兼容上游。**时钟端口**（§4.8）：新建 `aca::clock`（`SystemClock` 饱和不 panic + `ManualClock` 确定性），消除八处早于 1970 panic 路径。**常量时间比较文档**（§4.9）：公开摘要不构成安全边界。gsn-core **0.2.64**。验证 Rust 全量 **146** 单元 + 集成 0 failed/0 ignored，JS **20**，clippy `-D warnings` 清零，上游跨语言签名向量逐字节命中保持。详见 [releases/v2.6.4.md](releases/v2.6.4.md)。

### v2.6.5 - Topology Truth（分层拓扑确定性重写：真实 lca 跳数 + 边数线性验证 + 房间哈希分桶）

**核心内容**：按 GAP 重写分层拓扑三处缺陷。**route_hops 去常量**（§5.2）：不再对所有跨房间对返回 7，改按两房间在聚合树中的最近公共祖先 lca 真实计算 `2·(lca-1)`，未注册返回 None；fanout9/500 节点下同 Lv2 组两房间 2 跳、跨 Lv2 组 4 跳。**边数增长率验证**（§5.3）：`logical_edges` 改 O(1) 公式，新增 N→2N 规模对照（edges 比值 < 2.5）钉住线性而非二次。**房间归属确定性**（§5.4）：leaf_groups 由 HashMap 改 BTreeSet 排名分桶，与插入顺序无关（500 id 正序/逆序构建逐一对等），join 从 O(depth·N) 降为 O(log N)；fanin_of 补回漏算的上行边。**协议版本唯一来源**（§5.7）：Identify/MeshConfig 硬编码版本改 `env!("CARGO_PKG_VERSION")`。gsn-core **0.2.65**。Rust **147** 测试 0 failed/0 ignored，JS **20**，clippy 清零。详见 [releases/v2.6.5.md](releases/v2.6.5.md)。

### v2.6.6 - HTTP & Storage Hardening（锁毒化根除 + accept 容错 + 写操作 405 + 错误码前缀）

**核心内容**：按 GAP §6.2–§6.6 修存储/HTTP。**锁毒化**（§6.2）：persist.rs 22 处 `.lock().unwrap()` 改 `unwrap_or_else(|e| e.into_inner())`，持锁 panic 中毒后仍可取内部数据、不再永久杀死存储。**accept 容错**（§6.3）：daemon 接收循环瞬时 accept 错误（EMFILE/ECONNABORTED）记录并退避 100ms 继续，不再一次错误退出整个进程。**写操作强制 POST**（§6.4）：TaskMatch/Settle/Verify/Resume/Reopen/Bids/Results/Arbitrate/Deposit 等改状态端点非 POST 返回 **405**，GET 不再误触发结算/仲裁。**错误码类型化**（§6.5）：分类由机器码前缀 match 决定（`NOT_FOUND:`→404 / `CONFLICT:`→409 / `BAD_REQUEST:`→400 / 其余 422），14 处资源不存在错误加前缀，不再匹配中文字符串「不存在」。**url_decode**（§6.6）：off-by-one 修正，末尾 `%41` 与多字节 UTF-8（`%E4%B8%AD`=中）正确解码。gsn-core **0.2.66**。新增 4 项 rest 测试，全量 Rust 0 failed/0 ignored、JS **20**，clippy 清零。详见 [releases/v2.6.6.md](releases/v2.6.6.md)。

### v2.6.7 - Memory Hardening（真 SHA-256 链存载荷 + 真 LRU + 派生质量防污染 + 盲猜死循环）

**核心内容**：按 GAP §7.1/§7.2/§7.3/§7.4/§7.7 修三层记忆。**假哈希链**（§7.1）：layered.rs 的 `sha256_hex` 实为 DefaultHasher 16 hex（假名、非密码学、跨工具链不稳），两条链只存摘要丢载荷；改真实 `sha2::Sha256`（64 hex）同时存载荷，新增 `verify_chain()` 重算每环报首个断裂索引，篡改即被定位。**假 LRU**（§7.2）：enhanced.rs 按单调 access_count 淘汰实为 LFU；改 `last_used` 逻辑时钟真 LRU，新测试在 LFU 规则下会失败。**防污染从未执行**（§7.3）：评分 f64 改整数 `score_bps` 消除 NaN panic；shared_memory 删除发布者自报 weight，质量由成败次数派生（成功率×10000）不可谎报，`publish` 低于阈值返回 `BAD_REQUEST:` 拒绝，best_strategy 整数比较。**飞轮诚实标注**（§7.4）：is_spinning 是单调计数闩锁、未接真实拓扑，保留作计数快照并文档明确非已落地机制。**盲猜死循环**（§7.7）：swarm/memory.rs 拒绝采样在 budget>=choices 永不终止，加 break 防护与 choices==0 早退。gsn-core **0.2.67**。全量 Rust 0 failed/0 ignored、JS **20**，clippy 清零。详见 [releases/v2.6.7.md](releases/v2.6.7.md)。

### v2.6.8 - MCP Auth & Hardening（Bearer 闸门/默认拒绝动钱 + 握手状态机 + RequestId Null + 未知工具 isError + LLM 去 panic）

**核心内容**：按 GAP §7.5/§8.3/§8.4/§8.5/§8.8 修 MCP 与 LLM。**§8.8 严重无认证动钱**：HTTP `/api/v1/mcp` 此前不读任何 header/token 即分发 `market_deposit`/`arbitrate`/`settle_task`/`register_agent` 等动钱工具；`handle_post` 增加 `auth_header`/`expected_token` 参数，配置 `MCP_BEARER_TOKEN` 后须带 `Authorization: Bearer <token>`（否则 401），**未配置时默认拒绝全部 10 个写/动钱工具**（安全失败），node.rs 提取 Authorization 头传入。**§8.3**：`RequestId` 加 `Null` 变体，解析/非法请求回 `"id":null`。**§8.4**：`tools/call` 未知工具改正常 result + `isError:true`，不再回协议级 -32001。**§8.5**：McpServer 加 `initialized:bool` 握手状态机，未 initialize 前除 initialize 外全部返回 `-32002 NotInitialized`，`handle_initialize` 读取 `protocolVersion`。**§8.7 局部**：`ToolResult.is_error` 序列化改名 camelCase `isError`。**§7.5**：LLM 五个适配器 `chat()` 不再 `unwrap()`/空 choices `[0]` panic，错误与空数组友好降级。gsn-core **0.2.68**。全量 Rust 0 failed/0 ignored、JS **20**，clippy 清零。详见 [releases/v2.6.8.md](releases/v2.6.8.md)。

### v2.6.9 - Doc Honesty（文档/测试一致性：停止过度承诺）

**核心内容**：本版不改运行时行为，只消除 GAP §5.1/§7.6/§8.6/§9.6/§9.7 指出的「文档/测试承诺 > 代码事实」。`mcp/sse.rs` 模块头诚实说明 `handle_get` 只发一帧 `event: ready` 即关，真长连接未实现；`llm/network.rs::probe_all` 标注只按静态谓词打标记、不做真实 I/O；`test/regression.js` 头部说明断言跑在 JS 参考实现上、不构成 Rust 守护进程行为保证；`js/test/test.js` 注释从「14 项」更正为实际的「20 项」。gsn-core **0.2.69**。cargo 0 failed/0 ignored、JS 20、regression 14。详见 [releases/v2.6.9.md](releases/v2.6.9.md)。

### v2.7.0 - Client Release（首次全平台客户端分发大版本，gsn-core 0.2.70）

**核心内容**：自 v2.3.5 首次引入 Tauri 桌面壳以来，**第一次**随版本发布重建并交付全平台客户端；此后每 9 个小版本后的大版本才再构建一次。版本号统一到 2.7.0（npm ↔ gsn-core 0.2.70），覆盖 `desktop/` 与 `client/` 的 Cargo.toml/pyproject/aip-sdk 声明。CI 在对应 runner 构建 macOS aarch64 / Windows x64 / Linux x64 客户端与 `gsn-daemon-*` 各平台二进制，客户端内嵌 SDK `@twinsearth/agent-universe@2.7.0`。**无运行时逻辑变更**——是 v2.6.0–v2.6.9 十个修复版本的客户端分发节点。详见 [releases/v2.7.0.md](releases/v2.7.0.md)。

### v2.7.1 - stableStringify 拒非法/静默载荷（GAP §9.1，gsn-core 0.2.71）

**核心内容**：`js/lib/aca.js::stableStringify` 修三类不安全行为：对象 `undefined` 键**直接省略**、数组 `undefined` 转 `null`、`BigInt`/`Date`/顶层 `undefined` **显式抛 TypeError**（不再静默序列化为 `{}` 或抛晦涩 Node 异常）。身份双 DID 已统一：新身份 `did:nau:`，`parseDid` 同时接受上游 `did:aip:`。详见 [releases/v2.7.1.md](releases/v2.7.1.md)。

### v2.7.2 - NAT 占位检测返 Unknown 不猜测（GAP §5.6，gsn-core 0.2.72）

**核心内容**：`nat/mod.rs::detect_nat_type` 此前固定返回 `PortRestrictedCone` 却被上层当测量值写进拓扑。本版 `NatType` 新增 `Unknown` 变体，占位检测返回 `Unknown` 不再猜——真实 NAT 分类由 libp2p AutoNAT 判定。两处固化测试同步改断言 `Unknown`。cargo 0 failed、JS **21**、regression 14。详见 [releases/v2.7.2.md](releases/v2.7.2.md)。

### v2.7.3 - 实机部署修复：发布缺字段 422 + 认证验收后证据不升级（gsn-core 0.2.73）

**核心内容**：两个缺陷均来自 Mac/Windows 真机部署。**缺陷一**：`TaskSpec.state` 无 serde 默认但服务端又强制覆盖为 Open，缺省调用被 422——修复为加 `#[serde(default)]`，缺省即 Open。**缺陷二**：`Unverified` 结果经认证式 BFT 委员会判定 Stop、state=Accepted 后 evidence_grade 仍停在 Unverified，导致「验收通过却永远无法结算」死路——修复为委员会 Stop 时把对应结果提升为 `EvidenceGrade::Verified`。详见 [releases/v2.7.3.md](releases/v2.7.3.md)。

### v2.7.4 - 重启恢复：agents/tasks 注入内存 market（gsn-core 0.2.74）

**核心内容**：daemon 重启后账本/余额从 SQLite 正确恢复，但 `/agents`、`/tasks/{id}`、`/stats` 全空——根因是 `spawn_with_store` 恢复时只对 `load_agents/load_tasks` 取了 `.len()` 打日志，**未把业务对象注入内存 market**。修复：`TaskState::from_label` 反向解析磁盘大写字符串（未知/坏值回 Open），恢复路径把 agents/tasks 真正装进内存 HashMap。详见 [releases/v2.7.4.md](releases/v2.7.4.md)。

### v2.7.5 - DCUtR 直连升级 + 多 relay 多通道同时在线（gsn-core 0.2.75）

**核心内容**：P2P 网络层增强。**DCUtR**：relay 建连后跟踪 holepunching，成功则 `direct_peers` 记录并日志「直连升级成功」，打洞失败自动回退 relay；**多 relay 多通道**（方案 4）：`relay_listeners`/`ensure_channels` 正式作为默认，三端同时在线、失效自动切换。承接 Mac Mini + Windows + 云电脑三端跨网组网实测。详见 [releases/v2.7.5.md](releases/v2.7.5.md)。

### v2.7.6 - Agent Sandbox 核心架构（大版本重构第一步，gsn-core 0.2.76）

**核心内容**：智能体沙箱系统起点——只做核心架构与类型定义，不引入具体执行实现。新增 `gsn-core/src/sandbox/`：`error.rs`（EnvBlocked/IsolationViolation/ExecFailed/ResourceLimitExceeded 等统一错误+分类谓词）、`config.rs`（`IsolationLevel` Process/ContainerSharedKernel/MicroVM、`ResourceLimits` 全硬上限、`NetworkPolicy` 默认全拒绝、`FilesystemPolicy` 只读根+独立临时目录可写、`validate()` 拒绝零配额/逃逸路径）。明确沙箱在 Agent Infra 执行层的位置与安全默认。详见 [releases/v2.7.6.md](releases/v2.7.6.md)。

### v2.7.7 - 进程级隔离运行时（沙箱真正能跑代码，gsn-core 0.2.77）

**核心内容**：`sandbox/runtime/process.rs::ProcessSandbox` 实现 `Sandbox` trait，不依赖 Docker/KVM 即可在普通 PC 跑代码。隔离：独立临时目录 `au-sandbox-{id}` 作唯一工作目录、`safe_join` 拒绝对绝对路径/`..` 逃逸、`env_clear` 仅注入受限 PATH、`timeout -k 1` 强杀（退出码 124→ResourceLimitExceeded）+ `bash -c ulimit` 进程/句柄上限。支持 Python/JavaScript。详见 [releases/v2.7.7.md](releases/v2.7.7.md)。

### v2.7.8 - 沙箱生命周期管理与弹性供给（gsn-core 0.2.78）

**核心内容**：新增 `sandbox/manager.rs::SandboxManager`：`acquire/release/destroy` 批量管理；**预热池** warm pool 提前 start 空闲沙箱降冷启动；**休眠唤醒** release 时 pause 释放资源、wake 恢复；**Checkpoint/Fork**（工作目录快照与从活沙箱/checkpoint 复制新沙箱，对应训练评测 Reset/Fork/Replay）；`evict_idle` 回收过久休眠沙箱并补预热；`shutdown` 全清。详见 [releases/v2.7.8.md](releases/v2.7.8.md)。

### v2.7.9 - 沙箱安全边界（gsn-core 0.2.79）

**核心内容**：把 v2.7.6 的策略类型落到可调用检查。新增 `sandbox/security.rs`：`NetworkGuard::check_egress` 默认拒绝、`deny_raw_ip` 拒裸 IP、白名单精确 `host:port` + `:*` 端口通配 + `*.domain` 子域通配；`AuditLog` append-only（可 JSON Lines 落盘、按沙箱过滤）；`PermissionChecker` scope 精确匹配 + `*` 全权、缺失返 IsolationViolation。详见 [releases/v2.7.9.md](releases/v2.7.9.md)。

### v2.8.0 - 沙箱集成核心链路（大版本，gsn-core 0.2.80）

**核心内容**：把 v2.7.6–v2.7.9 的沙箱核心/进程运行时/生命周期/安全边界集成进 daemon 核心执行链路。

- **REST / E2B 兼容（`sandbox/api.rs`）**：纯函数 `handle_api(method, path, body, &mut SandboxManager)`，与 `rest::route` 风格一致、无 I/O 可单测；端点 `POST /api/v1/sandboxes`（201）、`GET`、`GET/{id}`、`POST/{id}/exec`（Paused 自动 wake）、`pause`/`resume`、`DELETE`；E2B 别名 `/v1/sandboxes`、`/v1/sandboxes/{id}/commands`、`/run`；错误码 404/403/400/429/422/500。
- **node.rs 接入**：`run_api_server` 增加 `Arc<Mutex<SandboxManager>>`，loop 内经 `spawn_blocking` 取锁处理（不阻塞异步运行时）。
- **MCP 工具（`mcp/sandbox_tools.rs`，7 个）**：`sandbox_create/list/get/run_code/pause/resume/destroy`，复用同一处理器（单一来源）；sse/stdio 双路径接入；mutating 工具无 token 默认拒绝。公共构造 `tool()`/`ParamBuilder` 上移 `mcp/tool.rs`。
- **Kubernetes API（`sandbox/k8s.rs`）**：生成 `AgentSandbox` CRD 清单 + 示例 + reconcile 步骤，可 `kubectl apply`；当前节点无 K8s，仅生成清单、诚实标注。
- **验证**：tests/v280_test 5 项；clippy 零警告；全量 Rust、JS 21、回归 14 全绿。详见 [releases/v2.8.0.md](releases/v2.8.0.md)。

### v2.8.1 - Windows exec 修复（gsn-core 0.2.81）

**核心问题**：v2.8.0 在 Windows 上沙箱创建 201 正常，但 `POST /api/v1/sandboxes/{id}/exec` body `{"language":"python","code":"print(1+1)"}` 一律 HTTP 500 空 body。

- **根因**：`process.rs` 硬编码 `Command::new("bash")` + `bash -c "timeout ... bash -c 'ulimit ...; exec <prog>'"`，且 Python 解释器写死 `python3`；Windows 实测只有 `python`/`node`，无 `bash`/`python3`，`.output()` io error → Internal → 500。
- **修复**：`build_platform_command` 拆为 `#[cfg(not(windows))]`/`#[cfg(windows)]` 两版；Windows 直接 spawn 解释器、不套 shell；Python 解释器按平台选 `python`/`python3`。
- **验证**：Linux 本地 smoke：daemon 0.2.81 建 sb-1，exec `print(1+1)` → 200 `{"exit_code":0,"stdout":"2\n","wall_ms":121}`；全量 cargo test 31 项全绿；Release/Publish CI 全绿，npm `@twinsearth/agent-universe@2.8.1` 已发。

### v2.8.2 - macOS CI 修复（gsn-core 0.2.82）

**核心问题**：main 分支 `.github/workflows/ci.yml` 仅 `rust-test` 的 `macos-latest` Test step 失败（Ubuntu 全绿），`gsn-core/tests/v277_test.rs` 6 个 exec 类测试在 macOS FAILED。

- **根因**：**macOS runner 无 GNU `timeout`（只有 coreutils 的 `gtimeout`）**，而 `process.rs` 硬编码 `timeout -k 1 N bash -c ...`；命令找不到 → exec 失败；超时测试因无 timeout 强杀返回 Ok，断言失败。
- **修复**：去掉对外部 `timeout`/`gtimeout` 的依赖，改用 Rust 原生超时——`exec` 改为 `spawn` + 两个读线程（stdout/stderr 各 `read_to_end`，防管道写满阻塞）+ 主线程 `try_wait` 轮询（10ms）；超时则 `child.kill()` + `wait()` + join 读线程后返回 `ResourceLimitExceeded{kind:"timeout"}`。Unix 分支保留 `bash -c ulimit`（bash 与 ulimit 内建 macOS 均有），`exec` 替换 bash 为目标程序后 kill 该 pid 无孤儿。
- **版本一致性**：`check-version.sh` 新增 `ci.yml 版本断言`（bump 必须改到 ci.yml 里硬编码的 `au.version!=='X.Y.Z'`，否则 js-test 红）。
- **验证**：见本版 release 记录；macOS 变绿后由 CI 确认。

### v2.8.3 - 账本日志哈希链（gsn-core 0.2.83）

**核心问题**：《v2.8.2 增量审计》§3.1【critical】——`ledger_entries` 表只有 `(seq, payload)`，无哈希链/签名/校验和/约束，插入一条 `Deposited` 即让 `restore` 与 `independent_audit` 同时报守恒（造币）；§3.6 水位用物理 COUNT（含坏行）、写入失败被 `let _` 吞掉、恢复失败永久关闭持久化。

- **哈希链 + 锚定 head（§3.1）**：表加 `prev_hash`/`record_hash`，`record_hash = SHA256(prev_hash || "|" || payload)`，head 锚定 `kv_meta`；新增 `verify_ledger_chain()` 报首个断链 seq（head 锚定不符用 `u64::MAX`）；旧库 `migrate_ledger_chain()` 幂等迁移重算；启动先校验，断链打印 `🚨 CRITICAL`。
- **水位/吞错/恢复（§3.6）**：新增 `load_ledger_records_checked()` 返回 `{records, corrupt}`，坏行显式报告不静默跳过；初始水位 = 成功恢复的逻辑记录数（不再用物理 COUNT）；写后逐条 append，成功才推进水位、失败 eprintln 下轮重试。
- **边界**：只声称 **tamper-evident**（能重写整份文件者仍可重算整链并更新锚定），非 tamper-proof。
- **验证**：新增 4 个对抗测试（payload 篡改→Err(1)、head 锚定篡改→Err(MAX)、append 链重开一致、坏行进 corrupt）；`cargo test --lib` 167 passed，集成 251 passed，0 failed。

### v2.8.4 - 重启恢复证据闸门（gsn-core 0.2.84）

**核心问题**：《v2.8.2 增量审计》§3.2【critical】——`restore_tasks_from_store` 对快照未存字段用安全默认值硬编码（`verification_policy: None` 豁免结算闸门、`winner_price: None` 按满额预算支付），使“重启守护进程”本身成为绕过路径；结果信封、信誉、质押从不落盘。

- **任务快照补齐结算字段**：`StoredTask` 加 `winner_price`/`verification_policy`/`requester`/`deadline` 4 字段；旧库 `migrate_tasks_v284()` 幂等加列；恢复时策略三分支（解析成功用值，损坏/遗留回 None 并 `eprintln!` 人工核查，不再静默），`winner_price` 缺失不再满额兜底。
- **结果信封落盘**：新增 `result_envelopes` 表与 `snapshot/restore_results`，公共 `get_result()`；已验收未结算任务重启后可结算。
- **信誉/质押落盘**：新增 `reputations`/`stakes` 两表；`ReputationManager` 增 export/import；公共 `is_eligible()`；重启后仍可出价、罚没路径不再是死的。
- **验证**：新增 `tests/v284_test.rs` 的“关闭再打开”对抗测试（策略不回 None、winner_price=4000、结果信封存活、质押恢复仍合格）；临时回退旧行为该测试如期失败（证明有效）；`cargo test --lib` 167 passed，集成 252 passed，clippy 无 warning，0 failed。

### v2.8.5 - REST 认证闸门/状态转换/罚没服务端定（gsn-core 0.2.85）

**核心问题**：《v2.8.2 增量审计》§3.5（REST）/ §3.7【critical】/ §3.8 / §2.2.6——REST 变更接口完全无认证（Bearer 只覆盖 MCP）、通配 CORS、无请求体上限/读超时；`open_dispute`/`arbitrate` 直接赋值状态绕过状态转换表、无仲裁者、罚没金额读请求体；`reject_task` 只在库层、REST/MCP 零调用。

- **REST fail-closed 认证**：新增 `rest_authorize()`，GET/HEAD/OPTIONS 放行，变更类（POST/PUT/PATCH/DELETE）配置 `REST_BEARER_TOKEN` 必须 Bearer 匹配，**未配置默认 401**，仅 `REST_ALLOW_UNAUTHENTICATED=1` 逃生口放行。
- **CORS 白名单 + body 上限/超时**：新增 `compute_cors()` 按 `REST_ALLOWED_ORIGINS` 白名单仅命中回显（消除通配 *）；新增 `REST_READ_TIMEOUT_SECS`（默认 30，超时 408）、`REST_MAX_BODY_BYTES`（默认 10MB，超限 413）、OPTIONS 预检 204。
- **状态转换表 + 罚没服务端定**：`open_dispute` 经 `transition(Disputed)`（仅 Accepted/Running/Verifying/Rework 可争议，Open/Matched/终态拒绝）；`arbitrate` 改 `(dispute_id, arbitrator, guilty)`，仲裁者必填、重复仲裁拒绝、罚没由服务端规则定（作恶罚 100% 质押），guilty 退预算→Slashed，not_guilty→Accepted；两条罚没路径统一 `slash_stake_synced`；证据提升移到转换成功后。
- **终局拒绝接线**：`reject_task` 通过 REST（`POST /tasks/{id}/reject`）、MarketActor（`RejectTask`）、MCP（`market_reject_task`）三处暴露。
- **验证**：新增 `tests/v285_test.rs`（8 测试）与 `node.rs` `http_security_tests`（CORS/认证）；全量 0 failed，clippy `-D warnings` 无 warning（同步修 examples/market_demo.rs）。

### v2.8.6 - MCP 校验接入生产传输/金额去浮点/签名载荷整数化（gsn-core 0.2.86）

**核心问题**：《v2.8.2 增量审计》§3.5（MCP）/ §4.1——参数校验只接在零生产调用的 `McpServer`，两个真实传输（sse/stdio）直接 `bridge.call` 绕过校验，`amount:"lots"` 经 `get_money` 浮点截断落空后静默存 0 并返回 deposited；`guilty:1` 经 `unwrap_or(false)`；ACA Receipt 计量字段仍为 f64，跨语言签名载荷字节不一致。

- **统一查找→校验入口并接入生产传输**：新增 `find_tool()`；`sse.rs`（生产 HTTP）在执行前接入 `find_tool→validate_arguments`，失败返 -32602（HTTP 400）；`stdio.rs`（Claude Desktop）用 match 块表达式接入同一校验，未知工具 isError。新增传输在结构上无法绕过校验。
- **金额入口只接受整数**：`get_money` 与 REST `AccountDeposit` 删除 `as_f64().map(|f| f as i64)` 截断，只取 `as_i64()`（`"lots"`/`10.5`/`10.0` 均拒）；CLI 改 `parse::<i64>`。
- **签名载荷整数化**：`ResourceMetering.bandwidth_mb`/`energy_joules` 由 f64 改 u64，规范载荷无浮点，三端逐字节一致。
- **验证**：新增 `tests/v286_test.rs`（8 测试）；实证测试有效性（临时令校验返回 Ok 后 6 failed/2 passed，恢复后全过）；全量 0 failed，clippy `-D warnings` 退出 0。

### v2.8.7 - Agent Sandbox 认证/所有权/资源限制/随机 id/孤儿清扫（gsn-core 0.2.87）

**核心问题**：《v2.8.2 增量审计》§3.3——沙箱 REST 路径无认证、无所有权模型（任意调用者可 exec/pause/destroy 任意 `sb-N`）；安全组件（NetworkGuard/PermissionChecker/AuditLog/ExecutionToken）生产零调用；id 为 `sb-N` 计数器，重启后 `sb-1` 复用并继承文件；stdout/stderr 无上限读取；请求体配置被丢弃。

- **变更类统一认证 + 所有权**：`handle_api` 对 create/exec/pause/resume/destroy 统一要求非空 `caller`（否则 401），create 绑定 owner，后续变更经 `check_owner` 校验，非所有者 403；认证主体由 `extract_caller`（Bearer → `sub:<sha256 前 16 hex>`）派生，sse 透传、stdio 固定 `local:stdio`。
- **随机 id**：`sb-` + 16 hex（rand），经单一 `validate_sandbox_id` 自检，不可猜、不重复，杜绝重启复用。
- **资源 / 输出上限**：stdout/stderr 有界读取截断到 1 MiB；请求体 env/initial_files/timeout/allowed_domains/resources 经 `config_from_body` 真正生效并校验；Windows 新增 Job Object（winjob.rs）强制内存/进程上限与进程树 kill。
- **孤儿清扫 + 锁加固**：启动清扫残留 `au-sandbox-*`；锁改用 `into_inner()`。
- **验证**：新增 `tests/v287_test.rs`（12 测试）；实证截断（改 MAX_OUTPUT 后失败）与认证（令 None 放行后失败）两项首次失败回归，恢复后全过；更新 v280 至新签名；全量 0 failed，clippy 零警告。

### v2.8.9 - 修复“声明大量不成立”（gsn-core 0.2.89）

**核心问题**：《v2.8.2 增量审计》§2.2——多条上游声称的修复在代码里没有支持：`is_weak_pubkey` 零生产调用、贡献验证只验签不绑定 DID↔公钥、生产时钟 panic 仍在、NAT 守卫 `!is_empty()` 任何编造值都能过、纠删码被指名测试把 6 分片全交回、`into_inner()` 毒化恢复静默无日志。

- **弱公钥拒绝接入**：`identity/mod.rs` 导出 `is_weak_pubkey`；ACA `register_peer`、REST `parse_committee_members`、PoC `verify_contribution_signed` 三个真实入口均先拒绝弱公钥。
- **贡献验证 DID 绑定**：`verify_contribution_signed` 验签前强制 DID 解析 + `Did::fingerprint(pubkey) == identifier`，杜绝一把密钥伪造不同 DID 绕过去重。
- **生产 panic 类型化**：审计点名的时钟 panic 全部改为类型化错误；net/peer.rs 真实 libp2p 固定构造保留并诚实标注。
- **守卫/假测试诚实化**：NAT 守卫改为 `assert_eq!(nat_type, "Unknown")`；纠删码测试重写为丢 2 个数据片仅靠校验片重建；v235/v273 伪造委员 DID 改为公钥派生。
- **毒化告警**：persist/沙箱/ffi 全部 `into_inner()` 恢复点加 eprintln 告警。
- **验证**：全量 0 failed / 0 ignored，clippy 零警告；委员绑定落地后旧伪造 DID 立即 400（实证首次失败）。v2.8.8（§3.4 七模块）按用户指示跳过。

### v3.2.1 - PMB 安全通信：消息 HMAC 签名 + nonce 防重放 + 总线接线（gsn-core 0.3.21）

**小版本（只更新 PMB 模块，对系统与其它插件零影响）**：让插件总线消息可认证、防伪造、
防重放，并把总线从"只有测试在收发"接成"宿主端到端可收发"。

- **问题根因**：PMB 此前零密码学保护，防伪造只靠进程内令牌绑定 source；`host.rs` 注册/
  状态用到 bus 但从不调 `dispatch`（dispatch 仅在 bus 单元测试中），`register` 返回的
  Receiver 全部被丢弃，消息没有落点。
- **消息认证**（`bus.rs`）：`PmbMessage` 加 `nonce`/`signature`；每路由注册时生成 32
  字节随机会话密钥；`signing_bytes`（规范 JSON 视图）、`compute_signature`（HMAC-SHA256）、
  `constant_time_eq_hex`（常量时间比较）。`validate_sender` 五→七道：第 6 道签名
  （缺失/不符即拒绝）、第 7 道 nonce（重复即拒绝，seen_nonces 超 1024 淘汰最旧）。
- **总线接线**（`host.rs`）：`dispatch_for_plugin`（先签名再 dispatch）；`send_to`
  （点到点 Request）、`publish`（广播 Event）；`open_inbox(id)`（注册纯收件箱路由、置
  RUNNING、返回 Receiver，桥接进程插件/外部网络传输；无能力令牌、只能接收不能发送）。
- **静态关卡修复**：`check-no-panics.mjs` 的 `stripLiterals` 三个 bug——`broadcast` 的
  `br` 误判 byte-raw 前缀、lifetime `'_` 被当 char 吞到文件末尾、重写时误删普通字符
  fallback；修复后关卡正确报出 7 处真实生产 panic（此前长期报 0）。
- **7 处 panic 类型化**：`McpMethod::from_wire`（protocol.rs 1 处）、bus.rs validate_sender
  expect 与 dispatch unwrap（2 处）、`RelayClass::from_wire`（relay_pool 4 处）。
- **验证**：cargo test 全量 **551 passed / 0 failed**；clippy `--all-targets` 零警告；
  fmt 已应用；no-panics 0 sites；unsafe-containment 通过。
- **边界**：签名为对称 HMAC（证明来自持有该路由密钥的一方），不提供非对称来源证明；
  跨节点端到端非对称签名留待后续。已知未修项沿用。

### v3.2.0 - 业务插件化深化：结算 audit + 任务 route + 卡片 validate（gsn-core 0.3.20）

**中版本（新功能，不改内核）**：再 3 个核心官方插件承载真实业务，5/9 个官方插件业务化。

- **问题根因**：v3.1.0 仅信誉/匹配 2 个插件承载业务；独立审计、任务路由、卡片校验等
  算法仍在单体，且 `assign_task` 存在 `budget/candidates`、`load/max` 除零隐患。
- **新增 3 个 entry 模块**（`official/mod.rs`）：
  - market-settle `audit`：移植 `independent_audit`，只信任流水独立重放、逐账户比对、
    校验总额/聚合一致，返回 `passed` 与 `mismatches`；
  - scheduler-task `route`：移植 `assign_task`，按负载/延迟评分选最佳、估算成本；
    除零隐患类型化（空候选→no_candidates、零容量→no_capacity、全饱和→all_saturated）；
  - agent-card `validate`：校验 DID/name/version/skills/分数区间/质押。
- **测试**：新增 7 个 process 隔离测试（settle 2/router 3/card 2），含边界与篡改反例。
- **验证**：`cargo test --lib` **258 passed/0 failed**（基线 251+7）；clippy 零警告；
  regression 14 通过；真实 daemon 端到端（host_version=0.3.20、13 插件 Running，
  audit 抓到 expected=100/actual=150、route 选 b/estimated_cost=50、validate 合法）。
- **边界**：5/9 官方插件承载业务，其余 4 个（agent-skill/swarm-emergence/chain-anchor/
  chain-bridge）仍为 exec；entry 为 Python。

### v3.1.0 - 业务插件化：官方插件 entry 模块 + 随内核自动装配（gsn-core 0.3.10）

**中版本（新功能，不改内核）**：让官方插件从“通用 exec 容器”升级为真正承载业务逻辑。

- **问题根因**：v3.0.0 的 process 官方插件只是通用 exec，manifest 的 `entry` 从未被加载，
  信誉/匹配真实算法仍在单体 `marketplace/`；且官方插件未在 boot 自动装配。
- **内嵌 entry 业务模块**（`official/mod.rs`）：新增 `EntrySource` 与
  `official_entry_source()`；economy-reputation 移植 `overall`（`q*0.35+s*0.20+h*0.30+
  a*0.15`）；market-match 移植 `match_task`（`cost=reputation/price`、
  `latency_penalty=1/(1+latency/1000)`、取最高分；空 bids→no_bids）。
- **process 加载/调用 entry**（`runtime/process.rs`）：spawn 写 entry 到 `plugin.py`；
  新增 `invoke_entry`（隔离进程 import plugin → json.loads → 调用 → json.dumps）；方法名
  必须是合法标识符（防注入），方法缺失（exit 2）/非法方法名均被拒绝。
- **boot_official 自动装配**：官方插件随内核构建可信、跳过签名（与 T0 同源），但仍走
  process 隔离与能力矩阵；node.rs 在 boot_system 后装配 9 个 T1。
- **测试并行修复**：同插件 id 共享目录改为 `unique_rt()` 独立 base。
- **验证**：cargo test 全量 **786 passed/0 failed**（lib 251 + 集成 535）；process 11
  单测；真实 daemon 端到端（4 T0 + 9 T1，overall→0.75、match winner=b/
  score=0.00818、empty→no_bids）。
- **边界**：仅信誉/匹配两个业务插件 entry 化，其余 7 个仍为 exec；entry 为 Python。

### v3.0.0 - 一切插件化架构：插件内核 + 热更新/热插拔/热兼容（gsn-core 0.3.0）

**大版本（架构重构）**：把系统从单体重构为「一切插件化」内核，从此开始支持热更新、
热插拔、热兼容（兼容低版本、安全通信）。

- **插件内核**：`gsn-core/src/plugin/`（17 文件）——注册中心 Registry、插件总线 Plugin Bus
  （PMB，pub/sub + request/response、发送方校验、速率限制、3 次违规隔离）、权限仲裁器
  Arbiter、生命周期 Lifecycle（状态机/合法边/终态）、能力模型、黑名单 Blacklist、
  清单 Manifest、宿主 PluginHost。
- **五级插件体系**：系统 T0（进程内，全能力，不可热插拔）/ 官方 T1 / 认证 T2（开发者+
  官方副签）/ 第三方 T3（最小能力、默认禁网）/ 黑名单 Ring-1（禁止加载）；id 前缀即分类。
- **热更新/热插拔/热兼容**：新版本预加载 + 总线原子切换 + 失败自动回滚（registry 专用
  `replace` 路径）；任意时刻加载/卸载；ABI 主版本协商加载低版本。
- **隔离**：T1/T2/T3 独立进程（复用 v2.9.1 ProcessSandbox + 能力闸门）；Windows Job Object；
  无法强制且无 waiver 即 `PolicyNotEnforceable`。
- **T0 系统插件真实运行**：identity（cast/resolve/fingerprint）、net/storage/chain（status
  真实声明能力）。
- **REST API**：`/api/v1/plugins` 列表/详情/安装/call/热更新/卸载；变更类走认证闸门。
- **版本映射升级**：Rust crate 规则改为 `0.X.(Y*10+Z)`，3.0.0 → 0.3.0；修复 bump/check
  脚本硬编码 major=2。
- **验证**：cargo test 528 passed/0 failed；plugin:: 72 passed；编译零警告；静态关卡
  panic=0、unsafe 全有 SAFETY；真实 daemon 端到端通过（含未认证 401）。
- **边界**：WASM 运行时为可选 feature、当前构建类型化拒绝（无 wasmtime）；9 个官方插件
  清单/承载就位，业务逻辑迁移在后续中版本。

### v2.9.2 - CI 工程化：Windows 矩阵 + 静态关卡（gsn-core 0.2.92）

- **Windows 矩阵接入**：rust-test 由 ubuntu/macos 扩为 ubuntu/macos/windows，fmt/build/test/
  clippy 全链在 Windows 上执行。winjob.rs（Job Object）、process.rs Windows 分支等此前
  无任何 CI 覆盖的专属代码，现在每个 PR 都被编译与测试。
- **静态关卡（static-gates job）**：
  - `check-no-panics.mjs`：剥离注释/字符串/char、移除 `#[cfg(test)]` 后扫描，匹配
    `.unwrap()/.expect()/panic!/unreachable!/todo!/unimplemented!`（不匹配 assert!），
    生产代码命中即红灯（本版 TOTAL=0）。
  - `check-unsafe-containment.mjs`：每个 unsafe 项必须有对应 `// SAFETY:` 理由，
    winjob.rs 3 unsafe / 3 SAFETY。
- **两脚本正式提交**：v2.9.1 时脚本本地就绪但 untracked，本版入库并接入 CI。
- **验证**：Linux 全量测试全绿（172 lib + 全部集成测试，0 failed/0 ignored）、clippy 零警告、
  cargo fmt 干净；Windows 与 macOS 由 CI 矩阵验证（本地为 Linux）。

### v2.9.1 - 沙箱能力声明闸门 + 生产 panic 归零（gsn-core 0.2.91）

**核心原则**：边界要么由代码强制执行，要么让请求失败——绝不接受一个策略然后静默忽略它。

- **能力声明系统**：新增 `sandbox/capability.rs`，12 项 Capability、显式带理由的 Waiver、
  `CapabilityDeclaration.check` 唯一消费点；进程后端按平台声明真实能力，无法强制且无 waiver 即
  `PolicyNotEnforceable`（422）；默认配置不执行，trusted_local 才本地执行；Unix 加
  `ulimit -v/-t/-u/-n`；mem_mb 默认 768 兼容 Node V8；daemon 读 `GSN_SANDBOX_BACKEND`，拼错值按禁用。
- **生产 panic 归零**：12 站点（market_actor / erasure Result 化 / llm / marketplace / net/peer /
  scheduler / swarm），消除 `unwrap`/`expect`/NaN 路径。
- **快照持久化告警**：5 处快照 `let _ =` 改 `warn_persist`，不静默吞掉落盘失败。
- **CI**：rust-test 加 `cargo fmt --check`；v2.9.1 矩阵 ubuntu/macos。
- **unsafe**：winjob.rs 3 个 unsafe 块全部补 `// SAFETY:`。
- **验证**：clippy 零警告、全量测试通过（0 failed / 0 ignored）；v278/v280/v287 改显式 trusted_local。
- **边界**：Windows 矩阵与静态关卡（panic/unsafe 机械检查）在 v2.9.2 接入。

### v2.9.0 - 桌面工作台大版本（gsn-core 0.2.90）

**核心内容**：深度参考 DeepSeek Harness 桌面版，把极简 Tauri 2 演示客户端升级为真正「工作台」；**继续用 Tauri 2，不迁移 Electron**。

- **9 大视图**：工作区总览、任务、智能体（卡片/团队）、终端、文件（含 Excel/CSV/TSV 预览）、工具（沙箱代码执行）、模型提供商、插件、设置。
- **产品形态**：默认工作区（免选文件夹）、过程展示分级（results/steps/full）、托盘常驻 + 单实例、关闭隐藏、后台任务、模型提供商统一入口。
- **daemon 托管**：桌面端查找/启动/健康检查/停止/重启 gsn-daemon，仅绑 loopback，不暴露局域网。
- **后端列表增强**：`GET /api/v1/agents?all=1`、`/api/v1/tasks?all=1` 返回真实列表（无参仍返回统计，兼容）。
- **构建 pipeline**：client-build 三平台改构建 desktop 工作台；ci.yml 新增 workbench-check。
- **验证**：gsn-core clippy 零警告、448 passed / 0 failed / 0 ignored；版本 10 点一致；前端 vite build 通过。
- **边界**：LLM 仍为 mock、插件为实验开关、沙箱依赖可执行后端、实时性为 4s 轮询；v2.8.8 跳过。

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
- ✅ **历史 Bug 回归套件 + CI 强制全量回归（失败禁止发版）**（v2.5.6）
- ✅ **跨实现身份一致性（统一 DID 派生 + conformance 签名逐字节命中上游）+ 客户端构建链路根治（file: 依赖）**（v2.5.7）
- ✅ **精确整数账本 Money(i64) + 发布即托管防铸币 + 三端共享金额向量逐值一致**（v2.5.8）
- ✅ **认证式 BFT（固定委员集 Ed25519 签名票）+ 可失败独立审计（只信任流水独立重放，能发现守恒盲区）+ nonce/时间窗重放保护 + 只追加结算流水**（v2.5.9）
- ✅ **状态机集中转移表 + NoQuorum/Rework 恢复边（消除吸收态）+ 证据分级强制结算闸门 + policy=None 提交即验收**（v2.6.0）
- ✅ **账本落盘 + 重启从流水重放恢复 + MCP 单一来源参数校验（-32602，杜绝静默降级）**（v2.6.1）
- ✅ **VERSION 唯一来源 + 全仓版本一致性断言（CI 强制）+ 合约真实可部署（Hardhat 18 测试）+ 真实 Reed-Solomon 纠删码 + 网络替身诚实化**（v2.6.2）
- ✅ **经济结算收尾：重复注册拒绝 + 投标报价与预算强校验 + Rejected / DuplicateWork 终局可达（返工重提相同结果自动拒绝、付0退预算罚没10%）**（v2.6.3）
- ✅ **共识/身份签名加固：BFT checked 算术 + 规范签名 Result 化/递归剥离 + JS 码点序 + PROTOCOL_VERSION 契约 + 弱公钥拒绝 + 时钟端口（消除 1970 panic）**（v2.6.4）
- ✅ **分层拓扑真相化：route_hops 按真实 lca（去常量7）+ 边数 N→2N 线性验证 + 房间 BTreeSet 确定性分桶（与插入顺序无关）+ 协议版本 env! 唯一来源**（v2.6.5）
- ✅ **存储/HTTP 加固：persist 锁毒化根除（into_inner）+ accept 瞬时错误容错不退进程 + 写操作强制 POST（405）+ 错误码机器前缀替代中文子串分类 + url_decode off-by-one**（v2.6.6）
- ✅ **记忆层加固：真 SHA-256 哈希链存载荷+verify_chain 篡改定位 + 真 LRU（last_used 时钟）替代假 LFU + 派生质量防污染（成败计数/整数 bps，拒绝低质）+ 盲猜死循环 break 防护**（v2.6.7）

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
