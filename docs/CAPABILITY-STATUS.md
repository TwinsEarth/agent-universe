# 能力边界与库面状态登记（Capability & Boundary Status Register）

> 本文随版本维护，登记两类「文档承诺 vs 代码事实」的差距，使它们**不随版本推进而消失**：
> 1. **已知能力缺口（DEV-01~05）**：最后一次陈述版本、当前代码事实、状态（明确未做 /
>    沉默 / 路线中）与结论。
> 2. **库面模块（DOC-05 / 自审 F-4）**：存在于 crate、但不在 `gsn-daemon` 启动运行图中
>    构造的模块，以及「接线启用 or 收敛删除」的待决记录。
>
> 维护规则：每个 minor/patch 发布前复核一次本文件；某条真正闭合时标注「已闭合 + 闭合版本 +
> 证据」，**不得静默删除**；状态变化（重新立项 / 决定不做）必须写明理由与版本。
> 所有「当前代码事实」均以 `gsn-core` 源码文本 + 生产调用点 grep 为准，非运行时猜测；
> 依赖运行期观测的结论标 `unverified`。
>
> 首次建立：v3.5.9（回应 DEVELOPMENT-GUIDANCE §5 DEV-01~05 与 §DOC-05/F-4、§DOC-08）。

---

## 一、库面模块：存在于 crate，但守护进程不构造（DOC-05 / F-4）

核实方式：在 `gsn-core/src` 内对每个公开类型做「模块外、非测试、非 `lib.rs` re-export」
的生产引用检索；并检查 `gsn-daemon` 唯一启动链（`bin/` → `node`）是否构造。

| 模块 | 代表类型 | daemon 是否构造 | 生产引用现状 | 运行面由谁承担 |
|---|---|---|---|---|
| `topology/` | `LayeredTopology`、`TopologyRouter`、`NeighborManager` | 否 | 仅 `lib.rs` re-export | 网络邻居/发现走 libp2p（`net::peer`） |
| `scheduler/` | `TaskRouter`、`LoadBalancer` | 否 | 仅 `lib.rs` re-export；官方插件内联了分派逻辑（`plugin/official` 对 `assign_task` 的移植注释） | 官方调度/匹配插件 |
| `memory/` | `AgentMemory`、`EnhancedMemory`、`SwarmMemory`、`IntergenMemory`、`Flywheel`、`TraceLedger` | 否 | 模块外仅 `lib.rs` re-export | 目前无运行面（市场/网络/插件不读写） |
| `swarm/` | `Swarm`、`LightweightConsensus`、`EmergenceDetector`、`run_experiment` | 否 | 仅 `lib.rs` re-export（注意：`net/peer` 里的 `libp2p::Swarm` 是同名的第三方类型） | 群体涌现由 swarm-emergence 官方插件自有实现承担 |
| `mesh/` | `MeshNode`、`DiscoveryTable`、`HeartbeatTracker`、`SessionRegistry` | 否 | 模块外仅 `lib.rs` re-export | Kademlia 发现 / GossipSub / AutoNAT / DCUtR / Relay（libp2p） |
| `nat/` | `NatTraversalManager` | 否（数据模型/教学占位） | 仅 `lib.rs` re-export；`gather_candidates` 返回空、不产生网络流量 | libp2p AutoNAT + DCUtR + Circuit Relay |

> `topology/` 与 `nat/` 的模块头在 v3.5.3 已诚实标注；v3.5.9 为 `scheduler/`、`memory/`、
> `swarm/`、`mesh/` 补齐同类头注释，并在 `README.md` 核心模块表后加库面/运行面说明。

### 待决记录 D-1：这批库面模块「接线启用 or 收敛删除」

- **背景**：长期保留两套实现（库面模拟实现 + libp2p/官方插件真实实现）会让读者高估
  运行能力，也增加维护与审计面。这正是 GAP 历史上「策略写了但无调用点」「文档承诺 >
  代码事实」的同一失效模式。
- **补丁版立场（v3.5.9）**：**不强行接线、不删除**，只如实标注 + 登记。接线会改变运行
  行为、需要端到端验证与崩溃/安全评估，属 minor 决策；删除属公开 API 破坏，属 major 决策。
- **建议的 minor 决策标准**（供后续版本采用，非本版结论）：
  1. 若某模块的能力**已有等价运行实现**（如 mesh/nat/topology 之于 libp2p、swarm 之于
     swarm-emergence 插件、scheduler 之于官方分派），优先**收敛删除或降级为示例**，避免双份。
  2. 若能力**尚无运行面但在路线上**（如 `memory/` 三层记忆），保留但继续标「实验性」，
     接线时必须先证明在真实 daemon 数据面被构造、有端到端用例，再摘掉库面标签。
- **验收关闭条件**：每个模块要么在某版本被 daemon 真实构造（附调用点与 E2E 证据、摘除
  头注释中的库面标注），要么被删除/移入 examples（附版本与迁移说明）。在此之前本记录保持开放。

---

## 二、已知能力缺口（DEV-01 ~ DEV-05）——状态随版本延续

状态词约定：**明确未做**（有实现级类型化拒绝/空实现且文档写明）｜**沉默**（历史提过、
之后无结论，读者无法判断完成与否）｜**路线中**（已排期）｜**已闭合**（附版本与证据）。

### DEV-01 — LLM 客户端全为 Mock，不发真实 HTTP、无流式、token 为估算

