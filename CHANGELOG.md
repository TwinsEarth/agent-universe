# Changelog

本文件记录 Agent Universe 各版本的重要变更。

## [v3.5.8] - 2026-10-02

### 补丁：守护进程真实端到端吞吐/延迟基准 + 历史无测量性能小节诚实化（#76 / DOC-07）

回应审计项 DOC-07（此前全文无守护进程吞吐/延迟基准，v2.1.8「性能指标」未经测量）。
不含产品功能变更。

- **新增可复现真实基准 `scripts/bench-e2e.mjs`**：拉起临时 data-dir 的 release
  `gsn-daemon`，经真实 HTTP/1.1 回环跑「充值→发布托管→投标→匹配→验收→结算」闭环 N 轮，
  记各阶段 avg/p50/p95/p99/max 与整轮吞吐；结束拉 `/api/v1/audit`、`/api/v1/conservation`，
  审计不过或资金不守恒即退出码 2。方法学/口径/快照见 `docs/benchmarks/`。
- **实测（gsn-core 0.3.58，Linux 云沙箱，n=200）**：**4.27 轮/秒**；publish 1.7 /
  bid 4.8 / **match 109.2** / result 1.9 / **settle 116.3** ms；audit passed
  （663 流水独立重放 expected=actual=12100）、conservation true。
- **瓶颈诚实归因**：match/settle 约 110ms 来自 SQLite 默认 rollback journal +
  `synchronous=FULL` 的逐提交 fsync（overlayfs 上约 100ms），与版本无关。本版刻意不改
  WAL/NORMAL（涉及崩溃一致性权衡），登记后续候选。
- **历史文档诚实化**：`releases/v2.1.8.md`「性能指标」改名「性能指标（设计目标，非实测）」
  并加勘误指针；那三项数字发布时未测量，原文保留备查。
- 口径：单机回环/单连接/顺序，含真实落盘与 HTTP，不含跨网 libp2p/LLM/并发/TLS，
  不可外推为公网多节点 TPS。详见 `releases/v3.5.8.md`。

## [v3.5.7] - 2026-10-02

### 补丁：MSRV 诚实化——显式声明并实测最低工具链 1.88.0（#75 / W-01）

回应审计项 W-01（历史曾把最低 Rust 写成 1.85，与锁定依赖实际要求不符）。把 MSRV 变成
可证实、有 CI 关卡的属性，不含功能与运行时变更。

- **下限由锁定依赖算出**：`Cargo.lock` 解析的 385 个包中 269 个声明了 `rust-version`，
  最高为 `time` 0.3.55 / `time-core` 0.1.9 / `time-macros` 0.2.32 要求的 **1.88.0**
  （其后 wasip2 1.87、uuid 1.26.1 / hashbrown 0.17.1 / deranged 1.85）。
- **实测支持**：rustc 1.88.0（2025-06-23）下 `cargo check --bins --locked` 通过（4m40s）。
- `gsn-core/Cargo.toml` 新增 `rust-version = "1.88"`；CI 新增 `rust-msrv` job
  （1.88.0 + `cargo check --bins --locked`），依赖升级抬高下限时会立即变红。
- MSRV 关卡只约束库 + 二进制；测试/clippy/fmt 仍以 stable 运行，避免 dev-dependency
  无意义抬高发布产物下限。两个 Tauri 桌面壳的 MSRV 由 Tauri 决定，本版不代其声明。
- 详见 `releases/v3.5.7.md`。

## [v3.5.6] - 2026-10-02

### 补丁：中继池遥测落盘失败显式化 + 市场层结算独立审计/防重复结算回归（#74）

本补丁收尾 GAP §3.6「持久化写入失败被静默吞掉」在**网络中继池运维路径**上的残留，
并为「中标价付款 + 差额退款」补齐市场层独立审计与防二次结算断言。不含功能新增，
不改变任何资金/共识语义；结算按中标价付执行者、预算余款退回需求方的实现早已在
v3.5.1–3.5.3 落地并由端到端用例钉住，本版只补观测性与测试覆盖，不重复实现。

- **中继池遥测落盘失败不再静默（AU-持久化·relay）**：`node.rs` 中 11 处对 SQLite
  的中继池写入（`set_relay_status` / `mark_relay_failed` / `delete_relay` / `set_meta`，
  覆盖 hop 自动入池、连接掉线/失败、reservation 续约、通道 listen 失败、周期 probe
  健康/失败/超时、手动删除、容量扩容、池启动元数据）旧实现一律 `let _ = store.…(...)`，
  磁盘/锁错误被完全丢弃，运维无从察觉内存中的中继池健康位/失败计数/容量正与磁盘漂移。
  本版新增统一 helper `log_relay_persist_err`，落盘失败时打印
  `⚠️ [relay-persist] <op>(<id>) 落盘失败…`（含操作、对象 id、根因）。
  **控制流刻意不中断**：这些是尽力而为的运维遥测，不能让一次磁盘错误打断 P2P/swarm
  主事件循环（与 `market_actor` 的 `warn_persist` 同一原则：降级路径必须留痕，但不阻断
  数据面）。资金路径不受影响——资金流水 `append_ledger_record` 早已是「失败返回错误且
  不推进水位」（v3.5.3 AU-14），本次不涉及。
- **市场层独立审计回归**：新增 `test_market_independent_audit_passes_after_winner_price_refund`：
  走完「充值→质押注册→发布托管 50→投标 10→匹配→验收→结算」完整市场状态机后，断言
  `independent_audit().passed == true`，且从只追加流水独立重放的 `expected_total` 与当前
  全部账户余额并集 `actual_total` 精确相等（均为总充值 150）。此前市场层只断言了
  `conservation_check`，未断言比守恒更严、能抓「自洽但未入账」变动的独立审计。
- **市场层防二次结算回归**：新增 `test_market_settle_task_twice_rejected`：同一任务
  首次结算成功后再次 `settle_task` 必须返回 `Err`（错误含「不能结算」，Settled 为终态、
  无入边），且执行者/需求方余额不再变动、`settled_count` 不增加。SettlementEngine 层
  早有等价测试，本版补齐**市场层经任务状态机拒绝**这条此前无断言的路径，防止状态机放宽
  后二次结算铸出第二笔款。

**验证**：`cargo +1.98.1 build --bins` 通过；新增 2 个市场层测试通过；
`cargo test --workspace -j2`、`cargo clippy --all-targets -- -D warnings`、
`cargo fmt --all --check`、`cargo metadata --locked`、no-panics/unsafe 静态关卡结果见
`releases/v3.5.6.md`（全量回归在发版前执行并回填）。版本 npm 3.5.6 ↔ gsn-core 0.3.56 一致。

**刻意不改**：8 处 `let _ = stream.flush().await`（HTTP 响应刷新，无持久化语义）、
`let _ = reply.send(…)`（oneshot 对端已离开，属正常取消）、测试临时目录清理，均非
持久化静默点，保持原样。

## [v3.5.5] - 2026-10-02

### 补丁：gsn CLI 鉴权/金额/仲裁契约加固（W-03）与 daemon 版本开关（W-02）（仅缺陷修复）

本补丁修复 `gsn` 命令行客户端在 daemon 开启鉴权后不可用、以及会把非法金额静默存成 0 的问题，
并补齐 `gsn-daemon --version`。均为 CLI 侧缺陷，不改变服务端资金/共识语义。

- **W-03-a（写操作无法带鉴权）**：`gsn market` 使用手写 HTTP/1.1 客户端，从不发送
  `Authorization` 头。daemon 配置 `REST_BEARER_TOKEN` 后，所有写操作（deposit/publish/bid/
  settle/arbitrate 等）一律 401。本版新增 `--token <t>` 选项与 `GSN_API_TOKEN` 环境变量，
  非空时注入 `Authorization: Bearer <t>`；未配置则不发该头（与无鉴权 daemon 向后兼容）。
- **W-03-b（非法金额静默存 0）**：`deposit` 旧实现 `p[1].parse::<i64>().unwrap_or(0)`，
  `gsn market deposit acct lots` 会向服务端发送 `{"amount":0}` 并返回成功（GAP §8.1 在 CLI 侧
  的复现）。本版新增 `parse_amount`：只接受十进制整数（可选 `+`/`-`），拒绝浮点、指数、
  千分位、空串、非数字与 i64 溢出，非法即以退出码 2 终止，**绝不发出会被理解成 0 的请求**。
- **W-03-c（arbitrate 契约漂移）**：旧 CLI 发送 `slash_amount`（f64）且仲裁者身份缺失。
  v2.8.5 起服务端**已忽略请求体 `slash_amount`、罚没由服务端规则 `slash_amount_by_rule` 决定**，
  并强制必填非空 `arbitrator`。本版把命令签名改为
  `arbitrate <dispute_id> <guilty> <arbitrator>`，请求体只含 `{"guilty","arbitrator"}`，
  不再发送会被忽略的 f64 罚没值；缺仲裁者直接报错。
- **W-02（daemon 无版本开关）**：`gsn-daemon` / `gsn daemon` 新增 `--version`/`-V`，
  输出 gsn-core 权威 crate 版本后退出，与 `gsn --version` 一致（参数在共享的
  `node::parse_daemon_args` 处理，两个入口同时生效）。

**回归测试（在旧实现上必失败）**：`gsn` 二进制内新增 4 个纯解析单测：
`amount_accepts_plain_integers_and_signs`（接受整数/符号）、
`amount_rejects_non_integer_instead_of_defaulting_to_zero`（拒绝 lots/10.5/1e3/1,000/空串/
i64 溢出——旧实现会把它们静默变 0）、`deposit_body_carries_parsed_integer_and_rejects_bad_amount`、
`arbitrate_requires_arbitrator_and_omits_client_slash_amount`（缺仲裁者报错、请求体不含
slash_amount——旧实现无法表达该契约）。

**验证**：`cargo +1.98.1 test --workspace -j2` **625 passed / 0 failed / 0 ignored**
（基线 621，净增 4）；`cargo clippy --workspace --all-targets -- -D warnings` 零警告；
`cargo fmt --all --check` 干净；`cargo metadata --locked` 通过；no-panics/unsafe 静态关卡全绿；
版本 npm 3.5.5 ↔ gsn-core 0.3.55 一致。

**诚实边界**：本版只改 CLI 与参数解析；Bearer 校验仍在服务端既有 fail-closed 逻辑
（配了 token 必校验、未配默认拒绝写操作，除非显式 `REST_ALLOW_UNAUTHENTICATED=1`），
CLI 无法也不应对服务端鉴权策略做任何旁路。

## [v3.5.4] - 2026-10-02

### 补丁：重启持久化全字段保真（W-04 任务规格 / W-05 智能体卡片）（仅缺陷修复，无破坏式重构）

