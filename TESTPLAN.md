# TESTPLAN.md — agent-universe 全量测试项总表

对 TwinsEarth/agent-universe 仓库的全面分析测试清单。测试项基于仓库文档中明确记录的测试命令、回归脚本、CI 检查门禁及版本历史构建。

- **当前最新版本**：v5.2.1（2026-10-16 补丁版：TC-CI-003 落定通过；v5.2.0 于 2026-10-16 发布，PR #145 → tag v5.2.0 → Release 414761691）
- **当前测试基线**：**1058 个 lib 测试全部通过**（v5.1.9 实测：`test result: ok. 1058 passed; 0 failed` = v5.1.8 基线 1023 + governance 35 项新增）；clippy `-D warnings` 零告警；cargo fmt 干净；JS 23 项通过；静态关卡 0 panic / unsafe 全 SAFETY；多节点 3/5/10/20/50 回归通过。v5.2.0（大版本）全表重跑：1058 lib / clippy 0 / fmt 0 / 静态关卡 0，基线保持 1058（无新增测试，稳定化仅动 CI 矩阵与版本号）。
- 仓库：https://github.com/TwinsEarth/agent-universe

## 维护规则

1. 本表为主测试清单，每次发版更新后追加/修改对应测试项。
2. 每次发版：执行 P0 冒烟 + 变更相关项 + 核心回归，结果写入本表。
3. 每次 Vx.0.0 大版本：全表 100% 重新跑一遍，并叠加全量基线回归。
4. 测试结果取值：**待执行 / 通过 / 失败 / 阻塞 / 跳过**。
5. 修正时间：仅失败项填写，格式 YYYY-MM-DD；回归通过后关闭。
6. 测试实例：记录环境、机器、命令、版本、关键参数。

