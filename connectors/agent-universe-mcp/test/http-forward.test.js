import { test } from 'node:test'
import assert from 'node:assert/strict'
import { forwardJsonRpc } from '../src/http-forward.js'

const URL = 'http://127.0.0.1:4002/api/v1/mcp'

/**
 * Build an injectable fetch capturing the request and returning a canned body.
 *
 * @param {{status?: number, body?: string, contentType?: string}} reply
 * @param {{captured?: object}} [sink]
 */
function fakeFetch(reply, sink = {}) {
  return async (url, init) => {
    sink.captured = { url, ...init }
    return new Response(reply.body ?? '', {
      status: reply.status ?? 200,
      headers: { 'Content-Type': reply.contentType ?? 'application/json' },
    })
  }
}

test('passes a JSON-RPC response through and pins the request id', async () => {
  const sink = {}
  // Daemon (mis)labels the response id 999; bridge must pin it back to 7.
  const fetchImpl = fakeFetch(
    { body: JSON.stringify({ jsonrpc: '2.0', id: 999, result: { tools: [] } }) },
    sink,
  )
  const out = await forwardJsonRpc(
    { jsonrpc: '2.0', id: 7, method: 'tools/list' },
    { url: URL, fetchImpl },
  )
  assert.equal(out.id, 7)
  assert.deepEqual(out.result.tools, [])
  assert.equal(sink.captured.url, URL)
  assert.match(sink.captured.headers.Accept, /text\/event-stream/)
  assert.equal(sink.captured.headers.Authorization, undefined)
})

test('attaches the bearer token when configured', async () => {
  const sink = {}
  const fetchImpl = fakeFetch({ body: '{"jsonrpc":"2.0","id":1,"result":{}}' }, sink)
  await forwardJsonRpc(
    { jsonrpc: '2.0', id: 1, method: 'initialize' },
    { url: URL, token: 'secret', fetchImpl },
  )
  assert.equal(sink.captured.headers.Authorization, 'Bearer secret')
})

test('a notification (no id) is delivered but yields no stdout frame', async () => {
  let calls = 0
  const fetchImpl = async () => { calls++; return new Response('', { status: 202 }) }
  const out = await forwardJsonRpc(
    { jsonrpc: '2.0', method: 'notifications/initialized' },
    { url: URL, fetchImpl },
  )
  assert.equal(calls, 1)
  assert.equal(out, null)
})

test('decodes an SSE data: frame carrying the JSON-RPC response', async () => {
  const body = 'event: message\r\ndata: {"jsonrpc":"2.0","id":3,"result":{"ok":true}}\r\n\r\n'
  const fetchImpl = fakeFetch({ body, contentType: 'text/event-stream' })
  const out = await forwardJsonRpc(
    { jsonrpc: '2.0', id: 3, method: 'tools/call' },
    { url: URL, fetchImpl },
  )
  assert.equal(out.id, 3)
  assert.equal(out.result.ok, true)
})

test('surfaces a daemon 401 JSON-RPC error body unchanged', async () => {
  const body = JSON.stringify({
    jsonrpc: '2.0',
    id: 4,
    error: { code: -32001, message: 'Unauthorized: MCP bearer token required' },
  })
  const fetchImpl = fakeFetch({ status: 401, body })
  const out = await forwardJsonRpc(
    { jsonrpc: '2.0', id: 4, method: 'tools/call' },
    { url: URL, fetchImpl },
  )
  assert.equal(out.error.code, -32001)
  assert.match(out.error.message, /Unauthorized/)
})

test('maps a network failure to a JSON-RPC -32000 error', async () => {
  const fetchImpl = async () => { throw new Error('ECONNREFUSED') }
  const out = await forwardJsonRpc(
    { jsonrpc: '2.0', id: 5, method: 'tools/list' },
    { url: URL, fetchImpl },
  )
  assert.equal(out.id, 5)
  assert.equal(out.error.code, -32000)
  assert.match(out.error.message, /ECONNREFUSED/)
})

test('maps a timeout to a JSON-RPC -32000 error', async () => {
  const fetchImpl = (_url, init) => new Promise((_resolve, reject) => {
    init.signal.addEventListener('abort', () => {
      const err = new Error('The operation was aborted')
      err.name = 'AbortError'
      reject(err)
    })
  })
  const out = await forwardJsonRpc(
    { jsonrpc: '2.0', id: 6, method: 'tools/list' },
    { url: URL, timeoutMs: 20, fetchImpl },
  )
  assert.equal(out.id, 6)
  assert.equal(out.error.code, -32000)
})

test('maps a non-JSON 200 body to -32603', async () => {
  const fetchImpl = fakeFetch({ body: '<html>bad gateway</html>', contentType: 'text/html' })
  const out = await forwardJsonRpc(
    { jsonrpc: '2.0', id: 8, method: 'tools/list' },
    { url: URL, fetchImpl },
  )
  assert.equal(out.id, 8)
  assert.equal(out.error.code, -32603)
})