本补丁把 Windows 实机（win-deploy-v3.5.0，提交 416b5bc/9603735）已验证的两个重启丢字段修复
回合到云开发树。根因同源：`spawn_with_store` 重启恢复时，业务对象只从 SQLite 的**少量扁平列**
重建，列里没有的丰富字段全部退化为默认值——任务的执行轨迹与卡片的能力声明在一次重启后丢失。

- **W-04（任务规格重启丢失）**：旧 `StoredTask` 只有 10 个扁平列，`restore_tasks_from_store`
  重建 `TaskSpec` 时把 `context` 清空、`todo` 退化为 `"(restored from disk)"`，`done`/`trace`/
  `required_skills` 全丢，导致重启后任务的执行上下文与技能要求不可用。本版为 `tasks` 表新增
  `spec_json TEXT`，快照写入完整 `TaskSpec` 规范 JSON，恢复时**优先反序列化完整规格**，再用
  扁平权威列覆盖经济/生命周期字段（`budget`/`winner_price`/`deadline`/`state`/`owner`/`goal`/
  `requester`/`created_at` 与单独解析的 `verification_policy`，保留 v2.8.4 坏值告警语义）。
- **W-05（智能体卡片重启保真）**：旧 `StoredAgent` 只有 6 个扁平列，`restore_agents_from_store`
  重建的卡片 `version` 恒为 `"0.0.0-restored"`，`description`/`modalities`/`models`/`endpoint`/
  `pricing`/`sla`/`total_calls`/`success_rate`/`evidence_grade`/`verified` 全部退化为默认值。
  本版为 `agents` 表新增 `card_json TEXT`，快照写入完整 `MarketAgentCard`，恢复时**优先反序列化
  完整卡片**，再用经济身份权威扁平列覆盖（`stake`/`reputation`/`created_at`，资金与信誉以列为准、
  不允许被 JSON 改写；`updated_at` 取 max），JSON 内 skills 为空时用扁平 `skills` 列兜底。

**迁移与健壮性**
- 新增幂等迁移 `migrate_add_text_column(conn, table, column)`：`PRAGMA table_info` 探测列是否
  存在，新库（CREATE TABLE 已含两列）与旧库（ALTER ADD TEXT）都安全，可重复打开不报错。
- 完整 JSON 缺失或损坏时**不 panic**：回退旧的安全默认构造并显式 `eprintln!` 告警（含 agent/
  task id 与错误），经济字段仍由扁平权威列恢复，不影响账本与结算。

**回归测试（在旧实现上必失败，非绿了也证明不了什么的测试）**
- `storage::persist::tests::task_spec_json_survives_reopen` / `agent_card_json_survives_reopen`：
  写入含丰富 `spec_json`/`card_json` 的记录，重开库后断言 context/todo/done/required_skills、
  version/modalities/models/pricing.price/sla.latency_p95_ms/total_calls 存活（旧实现必失败）。
- `json_column_migration_is_idempotent`：同一库连续三次 open + 写入，迁移幂等不报错。
- `tests/v274_test.rs` 两条恢复测试升级为携带丰富 JSON 并断言字段存活（W-04/W-05 端到端，
  覆盖 `restore_*_from_store` 生产恢复路径，而非仅存储层往返）。

**验证**：`cargo +1.98.1 test --workspace -j2` **621 passed / 0 failed / 0 ignored**
（基线 618，新增 3 个持久化测试；v274 两条为存量测试增强断言）；`cargo clippy --workspace
--all-targets -- -D warnings` 零警告；`cargo fmt --all --check` 干净；`cargo metadata --locked`
通过；`scripts/bump-version.sh 3.5.4` 全套版本声明点一致（npm 3.5.4 ↔ gsn-core 0.3.54）。

**诚实边界**：本版只解决"重启后内存重建丢字段"，不改动 v2.8.4 已有的证据闸门/账本哈希链/
资金守恒机制；`card_json`/`spec_json` 与扁平列的权威关系是"经济/生命周期以列为准、丰富声明以
JSON 为准"，不是把 JSON 当作可信资金来源。



### 补丁：数值健壮性 / JS·Python 客户端一致性 / 测试与文档诚实化（仅缺陷修复，无破坏式重构）

本补丁基于 v3.5.2，修复 AU-33/34/35/36/19/17/14/30/31/27/15/16/39/02/11/12/13/20/29/40/37/38。

**A 组：Rust 数值/正确性硬化**
- **AU-33（apply_decay 除零）**：`ReputationSystem::apply_decay` 在 `decay_halflife_secs==0`
  时旧实现 `elapsed / 0` 整数除零 panic；改为 halflife==0 视为"不衰减"并跳过。测试
  `au33_decay_halflife_zero_no_panic_and_no_decay`（sleep 令 elapsed>0，旧实现必 panic）。
- **AU-34（非有限 f64）**：`with_supply_demand` 对 NaN 静默落 2.0、`price()` 对 Inf 饱和成
  u64::MAX；改为入口拒绝非有限 ratio（`PricingError::NonFiniteSupplyDemandRatio`）、price()
  非有限/溢出显式报错（`NonFiniteOrOverflowPrice`）。测试覆盖 NaN/+Inf/-Inf/0/溢出。
- **AU-35（round 静默截断）**：MCP/REST 的 `round` 旧用 `as_u64().unwrap_or(0) as u32`
  （1.5 退 0、5e9 截断）；MCP schema 改 integer，两侧用 `u32::try_from` 严格校验，
  非整数/负数/越界返 -32602 风格参数错误。
- **AU-36（get_money 静默 0）**：取不到合法整数金额旧 `unwrap_or(Money::ZERO)`；改为显式工具
  错误，不再把金额静默写成 0。
- **AU-19（罚没两阶段顺序）**：`slash_stake_synced` 旧先改质押记录再扣账本，第二步失败留
  口径不一致；改为先扣账本、再改质押记录，记录步失败则把账本扣额补回（回滚），保证两视图一致。
- **AU-17（启动自审）**：`spawn_with_store` 恢复完成后自动跑一次 `independent_audit`；
  通过记日志，账实不符打印显著 CRITICAL 并留可观测信号（账本链篡改已由 v3.5.1 fail-closed 拦截）。
- **AU-14（双写竞态）**：node.rs POST /agents 后直写 `store.upsert_agent`（reputation 硬编码 0）
  与 market actor 写后快照同键 last-writer-wins；删除冗余直写，以 actor 为唯一 SQLite 写者
  （注册后读回走内存 market，不受异步快照时序影响），并移除 run_api_server 未用的 store 参数。

**B 组：沙箱运维诚实化与真实行为测试**
- **AU-30（非 root）**：run_daemon 早期加 `ensure_not_root(allow_root,euid)`——euid==0 且未设
  `GSN_ALLOW_ROOT=1` 拒绝启动；不引 libc，Linux 下解析 /proc/self/status 的 Uid 行。纯函数测试
  root+未允许拒 / root+允许放 / 非 root 放。
- **AU-31（cpu_time_ms 恒 0）**：`SandboxResult.cpu_time_ms: u64` 恒填 0 易被误读为"测得 0ms"；
  改 `Option<u64>`，进程后端如实返回 `None`=未测量（不为此新引 libc/unsafe，不填假值）。
- **AU-27（隔离真实行为测试）**：新增 v353_test 真跑 env 白名单清洗（宿主秘密变量不泄漏）、
  workdir 与宿主 cwd 隔离、白名单变量透传。内存 OOM/Windows Job Object/网络-FS-quota 因容器/平台
  不可确定性断言，**不写空洞测试**，在测试模块注释与本节明确列出。

**C 组：JS / Python 客户端一致性（已真实运行）**
- **AU-15（Bearer 注入）**：js/lib/mcp.js 与 aip-sdk-py mcp_client 增加可选 auth token，设置后发
  `Authorization: Bearer <token>`，不设置不发该头。两侧各加头断言测试。
- **AU-16（Python 三端签名钉向量）**：新增 tests/test_cross_lang_vector.py，读取
  conformance/vectors.json，断言 Python 由固定 seed 派生的公钥/DID/规范载荷签名与 Rust、JS
  逐字节一致。
- **AU-39（JS 恒真断言）**：`shardOf('key-0',16)===shardOf('key-0',16)` 恒真，改为"同 key 两次一致
  且结果落在 [0,16)"的有意义不变量。

**D 组：文档诚实化（只标注，不接线）**
- **AU-02/11/12/13/20/29/40**：topology 纯算法库、mcp/server.rs McpServer（线上 MCP 走
  Market/Sandbox bridge）、gossipsub（dormant，发现实际仅 Kademlia）、`gsn mcp --transport stdio`
  （独立内存市场演练场，无 SQLite/恢复，裸 default 沙箱会 422）、swarm/consensus.rs LightweightConsensus
  （模块头显著警告"未接线；接入前必须 weight=stake、去重、验签"）、sandbox NetworkGuard/ExecutionToken/
  SandboxIdentity（设计预留、当前不做短时令牌/网络强制）、official root 不预置且 T3 平台可达性——
  均在对应 doc 注释如实标注"library-only / dormant / 设计预留"，不再暗示已在 daemon 生产生效。
- **AU-37/38（README 数字订正）**：本版实测 Rust **618** 测试 / Python **19** / JS **22**；
  MCP 市场工具实测 **20** 个（含 market_audit、market_reject_task）。README 已从过时的
  155 Rust / 12 JS / 18 工具订正，并加"随 cargo test 增长、以发布时实测为准"。

**已知限制 / 后续（跨版残留，诚实声明）**
- v3.5.1 委员会仍可能被 ≥4 个各自足额质押的 Sybil 身份凑齐 Stop；按质押加权的验证人集与
  slashing 联动留待 minor。
- v3.5.2 QA nonce 为运行期跨请求去重；跨重启窗口内的重放由既有服务端时间窗约束。
- 本版未接线的库能力清单（topology/mesh/nat、McpServer、gossipsub 发布消费、LightweightConsensus、
  NetworkGuard/ExecutionToken/SandboxIdentity）见 D 组；进程后端不测量 CPU 时间（cpu_time_ms=None）。

## [v3.5.2] - 2026-10-02

### 补丁：插件 / PMB / 沙箱健壮性与隔离诚实化（仅缺陷/安全/正确性修复，无破坏式重构）

本补丁基于 v3.5.1，修复审计条目 AU-04/06/07/08/09/10/21/22/23/24/25/26/28。每组均带
「在旧实现上必败」的回归测试。

