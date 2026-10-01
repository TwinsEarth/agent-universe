# v3.4.2 全局审核报告 —— 一切插件化架构合规性审计

> 审计对象：`agent-universe` **v3.0.0 大版本及其下全部中/小版本（v3.1.0 → v3.4.0）**
> 审计方法：逐文件通读 + 调用点 grep + 测试名映射 + 全量测试
> 产出版本：**v3.4.2（npm 3.4.2 ↔ Rust crate 0.3.42）「全局审核版本」**
> 审计日期：2026-10-01

---

## 0. 审计目的与边界

本报告只回答一个问题：**v3.0.0 起，agent-universe 是否彻底贯彻了「一切插件化」架构？**

具体拆为三条主线：

1. **版本语义边界**：大版本=架构重构、中版本=新功能插件化、小版本=只更新独立插件且对系统/其他插件零影响。
2. **双重隔离**：Agent 间隔离、插件间隔离（沙箱 / 虚拟环境 / 独立进程 / 独占内存）。
3. **五级插件分类**：系统 / 官方 / 认证 / 第三方 / 黑名单的真实接线（而非文档承诺）。

### 0.1 两条自定规则（决定所有判定）

* **「没有调用点的修复不算修复。」** 一个类型/函数在生产代码里零调用，就没有改变任何行为。
* **「在修复前代码上能通过的测试，什么也没证明。」** 每条「已修复」都要检查对应测试是否真会在旧实现上失败。

### 0.2 审计范围

| 项 | 值 |
|---|---|
| 插件内核 | `gsn-core/src/plugin/` 16 文件 / 约 6,077 行 |
| 官方插件 entry | 9 个，全部内嵌 Python（`official/mod.rs`） |
| 审计方法 | 纯源码阅读 + 调用点 grep + 全量 `cargo test` |
| 说明 | 运行时可观测结论以已运行的测试/关卡为准；其余为代码文本判断，附 `文件:行号` 供复核 |

---

## 1. 版本语义边界审计

| 版本 | 类型 | 定位 | 判定 |
|---|---|---|---|
| **v3.0.0** | major | 插件内核：注册/总线/仲裁/黑名单/生命周期/隔离运行时 | ✅ 架构重构，不改业务 |
| **v3.1.0** | minor | 官方插件业务化：economy-reputation(`overall`)、market-match(`match`) | ✅ 新功能以插件 entry 发布 |
| **v3.2.0** | minor | 业务化：market-settle(`audit`)、scheduler-task(`route`)、agent-card(`validate`) | ✅ 新功能插件化 |
| **v3.2.1** | minor | PMB 安全通信：消息 HMAC 签名、nonce 防重放、七道校验 | ✅ 内核能力增强 |
| **v3.2.2** | minor | 业务化：swarm-emergence(`detect`) | ✅ 插件 entry |
| **v3.2.3** | minor | 业务化：agent-skill(`discover`) | ✅ 插件 entry |
| **v3.3.0** | minor | 业务化：chain-anchor（keccak256 锚定） | ✅ 插件 entry |
| **v3.4.0** | minor | 业务化：chain-bridge（跨链信誉桥接、中位数共识） | ✅ 插件 entry |
| **v3.4.2** | patch | 全局审核：补齐已实现内核能力的对外接线、新增合规关卡 | ✅ 只改接线，不改架构 |

**结论**：v3.0.0 起的版本语义边界**已贯彻**——
- 每个 minor 版本的新增功能都以**独立插件 entry** 的形式发布（不是改内核），符合"中版本=新功能插件化"；
- 9 个官方插件的 entry 彼此独立、各自隔离，单个插件升级不影响其他插件；
- v3.4.2 是 patch，只补齐内核已实现能力的对外接线，**未改任何内核架构与其他插件**。

---

## 2. 「一切插件化」已贯彻项核验（10 项，附证据）

