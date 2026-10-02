#!/usr/bin/env node
// @ts-check
/**
 * agent-universe MCP stdio ↔ Streamable-HTTP bridge.
 *
 * Lets any stdio-only MCP host (Claude Desktop-style launchers, generic MCP
 * clients that spawn a command) talk to a running agent-universe `gsn-daemon`
 * whose native MCP surface is stateless HTTP at `POST /api/v1/mcp`.
 *
 * Configuration (environment variables):
 *   AGENT_UNIVERSE_MCP_URL   Full endpoint (default http://127.0.0.1:4002/api/v1/mcp)
 *   AGENT_UNIVERSE_MCP_TOKEN Bearer token (also accepts MCP_BEARER_TOKEN)
 *   AGENT_UNIVERSE_MCP_TIMEOUT_MS  Per-request timeout (default 30000)
 *
 * No npm dependencies; requires Node.js >= 18 (global fetch).
 *
 * @fileoverview
 */

'use strict'

import { once } from 'node:events'
import { createRequire } from 'node:module'
import { FrameParser, encode } from './framing.js'
import { forwardJsonRpc } from './http-forward.js'

// Support being run directly (ESM) while keeping a require for diagnostics.
createRequire(import.meta.url)

/** @param {string} msg */
function log(msg) {
  process.stderr.write(`[agent-universe-mcp] ${msg}\n`)
}

function loadConfig() {
  return {
    url: process.env.AGENT_UNIVERSE_MCP_URL
      || 'http://127.0.0.1:4002/api/v1/mcp',
    token: process.env.AGENT_UNIVERSE_MCP_TOKEN
      || process.env.MCP_BEARER_TOKEN
      || '',
    timeoutMs: Number.parseInt(
      process.env.AGENT_UNIVERSE_MCP_TIMEOUT_MS || '30000',
      10,
    ) || 30_000,
  }
}

async function main() {
  const config = loadConfig()

  /**
   * Serialize forwarding so stdout responses are emitted in the same order the
   * host issued requests, even if the HTTP round-trips resolve out of order.
   * @type {Promise<void>}
   */
  let queue = Promise.resolve()

  const parser = new FrameParser((message) => {
    queue = queue.then(async () => {
      try {
        const response = await forwardJsonRpc(message, {
          url: config.url,
          token: config.token || undefined,
          timeoutMs: config.timeoutMs,
          log,
        })
        if (response !== null) {
          await writeStdout(encode(response))
        }
      } catch (err) {
        // Defensive: forwardJsonRpc already maps failures to JSON-RPC errors.
        const id = message && typeof message === 'object' && 'id' in message
          ? /** @type {{id: unknown}} */ (message).id
          : null
        await writeStdout(encode({
          jsonrpc: '2.0',
          id,
          error: {
            code: -32603,
            message: `bridge internal error: ${err instanceof Error ? err.message : String(err)}`,
          },
        }))
      }
    })
  })

  process.stdin.on('data', (chunk) => {
    try {
      parser.push(/** @type {Buffer} */ (chunk))
    } catch (err) {
      log(`framing error: ${err instanceof Error ? err.message : String(err)}`)
    }
  })
  process.stdin.on('error', (err) => log(`stdin error: ${err.message}`))

  const shutdown = () => {
    // Give in-flight responses a moment to flush, then exit.
    queue.finally(() => process.exit(0)).catch(() => process.exit(0))
  }
  process.on('SIGINT', shutdown)
  process.on('SIGTERM', shutdown)

  await once(process.stdin, 'end').catch(() => {})
  await queue.catch(() => {})
}

/**
 * Write framed bytes, awaiting backpressure instead of dropping on overflow.
 *
 * @param {Buffer} chunk
 * @returns {Promise<void>}
 */
function writeStdout(chunk) {
  return new Promise((resolve) => {
    if (process.stdout.write(chunk)) return resolve()
    process.stdout.once('drain', () => resolve())
  })
}

main().catch((err) => {
  log(`fatal: ${err instanceof Error ? err.stack || err.message : String(err)}`)
  process.exit(1)
})
