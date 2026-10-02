// @ts-check
/**
 * Agent Universe connector bundle for DeepSeek Harness (desktop / web / CLI).
 *
 * Thin turnkey wrapper around the published `@deepseek-ai/dsh-mcp-client`
 * Streamable HTTP transport. One bundle instance connects one running
 * `gsn-daemon` MCP endpoint (default `http://127.0.0.1:4002/api/v1/mcp`) and
 * publishes its tools to the model as `mcp__agent-universe__<rawTool>`.
 *
 * The wrapper owns only Agent-University-specific defaults (endpoint, Bearer
 * token, namespace). All transport, discovery, re-sync, naming and execution
 * semantics remain those of `dsh-mcp-client`, so there is no parallel MCP
 * implementation to drift.
 *
 * Targets the PUBLISHED `@deepseek-ai/dsh-mcp-client@0.0.1-rc.1` contract:
 * the client is a namespace plugin (`{ name, inject, Config, apply }`) and a
 * plain config object is passed to `ctx.plugin(...)`. rc.1 has NO
 * `McpClient.Config(...)` fork helper and NO `reconnect` option — do not copy
 * those from newer unreleased Harness source.
 *
 * @module @twinsearth/agent-universe-harness
 */

import z from '@deepseek-ai/schemastery'
import * as McpClient from '@deepseek-ai/dsh-mcp-client'
import {
  DEFAULT_SERVER_NAME,
  DEFAULT_TOOL_CALL_TIMEOUT_MS,
  DEFAULT_URL,
  resolveClientConfig,
} from './resolve.js'

/** Cordis plugin name used by loader diagnostics. */
export const name = 'agent-universe-harness'

/** The MCP client requires the tool registry; ensure it is present first. */
export const inject = ['tools']

const SERVER_NAME_PATTERN = /^[A-Za-z0-9_-]{1,32}$/

/**
 * User-facing configuration. Everything is optional — the bundle connects to
 * a local, unauthenticated daemon out of the box. Set `token` (or the
 * MCP_BEARER_TOKEN / AGENT_UNIVERSE_MCP_TOKEN env vars) for a daemon launched
 * with `MCP_BEARER_TOKEN`.
 */
export const Config = z.object({
  /** Agent Universe MCP endpoint. */
  url: z.string().default(DEFAULT_URL),
  /** Bearer token; env MCP_BEARER_TOKEN / AGENT_UNIVERSE_MCP_TOKEN used when empty. */
  token: z.string().default(''),
  /** Tool namespace; produces mcp__<serverName>__<rawTool>. */
  serverName: z
    .string()
    .pattern(SERVER_NAME_PATTERN)
    .default(DEFAULT_SERVER_NAME),
  /** Per-tool-call timeout in milliseconds. */
  toolCallTimeoutMs: z.number().min(1).default(DEFAULT_TOOL_CALL_TIMEOUT_MS),
  /** Reject bundle activation if the first connection / tool sync fails. */
  failOnStartupError: z.boolean().default(false),
  /** Extra HTTP headers; an explicit Authorization overrides the token. */
  headers: z.dict(String).default({}),
})

/**
 * Connect the daemon and expose its tools before activation settles.
 *
 * @param {import('@deepseek-ai/cordis').Context} ctx Cordis context carrying `tools`.
 * @param {{
 *   url?: string,
 *   token?: string,
 *   serverName?: string,
 *   toolCallTimeoutMs?: number,
 *   failOnStartupError?: boolean,
 *   headers?: Record<string, string>,
 * }} config Resolved user config.
 * @returns {Promise<void>} Resolves after the child plugin's initial sync settles.
 */
export async function apply(ctx, config) {
  const clientConfig = resolveClientConfig(config)

  // Fork the published MCP client plugin under our context. It is effect-scoped:
  // disposal disconnects, unregisters this server's tools and frees the namespace.
  const fork = ctx.plugin(McpClient, clientConfig)

  // Tie the child's lifetime explicitly to ours so HMR/unload tears the
  // connection down deterministically even if the child fork is non-contextual.
  ctx.effect(
    () => () => {
      try {
        if (typeof fork.dispose === 'function') {
          const maybe = fork.dispose()
          if (maybe && typeof maybe.then === 'function') {
            maybe.catch(() => {})
          }
        }
      } catch {
        // Disposal must never throw out of an effect; the child logs its own close.
      }
    },
    'agent-universe-harness.connection',
  )

  // Surface startup errors. With failOnStartupError=true the child rejects and
  // activation fails (rolling back); with false the child logs and loads with
  // zero tools, matching dsh-mcp-client's own documented default.
  await fork.await()
}

export { resolveClientConfig, DEFAULT_URL, DEFAULT_SERVER_NAME } from './resolve.js'
