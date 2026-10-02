import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  DEFAULT_SERVER_NAME,
  DEFAULT_TOOL_CALL_TIMEOUT_MS,
  DEFAULT_URL,
  resolveClientConfig,
} from '../lib/resolve.js'

test('defaults: local unauthenticated endpoint, no Authorization header', () => {
  const cfg = resolveClientConfig({}, {})
  assert.equal(cfg.transport, 'streamable-http')
  assert.equal(cfg.url, DEFAULT_URL)
  assert.equal(cfg.serverName, DEFAULT_SERVER_NAME)
  assert.equal(cfg.toolCallTimeoutMs, DEFAULT_TOOL_CALL_TIMEOUT_MS)
  assert.equal(cfg.failOnStartupError, false)
  assert.deepEqual(cfg.headers, {})
})

test('explicit token becomes a Bearer Authorization header', () => {
  const cfg = resolveClientConfig({ token: 'abc123' }, {})
  assert.deepEqual(cfg.headers, { Authorization: 'Bearer abc123' })
})

test('falls back to MCP_BEARER_TOKEN then AGENT_UNIVERSE_MCP_TOKEN env', () => {
  assert.equal(
    resolveClientConfig({}, { MCP_BEARER_TOKEN: 'env-tok' }).headers.Authorization,
    'Bearer env-tok',
  )
  assert.equal(
    resolveClientConfig(
      {},
      { AGENT_UNIVERSE_MCP_TOKEN: 'alt-tok' },
    ).headers.Authorization,
    'Bearer alt-tok',
  )
  // MCP_BEARER_TOKEN wins over AGENT_UNIVERSE_MCP_TOKEN.
  assert.equal(
    resolveClientConfig(
      {},
      { MCP_BEARER_TOKEN: 'a', AGENT_UNIVERSE_MCP_TOKEN: 'b' },
    ).headers.Authorization,
    'Bearer a',
  )
})

test('explicit token overrides env token', () => {
  const cfg = resolveClientConfig(
    { token: 'explicit' },
    { MCP_BEARER_TOKEN: 'env' },
  )
  assert.equal(cfg.headers.Authorization, 'Bearer explicit')
})

test('an explicit Authorization header is preserved and not duplicated', () => {
  const cfg = resolveClientConfig(
    { token: 'tok', headers: { authorization: 'Bearer custom' } },
    {},
  )
  assert.deepEqual(cfg.headers, { authorization: 'Bearer custom' })
})

test('extra headers are stringified and forwarded', () => {
  const cfg = resolveClientConfig(
    { headers: { 'X-Tenant': 7, 'X-Note': 'ok' } },
    {},
  )
  assert.deepEqual(cfg.headers, { 'X-Tenant': '7', 'X-Note': 'ok' })
})

test('custom url/serverName/timeout/fail flag are honored', () => {
  const cfg = resolveClientConfig(
    {
      url: 'https://daemon.example:9000/mcp',
      serverName: 'team-1',
      toolCallTimeoutMs: 1234,
      failOnStartupError: true,
    },
    {},
  )
  assert.equal(cfg.url, 'https://daemon.example:9000/mcp')
  assert.equal(cfg.serverName, 'team-1')
  assert.equal(cfg.toolCallTimeoutMs, 1234)
  assert.equal(cfg.failOnStartupError, true)
})

test('rejects non-http url', () => {
  assert.throws(
    () => resolveClientConfig({ url: 'ftp://x' }, {}),
    /url must be http\(s\)/,
  )
})

test('rejects invalid serverName', () => {
  // Empty/whitespace fall back to the default (validated separately), so they
  // are not rejection cases here.
  for (const bad of ['has space', 'bad/name', 'a'.repeat(33), 'a.b']) {
    assert.throws(
      () => resolveClientConfig({ serverName: bad }, {}),
      /serverName/,
    )
  }
})

test('accepts boundary serverName lengths and charset', () => {
  assert.equal(resolveClientConfig({ serverName: 'a' }, {}).serverName, 'a')
  assert.equal(
    resolveClientConfig({ serverName: 'A-z_0-9'.repeat(5).slice(0, 32) }, {})
      .serverName.length,
    32,
  )
})

test('rejects non-positive or non-finite timeout', () => {
  for (const bad of [0, -1, NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(
      () => resolveClientConfig({ toolCallTimeoutMs: bad }, {}),
      /toolCallTimeoutMs/,
    )
  }
})

test('whitespace-only url/serverName/token fall back like absent', () => {
  const cfg = resolveClientConfig(
    { url: '   ', serverName: '  ', token: '   ' },
    {},
  )
  assert.equal(cfg.url, DEFAULT_URL)
  assert.equal(cfg.serverName, DEFAULT_SERVER_NAME)
  assert.deepEqual(cfg.headers, {})
})
