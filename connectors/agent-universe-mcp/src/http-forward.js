// @ts-check
/**
 * Forward a single JSON-RPC message to the agent-universe daemon's stateless
 * Streamable HTTP MCP endpoint (`POST /api/v1/mcp`).
 *
 * The daemon endpoint is stateless: every request carries one JSON-RPC message
 * and receives exactly one JSON-RPC response, so this bridge is a thin,
 * connectionless adapter that lets any stdio-only MCP host drive the daemon.
 *
 * @fileoverview
 */

'use strict'

/**
 * @typedef {Object} ForwardOptions
 * @property {string} url                       - Full MCP HTTP endpoint URL.
 * @property {string} [token]                   - Optional bearer token (MCP_BEARER_TOKEN).
 * @property {number} [timeoutMs=30000]         - Per-request timeout.
 * @property {typeof fetch} [fetchImpl]         - Injectable fetch (for testing).
 * @property {(...args: unknown[]) => void} [log] - Diagnostics sink (stderr use).
 */

/**
 * Forward one JSON-RPC message.
 *
 * - Notifications (messages without an `id`) are delivered and return `null`;
 *   the daemon answers them with HTTP 202 and no stdout frame is produced.
 * - Requests return the daemon's JSON-RPC response, or a synthesized JSON-RPC
 *   error response (transport failures, timeouts, non-JSON bodies) carrying the
 *   same id. The bridge never crashes on upstream failure.
 *
 * @param {object} message - Parsed JSON-RPC message from the stdio host.
 * @param {ForwardOptions} options
 * @returns {Promise<object | null>} A JSON-RPC response, or null for notifications.
 */
export async function forwardJsonRpc(message, options) {
  const {
    url,
    token,
    timeoutMs = 30_000,
    fetchImpl = globalThis.fetch,
    log = () => {},
  } = options

  const expectResponse = hasId(message)

  const headers = {
    'Content-Type': 'application/json',
    Accept: 'application/json, text/event-stream',
  }
  if (token) headers.Authorization = `Bearer ${token}`

  /** @type {Response | undefined} */
  let res
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(new Error('timeout')), timeoutMs)
  try {
    res = await fetchImpl(url, {
      method: 'POST',
      headers,
      body: JSON.stringify(message),
      signal: controller.signal,
    })
  } catch (err) {
    if (!expectResponse) return null
    return rpcError(message.id, -32000, `MCP HTTP request failed: ${errorText(err)}`)
  } finally {
    clearTimeout(timer)
  }

  const raw = await res.text().catch(() => '')
  if (!expectResponse) return null

  const parsed = decodeBody(raw, res.headers.get('content-type') || '')
  if (parsed && typeof parsed === 'object') {
    // Trust a well-formed JSON-RPC response even on non-200 (e.g. daemon 401
    // returns a JSON-RPC error body). Preserve the request id.
    if ('id' in parsed) return withId(parsed, message.id)
    return { jsonrpc: '2.0', id: message.id, result: parsed }
  }

  return rpcError(
    message.id,
    -32603,
    `MCP HTTP ${res.status}: non-JSON response${raw ? `: ${truncate(raw)}` : ''}`,
  )
}

/**
 * @param {unknown} m
 * @returns {m is {id: unknown}}
 */
function hasId(m) {
  return typeof m === 'object' && m !== null && 'id' in m
}

/**
 * Decode either a plain JSON body or an SSE stream carrying `data:` frames.
 *
 * @param {string} raw
 * @param {string} contentType
 * @returns {unknown}
 */
function decodeBody(raw, contentType) {
  const direct = tryParse(raw)
  if (direct !== undefined) return direct

  if (contentType.includes('text/event-stream') || raw.includes('data:')) {
    let last
    for (const line of raw.split(/\r?\n/)) {
      const trimmed = line.trim()
      if (!trimmed.startsWith('data:')) continue
      const data = trimmed.slice(5).trim()
      if (!data || data === '[DONE]') continue
      const value = tryParse(data)
      if (value !== undefined) last = value
    }
    return last
  }
  return undefined
}

/**
 * @param {string} text
 * @returns {unknown} undefined when not parseable.
 */
function tryParse(text) {
  if (!text) return undefined
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

/**
 * Force the response id to match the request id (stateless proxy contract).
 *
 * @param {Record<string, unknown>} response
 * @param {unknown} id
 * @returns {object}
 */
function withId(response, id) {
  return { ...response, id }
}

/**
 * @param {unknown} id
 * @param {number} code
 * @param {string} message
 * @returns {{jsonrpc: string, id: unknown, error: {code: number, message: string}}}
 */
function rpcError(id, code, message) {
  return { jsonrpc: '2.0', id, error: { code, message } }
}

/** @param {unknown} err */
function errorText(err) {
  if (err instanceof Error) return err.message
  return String(err)
}

/** @param {string} s */
function truncate(s) {
  return s.length > 200 ? `${s.slice(0, 200)}…` : s
}