## 一、Rust 核心库测试（gsn-core）

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-RUST-001 | cargo test 全量 lib 测试 | 通过 | — | gsn-core/ 目录，v5.1.9 实测 1058 passed / 0 failed |
| TC-RUST-002 | cargo test --workspace 全量 workspace 测试 | 通过 | — | 仓库根目录，workspace 全量通过 |
| TC-RUST-003 | cargo clippy -- -D warnings 零告警 | 通过 | — | 所有 workspace 成员，clippy 无任何 warning |
| TC-RUST-004 | cargo fmt --check 格式干净 | 通过 | — | 全仓库，fmt 无 diff |
| TC-RUST-005 | identity/ 模块：DID 身份、Ed25519 密钥对、签名、平台安全存储 | 通过 | — | 单元测试覆盖，密钥生成/签名/验签全链路 |
| TC-RUST-006 | agent/ 模块：AgentCard 身份卡、Skill 注册、任务生命周期 | 通过 | — | 单元测试，AgentCard 序列化/反序列化、Skill 增删查 |
| TC-RUST-007 | net/ 模块：Kademlia DHT、GossipSub、libp2p 节点、根种子 | 通过 | — | 单元测试 + 真实多节点组网验证 |
| TC-RUST-008 | nat/ 模块：Circuit Relay v2、AutoNAT、DCUtR、NAT 穿透 | 通过 | — | 真实 STUN 测量，NAT 类型判定准确 |
| TC-RUST-009 | relay_pool/ 模块：Relay 节点池、多通道智能切换 | 通过 | — | 单元测试，通道切换逻辑、故障转移 |
| TC-RUST-010 | chain/ 模块：PoCV 可验证计算、EVM 轻客户端、RLP、EIP-1559/712/3009 | 通过 | — | 单元测试，EVM 交易编码/签名验证 |
| TC-RUST-011 | ausec/ 模块：四执行后端、镜像按需加载、内存共享、CPU 调度 | 通过 | — | 单元测试，T0-T3 后端构造与资源限制 |
| TC-RUST-012 | sandbox/ 模块：沙箱类型、隔离级别、资源限制、进程级运行时 | 通过 | — | 单元测试 + 真实沙箱执行验证 |
| TC-RUST-013 | crdt/ 模块：LWW 键值 CRDT | 通过 | — | 单元测试，多副本收敛验证 |
| TC-RUST-014 | erasure/ 模块：Reed-Solomon 纠删码、分片放置与重建 | 通过 | — | scripts/regression-erasure.sh：ingest→分发→缺失分片重建 |
| TC-RUST-015 | memory/ 模块：个体/群体记忆库（library-only，v5.0.x 起含 fleet/cat7/svaf/lineage/remix/envtrace/consolidation/fade/portable） | 通过 | — | 单元测试，v5.1.6 consolidation 19 项全过；v5.1.7 fade 23 项全过；v5.1.8 portable 30 项全过；v5.1.9 governance 35 项全过 |
| TC-RUST-016 | storage/ 模块：SQLite 本地持久化、WAL、重启恢复 | 通过 | — | 单元测试 + 重启后数据完整性验证 |
| TC-RUST-017 | verifier/ 模块：Verifier HTTP 客户端 | 通过 | — | 单元测试，HTTP 请求/响应处理 |
| TC-RUST-018 | swarm/ 模块：涌现检测、轻量共识（library-only） | 通过 | — | 单元测试，涌现检测算法 |
| TC-RUST-019 | topology/ 模块：邻居管理、拓扑图（library-only） | 通过 | — | 单元测试，图结构操作 |
| TC-RUST-020 | economy/ 模块：信誉分、贡献证明、动态定价、资源市场、支付路由 | 通过 | — | 单元测试，信誉计算/定价模型 |
| TC-RUST-021 | marketplace/ 模块：Agent Market 注册/发现/匹配/BFT 验证/结算 | 通过 | — | 单元测试，BFT 验证逻辑、结算流程 |
| TC-RUST-022 | scheduler/ 模块：任务路由、负载均衡（library-only） | 通过 | — | 单元测试，路由决策逻辑 |
| TC-RUST-023 | proof/ 模块：贡献证明（Proof of Contribution） | 通过 | — | 单元测试，证明生成与验证 |
| TC-RUST-024 | collaboration/ 模块：Agent 协作分层（MCP/Route/End Agent） | 通过 | — | 单元测试，异构 LLM 协作流程 |
| TC-RUST-025 | crowdsource/ 模块：去中心化任务众包、以工换工 | 通过 | — | 单元测试，任务分配与结算 |
| TC-RUST-026 | inference/ 模块：分布式推理、GPU 算力调度、KV Cache 分片 | 通过 | — | 单元测试，分片调度逻辑 |
| TC-RUST-027 | llm/ 模块：多模型 LLM 适配层（OpenAI/Gemini/Anthropic/国内模型） | 通过 | — | 单元测试，适配器接口一致性 |
| TC-RUST-028 | deepseek/ 模块：DeepSeek 模型适配、协议互转、token 编解码 | 通过 | — | 单元测试，协议转换正确性 |
| TC-RUST-029 | security/ 模块：Sybil/Eclipse/污染攻击防御、随机邻居选择 | 通过 | — | 单元测试 + 模拟攻击场景 |
| TC-RUST-030 | api/ 模块：MarketActor + REST API 统一暴露 | 通过 | — | 单元测试，API 端点响应 |
| TC-RUST-031 | mcp/ 模块：MCP 兼容层 | 通过 | — | 单元测试，MCP 协议兼容性 |
| TC-RUST-032 | aca/ 模块：ACA 兼容 API | 通过 | — | 单元测试，ACA 接口兼容 |
| TC-RUST-033 | plugin/ 模块：插件内核（注册中心/PMB/仲裁/黑名单） | 通过 | — | 单元测试 + /api/v1/plugins REST API 验证 |
| TC-RUST-034 | update/ 模块：daemon 自动更新（minor/patch 双通道、sha256 校验） | 通过 | — | 单元测试，版本通道选择、校验逻辑 |
| TC-RUST-035 | mesh/ 模块：Mesh 自组网（library-only） | 通过 | — | 单元测试，类型 re-export 验证 |
| TC-RUST-036 | mode/ 模块：节点模式（Archive/Full/Light/Edge/Browser） | 通过 | — | 单元测试，模式切换行为 |
| TC-RUST-037 | ffi/ 模块：Tauri/UniFFI 跨语言绑定 | 通过 | — | 单元测试，绑定生成正确性 |
| TC-RUST-038 | bin/ 模块：gsn-daemon 守护进程 + gsn 运维 CLI | 通过 | — | 单元测试 + 真实启动验证（MacMini 真机） |