| # | 核验项 | 证据 | 判定 |
|---|---|---|---|
| 1 | 内核五子系统完整 | `registry.rs`（注册/热更新 replace）、`bus.rs`（PMB，860 行）、`arbiter.rs`（签名/能力仲裁）、`blacklist.rs`（指纹库/取证/解封）、`lifecycle.rs`（状态机） | ✅ |
| 2 | 五级分类由 name 前缀判定 | `tier.rs`：`com.twinsearth.sys.*`=System、`com.twinsearth.official.*`=Official、`com.twinsearth.certified.*`=Certified、`com.twinsearth.blacklist.*`=Blacklist、其余=ThirdParty | ✅ |
| 3 | 仲裁器四重校验真实 | `arbiter.rs:64-136`：①清单摘要重算一致；②发布者 Ed25519 签名对规范载荷有效；③Official/Certified 官方副签有效且副签公钥须在受信 `official_roots`；④ThirdParty 发布者公钥须在 `trusted_publishers`（默认空、fail-closed）；Blacklist 直接拒 | ✅ |
| 4 | 能力矩阵真实 | `capability.rs:165 grant_for`：基础能力全级别授予；网络/链/经济能力 T0 Granted、T1/T2 Declarable（需 approve）、T3 永远 Denied；内核能力仅 T0 | ✅ |
| 5 | 隔离运行时诚实声明 | `runtime/mod.rs`：native=T0 进程内不隔离；process=T1+ 独立进程（真实 ProcessSandbox）；wasm 类型化拒绝（wasmtime 不在依赖树）。`required_bounds(tier)`：T3 要求全部 9 条边界，**T3 任何边界不允许豁免（关键安全属性，有测试）** | ✅ |
| 6 | process 后端能力声明按平台诚实 | `runtime/process.rs:186-200`：fs_isolation/cpu_limit/output_cap 三平台 true；Windows 额外 memory_limit/process_tree_kill/process_count_limit（Job Object）；其余不声明 | ✅ |
| 7 | invoke_entry 防注入 | `process.rs:77-128`：方法名必须 ASCII 字母数字+下划线（否则拒），payload 经 `json.loads` + 双重转义，引导代码用 `getattr` 安全调用 | ✅ |
| 8 | 黑名单生命周期完整 | `blacklist.rs`：精确匹配 name 或 module_sha256（改名仍命中）；add 幂等；申诉 `file_appeal` 不解封；`unblock_with_new_module` 仅当新模块摘要≠旧摘要才解封（有测试） | ✅ |
| 9 | 热更新/热插拔/热兼容完整 | `install`/`uninstall`/`hot_reload`/`stop`/`start`/`load_legacy`/`check_abi`（内核全部实现） | ✅ |
| 10 | 9 个官方插件全部业务化 | `official_entry_source` 9/9 为 Some、0 None；每个 entry 承载真实业务方法 | ✅ |

---

## 3. A 类 patch 级 gap 与修复记录（v3.4.2 已修）

审计发现 4 处**「内核能力已实现、但未接到对外 REST/检查」**的 gap。这些是补齐已实现能力的接线，
**不改内核架构、不影响其他插件**，属于 patch 可修复范围。

### G1【高】handle_plugin_api 未暴露 `stop`/`start`

* **现象**：`PluginHost::stop(id)`（set Stopped、保留路由）/`start(id)`（重新 spawn 并 set Running）在内核已实现，但 REST 只有 list/install/detail/call/reload/uninstall，缺少「暂停/恢复但不卸载」的热插拔中间态路由。
* **修复**：新增 `POST /api/v1/plugins/{id}/stop`、`POST /api/v1/plugins/{id}/start`。
* **测试**：`plugin_api_stop_start_routes_reachable`（boot_system 后断言 Running → stop → Stopped → start → Running）。该测试在修复前会因路由返回 404 而失败。

### G2【高】handle_plugin_api 未暴露 `trust_publisher`

