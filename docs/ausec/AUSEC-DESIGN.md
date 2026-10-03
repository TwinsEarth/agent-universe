# AUSec（Agent Universe Elastic Compute）总体技术方案

> 适用版本线：**v3.7.0 → v3.9.0**（在 v3.0.0 一切插件化内核之上的弹性计算基础设施）
>
> 本文档是开发依据：系统架构、原理图、流程图、版本切分、能力声明与**诚实边界**。
> 所有性能数字均标注来源；本仓**不把外部实测当作本项目实测**。

---

## 0. 一句话定位

AUSec **不是一个可热替换的业务插件，而是系统插件 `com.twinsearth.sys.ausec`**：
它是 v3.0.0 宿主内核里负责**"插件/Agent 在什么执行后端上、以什么隔离强度、占用多少资源、如何取镜像、如何存/分叉执行状态、谁在监控它"**的运行底座。

设计原则沿用本项目一以贯之的两条铁律：

1. **边界要么由代码强制执行，要么让请求具名失败——绝不接受一个策略然后忽略它。**
2. **能在代码里真实落地的才写进能力声明；Linux 专有原语在无对应平台/无权限时显式拒绝，不冒充已隔离。**

---

## 1. 与 v3.0.0 插件化架构的关系

v3.0.0 已经交付的事实（`gsn-core/src/plugin/`、`gsn-core/src/sandbox/`）：

- 五级插件分类 `Tier`：System / Official / Certified / ThirdParty / Blacklist，仅由
  插件 id 前缀判定，可伪造的额外字段不参与授权（`plugin/tier.rs`）。
- 能力令牌 `Capability` + `grant_for(tier, cap)`，基础能力 / 内核能力分级授予
  （`plugin/capability.rs`）；宿主 `PluginHost` 是唯一装配与 PMB 投递点
  （`plugin/host.rs`），系统插件经 `NativeRuntime` 进程内注册真实处理器
  （`plugin/system/mod.rs::register_handlers`）。
- 沙箱 `trait Sandbox` + **能力声明模式**（`sandbox/capability.rs` 的
  `CapabilityDeclaration / Waiver`）：后端声明自己**实际能强制**哪些边界，
  请求了无法强制的策略具名拒绝；随机 `sb-<16hex>` id + 单一路径组件校验 +
  owner 绑定；`checkpoint` / `fork_from` 已能复制工作目录
  （`sandbox/manager.rs`）。

AUSec 在其上补三件事：**执行后端的统一抽象与按分级映射、超大规模资源调度与
镜像分发（控制面）、状态快照/分叉与运行时安全治理**。映射关系：

| v3.0.0 分级 | 默认执行后端 | 典型负载 | 参考启动量级* |
|---|---|---|---|
| System（T0，进程内） | **FnCall** | 在线评测、无状态短任务 | < 1ms 级 |
| Official（T1） | **Container** | 软件工程、工具调用 | 毫秒级 |
| Certified（T2） | **MicroVM** | 需更强隔离的 Agent 任务 | 十毫秒级 |
| ThirdParty（T3） | **MicroVM / Full VM**（按风险评级） | 高风险/需图形界面 | 百毫秒级 |
| Full VM | 完整 OS、GUI、图形渲染、Android 应用 | — | — |

> \\*"参考启动量级"是设计目标/外部资料数量级，**不是本仓实测基准**；本仓 CI 在
> 三平台无 Docker/KVM/GUI 环境，无法也不会声称测到这些数字。

纵深防御三层（与本仓现有机制对齐）：

1. **应用级**：v3.0.0 能力令牌 + PMB 七道检查（`plugin/bus.rs`）。
2. **OS 级**（v3.9.0，Linux 专有）：AppArmor 文件/Socket 限制、eBPF 每沙盒网络白名单。
3. **内核/虚拟化级**：MicroVM / Full VM 隔离，阻断内核漏洞跨沙盒传播。

---

## 2. 总体架构

