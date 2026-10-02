import { test } from 'node:test'
import assert from 'node:assert/strict'
import { encode, FrameParser } from '../src/framing.js'

function drain(parser) {
  const got = []
  parser.onMessage = (m) => got.push(m)
  return got
}

test('round-trips a single message', () => {
  const msg = { jsonrpc: '2.0', id: 1, result: { ok: true, n: 27 } }
  const parser = new FrameParser(() => {})
  const got = drain(parser)
  parser.push(encode(msg))
  assert.equal(got.length, 1)
  assert.deepEqual(got[0], msg)
})

test('parses multiple frames in one chunk', () => {
  const a = { jsonrpc: '2.0', id: 1, result: {} }
  const b = { jsonrpc: '2.0', id: 2, method: 'tools/list' }
  const parser = new FrameParser(() => {})
  const got = drain(parser)
  parser.push(Buffer.concat([encode(a), encode(b)]))
  assert.deepEqual(got, [a, b])
})

test('retains an incomplete trailing body across pushes', () => {
  const msg = { jsonrpc: '2.0', id: 9, method: 'initialize' }
  const bytes = encode(msg)
  const parser = new FrameParser(() => {})
  const got = drain(parser)

  // Feed header one byte at a time, then body one byte at a time.
  for (let i = 0; i < bytes.length; i++) {
    parser.push(bytes.subarray(i, i + 1))
    if (i < bytes.length - 1) assert.equal(got.length, 0)
  }
  assert.equal(got.length, 1)
  assert.deepEqual(got[0], msg)
})

test('throws when Content-Length is missing', () => {
  const parser = new FrameParser(() => {})
  assert.throws(
    () => parser.push(Buffer.from('Content-Type: application/json\r\n\r\n{}')),
    /missing Content-Length/,
  )
})

test('throws on invalid JSON body', () => {
  const parser = new FrameParser(() => {})
  const bad = Buffer.from('Content-Length: 5\r\n\r\n{bad}')
  assert.throws(() => parser.push(bad), /invalid JSON/)
})