- **AU-04（QA nonce 跨请求去重 / 重放）**
  - 根因：`QaCommittee.seen_nonces` 是实例内 `HashSet`，而 `verify_result_authenticated`
    每请求新建委员会即清空，同一组签名 Stop 票在第二次验收调用里被当作新票重放并通过。
  - 修复：把 nonce 去重提升为 **`AgentMarket` 字段级**集合，键
    `(task_id, round, voter_did, nonce)`，跨请求存活；闸门 3（质押）后做只读预检，命中即
    返 `QA_NONCE_REPLAY`；整条验收成功后才 insert 键（失败不烧合法票）。QA 时间窗确为
    服务端强制（`cast_signed_vote` 拒绝 `now<issued_at`/`now>expires_at`）。
  - 残留（如实声明）：去重在**运行期**跨请求存活；跨进程/重启持久化需 schema 扩张，本补丁
    不做架构扩张——跨重启窗口内的重放由既有服务端时间窗约束。
  - 回归测试（tests/v352_test.rs）：同一组合法 Stop 票第二次原样重放被 `QA_NONCE_REPLAY` 拒绝。

- **AU-06（插件生命周期受守卫转换）**
  - 根因：`lifecycle.rs` 的合法转移图生产无人走；host 经 `bus.set_state` 无条件直改状态，
    终态插件可被静默回 Running。
  - 修复：`bus.set_state` 复用与 `PluginLifecycle` 同一终态谓词 `is_terminal()`
    （Refused/Quarantined/Archived）做**终态锁定后门**——终态插件再改写到任何其它状态
    返类型化 `InvalidTransition`。最小侵入，不改热更新正常路径语义。
  - 回归测试（bus.rs）：插件置 Quarantined 后 `set_state` 回 Running/Stopped/Discovered
    均失败、状态保持终态。

- **AU-08（拒绝外部自证系统命名空间）+ AU-23（spawn 失败不留孤儿注册）**
  - 根因：`com.twinsearth.sys.*` 在 arbiter 走 `Tier::System` 臂，「签名由宿主构建链保证」
    实际不验签；外部 install 自证系统名即可提权获进程内全能力。install 又先 `register`
    后 `spawn`，spawn 失败留下与实例 desync 的孤儿注册项。
  - 修复：`install()` 在 ABI 校验后立即拒绝 `Tier::System` 名
    （`PLUGIN_SYS_NAMESPACE_FORBIDDEN`）；系统插件只走 `boot_system()`。并把注册推迟到
    **spawn 成功之后**（spawn 失败不写 registry / 总线）。
  - 回归测试（host.rs）：①外部自签 `com.twinsearth.sys.*`→被拒；②构造 spawn 必败的清单
    （非法资源）后 install 失败，registry 与总线路由均无残留。

- **AU-07（黑名单可播种）+ AU-24（start 复检 / uninstall 清路由）**
  - 根因：`Blacklist::new()` 恒空、启动不播种；`start()` 不复检；`uninstall` 只置 Stopped
    而保留总线 `RouteEntry`（孤儿路由）。
  - 修复：新增 `Blacklist::load_operator_file`（每行一个 plugin-id、`#` 注释、文件不存在=空，
    不联网、无硬编码封禁）与 `host.seed_blacklist_from_env()`（读 `GSN_BLACKLIST_FILE`）；
    `start()` 重新过黑名单闸门；`uninstall()` 末尾 `bus.remove_route(id)`。
    `file_appeal` 确为死代码，不接线（现状保留）。**生产接线（复核补修）**：`run_daemon`
    （node.rs）在构造 `PluginHost` 后、`boot_system`/`boot_official` 之前真实调用
    `seed_blacklist_from_env()`——未设变量=空黑名单 no-op（现状）；已设但读取/解析失败则打印
    CRITICAL 并 fail-closed 拒绝启动插件子系统，绝不静默按空表继续。
  - 回归测试（host.rs/blacklist.rs）：播种某 id→install 被拒；停止期间拉黑→`start` 被拒；
    uninstall 后 `route_table` 无该 id；另加 `blacklist_seed_read_failure_is_error_not_silent_empty`
    证明"显式配置却读失败"返回错误（run_daemon 据此 fail-closed）、未设变量才是 no-op。

- **AU-09 + AU-22（outbox 有界与健壮解析）**
  - 根因：`collect_outbox` 先 `read_to_string` 整文件（无上限，内存 DoS）；非 UTF8 静默
    `Ok(())`；单坏行 `from_str?` 毒丸整次失败；仅全成功才清文件。
  - 修复：具名常量 `OUTBOX_MAX_BYTES=16MiB`，用 `Read::take` 有界读取（超限具名失败
    `OUTBOX_TOO_LARGE` + 审计）；逐行隔离坏 JSON / 未知 kind / send 缺 target（记
    `outbox_issues` 后继续，不毒丸）；非 UTF8 记 `OUTBOX_NON_UTF8` 审计并清理；合法行搬入
    outbox 后清文件。
  - 回归测试（plugin/runtime/process.rs）：超大 outbox 被拒；一行损坏不影响其余合法消息
    投递且有审计信号；非 UTF8 不再静默当空；缺 target 的 send 被隔离（旧实现整次失败）。

- **AU-21 + AU-25（PMB TTL 新鲜度 / 淘汰顺序 / 先验签后计速率）**
  - 根因：`ttl_ms` 入签名却从不比对时钟；nonce 窗口超 1024 用 `BTreeSet.iter().next()`
    按**字典序**淘汰（可能误删刚签发的合法 nonce）；速率槽在 HMAC 验签 / nonce 校验**之前**
    消耗——坏签名/重放攻击者可白嫖合法插件的速率配额。
  - 修复：①`PluginBus::set_server_clock_ms` 注入服务端时钟后，强制
    `now ∈ [issued_at, issued_at+ttl_ms]`，过期返 `PMB_MESSAGE_EXPIRED`。**生产接线（复核
    补修）**：时钟注入点从"仅测试"改为在宿主唯一发送路径 `PluginHost::dispatch_for_plugin`
    （`send_to`/`publish` 及 outbox 代投全部汇聚于此）签名后、dispatch 前用真实墙钟即时写入；
    `issued_at` 取宿主时钟（生产 `now_ms==0` 时退到真墙钟，避免自签消息被钉到 1970）。
    bus.rs 直接驱动 PluginBus 的单测仍在未注入时钟的确定性回放模式下（向后兼容）；
    ②nonce 改 `HashSet` membership + `BTreeSet<(issued_at,nonce)>` 时间序，超窗淘汰**时间最旧**
    而非字典序；③把速率桶消耗移到 HMAC 验签 + nonce + 新鲜度全部通过之后。
  - 回归测试：bus.rs 注入时钟到未来后过期消息被拒、字典序与时间序相反时按时间淘汰最旧、
    坏签名不消耗速率配额；**另加经真实 PluginHost 的 `au21_production_path_enforces_ttl_freshness`**
    （不手动 set_server_clock）：窗口内新鲜消息被接受投递，把宿主时钟钉到 epoch+1s（issued_at 陈旧）、
    总线仍按真墙钟判定时被 `PMB_MESSAGE_EXPIRED` 拒绝——证明生产发送路径真的强制了 TTL。

- **AU-10（waiver 理由入审计）**
  - 根因：`AuditEntry` 无 waiver 字段，`create` 审计点不传 `cfg.waivers`——「在缺边界后端
    带理由放行」不留痕。
  - 修复：`AuditEntry` 增加 `waivers: Vec<WaiverAuditEntry>`（边界名+理由，`#[serde(default)]`
    兼容历史日志）；create 成功/失败都把 `cfg.waivers` 序列化落 `sandbox-audit.log`。不改
    隔离运行时行为。
  - 回归测试（tests/v352_test.rs）：带 waiver（trusted_local）create 后，审计条目含
    `network_deny_all` 边界与非空理由。

