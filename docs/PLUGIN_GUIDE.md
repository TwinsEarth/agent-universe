# 插件开发指南（Plugin Guide）— Agent Universe v3.0.0

本指南面向 Agent Universe 插件作者。v3.0.0 起系统采用「一切插件化」架构：功能单元以
插件形式存在，可在运行时安装、调用、热更新、卸载。

## 一、版本语义

| 版本位 | 含义 | 插件影响 |
|---|---|---|
| Major（v3.0.0） | 架构重构 | 内核/契约变更；v3.0.0 起支持热更新/热插拔/热兼容 |
| Minor（v3.1.0、v3.2.0） | 新功能 | 以插件形式发布新功能 |
| Patch（v3.1.1、v3.1.2） | 单独插件更新 | 只影响该插件，对系统与其他插件零影响 |

## 二、五级插件与 id 命名

id 采用反向域名，**前缀即分类**（`Tier::from_name` 自动判定）：

| 级别 | id 前缀 | 承载 | 权限 |
|---|---|---|---|
| 系统 T0 | `com.twinsearth.sys.*` | 进程内 native | 全能力，不可热插拔 |
| 官方 T1 | `com.twinsearth.official.*` | 独立进程（WASM 可选） | 受限系统能力，官方副签 |
| 认证 T2 | `com.twinsearth.certified.*` | 独立进程 | 声明式能力，开发者+官方副签 |
| 第三方 T3 | `{发布者域名}.*` | 独立进程 | 最小能力，默认禁网 |
| 黑名单 | 精确匹配 | 禁止加载 | 无 |

## 三、清单结构（JSON，签名覆盖）

清单是**签名载荷**：版本、能力、依赖、模块摘要都在签名范围内，改一个字节即失效。
解析使用 JSON（非 TOML），并 `deny_unknown_fields`——打错键名是拒绝，不是被忽略。

```json
{
  "plugin": {
    "name": "com.example.myplugin",
    "version": "3.0.0",
    "abi": "3.0",
    "entry": "plugin.bin",
    "publisher": "did:nau:0x...",
    "module_sha256": "<entry 模块的 sha256，防替换>"
  },
  "capabilities": { "grant": ["net:dht:read", "net:gossip:subscribe"] },
  "limits": {
    "memory_bytes": 134217728,
    "cpu_ms": 10000,
    "disk_bytes": 268435456,
    "max_processes": 1,
    "max_output_bytes": 262144
  },
  "waivers": { "network_egress": "逐条写明接受该边界的理由（签名覆盖）" }
}
```

字段说明：
- `plugin`：`name`、`version`、`abi`、`entry` 必填；`publisher`、`module_sha256` 可空。
- `capabilities.grant`：申请的能力名（见下）。
- `limits`：资源限制；缺省按最严（T3）默认值。
- `waivers`：运行时强制不了的边界，逐条写明理由。
- `signature`：由签名工具填充，作者不手写。

## 四、能力模型（capability-based）

权限是**令牌**，不是约定：宿主加载时签发 `CapabilityToken`，无令牌的调用在总线入口即被拒。

基础能力（全级别）：
- `plugin:lifecycle:read`、`plugin:message:send`、`plugin:storage:own`。

网络/链能力（T0 全有，T1/T2 声明式，T3 拒绝）：
- `net:dht:read` / `net:dht:write`
- `net:gossip:publish` / `net:gossip:subscribe`
- `chain:evm:read` / `chain:evm:write`

业务能力：
- `economy:settle`、`agent:card:create` / `agent:card:update`、`swarm:consensus`。

内核能力（仅 T0）：
- `kernel:plugin:manage`、`kernel:policy:write`。

非法能力名（不在已知集合）在清单解析时即拒绝。

## 五、资源限制与 waiver

- 资源限制默认按最严级别；宿主按级别收紧或放宽校验。
- 当某条边界在承载后端**没有强制原语**时，作者必须在 `waivers` 中显式声明并写明理由；
  既无法强制、又无 waiver，即返回 `PolicyNotEnforceable`——**不会静默地不受限运行**。
- 进程后端在所有平台都无原语的边界（当前）：`fs_deny_host`、`network_egress`、`disk_quota`。
  T1/T2 可信插件以 waiver 接受；T3 不可信插件所需边界无法满足即拒绝。

## 六、插件生命周期

`PluginState` 状态机：

```
Discovered → Running → Stopping → Stopped
                │                     └→ Archived
                ├→ Unhealthy
                ├→ Refused（校验/权限不通过）
                └→ Quarantined（累计违规）
```

- 合法状态边由 `PluginLifecycle::allowed` 定义；终态（Archived/Refused/Quarantined）无转出边。
- 总线发送方校验：消息体大小、发送方 Running、令牌存在且 source 与能力一致、目标 Running、
  速率限制；累计 3 次违规转 Quarantined。

## 七、承载后端

| 后端 | 适用 | 说明 |
|---|---|---|
| native | T0 | 进程内，全能力；系统插件随内核装配 |
| process | T1/T2/T3 | 独立进程，复用 ProcessSandbox + 能力闸门；Windows 用 Job Object |
| wasm | 可选 feature | 当前构建无 wasmtime，`runtime/wasm.rs` 类型化拒绝；启用 wasm feature 后承载 |

## 八、签名与打包

- 开发者签名：`manifest.sign_with(&keypair)` 填充 publisher_key/manifest_digest/sig。
- 官方副签（T1/T2）：`manifest.counter_sign_with(&official_keypair)` 追加 counter_sig。
- 摘要与被签字节：紧凑规范 JSON（移除 signature），`compute_digest()` 重算可核对。

## 九、通过 REST API 安装与调用

变更类路由走认证闸门（`Authorization: Bearer <token>`）：

```bash
# 安装（body 为清单 JSON）
curl -X POST http://127.0.0.1:4001/api/v1/plugins \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d @plugin.json

# 调用方法
curl -X POST http://127.0.0.1:4001/api/v1/plugins/com.example.myplugin/call \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"method":"exec","payload":{"language":"python","code":"print(1)"}}'

# 热更新 / 卸载
curl -X POST http://127.0.0.1:4001/api/v1/plugins/com.example.myplugin/reload ...
curl -X DELETE http://127.0.0.1:4001/api/v1/plugins/com.example.myplugin ...
```

## 十、提交前检查清单

- [ ] id 前缀与目标级别一致；版本/abi 正确。
- [ ] 只申请最小必要能力；能力名合法。
- [ ] 无法强制的边界已逐条写明 waiver 理由。
- [ ] 开发者签名（T1/T2 含官方副签）；`module_sha256` 与 entry 一致。
- [ ] 本地 `cargo test`、clippy、fmt 通过；安装/调用/卸载端到端验证。