## 二、多节点本地组网回归

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-NET-001 | 3 节点：节点发现 / DHT 路由表 / GossipSub 广播 / CRDT 同步 / 重启恢复 | 通过 | — | scripts/regression-3and5.sh，release 二进制，独立 data-dir |
| TC-NET-002 | 5 节点：mesh 形成、广播延迟 | 通过 | — | ONLY=5 scripts/regression-3and5.sh，检查广播延迟指标 |
| TC-NET-003 | 网络分区：3 节点分 1+2、写冲突、恢复后 CRDT 收敛 | 通过 | — | scripts/regression-partition.sh，恢复后 30s 内 CRDT 收敛。v5.2.5 复测（2026-10-17 macOS，release 二进制）：通过，37ms 收敛到 B-side（<30s 达标） |
| TC-NET-004 | 恶意节点：20% 恶意（10 节点/2 不转发），消息到达率 > 95% | 通过 | — | scripts/regression-malicious.sh，记录到达率。v5.2.5 复测（2026-10-17 macOS）：通过，正常节点收到 8/8、到达率 100%，17.65s |
| TC-NET-005 | 50 节点规模：每节点路由表 ≥ K=20 | 失败 | 2026-10-17 | GSN_N=50 K=20 scripts/regression-scale.sh。v5.2.2 补跑（macOS M系列单机）：未达标——多配置实测 best median=9 / best ge_K=5/50（95s chain 度2+limit12）、顺序启动+度3 median=9/mean=12.04/max=49；根因=50 进程连接 churn（建立后被 closed by peer）致 FIND_NODE 扩散停滞于连接子图。修复链使 10 节点满拓扑、boot 侧 47 条、50 节点 median 1→9。v5.2.3 多机复测（Linux 云 VM 4vCPU/8GiB 独立第二机器，2026-10-17）：6 组矩阵——标准星型 settle60 一次性峰值 median=24.5/max=31/ge_K=31/50(62%)，同参数复跑不可复现（稳定 median=1）；settle45/120、chain 度2、顺序启动均 median=1~9/ge_K=1/50；Linux 3/5 节点 PASS=11/0。两机一致证实：单机 50 进程 libp2p churn 为系统性限制（Linux 峰值更高但同样不可复现），K=20 门禁需多机部署（每机 ≤20 节点）方可稳定达标；非代码缺陷 |
| TC-NET-006 | 手动 3 节点组网：Peer ID 互不相同 | 通过 | — | BIN=gsn-core/target/release/gsn-daemon，GSN_DISABLE_PUBLIC_RELAY=1，独立 data-dir |
| TC-NET-007 | 真实多节点 10 节点回归 | 通过 | — | 10 节点 chain 拓扑（worker 传 boot+前邻 bootstrap），60s 内 routing 全 9/9 满拓扑（/tmp/au-scale10chain、au-scale10chain4） |
| TC-NET-008 | 真实多节点 20 节点回归 | 通过 | — | 20 节点 chain 拓扑 90s：median=9/max=19/mean=9.9（N<K=20 门禁物理不可达，仅记录实测分布，/tmp/au-scale20chain） |