- **AU-26（Windows 绝对路径逃逸）**
  - 根因：`safe_join` 仅挡 `/` 前缀与 `..`，未查 Windows 盘符 `C:\` / UNC `\\server`；
    `config.validate` 同。因 `Path::is_absolute()` 语义随编译平台变化，Linux 上构造
    `C:\foo` 会被判相对路径。
  - 修复：新增跨平台 `path_escapes_sandbox`，按字符串同时拒绝前导 `/`、前导 `\`、`X:` 盘符
    （X 为 ASCII 字母），并以 `is_absolute()` 兜底 + ParentDir 组件；`safe_join` 与
    `config.validate` 统一复用。
  - 回归测试（sandbox/config.rs）：`/etc/passwd`、`../x`、`C:\Windows`、`C:/x`、
    `\\server\share`、`\unc` 均判逃逸；合法相对路径放行。

- **AU-28（隔离级别不静默降级）**
  - 根因：`cfg.isolation` 从不与后端实际可达级别比对；进程后端硬编码返 `Process`，调用方
    请求 MicroVM 也被静默按 Process 跑。
  - 修复：`ProcessSandbox::create` 在能力校验后，若
    `cfg.isolation.strength() > self.isolation().strength()`（MicroVM/Container 请求落在
    Process 后端）且无带非空理由的 `Waiver(Capability::IsolationLevel)` 豁免，返具名
    `PolicyNotEnforceable`（详情含 `SANDBOX_ISOLATION_UNAVAILABLE`），绝不静默降级。
  - 回归测试（tests/v352_test.rs）：请求 MicroVM 在进程后端且无豁免→具名拒绝；显式豁免→
    按现状接受。

## [v3.5.1] - 2026-10-02

### 补丁：QA 委员会自我批准等安全缺陷修复（仅缺陷/安全/正确性修复，无破坏式重构）

本补丁基于 v3.5.0 基线，修复 QA 审计条目 AU-01/AU-05/AU-03/AU-18/AU-32。每条修复均带
「在旧实现上必败」的回归测试。

- **AU-01 / AU-05（Critical，服务端质押锚定 QA 委员会）**
  - 根因：`verify_result_authenticated` 只验委员票的签名，不校验委员身份是否在服务端
    持有有效质押、是否与任务执行者利益冲突，也不设 BFT 人数下限。调用方带一把自造密钥、
    `n=1`（f=0）即可凑成 Stop「自我批准」并放款。
  - 修复：在构造委员会 / 验票**之前**新增三道服务端资格闸门（类型化 `Result<String>`，
    错误前缀稳定可断言 `COMMITTEE_TOO_SMALL` / `COMMITTEE_EXECUTOR_CONFLICT` /
    `COMMITTEE_NOT_STAKED`）：
    1. 市场层具名常量 `MIN_QA_COMMITTEE_SIZE = 4`（BFT 依据：`f=(n-1)/3`，n=1/2/3→f=0
       单人即可自批；n≥4 才有 f≥1、quorum≥3，容忍 1 个恶意/宕机委员）；
    2. 任务执行者回避：先取任务（不存在返回 `NOT_FOUND`），其 `owner`（中标执行者）
       不得出现在委员 DID 集合；
    3. 服务端质押锚定：新增 `ReputationManager::has_locked_stake(did)->bool`
       （`status==Locked` 且 `amount>=min_stake`，阈值复用质押管理器内部口径），任一委员
       不满足即拒绝。
    - 不修改通用 `QaCommittee` 库逻辑，不改密码学验签。
  - 残留风险（如实声明）：本补丁锚定「独立、已质押、非执行者」身份并设 BFT 下限；掌握
    ≥4 个各自足额质押身份的策划型 Sybil 仍可凑齐 Stop。按质押加权的验证人集与 slashing
    联动属后续 minor，不在本补丁范围。
  - 回归测试（tests/v351_test.rs）：①单把自造密钥 n=1 自签 Stop→`COMMITTEE_TOO_SMALL`、
    不升 Verified、不放款；②委员含执行者→`COMMITTEE_EXECUTOR_CONFLICT`；③委员未/非
    Locked 质押→`COMMITTEE_NOT_STAKED`；④4 个独立足额质押且非执行者委员合法 Stop→
    通过并可 settle（不误伤合法流程）。

- **AU-03（账本哈希链恢复 fail-closed）**
  - 根因：`spawn_with_store` 恢复路径在 `verify_ledger_chain` 检出断链/锚定不符时，仅
    eprintln 告警后仍照常 restore 并进入可写/结算数据面——被篡改的账本照样能放款。
  - 修复：断链/锚定不符时**默认不恢复账本、不进入可写/结算数据面**，进入拒服态——对一切
    命令回复 `LEDGER_TAMPERED` 错误（不取 oneshot 不悬挂）。仅当显式逃生开关
    `GSN_ALLOW_TAMPERED_LEDGER=1` 时保留旧的告警+容错恢复，并打印显著 CRITICAL 告警。
    开关默认关。
  - 回归测试：构造断链账本，无开关时恢复拒服且不能 settle；开关=1 时（子进程隔离环境变量）
    恢复旧容错行为、进入数据面。

- **AU-18（提交结果入库即降为未验证）**
  - 根因：`submit_result` 原样入库客户端自报的 `evidence_grade`，执行者自报 Verified 即可
    满足结算可信闸门。
  - 修复：入库前**强制** `evidence_grade = EvidenceGrade::Unverified`；只有认证 QA Stop
    或仲裁路径可提升（保持现有服务端签发）。
  - 回归测试：提交时自报 Verified，入库后断言为 Unverified，且不能直接 settle。

- **AU-32（Bearer 常量时间比较）**
  - 根因：REST（`node.rs` `rest_authorize`）与 MCP（`mcp/sse.rs`）用朴素 `==`/`trim()==`
    比较 Bearer 令牌，逐字节短路泄露前缀长度（时序侧信道）。
  - 修复：新增 `security::constant_time_eq[_str]`（逐字节 XOR 累积、长度不等也不提前返回，
    不引入新第三方依赖，与 `plugin::bus::constant_time_eq_hex` 同范式），两处鉴权统一复用。
  - 回归测试：`security` 模块内常量时间比较正确/错误用例（功能正确性；时序不在单测断言范围）。

- **测试影响面调整**：既有走认证验收的集成测试（v235/v273）原用临时自造密钥当委员且未
  注册质押，已改为注册 4 个相互独立、各自足额（100）锁定质押、且均非任务执行者的委员后
  再投票——即正确新流程。
- 全量回归：在 v3.5.0 基线 582 passed 之上新增上述回归用例；`cargo fmt`、
  `cargo clippy --workspace --all-targets -- -D warnings` 零警告。

## [v3.5.0] - 2026-10-01

### 中版本：B2 —— 进程插件 outbox 主动通信（新能力）

- **B2 核心**：一次性、无状态的进程插件（Python）可在一次 entry 调用内主动发起 PMB 通信。
  spawn 时向隔离工作目录注入 `host.py`，`import host` 后调 `host.send_to(target, payload)` /
  `host.publish(payload)`；这些函数**不开网络**，只把消息逐行写入 `outbox.jsonl`。
- **宿主回收 + 代投**：`invoke_entry` 子进程 exit 0 后 `collect_outbox()` 读回解析为
  `OutboxMessage` 并清空文件；`PluginHost::call` 取出 outbox 后**代表插件**逐条经 PMB
  `send_to`/`publish` 投递，最后返回方法返回值。
- **安全边界**：插件只「声明意图」，宿主仍是唯一 PMB 投递点，完整经过七道检查
  （大小/发送方状态/能力令牌/能力已授予/速率/HMAC 签名/nonce 防重放）+ 目标 RUNNING 校验。
- **capability 规范**：`host.py` 默认用基础能力 `plugin:message:send`；不支持裸 `"message"`/
  `"event"`（端到端测试首次跑出 `Bus("未知能力名 message")`，修正后通过）。
- **spawn_concrete 重构**：trait `spawn` 装箱独立 inherent `spawn_concrete`（返回具体
  `ProcessInstance`，便于测试访问沙箱字段）。
- **新增 5 个回归测试**（修复前会失败）；全量 582 passed / 0 failed（v3.4.5 基线 577 + 5）。
- B2 属 minor、与 v3.5.0 版本语义一致，无偏离；B1/B2/B3 至此全部闭环。
  详见 `releases/v3.5.0.md`。

## [v3.4.5] - 2026-10-01

### 小版本：A 类接地修复 + B1 主数据面编排接管 + B3 系统插件接线

- **A 类接地核实**：v3.4.2 §7「已知未修项」全部过时并更正——`persist` 调用零命中、
  按中标价结算已在 v2.9.1（`6f6b842`）修复、REST 现用单一 `REST_BEARER_TOKEN` 无 DID 问题。
- **A 类实际修复**：`marketplace/mod.rs` 补 `use serde_json::{json, Value}`（修复半成品
  8 编译错误）；`market_actor.rs` 补齐 `bids_for_plugin`/`match_task_with_winner`/
  `reputation_dimensions` 三个公开方法。
- **B1 主数据面接管**：编排器重写为持有 market 的业务网关，6 个纯插件决策 + 3 个 gated
  业务闸门（register_agent_gated / match_task_gated / settle_task_gated）；rest.rs 三点
  走「插件决策→宿主应用」，node.rs 接线，未构造编排器时 Option 降级到单体 market。
- **B3 系统插件接线**：新增 `SystemHandles`（store/peer）+ `block_on_net`；SYS_NET 真实
  peer_info/list_peers/nat_status、SYS_STORAGE 真实 stats/list_agents/list_tasks、SYS_CHAIN
  真实 record_anchor/list_anchors（status 诚实标注 rpc offline）；`boot_system(handles)`。
- **版本语义偏离**：B1/B3 原被 v3.4.2 判为 major（v4.0.0），经明确指令在本 patch 推进，
  已在发布文档说明并保留原判定。
- **新增 11 个回归测试**（编排器 7 + B3 4，修复前会失败）；全量 577 passed / 0 failed。
- 详见 `releases/v3.4.5.md`。B2（插件主动通信）留待 v3.5.0。

## [v3.4.2] - 2026-10-01

### 小版本：全局审核版本 —— 补齐插件生命周期/信任/黑名单接线 + 架构合规关卡

- **审计**：重新审视 v3.0.0 大版本及其下全部中/小版本，判定是否彻底贯彻「一切插件化」。
  完整报告见 `docs/AUDIT-v3.4.2-plugin-architecture.md`。结论：内核与插件层已彻底贯彻（10 项）。
- **A 类 patch 级 gap 修复（4 项，均在 `handle_plugin_api`）**：
  - G1：新增 `POST /plugins/{id}/stop`、`/start`（暂停/恢复但不卸载的热插拔中间态）；
  - G2：新增 `POST /plugins/trust`（信任第三方发布者，补齐 T3 安装前置）；
  - G3：新增 `GET /plugins/blacklist`（黑名单查询）、`POST /plugins/{id}/unblock`（解封）；
  - G4：entry_source 合规关卡升级为自动遍历 `official_ids()`（未来漏配 entry 即失败）。
- **新增 4 个回归测试**（修复前会失败）：stop/start 可达、trust fail-closed、
  blacklist 查询与解封、9 entry 自动遍历。
- **认证**：新增 POST 管理路由均在 `rest_authorize` 之后，受 fail-closed 保护；
  GET blacklist 只读放行。
- **B 类架构性 gap（v4.0.0 major 路线，不在 patch 实施）**：主数据面接管、插件主动通信、
  系统插件完整接线。
- 验证：cargo test 全量 **0 failed**；clippy 零警告；fmt 通过；no-panics/unsafe/version 关卡通过。

## [v3.4.0] - 2026-10-01

### 中版本：业务化第 9 个官方插件 chain-bridge（跨链信誉桥接，9/9 全业务化）

- **背景（根因）**：v3.3.0 后仅剩 1 个通用 exec 承载。chain-bridge 真实算法是
  `contracts/src/ReputationRegistry.sol`（207 行，原 ReputationBridge；`PoCVSettlement.sol`
  是独立任务结算、非 bridge 核心）。按「没有调用点的修复不算修复」，本版离线移植并业务化。
- **bridge entry**（`plugin/official/mod.rs` 新增 `BRIDGE_ENTRY`，并接入
  `official_entry_source`），内嵌纯 Python keccak256：
  - `add_verifier`/`remove_verifier`：仅 owner（`onlyOwner`），非空/不重复/上限 32，
    修复 GAP §9.4（此前 onlyVerifier 可无限铸验证者）；
  - `record`：验证者才可提交，四维 0–10000 bps，重复提交相同=幂等、不同=冲突；
  - `finalize`：法定人数 `verifier_count/2+1`，四维各取**中位数**（插入排序）定稿；
  - `get_latest`：未知 DID 返回全零快照（不报错）；另有 `submission_count`。
- **原型验证**：`bridge_proto.py` 经 `test_bridge.py` 全量 PASS（约 28 断言）；首跑 2 个
  失败的根因是测试脚本自身状态传递 bug（v1 后未 carry），修复后通过，插件代码无问题。
- **诚实边界**：离线不做 RPC、不读 `block.timestamp`，不声称真实跨链；`finalizedAt`
  由调用方传入；epoch key 用字符串组合，验证者用 DID 而非 address。
- 验证：cargo test 全量 **563 passed / 0 failed**（基线 560 + 3 个隔离测试）；clippy
  零警告；fmt 已应用；no-panics **0 sites**；unsafe-containment 通过（未新增 unsafe）。
- **里程碑：9 个官方插件全部业务化。**

## [v3.3.0] - 2026-10-01

### 中版本：业务化第 8 个官方插件 chain-anchor（离线锚定构造与双校验）

- **背景（根因）**：v3.2.3 后仅剩 2 个通用 exec 承载。进一步探查发现 chain-anchor 的真实
  算法不在 Rust 单体，而在真实 Solidity 合约 `contracts/src/AgentCardAnchor.sol`。按
  「没有调用点的修复不算修复」，本版把它离线移植并业务化。
- **技术闸门**：合约锚定键是 keccak256（Ethereum，domain byte `0x01`），不是 NIST
  SHA3（`0x06`）；Rust 端无 keccak 依赖，故在 entry 内纯 Python 实现 Keccak-f[1600]。
  - 调试闭环：分段二分锁定 bug 在 permutation 内（padding 换成 0x06 仍不符），**根因
    是硬编码 24 个轮常数 RC 表从 RC[2] 起约 12 个抄错**；以 pycryptodome `keccak.c:275`
    权威表替换后，空串 `c5d246…`、`abc` `4e0365…` 及 0/15/135/136/137/200/256/272
    长度全部通过。
- **anchor/verify entry**（`plugin/official/mod.rs` 新增 `ANCHOR_ENTRY`，并接入
  `official_entry_source`）：
  - `anchor`：cid/agent_did 非空、锚定者已授权、cidHash 首写后不可变（不可覆盖），
    写 `{cidHash, agentDidHash, anchoredAt, anchorer}`；
  - `verify`：记录存在 && `anchoredAt>0` && `agentDidHash==keccak256(agent_did)`（双校验）；
  - 另有 `get_anchor`、owner 管理的 `add_anchorer`/`remove_anchorer`。
- **诚实边界**：离线不做 RPC、不读 `block.timestamp`，不声称真实上链；`anchoredAt` 由
  调用方传入单调时间戳；状态经 payload 传入、写方法返回更新后状态。
- 验证：cargo test 全量 **560 passed / 0 failed**（基线 557 + 新增 3 个隔离测试）；
  clippy `--all-targets` 零警告；fmt 已应用；no-panics 关卡 **0 sites**；unsafe-containment
  通过（本版未新增 unsafe）。
- 边界：仅剩 1 个通用 exec 插件（chain-bridge，对应跨链信誉桥接、算法更复杂）。

## [v3.2.3] - 2026-10-01

### 小版本：业务化第 7 个官方插件 agent-skill（按技能发现智能体）

- **背景（根因）**：9 个官方插件中仍有 3 个是通用 exec 承载。按「没有调用点的修复不算
  修复」，本版把 `agent-skill` 业务化——它在单体中有真实的
  `marketplace/mod.rs::discover_by_skill`（配合 `register_agent` 维护的技能反向索引）。
- **discover entry**（`plugin/official/mod.rs` 新增 `SKILL_ENTRY`，并接入
  `official_entry_source`）：输入 `skill`（技能标签）、`agents`（卡片列表）。先按卡片
  声明的技能构建反向索引（对齐 `register_agent` 的 `skill_index` 构建），再做
  **精确**（忽略大小写/首尾空白）标签匹配，返回所有声明该技能的卡片。
  - 精确匹配而非子串包含（单体 `discover_by_skill` 用 `get`，`search_agents` 才用
    `contains`）；`"Python"` 命中查询 `"python"`，`"Python Developer"` 不命中。
  - 同一标签重复声明时索引对同一 id 去重。
- **诚实边界**：探查确认 `chain-anchor` / `chain-bridge` 在单体中**无独立算法**
  （`chain/mod.rs` 全文仅 `pub mod pocv;`），不凭空编造，本版不业务化、保留为通用 exec。
- 验证：cargo test 全量 **557 passed / 0 failed**（基线 554 + 新增 3 个隔离测试）；
  clippy `--all-targets` 零警告；fmt 已应用；no-panics 关卡 **0 sites**；unsafe-containment
  通过（本版未新增 unsafe）。
- 边界：仅剩 2 个通用 exec 插件（chain-anchor / chain-bridge，单体无独立算法）。

## [v3.2.2] - 2026-10-01

### 小版本：业务化第 6 个官方插件 swarm-emergence（群体智能涌现检测）

- **背景（根因）**：9 个官方插件中仍有 4 个是通用 exec 承载（清单在、插件能跑，但真实
  算法锁在单体、插件本身没有可调用业务方法）。按「没有调用点的修复不算修复」，本版把
  `swarm-emergence` 业务化——它在单体中有真实、独立的 `swarm/emergence.rs::detect`。
- **detect entry**（`plugin/official/mod.rs` 新增 `EMERGENCE_ENTRY`，并接入
  `official_entry_source`）：输入 `history`（每条 `[timestamp, throughput, latency]`）、
  `threshold`、`window_size`。取最近窗口与更早窗口对比：
  - 吞吐量增长超阈值 → `collaboration`（协同涌现）；
  - 延迟下降超阈值 → `load_balancing`（负载均衡涌现）。
  窗口不足/无对比基线 → 空信号；严格对齐单体的倒序窗口（older 可能不足 window）与
  严格 `>` 判定。
- 性能指标（throughput/latency/strength）用 f64 合理（非金额 Money）；entry 从 JSON 取数，
  天然不含 NaN/Infinity。
- 验证：cargo test 全量 **554 passed / 0 failed**（基线 551 + 新增 3 个隔离测试）；
  clippy `--all-targets` 零警告；fmt 已应用；no-panics 关卡 **0 sites**；unsafe-containment
  通过（本版未新增 unsafe）。
- 边界：仍剩 3 个通用 exec 插件（agent-skill / chain-anchor / chain-bridge）；
  `fault_tolerance`/`evolution` 在单体 detect 本就不产出，忠实移植未额外实现。

## [v3.2.1] - 2026-10-01

### 小版本：PMB 安全通信 —— 消息 HMAC 签名 + nonce 防重放 + 总线端到端接线

- **背景（根因）**：插件总线此前**没有任何密码学保护**，防伪造只靠进程内令牌绑定
  `plugin_id`；`host.rs` 注册/状态用到 bus 但**从不调用 `dispatch`**（dispatch 仅在
  bus 单元测试中被调用），`register` 返回的 Receiver 全部被丢弃，消息没有落点。
- **消息认证**（`plugin/bus.rs`）：`PmbMessage` 新增 `nonce`、`signature`；每个路由
  注册时生成 32 字节随机会话密钥；新增 `signing_bytes`（规范 JSON 视图）、
  `compute_signature`（**HMAC-SHA256**）、`constant_time_eq_hex`（**常量时间比较**）。
  `validate_sender` 由五道扩为七道：第 6 道签名（缺失/不符即拒绝，防篡改/伪造）、
  第 7 道 nonce（重复即拒绝，防重放；seen_nonces 超 1024 淘汰最旧）。
- **总线接线**（`plugin/host.rs`）：私有 `dispatch_for_plugin`（先签名再 dispatch，宿主
  发的每条消息都走同一认证闸）；新增 `send_to`（点到点 Request）、`publish`（广播
  Event）；新增 **`open_inbox(id)`**（注册纯收件箱路由、置 RUNNING、返回 Receiver，
  用于桥接进程插件/外部网络传输；**无能力令牌、只能接收不能发送**；id 已存在报错）。
- **静态关卡修复**（`check-no-panics.mjs`）：`stripLiterals` 三个真实 bug——①
  `broadcast` 的 `br` 被误判为 byte-raw-string 前缀导致 brace 错位（raw 改为严格正则
  先跑）；② lifetime `'_`（`Formatter<'_>`）被当 char literal 吞到文件末尾（char 分支
  重写为转义/单字符/lifetime 三类）；③ 重写时误删普通字符 fallback 导致无限循环（已加回）。
  修复后关卡正确报出 7 处真实生产 panic（此前长期报 0）。
- **7 处生产 panic 类型化**：`mcp/protocol.rs` 新增 `McpMethod::from_wire`（消除 parse
  unwrap）；`bus.rs` validate_sender 的 expect 改 `ok_or_else`、dispatch 目标分支合并 get
  去 unwrap；`relay_pool/mod.rs` 新增 `RelayClass::from_wire`（消除 4 处 parse unwrap）。
- 验证：cargo test 全量 **551 passed / 0 failed**；clippy `--all-targets` 零警告；fmt
  已应用；no-panics 关卡 **0 sites**；unsafe-containment 关卡通过。
- 边界：消息签名为**对称 HMAC**（证明来自持有该路由密钥的一方），不提供非对称来源证明；
  跨节点端到端非对称签名留待后续版本。已知未修项沿用（persist 错误被丢弃、结算按托管
  总额、API token 无法表达标准 DID）。

## [v3.2.0] - 2026-10-01

### 中版本：业务插件化深化 —— 再 3 个核心官方插件承载真实业务 entry

- **背景**：v3.1.0 只让 economy-reputation、market-match 两个插件承载真实业务，其余 7 个
  官方插件仍是通用 `exec` 容器。v3.2.0 继续把单体真实算法移植到插件，使 9 个官方插件中
  **5 个承载真实业务**（中版本只发新功能插件，不动内核）。
- **新增 3 个内嵌 entry 业务模块**（`plugin/official/mod.rs`）：
  - **market-settle `audit`**：移植自 `marketplace/settlement.rs::independent_audit`。
    **只信任流水 records**，逐笔独立重放（Deposited→to 加、Slashed→from 减、其余搬运），
    与当前 balances 逐账户比对（重放账户 ∪ 当前账户，覆盖 ghost/缺失/篡改），校验
    `expected_total == actual_total`，并在提供引擎聚合时校验聚合一致。返回 `passed`、
    首个不匹配账户的 `expected/actual`、`mismatches`；
  - **scheduler-task `route`**：移植自 `scheduler/router.rs::assign_task`，按负载与延迟
    评分选最佳执行者（`score = 1-(load/max)*0.3-min(lat/1000,1)*0.2`），估算成本
    `budget // 候选数`。把单体潜在的除零隐患改为类型化结果：空候选→`no_candidates`、
    `max_concurrent<=0`→`no_capacity`、全饱和→`all_saturated`；
  - **agent-card `validate`**：校验 `MarketAgentCard` 字段——agent_id 须为 DID、
    name/version 非空、skills 为列表、reputation_score/success_rate ∈ [0,1]、stake 非负，
    返回 `valid` 与逐项 `errors`。