- **最后陈述**：v2.9.0；此前为「沉默」。
- **v3.5.9 代码事实**：`gsn-core/Cargo.toml` **无 `reqwest`/`ureq`/`hyper` 等 HTTP 客户端
  依赖**；`llm/` 下各家族（openai/anthropic/gemini/doubao/domestic 等）仍为 mock 适配器。
  `llm/network.rs` 只是**端点分组/区域/代理与故障转移的配置元数据**（`LlmEndpoint` 等），
  不发起任何 HTTP 请求。流式响应与真实 token 计费因此同样未实现。
- **状态**：**明确未做（本版确认仍未闭合）**。
- **方向**：真实 HTTP/流式 provider 是实质功能，按 minor 排期（需引入真实客户端、超时/重试、
  流式解析、密钥管理与对应回归）。在闭合前，文档不得把 LLM 层描述为「可对接真实模型」。

### DEV-02 — 应用层 P2P 数据面未实现（GossipSub/Kademlia/DCUtR/relay hop 的自研应用层版本被拒做）

- **最后陈述**：v2.6.9。
- **v3.5.9 代码事实**：与 D-1 一致——自研的 mesh/nat/topology 库面模块零构造；节点真实的
  发现/广播/打洞/中继由 **libp2p**（Kademlia、GossipSub、AutoNAT、DCUtR、Circuit Relay）
  在 `net/peer` 承担。
- **状态**：**明确决定（不自研应用层 P2P 数据面，采用 libp2p）**。本版把「为什么」写明：
  避免重复实现未经安全审计的网络栈；libp2p 提供经过验证的 Noise 加密、签名 GossipSub、
  AutoNAT 与 relay。**不计划重开**；若未来重开，必须在本文件显式记录理由与安全评估。

### DEV-03 — TEE attestation 未对接；WASM 未原生承载（类型化拒绝）；fault_tolerance/evolution 仅枚举

- **v3.5.9 代码事实**：
  - TEE：`aca/envelope.rs`、`aca/manifest.rs` 仅有 `TEE` 等**枚举/等级与 `tee_quote:
    Option<String>` 字段**，没有任何 attestation 报价的生成或验真逻辑。
  - WASM：插件运行时对「WASM 原生承载」采取**类型化拒绝**（不假装支持），无 `wasmtime`/
    `wasmer` 依赖（grep 零命中）。
  - `fault_tolerance` / `evolution`：仅作为枚举/配置值存在，无对应运行实现。
- **状态**：**明确未做（路线项，非本版承诺）**。结论：这三项是**路线**而非「已具备」；
  在真正落地前，相关枚举只能表达「不可达/被拒绝」的等级，文档不得暗示硬件级证明可用。

### DEV-04 — 对称 HMAC（非非对称来源证明）；chain-anchor/bridge 离线构造无 RPC；outbox 非长驻

- **来源**：v3.5.0 自列边界，要求随版本延续。
- **v3.5.9 代码事实**：
  - 插件进程间（PMB）来源鉴别使用**每插件随机对称 HMAC + 常量时间比较**，能防伪造 sender、
    但**不构成非对称来源证明**（不向第三方证明「某开发者发布了某插件」）；非对称来源证明需
    依赖插件签名/发布者证书链，当前不在 PMB 层。
  - `chain-anchor` / `chain-bridge` 官方插件为**离线构造**（本地 keccak/编码与共识聚合），
    `chain/` 与插件内**无 RPC/provider 依赖**（grep `rpc|provider|http(s)://` 零命中），
    不直接读写链。
  - 进程插件 **outbox 由宿主代投、非长驻发送进程**（`open_inbox` 只收不发；host 侧轮询/代投）。
- **状态**：**明确边界（v3.5.0 决策，本版保持，未闭合）**。闭合分别需要：发布者签名与证书
  验证链、真实链 RPC/网关与重试、长驻/可靠投递 outbox（含 ACK 与重放保护）。

### DEV-05 — 流式响应未处理；QA 委员会授权门槛未做；Unix/macOS 无强制隔离原语

- **此前状态**：三项均为「沉默」（最后分别陈述于 v2.5.1 / v2.9.0 / v3.4.2）。本版补状态。
- **v3.5.9 代码事实 / 状态**：
  - **流式响应**：与 DEV-01 同源——LLM 无真实 HTTP/SSE 流式；MCP 侧 `sse.rs` 的
    `text/event-stream` 仅用于传输握手/就绪帧，**不是模型 token 流式输出**。状态：**明确未做**。
  - **QA 委员会授权门槛**：委员会/QA 流程提供签名校验与证据等级，但「谁有资格成为委员」
    的**授权门槛（注册、质押、信誉、与请求者利益隔离）未作为运行强制**。状态：**明确未做**，
    路线项；落地前不得把「已签名」表述为「已授权」。
  - **Unix/macOS 强制隔离原语**：进程沙箱在 Linux/macOS 对无法强制的边界（出站网络、文件系统
    隔离、磁盘配额等）采取**类型化拒绝（422）**，真实强隔离（Job Object 整树）仅 Windows
    具备；Linux seccomp/Landlock、macOS 沙箱未接入。状态：**明确边界（能力缺口，非漏洞）**，
    在接入原语前非 Windows 平台不声称可强隔离执行不可信插件。

---

## 三、复核日志

| 版本 | 复核结论 |
|---|---|
| v3.5.9 | 首次建立：补齐 F-4 六模块头注释 + README 库面说明 + 待决记录 D-1；将 DEV-01~05 由「沉默/分散」收敛为本表并逐条给出当前代码事实与状态；另见 DOC-08 对 v2.4.1（route_hops 常量 7）、v2.4.4（LFU 误称 LRU）的历史勘误指针。 |