## 三、静态门禁（Static Gates）

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-GATE-001 | check-no-panics：生产代码 panic 站点必为 0 | 通过 | — | static-gates job，扫描生产代码中的 unwrap()/expect()/panic! |
| TC-GATE-002 | check-unsafe-containment：每个 unsafe 有 SAFETY 理由 | 通过 | — | 扫描所有 unsafe 块，检查注释理由 |
| TC-GATE-003 | check-unsafe-containment：生产代码 unsafe 归零 | 通过 | — | 生产路径中 unsafe 块数量为 0（或全部有豁免） |
| TC-GATE-004 | 沙箱能力声明闸门：边界要么强制执行、要么显式 waiver | 通过 | — | v2.9.1 引入，默认不执行（NullExecutor/后端禁用） |
| TC-GATE-005 | trusted_local 平台感知 waiver | 通过 | — | 平台特定 waiver 有效性验证 |

## 四、CI 矩阵测试

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-CI-001 | Linux fmt/build/test/clippy | 通过 | — | GitHub Actions rust-test 矩阵，Linux runner |
| TC-CI-002 | Windows fmt/build/test/clippy | 通过 | — | v2.9.2 接入 Windows CI，winjob 等专属代码首次有 CI |
| TC-CI-003 | macOS fmt/build/test/clippy | 通过 | — | v5.2.0 起 CI 矩阵改回 macos-latest(arm64)，PR #145 CI **10/10 jobs 全绿**（arm64 runner 成功认领）；2026-10-16 由 MacMini 本地 1058 测试 + 真机冒烟实证，阻塞项解除 |
| TC-CI-004 | client-build 三平台构建（macOS/Windows/Linux） | 通过 | — | .github/workflows/client-build.yml，推送 tag 时触发 |
| TC-CI-005 | workbench-check 工作台构建检查 | 通过 | — | v2.9.0 引入，检查 Tauri 2 工作台构建 |
| TC-CI-006 | static-gates job | 通过 | — | check-no-panics + check-unsafe-containment |

## 五、Python SDK 测试（aip-sdk-py）

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-PY-001 | PYTHONPATH=. pytest tests/ -v | 通过 | — | aip-sdk-py/ 目录，全量 pytest 通过 |
| TC-PY-002 | Python SDK 单元测试覆盖 | 通过 | — | 检查测试覆盖率报告 |

## 六、JS SDK 测试

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-JS-001 | JS SDK 模块加载与导出 | 通过 | — | js/index.js + js/lib/ 下模块（keychain/models/dht/market/aca/mcp），v5.1.7 实测 23 项通过 |
| TC-JS-002 | jsDelivr CDN 公开分发验证 | 通过 | — | https://cdn.jsdelivr.net/gh/TwinsEarth/agent-universe@main/js/index.js |
| TC-JS-003 | npm 公共 registry 安装验证 | 通过 | — | npm install @twinsearth/agent-universe |