- **测试**：新增 7 个 process runtime 隔离测试（settle 2：一致通过 / 篡改余额必败；
  router 3：选低延迟 / 空候选 / 零容量；card 2：合法通过 / 非法≥4 项错误）。
- **热更新/并行竞态修复（治本）**：macOS CI 上 `load_legacy_abi_2_plugin` 偶发失败
  （`entry 写入失败: No such file or directory`）。根因是工作目录名只含插件 id，
  并行测试与 `hot_reload` 双缓冲的同 id 多实例共享目录，一个 `destroy` 会删掉另一个
  实例正在写 entry 的目录；附带发现 `hot_reload` 后旧 `destroy` 会删除新版本仍在用的
  目录（生产 bug，原测试只断言版本、从不调用新版本 entry 故未发现）。修复：新增全局
  `SANDBOX_SEQ`，工作目录改为 `au-sandbox-{id}-{pid}-{seq}` 使每个实例独占；
  `fork_from` 改为从 `sb.work_dir()` 取实际目录。新增回归测试
  `hot_reload_new_entry_remains_callable`（旧代码稳定失败 5/5）。
- 验证：cargo test `--lib` **259 passed / 0 failed**（连跑 8 轮稳定）；cargo test 全量
  **543 passed / 0 failed**（连跑 3 轮）；clippy 零警告；`node test/regression.js` 14 通过；
  真实 daemon 端到端通过
  （host_version=0.3.20，13 插件全 Running；audit 抓到 A expected=100/actual=150 →
  passed=false；route 选 b、estimated_cost=50、零容量→no_capacity；validate 合法→valid）。