```
┌───────────────────────────────────────────────────────────────┐
│              v3.0.0 宿主内核 PluginHost                        │
│   生命周期管理 · PMB 通信总线 · 注册/仲裁/黑名单 · 能力令牌      │
├───────────────────────────────────────────────────────────────┤
│        AUSec 系统插件 com.twinsearth.sys.ausec（T0）           │
│  ┌──────────────┐ ┌───────────────┐ ┌───────────────────────┐  │
│  │ 后端选择器    │ │ 镜像/块分发    │ │ 资源调度（CPU/内存）   │  │
│  │ backend.rs   │ │ image.rs(3.7) │ │ scheduler.rs(3.7)     │  │
│  └──────────────┘ └───────────────┘ └───────────────────────┘  │
│  ┌──────────────┐ ┌───────────────┐ ┌───────────────────────┐  │
│  │ 快照/分叉     │ │ Agent 委员会   │ │ 安全组织(3.9)         │  │
│  │ snapshot(3.8)│ │ council(3.8)  │ │ police/…/tribunal     │  │
│  └──────────────┘ └───────────────┘ └───────────────────────┘  │
├───────────────────────────────────────────────────────────────┤
│  执行后端：FnCall · Container · MicroVM · Full VM             │
│  （真实执行的只有具备平台原语的后端；其余 RequiresProbe/拒绝）  │
├───────────────────────────────────────────────────────────────┤
│  网络与存储：libp2p DHT/GossipSub/relay · 内容寻址块存储        │
└───────────────────────────────────────────────────────────────┘
```

---

## 3. 版本切分（中版本=功能域，小版本=可独立验证的插件级增量）

> 遵循版本语义：中版本引入新功能域，小版本只动该域内的一个能力，且每个小版本都
> **自带"在旧实现上会失败"的测试**（修复/特性若测试在旧代码也能通过，则什么都没证明）。

### 3.1 v3.7.0 跨网络 P2P 超大规模沙盒（1 + 9）

| 版本 | 增量（均为可独立验证的控制面/校验/调度逻辑） |
|---|---|
| **3.7.0** | AUSec 基座：四后端枚举、tier/风险→后端映射、平台可用性诚实模型（`Available/RequiresProbe/Unsupported`）、注册 `com.twinsearth.sys.ausec`、新增 `sandbox:*` 能力令牌及授权矩阵。 |
| 3.7.1 | 内容寻址镜像**块清单**（chunk manifest：偏移/长度/sha256/顺序），解析与完整性基线。 |
| 3.7.2 | 块存储 `BlockStore`：本地仅存元数据 + 按需取块，缺块即取、命中复用、只读共享。 |
| 3.7.3 | P2P 种子/健康度真实核算（副本数→健康度的确定性计算）+ 每块 Ed25519 签名与 sha256 锚定校验（远端块源 `BlockSource` 抽象；UDOS 为具名远端、本地种子源真实）。 |
| 3.7.4 | 内存配额池与**共享额度**记账：多沙盒不重复计只读共享额度，超卖比准入。 |
| 3.7.5 | "等待期保内存不释放 + 空闲优先回收"策略（纯记账决策，确定性测试）。 |
| 3.7.6 | 内存回收/超卖统计与越界拒绝；virtio-pmem/DAX/DAMON/balloon 作为 Linux-MicroVM 专有原语**声明但在无原语平台具名拒绝**。 |
| 3.7.7 | CPU 优先级模型：时延敏感 / 时延容忍两级 + 优先级与权重。 |
| 3.7.8 | 竞争下的确定性 CPU 配额分配（仿真时钟测试：敏感任务优先，剩余给容忍任务）。 |
| 3.7.9 | 突发涌入准入控制与统一 AUSec 调度状态（status 汇总：后端/块/CPU/内存）。 |

