# @twinsearth/agent-universe-harness

A [DeepSeek Harness (`dsh`)](https://github.com/deepseek-ai/deepseek-harness)
**profile bundle** that connects a local Agent Universe `gsn-daemon` MCP endpoint
and publishes its tools to the model as `mcp__agent-universe__*`.

It is a thin, configuration-only wrapper around DeepSeek's published
[`@deepseek-ai/dsh-mcp-client`](https://www.npmjs.com/package/@deepseek-ai/dsh-mcp-client)
(streamable-http transport). No stdio bridge, no native code.

## Install

```bash
# spec may be a package name, local absolute path, ./x.tgz, or github:org/repo
dsh plugin --profile desktop add @twinsearth/agent-universe-harness@3.5.0
MCP_BEARER_TOKEN=<your daemon token> dsh start
```

The bundle ships `dsh.bundle.patch` (a single loader row, `id: agent-universe`),
so it self-registers the MCP client in the `desktop` profile.

## Configuration

Resolved by `lib/resolve.js` (pure, unit-tested):

| Key | Env override | Default |
| --- | --- | --- |
| `url` | `MCP_URL` / `AGENT_UNIVERSE_MCP_URL` | `http://127.0.0.1:4002/api/v1/mcp` |
| `serverName` | — | `agent-universe` (`^[A-Za-z0-9_-]{1,32}$`) |
| `token` | `MCP_BEARER_TOKEN` / `AGENT_UNIVERSE_MCP_TOKEN` | none |
| `failOnStartupError` | — | `false` |
| `toolCallTimeoutMs` | — | positive integer |

- Explicit `config.headers.Authorization` is never duplicated; an explicit token
  takes precedence over env; whitespace-only tokens are ignored.
- `failOnStartupError: false` (default) loads with zero tools when the daemon is
  down; `true` rejects plugin activation instead.

## Verified

With the published runtime (`@deepseek-ai/cordis@4.0.4`,
`dsh-mcp-client@0.0.1-rc.1`) against a live v3.5.0 daemon: registers **27**
tools, a live `market_stats` call returns real data, correct/wrong Bearer behave
as 200/rejection, and both daemon-down startup modes behave as documented.

> Standalone `npm install` of the rc.1 peer set may need `--legacy-peer-deps`
> (DeepSeek's rc graph mixes dsh-agent rc.5 peers with rc.1). Inside Harness,
> pnpm aligns these. See the workspace `connectors/README.md` verification matrix.

## Alternative (no bundle)

Use the official client directly with
`../examples/cordis.patch.agent-universe.yml`.