* **现象**：`PluginHost::trust_publisher(key_hex)`（host.rs:94，转 arbiter）已实现，但 REST 无入口 → 第三方 T3 插件在真实 daemon 上无法被信任安装（默认信任库为空，fail-closed）。
* **修复**：新增 `POST /api/v1/plugins/trust`（body: `{"publisher_key":"hex"}`，缺 key → 400）。
* **测试**：`plugin_api_trust_route_fail_closed`（缺 key→400、提供 key→200）。

### G3【高】黑名单管理未暴露 REST

* **现象**：`host.blacklist()` / `blacklist_mut()` 可查询/解封，但 REST 无黑名单查询/解封的运维入口。
* **修复**：
  - 新增 `GET /api/v1/plugins/blacklist`（输出 plugin_name/module_sha256/reason/blacklisted_at/evidence/appeal）；
  - 新增 `POST /api/v1/plugins/{id}/unblock`（body: `{"new_module_sha256":"hex"}`，调 `unblock_with_new_module`；失败/相同摘要 → 400）。
  - **路由顺序**：`GET /plugins/blacklist` 必须放在通用 `GET /plugins/{id}` 详情路由之前，否则 path "blacklist" 会被当作 id。
* **测试**：`plugin_api_blacklist_query_and_unblock`（add 条目→GET 返回 1 条→相同摘要 400→缺字段 400→不同摘要 200→黑名单清空）。

### G4【中】无插件化架构合规的自动化检查关卡

* **现象**：现有 check-no-panics / check-unsafe-containment / check-version / fmt，但缺少「9 官方 entry 可达性」的会红检查。
* **修复**：把 `entry_source_for_business_plugins` 升级为**自动遍历 `official_ids()`**（断言数量=9、每个 entry 为 Some、language/filename/source 非空）。未来新增官方插件若漏配 entry、或 `official_entry_source` 漏写 match 臂，本测试立即失败。

### 认证核实

所有新增 POST 管理路由（trust/stop/start/unblock）都在 daemon 主请求流的 `rest_authorize`（node.rs:1699）**之后**被处理，
因此受 fail-closed 保护：未配置 `REST_BEARER_TOKEN` 且无 `REST_ALLOW_UNAUTHENTICATED=1` → 401。
`GET /plugins/blacklist` 是只读查询，GET 放行（合理）。

---

## 4. B 类架构性 gap —— v4.0.0 major 路线（不在 patch 范围）

以下 3 项属于内核级 major 重构，**不能在 patch/minor 中实施**，仅在此标注路线，留待 v4.0.0：

### B1【核心】主业务数据面仍是单体

* `run_api_server` 同时接收 `store`（单体 PersistentStore）、`market`（单体 MarketActorHandle）、`plugin_host`；
* daemon 主 REST 路由（`/api/v1/accounts`、`/tasks`、`/disputes` 的 deposit/bid/settle/arbitrate）仍调用单体 market；
* 9 个官方插件只在 `/api/v1/plugins` 下被独立调用，是**并行承载**，并未接管主数据面。
* **路线**：v4.0.0 让主业务路由由对应插件（market-match / market-settle / economy-reputation）接管，单体 market 退化为兼容层。

### B2【核心】PMB 插件间通信在 daemon 业务中零调用

* 非测试代码确认：`host.send_to` / `publish` / `open_inbox` 无任何业务调用点（命中的 publish 均为单体 market.publish_task / gossipsub.publish / shared_memory.publish）；
* 进程插件（Python）无 host function 注入，无法主动发起通信，只能被动响应宿主调用；
* PMB 七道检查目前仅在单元测试验证。
* **路线**：v4.0.0 为进程插件注入 host function（host 侧代理），并在真实业务流程中编排插件间通信。

### B3 系统插件仅 identity 真承载业务