**外部实测数字（仅引用，不作本仓基准）**：镜像实际访问占比 **4.2%–13.3%**、
按需加载约 **35 分钟 vs 全量 60+ 分钟（≈1.71×）**、磁盘写入降约 **57%**；
约 **90%** 沙盒平均 CPU < 申请量 **5%**、超卖率 **50×+**；virtio-pmem+DAX
峰值内存降 **40.2%**、DAMON+balloon 累计再降 **21.2%**；节点 50% CPU 被占时
时延敏感任务受影响 **45.2%→17.3%**；种子"**1000% 以上健康度**"。
来源：DeepSeek DSEC / AUSec 公开资料（`aiwiki.ai/wiki/dsec#2`、
`byteiota.com/.../deepseek-dsec-agent-training-sandbox`）。本仓用**自己的确定性
单元/仿真测试**验证调度与校验逻辑，不复用、不冒充上述生产数字。

### 3.2 v3.8.0 Agent 委员会（1 + 6）

| 版本 | 增量 |
|---|---|
| **3.8.0** | 官方插件 `com.twinsearth.official.agent-council` 注册与组织/资源/任务编排面；`sandbox:snapshot`/`sandbox:restore` 能力。 |
| 3.8.1 | `pack_diff` 内容寻址 CoW 增量快照：父链 + 每文件 sha256 manifest，只存新增/变更块。 |
| 3.8.2 | 快照恢复（restore）：从任意快照精确恢复工作集，校验父链完整性。 |
| 3.8.3 | 轨迹分叉：第 k 步同状态恢复多沙盒，前序共享、仅记各分支增量，不重放前 k 步。 |
| 3.8.4 | pack_diff / 分叉操作全部经 PMB 留痕（审计可回放）。 |
| 3.8.5 | 执行状态与可抢占计算资源解耦：快照-恢复路径（为"执行中热更新"提供 10–100ms 级路径；数字为参考）。 |
| 3.8.6 | 四后端在 Agent 委员会任务上的统一分派与策略校验收口。 |

### 3.3 v3.9.0 Agent 安全组织（1 + 7）

| 版本 | 增量 |
|---|---|
| **3.9.0** | 安全组织状态机与证据模型（举报→受理→调查→裁决→处罚/申诉→解封）骨架 + 接入既有 `Blacklist`。 |
| 3.9.1 | `…security.registry` 备案：身份/签名/信任存储登记（复用插件注册与签名）。 |
| 3.9.2 | `…security.report` 报案：异常上报、证据收集、链上存证（证据 hash）。 |
| 3.9.3 | `…security.surveillance` 监察：沙盒状态审计、资源配额检查（读调度器真实计数）。 |
| 3.9.4 | `…security.audit` 审查：行为画像/风险评估/作弊向量规则（读残留答案、伪造 RPC、覆盖 /bin/bash、XFS_IOC_SWAPEXT）。 |
| 3.9.5 | `…security.police` 警察：实时违规拦截与隔离执行（转 QUARANTINED）。 |
| 3.9.6 | `…security.tribunal` 审判：裁决、处罚执行、解封审批；结果同步黑名单 + 紧急广播通道。 |
| 3.9.7 | Linux OS 加固面：AppArmor profile 与 eBPF 网络白名单规则**真实生成**，含 XFS ioctl 作弊向量的拦截规则；**仅在 Linux 且具备权限时可下发**，其余平台具名拒绝/仅导出规则文件，绝不声称已生效。 |

> 关于内核漏洞（如经 `XFS_IOC_SWAPEXT` 绕过访问控制、损坏 XFS 元数据致文件系统
> 下线）：AppArmor 可拦已知 ioctl/路径，但**不存在通用内核漏洞防御**；终局防线是
> MicroVM/Full VM 虚拟化隔离，且攻防持续。AUSec 的价值是把暗箱操作变成
> **可审计、可回放、可治理**的工程流程，而非"彻底解决"。

---

## 4. 能力令牌（在 `plugin/capability.rs::Capability` 上扩展）