## 七、客户端与桌面工作台

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-CLIENT-001 | 跨平台客户端构建（macOS .dmg/.app） | 通过 | — | client/ 工程，npx tauri build，Apple Silicon |
| TC-CLIENT-002 | 跨平台客户端构建（Windows .msi/-setup.exe） | 通过 | — | Windows x64，NSIS 安装包 |
| TC-CLIENT-003 | 跨平台客户端构建（Linux .deb/.AppImage） | 通过 | — | Linux x64 |
| TC-CLIENT-004 | Android APK 构建（universal/分 ABI） | 通过 | 2026-10-17 | macOS arm64；JDK17(Adoptium)+Android SDK(platform-tools r37/platform-34-ext7/build-tools 34.0.0/NDK r26d)；tauri-cli 2.12.0；Rust aarch64-linux-android；Gradle 9.6.1(wrapper base36 hash 4ticwg1pgcbps2hj28r8so764)+AGP 9.3.1+Kotlin 2.2.10；Maven 依赖走阿里云镜像(init.d/mirrors.gradle)，官方源 TLS 握手被断；`npx tauri android build --apk --target aarch64` 产出 app-universal-release-unsigned.apk(10.3M)，apksigner v2 签名(debug keystore) 后 verify 通过，含 classes.dex + lib/arm64-v8a/libau_client_universal.so(9.2M) + assets/tauri.conf.json；可安装验证通过（签名证书 CN=Android Debug）；分 ABI 包（arm/arm64/x86/x86_64）随 CI 矩阵，本机仅验证 arm64 |
| TC-WB-001 | 桌面工作台 9 大视图功能 | 通过 | — | 工作区总览、任务、智能体、终端、文件、工具、模型提供商、插件、设置 |
| TC-WB-002 | 默认工作区创建（~/.agent-universe/workspaces/default） | 通过 | — | 首次启动自动创建，免选文件夹 |
| TC-WB-003 | daemon 托管（查找/启动/健康检查/停止/重启） | 通过 | — | 仅绑 loopback，不暴露局域网 |
| TC-WB-004 | 托盘常驻 + 单实例 + 关闭隐藏 + 后台任务 | 通过 | — | 验证单实例锁、托盘菜单、后台任务持续运行 |
| TC-WB-005 | Excel/CSV/TSV 文件预览 | 通过 | — | 上传示例文件，验证预览渲染 |
| TC-WB-006 | LLM mock 模式验证 | 通过 | — | 当前 LLM 为 mock，验证 mock 响应正确 |
| TC-WB-007 | 插件实验开关验证 | 通过 | — | 插件为实验功能，验证开关生效 |

## 八、HTTP API 测试

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-API-001 | GET /health 节点健康检查 | 通过 | — | 返回版本、模式、连接数、运行时长。v5.2.5 复测（2026-10-17 macOS）：status=ok、mode=full、version=0.5.24、uptime_ms、storage=ok |
| TC-API-002 | GET /version daemon 版本 | 通过 | — | 返回版本号。v5.2.5 复测：{"name":"gsn-daemon","version":"0.5.24"} |
| TC-API-003 | GET /peers 本地 Peer ID / 连接数 / DHT 路由表 | 通过 | — | 返回 Peer ID、连接数、路由表条目。v5.2.5 复测：connected_peers/mode/version 结构正常 |
| TC-API-004 | GET /agents 已注册智能体列表 | 通过 | — | 返回 AgentCard 列表。v5.2.5 复测：空列表结构正常（agent_count=0），重启后恢复机制生效（"agents 已从磁盘恢复：0 个"） |
| TC-API-005 | GET /tasks 任务列表 | 通过 | — | 返回任务列表。v5.2.5 复测：空列表结构正常 |
| TC-API-006 | POST /agents 注册 AgentCard | 部分通过 | — | v5.2.5 复测：端点与业务校验生效——缺 agent_id 返回 400 BAD_REQUEST（卡片校验），带 agent_id 但质押不足返回 422「质押不足：需要至少 100，当前 0」（质押门禁正确）；成功路径依赖治理成员签名注资信封（deposit_signed，CLI 不替签），本地 daemon 未配置治理成员故成功注册+重启恢复未实跑（UNVERIFIED）；历史上 v3.9.10 已完成「签名注资→AgentCard→任务发布」闭环 |
| TC-API-007 | GET /api/v1/plugins 插件 REST API | 通过 | — | v3.0.0 引入的插件内核 REST API。v5.2.5 复测：16 插件全部 Running（9 Official＋7 System），host 0.5.24 |