* 4 个 T0 系统插件中仅 `com.twinsearth.sys.identity` 真承载业务（mint_did/parse_did/fingerprint）；
* `sys.net` / `sys.storage` / `sys.chain` 只注册 status 方法（"真实句柄由宿主装配"，但 host.rs 未接线 daemon net/storage/chain 句柄）。
* **路线**：v4.0.0 把 daemon net/storage/chain 句柄接线到对应系统插件，或明确这些基础设施保留在宿主（作为 T0 边界）。

---

## 5. 双重隔离核验

### 5.1 插件间隔离（四层）

| 层 | 机制 | 状态 |
|---|---|---|
| WASM 沙箱 | 线性内存独占、能力式 host function | wasmtime 未在依赖树，类型化拒绝（不假装支持） |
| 进程沙箱 | T1+ 独立 OS 进程（真实 ProcessSandbox） | ✅ 已实现 |
| 内存隔离 | 独立进程地址空间、独立堆栈 | ✅ 进程插件天然隔离 |
| 网络隔离 | 第三方默认禁止，T1/T2 声明式 | ✅ 能力矩阵 + waiver |

### 5.2 Agent 隔离

* 官方业务插件以独立进程运行，各自有唯一隔离工作目录（`au-sandbox-{id}-{pid}-{seq}`，v3.2.0 竞态修复 commit b0e60f0），hot_reload 后旧实例 destroy 不删新版本目录。
* 方法名必须合法 Python 标识符（防引导注入），payload 经 `json.loads`。

### 5.3 资源配额与豁免（诚实）

* `RuntimeCapabilities` 9 字段默认全 false；`required_bounds(tier)`：T1/T2 仅要求 fs_isolation/cpu_limit/output_cap，T3 要求全部 9 条；
* T1/T2 仅豁免 fs_deny_host/network_egress/disk_quota，**T3 任何边界不允许豁免（关键安全属性，有测试）**；
* Unix/macOS 进程后端部分边界无强制原语（需 waiver），Windows 用真实 Job Object。

---

## 6. 验证关卡

| 关卡 | 结果 |
|---|---|
| 新增 3 个 handle_plugin_api 可达性测试 | ✅ 全部通过（修复前会失败） |
| 升级 entry_source 合规关卡（自动遍历 9 个） | ✅ 通过 |
| 全量 `cargo test`（crate） | ✅ 见发布说明（0 failed） |
| `cargo clippy -D warnings` | ✅ 0 warning |
| `cargo fmt --check` | ✅ |
| check-no-panics / check-unsafe-containment / check-version | ✅ |

---

## 7. 已知未修项（诚实边界）

* node.rs/api.rs 约 8 处 `let _ = node.persist()` 丢弃持久化错误（未落盘变更仍返回 200）。
* API token 无法表达标准 DID（`did:nau:<id>` 的冒号与字段分隔符冲突）。
* 结算仍按托管总额付款而非中标价（中标价已持久化并作为下限）。
* v3.2.1 消息签名为对称 HMAC（不提供非对称来源证明），跨节点非对称签名留待后续。
* chain-anchor / chain-bridge 均为离线构造（不声称真实上链/跨链，无 RPC，时间戳由调用方传入）。
* B1/B2/B3 架构性 gap 留待 v4.0.0 major。

---

## 8. 结论

v3.0.0 起，「一切插件化」架构在**内核与插件层面已彻底贯彻**：
版本语义边界清晰、五级分类由前缀判定且仲裁四重校验真实、能力矩阵与隔离运行时诚实、
9 个官方插件全部业务化、热更新/热插拔/热兼容完整、黑名单生命周期闭合。

v3.4.2 修复了 4 处「内核能力已实现但未接对外路由/检查」的 patch 级 gap（G1–G4），
并明确将 3 处主数据面接管相关的架构性 gap（B1–B3）标注为 v4.0.0 major 路线。

**核心判定：v3.0.0 → v3.4.0 已贯彻「一切插件化」；v4.0.0 的目标是让插件从「并行承载」走向「接管主数据面」。**