| 能力 | serde 名 | System | Official | Certified | ThirdParty | 说明 |
|---|---|---|---|---|---|---|
| `SandboxLifecycle` | `sandbox:lifecycle` | Granted | Declarable | Declarable | Denied | 创建/驱动沙盒生命周期 |
| `SandboxMessage` | `sandbox:message` | Granted | Declarable | Declarable | Denied | 经 PMB 驱动沙盒动作 |
| `SandboxCreate` | `sandbox:create` | Granted | Declarable | Declarable | Denied | 创建沙盒（与"配置隔离参数"分离） |
| `SandboxSnapshot` | `sandbox:snapshot` | Granted | Declarable（agent-council） | Declarable | Denied | 生成增量快照 |
| `SandboxRestore` | `sandbox:restore` | Granted | Declarable（agent-council） | Declarable | Denied | 从快照恢复/分叉 |
| `SandboxConfigure` | `sandbox:configure` | Granted | Denied | Denied | Denied | 配置隔离参数，**需额外审批** |
| `SandboxPolicyApply` | `sandbox:policy:apply` | Granted | Denied | Denied | Denied | 下发 AppArmor/eBPF/策略 |
| `SandboxBlacklistSync` | `sandbox:blacklist:sync` | Granted | Denied | Denied | Denied | 安全黑名单同步/紧急广播 |

权限拆分原则：**`sandbox:create` 与 `sandbox:configure` 分离**——能创建沙盒不代表
能改隔离参数；后者是潜在提权路径，仅 System 持有，代码审计强度等同 T2。

---

## 5. 诚实边界（写进代码注释与 `docs/VERIFICATION`）

- FnCall 是**进程内函数派发**，启动开销极小但**不提供 OS 级隔离**；它只用于
  可信（System）短任务，能力声明中如实标注"无文件系统/网络强制边界"。
- Container / MicroVM / Full VM 的**真实执行依赖平台原语**：
  - 未探测到运行时（docker/podman、`/dev/kvm`+firecracker/cloud-hypervisor、
    Hyper-V、Apple Virtualization.framework）时状态是 `RequiresProbe(前置条件)`，
    **不是 Available**；macOS/Windows 无 Firecracker/KVM，对应后端 `Unsupported(原因)`。
  - v3.7.0 只交付**选择与声明**；真正探测与执行在具备运行时的后续补丁里按
    "探测成功才 Available、否则拒绝"接入，沿用现有 `sandbox/` 后端诚实模式。
- virtio-pmem / DAX / DAMON / balloon / seccomp / landlock / AppArmor / eBPF 均为
  **Linux 专有**；内存共享在本仓以**配额记账/共享额度策略**真实落地（跨平台可测），
  底层页缓存共享只在 Linux MicroVM 后端声明、无原语时具名拒绝。
- UDOS 分布式文件系统是**具名远端块源**；本仓定义 `BlockSource` 契约并用本地种子源
  做真实测试，不伪造跨网络传输，也不把"种子健康度"当成已经在真实 P2P 网络测到。
- 所有外部性能数字一律标注来源与"非本仓实测"。

---

## 6. 验证策略

- 每个小版本：`cargo test -p gsn-core <ausec 模块>` 新增**首次失败回归**测试；
  `cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`、
  `scripts/check-no-panics.mjs`、`scripts/check-unsafe-containment.mjs`、
  `scripts/check-version.sh` 全绿。
- 调度/超卖/CPU 分配用**确定性仿真**（注入时钟与用量序列），不依赖真实负载。
- 镜像完整性/签名用真实 SHA-256 与 Ed25519（仓库已有 `sha2`/`ed25519-dalek`）。
- 平台差异用 `cfg!(...)` 单测覆盖 Linux 与非 Linux 的可用性分支。
- 发布遵循 v2.8.0 起 SOP：版本一致性 → 全量关卡 → PR → CI 全绿 → 合并 → tag →
  Release/Publish → 发布后实测 → 文档回填。
