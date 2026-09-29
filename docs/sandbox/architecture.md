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
├── docker.rs     # 容器共享内核后端
├── firecracker.rs# microVM 强隔离后端
├── runtime/      # v2.7.7：进程级隔离运行时
├── manager.rs    # v2.7.8：生命周期与弹性供给
└── security.rs   # v2.7.9：网络策略、审计、权限
```

## 5. 隔离级别（如实标注，不静默冒充）

| 级别 | 边界 | 强度 | 不可用时 |
|---|---|---|---|
| Process | 独立临时目录 + 超时/资源约束 | 1 | —— fallback |
| ContainerSharedKernel | 共享宿主内核（runc） | 2 | 降级 Process |
| MicroVM | 硬件虚拟化，内核独立 | 3 | 降级 Container |

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
