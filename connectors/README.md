# Agent Universe — Desktop MCP Connectors (v3.5.0)

Connect a local **`gsn-daemon`** (the Agent Universe node; MCP surface at
`POST /api/v1/mcp`) to desktop AI clients. The daemon speaks **stateless
Streamable HTTP** (MCP `2024-11-05`), so modern clients connect directly —
no bridge, no extra process.

| Client | Transport | Package / path |
| --- | --- | --- |
| **DeepSeek Harness** (desktop / web / CLI) | Streamable HTTP (direct) | [`@twinsearth/agent-universe-harness`](./harness) — a dsh **bundle** wrapping `@deepseek-ai/dsh-mcp-client` |
| **Doubao desktop (豆包电脑端)** | Streamable HTTP (direct) | Custom HTTP connector — copy/paste config in [`examples/doubao-connector.json`](./examples/doubao-connector.json) |
| **Any stdio-only MCP host** (Claude Desktop, etc.) | stdio ↔ HTTP bridge | [`@twinsearth/agent-universe-mcp`](./agent-universe-mcp) — zero-dependency |

```
client  ──Streamable HTTP──►  http://127.0.0.1:<port>/api/v1/mcp  ──►  gsn-daemon
        (Bearer token optional, loopback)
```

## 1. Run a daemon

```bash
# prebuilt: see install.sh / install.ps1, or:
MCP_BEARER_TOKEN=$(openssl rand -hex 32) \
  gsn-daemon --listen 127.0.0.1 --api-port 4002 --mode light
```

- **No token**: read-only MCP tools are allowed; **write / money tools are fail-closed** and return Unauthorized.
- **With `MCP_BEARER_TOKEN`**: every MCP request must send `Authorization: Bearer <token>`.
- Bind to loopback (`127.0.0.1`) for desktop use; do not expose the port without a token.

## 2. Connect

### DeepSeek Harness

Install the bundle into the `desktop` profile (spec = package name, local absolute path, `./x.tgz`, or `github:org/repo`):

```bash
dsh plugin --profile desktop add @twinsearth/agent-universe-harness@3.5.0
MCP_BEARER_TOKEN=<token> dsh start
```

Tools appear to the model as `mcp__agent-universe__<rawTool>` (e.g.
`mcp__agent-universe__market_stats`). No bundle? Use the official client
directly with [`examples/cordis.patch.agent-universe.yml`](./examples/cordis.patch.agent-universe.yml).

### Doubao desktop

左栏「连接器·技能·伙伴」→ 右上「添加」→「新建自定义连接器」→ 传输方式选 **HTTP** →
URL 填 `http://127.0.0.1:4002/api/v1/mcp`，请求头填
`Authorization: Bearer <MCP_BEARER_TOKEN>`（无 token 的本地只读实例可删除请求头）。
参考：[`examples/doubao-connector.json`](./examples/doubao-connector.json)。

### stdio-only hosts

```bash
AGENT_UNIVERSE_MCP_URL=http://127.0.0.1:4002/api/v1/mcp \
AGENT_UNIVERSE_MCP_TOKEN=<token> \
  npx @twinsearth/agent-universe-mcp
```

## 3. Installers (optional)

- macOS / Linux: [`install.sh`](./install.sh) — Linux x86_64 downloads the prebuilt daemon; **macOS builds from source** (no darwin prebuilt in v3.5.0).
- Windows: [`install.ps1`](./install.ps1) — downloads `gsn-daemon-x86_64-pc-windows-msvc.zip`.

Both generate a token, write a start script, and print the connector URL/header.

## 4. Verification matrix (what was actually tested)

Evidence below is from a live `gsn-daemon` v3.5.0 (`serverInfo.version = 0.3.50`) on macOS arm64.

| Check | How | Result |
| --- | --- | --- |
| Direct HTTP MCP `initialize` / `tools/list` | curl against live daemon | ✅ `gsn-agent-market-http` / `2024-11-05`, **27 tools** |
| Unauthorized request with token configured | curl, no / wrong `Authorization` | ✅ **401** |
| Authorized request | curl, correct Bearer | ✅ **200** |
| Money tool without token | `market_deposit` no header | ✅ **401 before dispatch** (fail-closed) |
| Invalid money argument with token | `{"amount":"lots"}` | ✅ **-32602「应为 integer」** — no silent 0 deposit (single dispatch validates first) |
| **Harness bundle** tool discovery | minimal Cordis 4.0.4 host + published `dsh-mcp-client@0.0.1-rc.1` → live daemon | ✅ registers **27** `mcp__agent-universe__*` tools |
| **Harness bundle** live tool call | execute registered `market_stats` | ✅ real JSON returned |
| Harness bundle auth | correct vs wrong Bearer | ✅ 27 tools vs activation rejection |
| Harness bundle daemon-down modes | `failOnStartupError` false / true | ✅ graceful zero-tools vs rejects activation |
| **stdio bridge** e2e | subprocess driver, framed stdin/stdout → live daemon (auth + read-only) | ✅ initialize + **27 tools** on both |
| Resolver unit tests | `node --test` (harness) | ✅ 12/12 |
| Bridge unit tests | `node --test` (stdio bridge) | ✅ 13/13 |
| `install.sh` | temp home, seeded binary, re-run | ✅ token + start script, idempotent |

**Honest boundaries:**

- End-to-end loading was verified against the **real published Harness runtime
  packages** (`@deepseek-ai/cordis@4.0.4`, `dsh-mcp-client@0.0.1-rc.1`) in a
  minimal Cordis host — **not** by clicking through the Harness desktop GUI. The
  bundle uses only the documented `ctx.plugin(McpClient, config)` surface, so a
  GUI install exercises the same path, but that specific action was not performed.
- Installing the rc.1 peer set under **standalone npm** needs
  `--legacy-peer-deps`: DeepSeek's published rc graph mixes `dsh-agent@rc.5`
  (peer-wants `dsh-invariants@rc.5`) with rc.1 packages. Inside the real Harness
  app, pnpm aligns these, so this only affects the standalone harness, not the bundle code.
- `install.ps1` was structurally validated; its live run is covered on Windows.

## Layout

```
connectors/
├── harness/                 # @twinsearth/agent-universe-harness (dsh bundle)
├── agent-universe-mcp/      # @twinsearth/agent-universe-mcp (stdio bridge)
├── examples/                # Doubao config + official-client patch alternative
├── install.sh / install.ps1 # per-platform daemon installer
└── README.md                # this file
```