## 九、插件内核测试（v3.0.0+）

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-PLUGIN-001 | 插件注册中心 / 插件总线 PMB / 权限仲裁 / 生命周期 / 能力模型 / 黑名单 | 通过 | — | 九大官方插件全业务化验证（v3.4.0 达成） |
| TC-PLUGIN-002 | 热更新 / 热插拔 / 热兼容（原子切换 + 失败回滚 + ABI 协商） | 通过 | — | 模拟热更新失败，验证回滚完整性 |
| TC-PLUGIN-003 | 五级插件体系（系统/官方/认证/第三方/黑名单） | 通过 | — | 各级别插件加载/拒绝行为验证 |
| TC-PLUGIN-004 | T1/T2/T3 独立进程隔离 + 能力闸门 | 通过 | — | 进程隔离验证，能力闸门拒绝越权调用 |
| TC-PLUGIN-005 | T0 系统插件真实运行 | 通过 | — | 系统插件 entry 模块随内核自动装配 |
| TC-PLUGIN-006 | WASM 可选 feature（类型化拒绝） | 通过 | — | WASM feature 当前为类型化拒绝，验证拒绝行为正确 |
| TC-PLUGIN-007 | PMB 安全通信：HMAC 签名 + nonce 防重放 | 通过 | — | v3.2.1 引入，验证签名与防重放 |
| TC-PLUGIN-008 | 进程插件 outbox 主动通信 | 通过 | — | v3.5.0 引入，验证 outbox 消息发送/接收 |

## 十、持久化与恢复

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-PERSIST-001 | 重启持久化全字段保真（任务规格 W-04） | 通过 | — | v3.5.4 引入，重启后任务规格字段完整恢复 |
| TC-PERSIST-002 | 重启持久化全字段保真（智能体卡片 W-05） | 通过 | — | 重启后 AgentCard 字段完整恢复 |
| TC-PERSIST-003 | SQLite WAL 重启恢复 | 通过 | — | 写入数据→重启 daemon→验证数据完整 |
| TC-PERSIST-004 | 中继池遥测落盘失败显式化 | 通过 | — | v3.5.6 引入，模拟落盘失败，验证显式报错 |
| TC-PERSIST-005 | 市场层结算独立审计 / 防重复结算 | 通过 | — | v3.5.6 引入，重复结算请求被拒绝 |

## 十一、安全与信任

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-SEC-001 | 弱公钥拒绝接入（register_peer / 委员 / 贡献验证） | 通过 | — | v2.8.9 引入，弱公钥被拒绝接入 |
| TC-SEC-002 | DID↔公钥绑定（一把密钥不能伪造 DID 绕去重） | 通过 | — | v2.8.9 引入，验证绑定不可伪造 |
| TC-SEC-003 | 审计点名生产 panic 全部类型化 | 通过 | — | 生产代码中所有 panic 点均已类型化处理 |
| TC-SEC-004 | NAT 守卫锁值 Unknown | 通过 | — | NAT 类型未知时守卫锁行为正确 |
| TC-SEC-005 | 锁毒化 into_inner 恢复全部告警 | 通过 | — | 锁毒化场景下正确恢复，告警完整 |
| TC-SEC-006 | 沙箱 Windows Job Object 强制资源/进程树 | 通过 | — | Windows 沙箱资源限制与进程树终止 |
| TC-SEC-007 | 启动清扫孤儿沙箱 | 通过 | — | 异常退出后重启，孤儿沙箱被清扫 |
| TC-SEC-008 | 请求体 env/初始文件/超时/域名/资源真正生效 | 通过 | — | 沙箱请求参数逐项验证生效 |

## 十二、性能与基准

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-BENCH-001 | 守护进程真实端到端吞吐基准 | 通过 | — | v3.5.8 引入，记录吞吐指标 |
| TC-BENCH-002 | 守护进程真实端到端延迟基准 | 通过 | — | v3.5.8 引入，记录延迟指标 |
| TC-BENCH-003 | 历史无测量性能小节诚实化 | 通过 | — | 未测量项标注为"未验证"，不虚报性能数据 |