- 边界：9 个官方插件中 5 个已承载真实业务，其余 4 个（agent-skill/swarm-emergence/
  chain-anchor/chain-bridge）仍为通用 exec；entry 为 Python，依赖运行环境的 Python3。

## [v3.1.0] - 2026-10-01

### 中版本：业务插件化 —— 官方插件真正承载业务逻辑（entry 模块 + 自动装配）

- **背景**：v3.0.0 建立了插件框架，但 process 官方插件只是通用 `exec` 容器，manifest 的
  `entry` 字段未被加载，真实业务算法（信誉计算、市场匹配）仍留在单体 `marketplace/` 里，
  插件“有壳无业务”。v3.1.0 让官方插件承载业务。
- **内嵌 entry 业务模块**（`plugin/official/mod.rs`）：新增 `EntrySource` 与
  `official_entry_source()`，把单体真实算法移植为插件 Python 模块：
  - **economy-reputation**：移植自 `marketplace/reputation.rs::overall`，
    `overall = quality*0.35 + speed*0.20 + honesty*0.30 + availability*0.15`；
  - **market-match**：移植自 `marketplace/mod.rs::match_task`，遍历 bids，跳过
    price≤0，`cost=reputation/price`、`latency_penalty=1/(1+latency_ms/1000)`，
    `score=cost*latency_penalty` 取最高；空 bids 返回 `no_bids`、无有效 bid 返回
    `no_valid_bid`；
  - 其余 7 个官方插件暂为通用 exec 承载（entry 来源为 None）。
- **process runtime 加载/调用 entry**（`runtime/process.rs`）：`ProcessInstance` 增加
  `has_entry`；spawn 时把 entry 写入隔离工作目录；新增 `invoke_entry()`（隔离进程中
  `import plugin` → `json.loads(payload)` → 调用方法 → `json.dumps` 输出）。方法名必须是
  合法 Python 标识符（否则拒绝，防引导注入），payload 必须为合法 JSON，entry 方法不存在
  （exit 2）与非法方法名均被类型化拒绝。
- **官方插件随内核自动装配**：新增 `PluginHost::boot_official()`，在 `boot_system()` 后
  装配 9 个 T1 官方插件。官方插件随内核构建、构建链路可信，故跳过发布者签名校验（与 T0
  同一信任来源），但仍走 **process 进程隔离**与 **能力矩阵**（能力按 `Tier::Official`
  解析、越权拒绝）。修复了此前 boot 后 GET 只有 4 个系统插件、调用官方插件“未找到”的缺陷。
- **测试并行修复**：相同插件 id 的多个测试共享工作目录互相 destroy，改为 `unique_rt()`
  为每个测试提供独立 base 目录。
- 验证：cargo test 全量 **786 passed / 0 failed**（lib 251 + 集成 535）；其中 process
  runtime 11 单测（overall 精确值 0.75、match 选中高信誉者、空 bids、status、方法缺失、
  非法方法名）；编译零警告；真实 daemon 端到端通过（boot 打印 4 T0 + 9 T1，
  overall→0.75、match→winner=b/score=0.00818、empty→no_bids/winner=null）。
- 边界：仅信誉与匹配两个业务插件完成 entry 化，其余 7 个官方插件仍为通用 exec；
  entry 当前为 Python，跨平台依赖运行环境的 Python3。

## [v3.0.0] - 2026-10-01

### 大版本：一切插件化架构重构（热更新/热插拔/热兼容、五级插件体系）

- **插件内核**：新建 `gsn-core/src/plugin/`（17 文件）——注册中心 Registry、插件总线
  Plugin Bus（PMB）、权限仲裁器 Arbiter、生命周期 Lifecycle、能力模型、黑名单 Blacklist、
  清单 Manifest、宿主 PluginHost。
- **五级插件体系**：系统（T0/Ring0，进程内）、官方（T1）、认证（T2，开发者+官方副签）、
  第三方（T3，最小能力、默认禁网）、黑名单（Ring-1，禁止加载）；id 前缀即分类。
- **热更新/热插拔/热兼容**：新版本预加载 + 总线原子切换 + 失败自动回滚（registry 专用
  `replace` 路径）；任意时刻加载/卸载；ABI 主版本协商加载低版本插件。
- **隔离**：T1/T2/T3 独立进程（复用 v2.9.1 已验证 ProcessSandbox + 能力闸门）；Windows
  Job Object；无法强制且无 waiver 即 `PolicyNotEnforceable`。
- **T0 系统插件真实运行**：identity（cast/resolve/fingerprint）、net/storage/chain（status
  真实声明能力）。
- **REST 外部 API**：`/api/v1/plugins` 列表/详情/安装/call/热更新/卸载；变更类走认证闸门。
- **版本映射升级**：Rust crate 规则改为 `0.X.(Y*10+Z)`，3.0.0 → 0.3.0；修复 bump/check
  脚本硬编码 major=2 导致 major 3 误映射。
- 验证：cargo test 528 passed/0 failed；plugin:: 72 passed；编译零警告；静态关卡
  （panic=0、unsafe 全有 SAFETY）；真实 daemon 端到端通过（含未认证 401）。
- 边界：WASM 运行时为可选 feature、当前构建类型化拒绝（无 wasmtime）；9 个官方插件
  清单/承载就位，业务逻辑迁移在后续中版本。

## [v2.9.2] - 2026-10-01

### CI 工程化：Windows 矩阵 + 静态关卡接入

- **Windows 矩阵**：rust-test 矩阵加入 `windows-latest`，cargo fmt/build/test/clippy 首次在
  Windows 上跑（此前 v2.9.1 仅 ubuntu/macos；winjob.rs 等 Windows 专属代码此前无 CI 覆盖）。
- **静态关卡接入 CI**：新增 `static-gates` job，跑 `check-no-panics.mjs`（剥离注释/字符串/
  char 后扫描，生产代码 panic 站点必须为 0）与 `check-unsafe-containment.mjs`（每个 unsafe
  必须有对应 `// SAFETY:` 理由）。两个脚本随本版正式提交（v2.9.1 仅本地就绪）。
- **Windows 矩阵实际抓到并修复 node abort 134**：根因是 `env_clear()` 后丢失 `SystemRoot`，
  Node 启动时 CSPRNG 初始化断言失败（`ncrypto::CSPRNG(nullptr, 0)`）；修复保留 SystemRoot、
  TEMP/TMP 指向沙箱内 tmp，并给仅 Unix 使用的 `shell_quote` 加 `#[cfg(not(windows))]`。
- 验证：CI 全矩阵（10 job，含 Windows）全绿；Windows 上 fmt/build/test/clippy/daemon help
  全部通过；Linux 本地 clippy 零警告、cargo fmt 干净。

## [v2.9.1] - 2026-10-01

### 沙箱能力声明闸门 + 生产 panic 归零 + 快照持久化告警

- **能力声明系统**（`sandbox/capability.rs`）：12 项 Capability、显式带理由的 Waiver、
  `CapabilityDeclaration.check` 唯一消费点；进程后端按平台声明真实能力，无法强制且无 waiver
  即 `PolicyNotEnforceable`（422）；**默认配置不执行**，trusted_local 显式 waiver 才本地执行；
  Unix 加 `ulimit -v/-t/-u/-n`（内存/CPU/进程/文件），mem_mb 默认 768 兼容 Node V8；
  daemon 读 `GSN_SANDBOX_BACKEND`，拼错值按禁用处理（安全配置 typo 不静默选中更弱后端）。
- **生产 panic 归零**：market_actor、erasure（Result 化）、llm、marketplace、net/peer、
  scheduler、swarm 共 12 站点，消除 `unwrap`/`expect`/NaN 路径。
- **快照持久化告警**：5 处快照 `let _ =` 改 `warn_persist`，不再静默吞掉落盘失败。
- **CI**：rust-test 加 `cargo fmt --check`；v2.9.1 矩阵 ubuntu/macos。
- **unsafe**：winjob.rs 3 个 unsafe 块全部补 `// SAFETY:`。
- 验证：clippy 零警告、全量测试通过（0 failed / 0 ignored）；v278/v280/v287 改显式 trusted_local。
- 边界：Windows 矩阵与静态关卡（panic/unsafe 机械检查）在 v2.9.2 接入；v2.8.8 跳过。

## [v2.9.0] - 2026-09-30

### 桌面工作台大版本（Workbench，继续 Tauri 2）

- 极简 Tauri 客户端升级为「工作台」，深度参考 DeepSeek Harness（不迁移 Electron）；
- **9 大视图**：工作区总览、任务、智能体（卡片/团队）、终端、文件（Excel/CSV/TSV 预览）、工具（沙箱执行）、模型提供商、插件、设置；
- 默认工作区（免选文件夹）、过程展示分级（results/steps/full）、托盘常驻/单实例/关闭隐藏、后台任务、模型提供商统一入口；
- daemon 托管：查找/启动/健康检查/停止/重启，仅绑 loopback；
- 后端列表：`GET /api/v1/agents?all=1`、`/api/v1/tasks?all=1` 返回真实列表（无参仍统计）；
- 构建 pipeline：client-build 三平台改构建 desktop，ci.yml 新增 workbench-check；
- 验证：clippy 零警告、448 测试通过、版本 10 点一致、vite build 通过。
- 边界：LLM 仍 mock、插件为实验开关、沙箱依赖可执行后端、实时为轮询；v2.8.8 跳过。

## [v2.8.0] - 2026-09-29

### Agent Sandbox 集成核心链路（大版本）

- 新增 `sandbox/api.rs`：REST + E2B 兼容纯函数处理器 `handle_api`；
  端点 /api/v1/sandboxes（创建/list/get/exec/pause/resume/destroy）；
  E2B 别名 /v1/sandboxes、/commands；错误码 404/403/400/429/422/500；
- node.rs 经 `spawn_blocking` 独立分流接入 SandboxManager（不阻塞异步运行时）；
- 新增 `mcp/sandbox_tools.rs`：7 个沙箱 MCP 工具（create/list/get/run_code/pause/resume/destroy）；
  sse/stdio 双路径接入；mutating 工具无 token 默认拒绝；
  公共构造 tool()/ParamBuilder 上移 mcp/tool.rs；
- 新增 `sandbox/k8s.rs`：AgentSandbox CRD 清单 + 示例 + reconcile 步骤（可 kubectl apply）；
- v280 测试 5 项全过；clippy 零警告；全量 Rust、JS 21、回归 14 全绿。

