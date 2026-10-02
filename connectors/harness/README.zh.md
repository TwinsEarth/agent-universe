# @twinsearth/agent-universe-harness

[DeepSeek Harness（dsh）](https://github.com/deepseek-ai/deepseek-harness)
桌面 profile 插件包：连接本机 Agent Universe `gsn-daemon` 的 MCP 端点，
把其工具以 `mcp__agent-universe__*` 的形式注册给模型。

它只是对官方发布的
[`@deepseek-ai/dsh-mcp-client`](https://www.npmjs.com/package/@deepseek-ai/dsh-mcp-client)
（streamable-http 传输）的一层配置封装，不引入 stdio 桥、不含原生代码。

## 安装

```bash
# spec 可以是包名、本地绝对路径、./x.tgz 或 github:org/repo
dsh plugin --profile desktop add @twinsearth/agent-universe-harness@3.5.0
MCP_BEARER_TOKEN=<daemon token> dsh start
```

包内带 `dsh.bundle.patch`（一行 loader，`id: agent-universe`），会在
`desktop` profile 自动注册 MCP 客户端。

## 配置

由 `lib/resolve.js`（纯函数、有单测）解析：

| 字段 | 环境变量覆盖 | 默认值 |
| --- | --- | --- |
| `url` | `MCP_URL` / `AGENT_UNIVERSE_MCP_URL` | `http://127.0.0.1:4002/api/v1/mcp` |
| `serverName` | — | `agent-universe`（`^[A-Za-z0-9_-]{1,32}$`） |
| `token` | `MCP_BEARER_TOKEN` / `AGENT_UNIVERSE_MCP_TOKEN` | 无 |
| `failOnStartupError` | — | `false` |
| `toolCallTimeoutMs` | — | 正整数 |

- 显式 `config.headers.Authorization` 不会被重复添加；显式 token 优先于环境变量；纯空白 token 会被忽略。
- `failOnStartupError: false`（默认）在 daemon 不可达时以零工具加载；设为 `true` 则直接拒绝插件激活。

## 已验证

使用发布版运行时（`@deepseek-ai/cordis@4.0.4`、`dsh-mcp-client@0.0.1-rc.1`）
连接真实运行的 v3.5.0 daemon：注册 **27** 个工具；活调 `market_stats`
返回真实数据；正确/错误 Bearer 分别为 200/拒绝激活；daemon 不可达的两种
启动模式均符合预期。

> 独立 `npm install` rc.1 peer 集合时可能需要 `--legacy-peer-deps`
> （DeepSeek 的 rc 依赖图混用了 dsh-agent rc.5 的 peer 与 rc.1）。在 Harness
> 内由 pnpm 对齐，不受影响。详见 `connectors/README.md` 验证矩阵。

## 不用本包的替代方案

可直接用官方 MCP 客户端，配合
`../examples/cordis.patch.agent-universe.yml`。
