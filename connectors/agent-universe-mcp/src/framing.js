// @ts-check
/**
 * LSP-style MCP stdio framing: each JSON-RPC message is prefixed by a
 * `Content-Length: N` header block, terminated by CRLF CRLF.
 *
 * This module is deliberately dependency-free and works in Node >= 18.
 *
 * @fileoverview
 */

'use strict'

const HEADER_END = Buffer.from('\r\n\r\n')

/**
 * Encode one JSON-RPC value into a framed MCP stdio message.
 *
 * @param {unknown} message - A JSON-serializable JSON-RPC value.
 * @returns {Buffer} The framed bytes to write to stdout.
 */
export function encode(message) {
  const payload = Buffer.from(JSON.stringify(message), 'utf8')
  return Buffer.concat([
    Buffer.from(`Content-Length: ${payload.length}\r\n\r\n`, 'ascii'),
    payload,
  ])
}

/**
 * Incremental, stateful parser for framed stdio messages. Feed it arbitrary
 * chunks via {@link FrameParser#push}; fully received messages are delivered
 * through the callback. Incomplete trailing data is retained across calls.
 */
export class FrameParser {
  /**
   * @param {(message: object) => void} onMessage - Called once per complete
   *   JSON-RPC message. Throwing from the callback does not corrupt the buffer.
   */
  constructor(onMessage) {
    /** @type {Buffer} */
    this.buffer = Buffer.alloc(0)
    this.onMessage = onMessage
  }

  /**
   * Append a chunk and drain every complete frame currently available.
   *
   * @param {Buffer | string} chunk
   */
  push(chunk) {
    this.buffer = this.buffer.length === 0
      ? Buffer.from(chunk)
      : Buffer.concat([this.buffer, Buffer.from(chunk)])

    for (;;) {
      const headerEnd = this.buffer.indexOf(HEADER_END)
      if (headerEnd === -1) return

      const headerBlock = this.buffer.subarray(0, headerEnd).toString('ascii')
      const length = parseContentLength(headerBlock)
      if (length === null) {
        // Malformed framing cannot be resynchronized safely; surface clearly.
        throw new Error(`invalid MCP stdio framing, missing Content-Length: ${headerBlock.trim()}`)
      }

      const bodyStart = headerEnd + HEADER_END.length
      const bodyEnd = bodyStart + length
      if (this.buffer.length < bodyEnd) return // wait for the rest of the body

      const body = this.buffer.subarray(bodyStart, bodyEnd)
      this.buffer = this.buffer.subarray(bodyEnd)

      let message
      try {
        message = JSON.parse(body.toString('utf8'))
      } catch (err) {
        throw new Error(`invalid JSON in MCP frame: ${/** @type {Error} */ (err).message}`)
      }
      this.onMessage(message)
    }
  }
}

/**
 * Extract the Content-Length value from a raw header block.
 *
 * @param {string} headerBlock
 * @returns {number | null}
 */
function parseContentLength(headerBlock) {
  for (const line of headerBlock.split(/\r?\n/)) {
    const idx = line.indexOf(':')
    if (idx === -1) continue
    const name = line.slice(0, idx).trim().toLowerCase()
    if (name === 'content-length') {
      const value = Number.parseInt(line.slice(idx + 1).trim(), 10)
      return Number.isFinite(value) && value >= 0 ? value : null
    }
  }
  return null
}
