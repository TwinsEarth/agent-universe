# Agent Sandbox 架构设计（v2.7.6 起）

## 1. 沙箱为什么进入 Agent Infra 核心执行链路

模型时代的基础设施服务于"让 AI 思考"——关注推理时延、吞吐、算力成本。Agent 进入生产后，执行本身成为基础设施问题：

- Agent 要搜索资料、读文件、运行代码、调用 API，并根据环境返回调整计划；
- 企业必须确定 Agent 能访问哪些数据/系统，代码、浏览器、工具在什么环境执行，任务如何隔离；
- 请求突增时要快速准备大量独立环境；长程任务要保存执行状态、临时文件、中间结果。

沙箱从"隔离不可信代码"的安全机制，演化为承载 Agent 执行的底座。它是 Agent 与云基础设施之间的**安全执行底座**：

- 向上提供开发者友好的接口与生态兼容；
- 向下连接计算、存储、网络；
- 把任务创建、资源隔离、权限控制、日志审计沉淀为统一执行环境。

## 2. 在 Agent Infra 中的位置

```text
┌─────────────────────────────────────────────────────┐
│  接入层：Developer API（E2B 兼容） / Kubernetes API │
├─────────────────────────────────────────────────────┤
│  编排层：任务分解 / 工具调用编排 / 上下文            │
├─────────────────────────────────────────────────────┤
│  模型层：推理 / 生成下一步行动                       │
├─────────────────────────────────────────────────────┤
│  执行层：Agent Sandbox ◀──── 本设计                 │
│    进程 · 文件 · 网络 · 存储 · 权限边界              │
├─────────────────────────────────────────────────────┤
│  资源层：CPU / 内存 / 磁盘 / 网络 / 虚拟化           │
└─────────────────────────────────────────────────────┘
```

## 3. 两类工作负载

| 维度 | 训练评测（Agentic RL / RSI） | 生产应用（Agent Serving / Use） |
|---|---|---|
| 目标 | 反复 Rollout 获 Reward，自主迭代 | 承载真实请求的执行 |
| 环境特征 | 大量并行、相互隔离 | 快速启动、突发弹性 |
| 状态复用 | 内存级 Checkpoint、Reset/Fork/Replay | 休眠/唤醒、任务恢复 |
| 关注指标 | 有效 Rollout 吞吐 | 冷启动时延、安全边界 |

## 4. 模块结构

```text
gsn-core/src/sandbox/
├── mod.rs        # 定位、Sandbox trait、SandboxResult
├── error.rs      # 统一错误（环境/隔离/执行/超限/配置/网络）
├── config.rs     # SandboxConfig、ResourceLimits、NetworkPolicy、FilesystemPolicy、IsolationLevel
├── state.rs      # 生命周期状态机
├── identity.rs   # 沙箱身份、Agent Identity、短时执行令牌
├── docker.rs     # 容器后端：仅 PATH 探测（未实现 Sandbox trait）
├── firecracker.rs# microVM 后端：仅 /dev/kvm 探测（未实现 Sandbox trait）
├── runtime/      # v2.7.7：进程级隔离运行时（默认且唯一真跑的后端）
│   ├── process.rs
│   └── winjob.rs # v2.8.7：Windows Job Object（cfg windows）
├── manager.rs    # v2.7.8：生命周期与弹性供给
└── security.rs   # v2.7.9：网络策略、审计、权限
```

## 5. 隔离级别（如实标注，不静默冒充）

| 级别 | 边界 | 状态 |
|---|---|---|
| Process | 独立临时目录 + `env_clear` + `ulimit`（Unix）/ Job Object（Windows）+ 超时强杀 | **当前唯一真跑的后端** |
| ContainerSharedKernel | 共享宿主内核（runc） | 未实现（docker.rs 仅探测） |
| MicroVM | 硬件虚拟化，内核独立 | 未实现（firecracker.rs 仅探测；无 `/dev/kvm` 环境） |

> 进程级隔离不提供内核级边界，不可用于运行完全不可信的代码。
> 后端不静默冒充：请求了无法执行的隔离级别即返回错误，而不是降级后仍声称强隔离。

## 6. 生命周期状态机

```text
Pending ─create→ Creating ─start→ Starting ─→ Running
                                                │ pause
                                          Pausing→ Paused
                                                │ resume
                                          Resuming→ Running
   任意运行态 ─stop→ Stopping ─→ Stopped
   任意非终态 ─error/limit→ Failed
```

**关键边界**：状态机只管理沙箱自身运行状态（内存/文件/进程）。业务状态（任务进度、DB 写入、外部系统）由上层 Agent/编排记录。

## 7. 安全默认（最小权限）

- 默认无网络、只读根文件系统、非 root；
- 有界 CPU/内存/磁盘/进程数/超时；
- 长期密钥不进入沙箱；
- 沙箱内代表 Agent 调用须经短时令牌（最小权限 + TTL + 可吊销）。

## 8. 五版本切分

| 版本 | 内容 |
|---|---|
| v2.7.6 | 核心架构：error/config/state/identity + 设计文档 |
| v2.7.7 | 进程级隔离运行时：独立 FS、超时/资源限制、代码执行 |
| v2.7.8 | 生命周期管理：SandboxManager、预热池、休眠唤醒、checkpoint |
| v2.7.9 | 安全边界：网络策略、审计日志、权限、Agent Identity |
| v2.8.0 | 集成核心链路：E2B 兼容 API、MCP/CLI/REST 接入、客户端构建 |

## 9. v2.8.7 安全加固（GAP §3.3）

v2.8.0 落地后，审计发现沙箱安全边界此前只存在于文档与单元测试：

- **变更类统一认证**：`handle_api` 对 create/exec/pause/resume/destroy 要求非空 `caller`（否则 401），
  认证主体由 `extract_caller`（Bearer 令牌 → `sub:<sha256 前 16 hex>`，不泄露明文）派生；
  sse 透传、stdio 固定 `local:stdio`。
- **所有权绑定**：create 后 `bind_owner`，后续变更经 `check_owner` 校验，非所有者返 403。
- **随机 id**：`sb-` + 16 hex，经单一 `validate_sandbox_id` 校验，不可猜、不重复、重启不复用。
- **输出 / 资源上限**：stdout/stderr 有界读取截断到 1 MiB；请求体配置经 `config_from_body` 真正生效；
  Windows 用 Job Object 强制内存/进程数上限与整棵进程树 kill。
- **孤儿清扫**：`SandboxManager::new` 启动即清理残留 `au-sandbox-*`。

验证：`tests/v287_test.rs`（12 测试），并实证截断与认证两项首次失败回归；全量 0 failed、clippy 零警告。