## [v2.7.9] - 2026-09-29

### Agent Sandbox 安全边界

- 新增 `sandbox/security.rs`：
- NetworkGuard 出站策略落地（默认拒绝/白名单/裸IP/通配）；
- AuditLog append-only 审计（含 JSON Lines 落盘）；
- PermissionChecker scope 权限校验；
- EvidenceGrade 三级证据分级（lowercase 序列化）；
- v279 测试 13 项全过。

## [v2.7.8] - 2026-09-29

### Agent Sandbox 生命周期管理与弹性供给

- 新增 `sandbox/manager.rs`：SandboxManager；
- 预热池（warm pool）、acquire/release、休眠唤醒 wake；
- checkpoint 工作目录快照、fork_from 状态复用（活沙箱或 checkpoint）；
- evict_idle 回收休眠沙箱并补预热、shutdown 清理；
- SandboxConfig 新增 work_dir_base，统一沙箱目录；
- v278 测试 13 项全过。

## [v2.7.7] - 2026-09-29

### Agent Sandbox 进程级隔离运行时

- 新增 `sandbox/runtime/process.rs`：`ProcessSandbox` 实现 Sandbox trait；
- 独立临时目录、safe_join 拒绝对路径/`..` 逃逸、env_clear 不继承宿主环境；
- timeout 强杀 + ulimit 进程/句柄上限；白名单解释器，shell 需显式 allow_shell；
- run_code(CodeLanguage, code) 支持 Python/JavaScript；write_file/read_file 沙箱间隔离；
- 修复默认 max_processes 64 导致 node V8 worker 线程 abort，提升到 512；
- v277 测试 17 项全过（含超时强杀、路径逃逸、文件隔离）。

## [v2.7.6] - 2026-09-29

### Agent Sandbox 核心架构（大版本重构第一步）

- 新增 `sandbox/error.rs`：统一错误类型（EnvBlocked/IsolationViolation/ExecFailed/ResourceLimitExceeded/InvalidConfig/InvalidLifecycle/NetworkDenied 等）；
- 新增 `sandbox/config.rs`：IsolationLevel、ResourceLimits、NetworkPolicy、FilesystemPolicy、SandboxConfig + validate；
- 新增 `sandbox/state.rs`：生命周期状态机（Pending→Creating→Starting→Running⇄Paused→Stopping→Stopped/Failed）；
- 新增 `sandbox/identity.rs`：沙箱临时身份、Agent 长期身份、短时执行令牌；
- docker/firecracker 更新为显式环境探测，无 daemon/KVM 即 EnvBlocked；
- 设计文档 `docs/sandbox/architecture.md`；新增 v276 测试 15 项全过。

## [v2.7.5] - 2026-09-29

### DCUtR 直连升级 + 多 relay 多通道同时在线

- `P2pPeer.direct_peers` 跟踪 DCUtR holepunch 升级成功的对端，`/relays` 响应新增 `direct_peers` 字段；
- `process_swarm_event` Dcutr 分支：成功打直连成功日志、失败回退 relay；
- `ensure_channels` 默认 3 条 relay 通道同时在线，relay 掉线自动切换；
- `select_parallel` 按分类优先级选健康 relay；容量策略沿用 v2.5.5 不回退。

## [v2.7.4] - 2026-09-29

### 重启恢复修复：agents/tasks 未注入内存 market

- 修复 Mac 真机 Bug：重启 daemon 后账本/余额/守恒从 SQLite 正确恢复，但 `/agents`、`/tasks/{id}`、`/stats` 全空/0/not_found；
- 根因：`spawn_with_store` 对 `load_agents()/load_tasks()` 只取 `.len()` 打日志，未把业务对象注入内存 market；
- 新增 `TaskState::from_label()`（大写 label 反解析，坏值回 Open）；
- 新增 `restore_agents_from_store()` / `restore_tasks_from_store()`，启动时真正注入内存；
- 新增持久回归 `tests/v274_test.rs`（3 项）；gsn-core `0.2.74`。

## [v2.7.3] - 2026-09-29

### 实机部署修复：发布缺字段 422 与认证验收后证据不升级

- `TaskSpec.state` 加 `#[serde(default = "default_task_state")]`，缺省即 Open，修复缺 state 即 422、传了又被覆盖的矛盾；
- 认证式 BFT 委员会判定 Stop 后，将不可信结果证据提升为 `Verified`，消除「验收通过却无法结算」死路；非认证 `verify_result` 不提升；
- 新增持久回归 `tests/v273_test.rs`（2 项）；gsn-core `0.2.73`。

## [v2.7.2] - 2026-09-28

### NAT 占位检测不再猜测类型（GAP §5.6）

- `NatType` 新增 `Unknown` 变体；`detect_nat_type` 返回 Unknown 而非硬编码 PortRestrictedCone；
- 同步两处固化测试；`MeshTopology.nat_type` 不再暴露假精确测量值。

## [v2.7.1] - 2026-09-28

### 安全：stableStringify 拒绝非法/静默错误载荷（GAP §9.1）

- 对象中 `undefined` 键省略、数组 `undefined` 元素转 null，不再产出非法 JSON；
- BigInt / Date / 顶层 undefined 显式抛 TypeError，不再静默序列化为 `{}` 或抛晦涩异常；
- 新增第 17 项 JS 测试覆盖正反例（JS SDK 测试 20→21）。

## [v2.7.0] - 2026-09-28

### 全平台客户端大版本

- 首次随版本重建并交付全平台客户端（desktop/ Tauri + client/ 跨端壳），覆盖 macOS / Windows / Linux；内嵌 SDK `@twinsearth/agent-universe@2.7.0`。
- 版本统一 2.7.0 ↔ gsn-core `0.2.70`。
- 无运行时逻辑变更（v2.6.0–v2.6.9 修复的分发节点）。

## [v2.6.9] - 2026-09-28

### 文档/测试一致性：停止过度承诺（GAP §5.1 / §7.6 / §8.6 / §9.6 / §9.7）

- §8.6：MCP SSE 模块头诚实标注 `handle_get` 发一帧即关、非真长连接，不再宣称「服务端推送」；
- §7.6：`probe_all` 注释标注为静态区域谓词查表、不做真实 I/O；
- §9.7：`test/regression.js` 头部说明 REG-020/021/022/030/031 跑在 JS 参考实现上，不等价于 Rust 守护进程行为；
- §9.6：`js/test/test.js` 头注释测试数 14→20；
- 无运行时行为变更。

## [v2.6.8] - 2026-09-28

### 安全 + 修复：MCP 认证闸门与 LLM 去 panic（GAP §7.5 / §8.3 / §8.4 / §8.5 / §8.8）

- **§8.8 严重**：HTTP MCP 端点此前无认证即挂 `market_deposit`/`arbitrate`/`settle_task` 等动钱工具；新增 `MCP_BEARER_TOKEN` Bearer 闸门，未配置令牌时默认拒绝全部写/动钱工具（安全失败），配错令牌返回 401；
- §8.3：`RequestId` 增加 `Null`，解析/非法请求回 `"id": null`；
- §8.4：`tools/call` 未知工具改正常 result + `isError:true`，不再回协议级错误；
- §8.5：MCP server 增加握手状态机，未 initialize 前除 initialize 外全部返回 `-32002`；`handle_initialize` 读取 `protocolVersion`；
- `ToolResult.is_error` 序列化改名 camelCase `isError`；
- §7.5：LLM 五个适配器 `chat()` 不再 `.unwrap()` panic，请求失败/空 choices 友好降级为错误文案。

## [v2.6.7] - 2026-09-28

### 修复：记忆层加固（GAP §7.1 / §7.2 / §7.3 / §7.4 / §7.7）

- 跨代/审计哈希链从假 `DefaultHasher` 16 hex 改为真实 SHA-256（64 hex），同时保存载荷，新增 `verify_chain()` 重算并报首个断裂索引；
- 个体记忆淘汰从按访问计数（实为 LFU）改为真 LRU（`last_used` 逻辑时钟），测试在 LFU 规则下会失败；
- 防污染落地：评分整数 bps、消除 NaN panic；群体库质量由成败次数派生（不可由发布者谎报），低于阈值拒绝上库，整数排序；
- 飞轮 `is_spinning` 诚实标注为计数快照、未接真实拓扑闭环；
- `swarm/memory.rs` 盲猜拒绝采样在 budget≥choices 时的死循环加 break 防护。

## [v2.6.6] - 2026-09-28

### 修复：存储 / HTTP 加固（GAP §6.2 / §6.3 / §6.4 / §6.5 / §6.6）

- persist.rs 22 处 `.lock().unwrap()` 改 `into_inner()`，锁中毒不再杀死存储层；
- accept 瞬时错误记录并退避继续，不再一次错误终止整个 daemon；
- 写操作端点强制 POST，GET 误触发结算/仲裁等改状态操作返回 405；
- 错误分类改机器码前缀（NOT_FOUND/CONFLICT/BAD_REQUEST），不再匹配中文字符串「不存在」；
- url_decode off-by-one 修正，末尾 %XX 与多字节 UTF-8 正确解码。

## [v2.6.5] - 2026-09-28

### 修复：分层拓扑确定性重写（GAP §5.2 / §5.3 / §5.4 / §5.7）

- `route_hops` 不再恒返回 7：按两房间在聚合树中的最近公共祖先（lca）真实计算，未注册节点返回 `None`；
- `logical_edges` 补 N vs 2N 规模对照测试，证明边数线性而非二次；
- 房间归属改为 BTreeSet 排名 ÷ fanout，纯确定性、与插入顺序无关（500 id 正序/逆序构建逐一对等），`join` 退化为 O(log N)；
- `fanin_of` 补回漏算的上行边；
- Identify / MeshConfig 协议版本改 `env!("CARGO_PKG_VERSION")` 唯一来源，消除 `/gsn/0.2.64` vs `gsn/0.2.53` 漂移。

## [v2.6.4] - 2026-09-28

### 修复：共识 / 身份签名加固（GAP §3.3 / §3.6 / §4.2–§4.9）

- **BFT 委员会算术安全（§3.3）**：`min_n = 3f+1` 与 `quorum` 改 checked，溢出即 Err/安全降级，不再普通乘法溢出。
- **贡献验证带签名身份（§3.6）**：新增 `verify_contribution_signed`（验签覆盖 `hash||verifier_did`、禁自验、去重、饱和计数），杜绝重复投票凑数与自验；旧方法同步硬化。
- **规范签名 Result 化 + 递归剥离（§4.2/§4.3）**：`canonical_object` 要求根为对象、`strip_signatures` 任意深度递归剥离；`sign_hex/verify_hex/canonical_payload` 全改 `Result`，不可序列化对象不再静默签空字节。
- **跨语言键序码点化（§4.4）**：JS 排序器从 UTF-16 码元序改为 Unicode 码点序，新增 astral 平面键序向量钉住三端一致。
- **规范字节契约（§4.5）**：引入 `PROTOCOL_VERSION = "nau/1"`，签名覆盖规范形式，改规范字节必须 bump 版本；重放由 nonce+时间戳承担。
- **弱公钥拒绝（§4.6）**：`is_weak_pubkey` 拒绝长度错 / 全零 / 全 0xFF / 全相同公钥，保留 8 字节指纹兼容上游。
- **时钟端口注入（§4.8）**：新建 `aca::clock`（`SystemClock` 饱和不 panic + `ManualClock` 确定性），消除八处早于 1970 即 panic 的路径。
- **常量时间比较文档（§4.9）**：明确公开摘要 `==` 不构成安全边界。