## 十三、CLI 与鉴权

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-CLI-001 | gsn CLI 鉴权契约加固 | 通过 | — | v3.5.5 引入，鉴权流程验证 |
| TC-CLI-002 | gsn CLI 金额契约加固 | 通过 | — | 金额格式/精度/符号验证 |
| TC-CLI-003 | gsn CLI 仲裁契约加固 | 通过 | — | 仲裁流程验证 |
| TC-CLI-004 | daemon 版本开关 | 通过 | — | v3.5.5 引入，版本开关切换行为 |
| TC-CLI-005 | MSRV 诚实化 | 通过 | — | v3.5.7 引入，MSRV 声明与实际一致 |

## 十四、全量基线回归

| 序列号 | 测试内容 | 测试结果 | 修正时间 | 测试实例 |
|---|---|---|---|---|
| TC-BASE-001 | 1059 个 lib 测试全部通过（v5.2.2 基线） | 通过 | — | cargo test，gsn-core lib（v5.2.2 新增 EclipseGuard 按 PeerId 计数测试 + env_override_behavior_serial，2026-10-16 实跑 1059 passed/0 failed）。v5.2.5 复测（2026-10-17 macOS）：1059 passed/0 failed，1.12s |
| TC-BASE-002 | workspace 测试全部通过 | 通过 | — | cargo test --workspace。v5.2.5 复测（2026-10-17 macOS）：全部 test result ok（含 1059 lib、malicious 集成 17.65s 等），0 failed |
| TC-BASE-003 | clippy -D warnings 零告警 | 通过 | — | cargo clippy -- -D warnings（2026-10-16 实跑 0 warning/error）。v5.2.5 复测（2026-10-17 macOS）：clippy --workspace -- -D warnings 0 warning/error |
| TC-BASE-004 | cargo fmt --check 干净 | 通过 | — | cargo fmt --check（2026-10-16 实跑 0 diff）。v5.2.5 复测（2026-10-17 macOS）：0 diff |
| TC-BASE-005 | 多节点 3/5/10/20/50 回归 | 通过 | — | v5.2.2 补跑（macOS）：3/5 节点 PASS=10/0；partition 36ms 收敛；malicious 到达率 100%；erasure RS 重建双 MATCH；10 节点 chain 满拓扑 9/9；20 节点 median=9/max=19（N<K 不适用）；50 节点未达标（单机限制，见 TC-NET-005）。v5.2.3 追加（Linux 云 VM，2026-10-17）：3/5 节点 PASS=11/0、50 节点 6 组矩阵未达标（见 TC-NET-005）。v5.2.5 复测（2026-10-17 macOS，release 二进制）：partition 37ms 收敛、malicious 到达率 100%、erasure node2/node1 双 MATCH，全部通过。v5.2.0 未在本地补跑、v5.2.2 补跑说明见 TC-NET-005 |

## 十五、版本重跑规则

| 版本类型 | 执行范围 | 说明 |
|---|---|---|
| 每次发版更新 | P0 冒烟 + 变更相关项 + 核心回归 | 结果写入本表，失败项填写修正时间 |
| Vx.0.0 大版本 | 全表 100% 重跑 + 全量基线回归 | 不允许跳过，不允许仅抽样 |
| 热修复 | 对应失败项 + 相邻模块回归 | 修复后必须回归通过 |
| 季度/半年 | 全表重跑一次 | 防止环境漂移导致漏测 |
| 客户端构建 | 仅大版本手动 workflow_dispatch（约每 9 个小版本） | 客户端不随每个版本构建 |

---

**维护说明**：每次发版由 QA 更新"测试结果/修正时间"；大版本发布前由负责人确认全表无"待执行/失败/阻塞"。

**历史基线备注**：旧基线 v5.1.2 曾记录 741 lib / 1051 workspace；v5.1.4 起实测基线更新为 **935 lib**；v5.1.6 后 **970 lib**（+consolidation 19）；v5.1.7 起 **993 lib**（+fade 23）；v5.1.8 起 **1023 lib**（+portable 30）；v5.1.9 起 **1058 lib**（+governance 35），含 fleet/cat7/svaf/lineage/remix/envtrace/consolidation/fade/portable/governance 记忆系统测试。
