// @ts-check
/**
 * Pure configuration resolver for the Agent Universe → DeepSeek Harness bundle.
 *
 * This module deliberately imports nothing from the Harness host (no cordis,
 * no schemastery, no @deepseek-ai/*) so it can be unit-tested in plain Node
 * and reused by both the plugin entry and tooling. The host-facing wrapper in
 * `index.js` feeds the resolved object straight into `@deepseek-ai/dsh-mcp-client`.
 *
 * @fileoverview
 */

/** Default Agent Universe MCP endpoint (gsn-daemon stateless Streamable HTTP). */
export const DEFAULT_URL = 'http://127.0.0.1:4002/api/v1/mcp'

/** Default local tool namespace; models see `mcp__agent-universe__<rawTool>`. */
export const DEFAULT_SERVER_NAME = 'agent-universe'

/** Per-tool-call timeout matching dsh-mcp-client's own default. */
export const DEFAULT_TOOL_CALL_TIMEOUT_MS = 60_000

const SERVER_NAME_PATTERN = /^[A-Za-z0-9_-]{1,32}$/

/**
 * @typedef {Object} BundleConfig
 * @property {string} [url]              Agent Universe MCP endpoint.
 * @property {string} [token]            Bearer token; falls back to env when empty.
 * @property {string} [serverName]       Local namespace for public tool names.
 * @property {number} [toolCallTimeoutMs] Per-call timeout.
 * @property {boolean} [failOnStartupError] Reject activation if initial sync fails.
 * @property {Record<string, string>} [headers] Extra/override HTTP headers.
 */

/**
 * Validate the user-facing bundle config and resolve it into the exact
 * Streamable HTTP config accepted by the published `@deepseek-ai/dsh-mcp-client`
 * (rc.1: `transport/serverName/url/headers/toolCallTimeoutMs/failOnStartupError`).
 *
 * Throws `TypeError` on invalid input so the Cordis schema + runtime both fail
 * loudly rather than silently connecting to the wrong endpoint.
 *
 * @param {BundleConfig} [raw]
 * @param {NodeJS.ProcessEnv} [env] Injectable environment (defaults process.env).
 * @returns {{
 *   transport: 'streamable-http',
 *   serverName: string,
 *   url: string,
 *   headers: Record<string, string>,
 *   toolCallTimeoutMs: number,
 *   failOnStartupError: boolean,
 * }}
 */
export function resolveClientConfig(raw = {}, env = (globalThis.process?.env ?? {})) {
  const url = nonEmpty(raw.url) ?? DEFAULT_URL
  if (!/^https?:\/\//i.test(url)) {
    throw new TypeError(`agent-universe-harness: url must be http(s), got: ${String(url)}`)
  }

  const serverName = nonEmpty(raw.serverName) ?? DEFAULT_SERVER_NAME
  if (!SERVER_NAME_PATTERN.test(serverName)) {
    throw new TypeError(
      'agent-universe-harness: serverName must match ^[A-Za-z0-9_-]{1,32}$',
    )
  }

  const explicitToken =
    raw.token === undefined || raw.token === null
      ? ''
      : String(raw.token).trim()
  const envToken =
    (env.MCP_BEARER_TOKEN && String(env.MCP_BEARER_TOKEN).trim()) ||
    (env.AGENT_UNIVERSE_MCP_TOKEN && String(env.AGENT_UNIVERSE_MCP_TOKEN).trim()) ||
    ''
  const token = explicitToken || envToken

  const toolCallTimeoutMs =
    raw.toolCallTimeoutMs === undefined
      ? DEFAULT_TOOL_CALL_TIMEOUT_MS
      : Number(raw.toolCallTimeoutMs)
  if (!Number.isFinite(toolCallTimeoutMs) || toolCallTimeoutMs < 1) {
    throw new TypeError(
      `agent-universe-harness: toolCallTimeoutMs must be a positive number, got: ${String(raw.toolCallTimeoutMs)}`,
    )
  }

  const failOnStartupError =
    raw.failOnStartupError === undefined ? false : Boolean(raw.failOnStartupError)

  /** @type {Record<string, string>} */
  const headers = {}
  const extra = raw.headers ?? {}
  for (const [key, value] of Object.entries(extra)) {
    if (value !== undefined && value !== null) headers[key] = String(value)
  }
  // Attach the Bearer token unless the operator already set Authorization.
  // Empty token → no header, so an explicitly token-less local daemon still works.
  if (token && !hasKeyCaseInsensitive(headers, 'authorization')) {
    headers.Authorization = `Bearer ${token}`
  }

  return {
    transport: 'streamable-http',
    serverName,
    url,
    headers,
    toolCallTimeoutMs,
    failOnStartupError,
  }
}

/**
 * @param {string|undefined} value
 * @returns {string|undefined}
 */
function nonEmpty(value) {
  if (value === undefined || value === null) return undefined
  const s = String(value).trim()
  return s.length ? s : undefined
}

/**
 * @param {Record<string, string>} obj
 * @param {string} key
 * @returns {boolean}
 */
function hasKeyCaseInsensitive(obj, key) {
  const target = key.toLowerCase()
  return Object.keys(obj).some((k) => k.toLowerCase() === target)
}