### 验证

- Rust 全量 **146** 单元 + 全部集成 **0 failed / 0 ignored**；JS SDK **20** passed；clippy `-D warnings` 清零；check-version 通过（2.6.4 / 0.2.64）；上游跨语言签名向量逐字节命中保持。

## [v2.6.3] - 2026-09-28

### 修复：经济结算收尾——重复注册拒绝、出价脱钩校验、Rejected / DuplicateWork 可达（GAP §2.6/§2.7/§2.8）

- **重复注册拒绝（GAP §2.6）**：`register_agent` 新增 `contains_key` 检查，同一 agent_id 再次注册即报错，不再重复锁定质押 / 重复索引 / 静默覆盖，与 JS 侧和 REG-020 对齐。
- **投标报价强校验（GAP §2.7）**：`submit_bid` 校验报价必须为正、不超预算、任务处于 Open；0 / 负 / 超预算 / 已关闭一律拒绝；`match_task` 成本打分简化为 `rep_score / price`，删除 price<=0 特殊分支。
- **终局拒绝与重复劳动可达（GAP §2.8）**：`TaskState` 新增 `Rejected` 终态与转移边；`SettlementReason` 新增 `DuplicateWork`；新增结果内容哈希（SHA256）检测——返工后提交完全相同结果自动转 Rejected 并标记 DuplicateWork；新增 `reject_task`；拒绝结算付 0、退全额预算、罚没 10% 质押、记信誉失败；NoQuorum 流程重开清除哈希豁免合法重试。

### 验证

- Rust 全量 0 failed / 0 ignored；v234 套件 **74** passed（新增 test_bid_price_validation / test_end_to_end_rejected_flow / test_end_to_end_duplicate_work_flow）；clippy `-D warnings` 清零；check-version 通过（2.6.3 / 0.2.63）。

## [v2.6.2] - 2026-09-28

### 修复：版本唯一来源、合约可部署、纠删码真修与网络替身诚实化（GAP §9.3/§9.4/§5）

- **版本唯一来源 + 一致性断言**：版本号此前手工登记、已漂移。新建根 `VERSION`（唯一权威）与 `scripts/check-version.sh`（断言根 / js / client / desktop package.json、aip-sdk-py、Tauri 壳、gsn-core 全部一致，npm X.Y.Z ↔ gsn-core 0.2.(Y*10+Z)）；ci / publish / release 均在 gate 跑，漂移即红；修复表格 awk 把 markdown 转义竖线 `\|` 误当字段分隔导致“当前值”列不刷新。
- **Solidity 合约重写可部署**：此前 4 个中 3 个不可部署。重写 GovernorToken / AgentCardAnchor / PoCVSettlement / ReputationRegistry，接入 Hardhat，编译通过、18 个逐缺陷测试全过；ci 新增 contracts-check。
- **网络替身诚实命名**：`GsnNode / KademliaClient / GossipSub`（纯 HashMap 进程内替身）重命名为 `InMemoryNode / InMemoryKademlia / InMemoryGossip`，同步 lib / ffi / 测试。
- **真实 Reed-Solomon 纠删码**：旧“校验片 = 数据片副本 + SHA256”是假实现。引入 reed-solomon-erasure v6.0.0，数据片丢失可靠校验片真实重建，超额丢失明确报错。
- **NAT / TEE 诚实标注**：nat 模块不做真实打洞、`with_tee` 只置标志位不实例化 enclave / 不做远程证明，均加文档指向真实实现。

### 验证

- Rust 全量约 **325** passed / 0 failed / **0 ignored**（lib 138）；JS SDK **19**、历史回归 **14**、Hardhat **18** passed；clippy `-D warnings` 清零；check-version 通过。

## [v2.6.1] - 2026-09-27

### 修复：账本落盘与重放恢复、MCP 参数校验（GAP §6.1/§8.1）

- **只追加流水落盘 + 重放恢复**：旧 `PersistentStore` 无余额 / 流水表，`load_agents/load_tasks/upsert_task` 零调用，daemon 打开库只打印计数、actor 总以空市场启动，重启即丢账。新增 `replay_records`（只信任流水、有符号增量逐笔重放、checked 防溢出）与 `SettlementEngine::restore`（重建余额 / 充值 / 罚没 / 已结算任务）；新建 `ledger_entries` 表（JSON 列 + SQLite 事务，无半行）与 `append/load/count`，损坏行容错跳过；actor 新增 `spawn_with_store`（启动恢复账本、写后增量 append + 快照 upsert），daemon 改用之。
- **MCP 单一来源参数校验**：旧 schema 是两份手工列表且从不校验，错误参数被 `unwrap_or(0.0)/unwrap_or("")` 静默吞掉。新增 `validate_arguments`（缺必填 / null 占必填 / 类型错误指名参数），tools/call 在执行器前用工具自身 inputSchema 校验，失败返回 **-32602**；schema 与 tools/list 同源。
- `independent_audit` 重构为复用 `replay_records`，统一审计与重放口径。

### 验证

- Rust v261 **5** passed、lib **138** passed（含 storage 持久化 3 用例：drop/reopen 往返、同 id 覆盖、损坏行容错），全量 `cargo test --jobs 2` 0 failed / 0 ignored；`cargo check --tests` 通过。

## [v2.6.0] - 2026-09-27

### 新增：状态机恢复边、证据分级结算闸门（GAP §3.4 及证据谓词零调用）

- **集中状态转移表 + 恢复边**：旧状态机无集中转移合法性表，任务进入 `NoQuorum` 后无任何出边（吸收态，永久卡死），`Rework` 也缺回 `Running` 的边。`TaskState` 新增 `can_transition_to`（集中合法边表）与 `transition`（非法转移报错），补上 **NoQuorum→Open**、**Rework→Running** 两条恢复边；`AgentMarket` 新增 `resume_after_rework` / `reopen_after_no_quorum`（均显式校验前置态），匹配 / 提交 / 验收 / 结算状态赋值全部改走 transition。
- **证据分级强制结算闸门**：旧 `settle_task` 只看 `envelope.is_success()`，信封缺失时 `unwrap_or(true)` 放行；`is_trustworthy()` 已定义但全仓零调用。现 policy 非 None 时要求结果信封存在且 `evidence_grade.is_trustworthy()`（Verified/CpuProto），Unverified 或缺信封即拒付、状态保持 Accepted。
- **policy=None 提交即验收**：`submit_result` 在 policy=None 时直接转 Accepted（无需 QA），状态表加 Matched/Running→Accepted 边；同时豁免证据门禁。
- **三端同步**：Rust（task.rs / mod.rs / rest.rs 新增 resume、reopen 端点 / market_actor.rs）、JS（models.js 加 NO_QUORUM 与恢复边、market.js 加 resumeAfterRework/reopenTask 与 settle 证据门禁、completeTask 支持证据/policy）、Python（market_client.py 加 resume_after_rework/reopen_task）。

### 验证

- Rust v234 **71** passed（新增状态转移表、两条恢复边、Unverified 拒付、policy=None 豁免 5 用例）；JS SDK **19** 项通过；Python py_compile 通过；`cargo build` 通过。

## [v2.5.9] - 2026-09-27

### 修复：认证式 BFT、可失败独立审计、重放保护（GAP §2.4/§2.5/§3.1/§4.7）

- **认证式 BFT-lite（签名投票 + 固定委员集）**：旧版 QA 验收由调用方经 query 传 `approvals` / `committee_size`，服务端据此合成 `qa-0..n` 委员与赞成票，调用方可任意自我批准。新增 `SignedQaVote`（`task_id/round/voter/vote/nonce/issued_at/expires_at/signature`），委员用 Ed25519 对固定格式 `signing_bytes` 签名；`with_fixed_members` 绑定固定委员集（n、f=(n-1)/3），`cast_signed_vote` 顺序校验：委员在固定集 → 任务 / 轮次匹配 → 时间窗 → 拒绝 Silent → **验签** → nonce 去重 → 计票；模棱两可（equivocation）票作废，`advance_view` 换轮恢复。REST/MCP/CLI/actor 全链路改为认证式。
- **可失败的独立审计一等 API**：旧 `audit_full_scan` 直接调用 `conservation_check`（同算法同数据源=自指），且全仓零调用点。新增 `independent_audit()`：**只信任只追加流水**，独立逐笔重放出期望余额，再与当前余额取并集逐户比对（覆盖幽灵账户 / 缺失 / 篡改 / 拆账），并独立核对充值 / 罚没聚合与总额；返回 `AuditReport {passed, replayed_records, replayed_deposits, replayed_slashed, expected_total, actual_total, aggregate_matches, mismatches}`。测试证明：把一户 100 拆成两户各 50 时 `conservation_check` 仍判守恒（被蒙蔽）、`independent_audit` 判失败并列出两户差异。
- **重放保护（nonce + 时间戳 + 过期）**：每张签名票携带一次性 nonce（`seen_nonces` 去重，重放即拒）与签发 / 过期时间窗（`issued_at ≤ now ≤ expires_at`）；旧接口无 nonce、无主体、无状态前置，可重放刷信誉。
- **只追加结算流水**：充值（Deposited）、质押（Staked）、托管（Escrowed）、支付（Completed）、退款（Refunded）、罚没（Slashed）全部 push 不可变记录，作为独立审计唯一信任源；此前 deposit / 质押 / 托管直接改余额不留痕，无法仅从流水完整重放。
- **三端同步**：Rust（qa_committee.rs / settlement.rs / mod.rs / rest.rs / market_actor.rs / market_tools.rs / gsn.rs）、JS（market.js：`verifyResultAuthenticated` / `independentAudit` / Ed25519 验签，无 crypto 的 WKWebView 抛明确错误而非静默通过）、Python（market_client.py：`verify_result` 认证式 body、新增 `audit()`）。

### 验证

- Rust：lib **135** passed（含 3 个 independent_audit、认证 qa_committee 测试）、集成全绿（v235 认证式 verify 10、v234 66、cross_lang 2 等），0 failed / 0 ignored；JS SDK **16** 项通过（新增认证 QA 伪造 / 重放拒绝、独立审计拆账捕获）；Python `market_client.py` py_compile 通过。

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
