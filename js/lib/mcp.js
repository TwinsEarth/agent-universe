// lib/mcp.js
// MCP（Model Context Protocol）HTTP 客户端。
// 通过 gsn-daemon 的 POST /api/v1/mcp JSON-RPC 端点调用市场工具，
// 完整走 initialize → notifications/initialized → ping / tools/list /
// tools/call。tools/call 由 daemon 端 MarketMcpBridge 真实路由执行。
// 使用 Node 18+ 内置 fetch，零额外依赖。

const MCP_PROTOCOL_VERSION = '2024-11-05';

class McpError extends Error {
  constructor(code, message) {
    super(`[JSON-RPC ${code}] ${message}`);
    this.code = code;
  }
}

class McpHttpClient {
  constructor(
    baseUrl = 'http://127.0.0.1:4002',
    path = '/api/v1/mcp',
    timeout = 10000
  ) {
    this.endpoint = baseUrl.replace(/\/$/, '') + path;
    this.timeout = timeout;
    this._id = 0;
    this.initialized = false;
  }

  async _post(payload, awaitResponse = true) {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), this.timeout);
    let res;
    try {
      res = await fetch(this.endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'application/json',
        },
        body: JSON.stringify(payload),
        signal: ctrl.signal,
      });
    } finally {
      clearTimeout(timer);
    }

    const text = await res.text();
    if (!awaitResponse || !text) return null;
    const rpc = JSON.parse(text);
    if (rpc.error) {
      throw new McpError(rpc.error.code, rpc.error.message);
    }
    return rpc.result;
  }

  async initialize() {
    const result = await this._post({
      jsonrpc: '2.0',
      id: ++this._id,
      method: 'initialize',
      params: {
        protocolVersion: MCP_PROTOCOL_VERSION,
        clientInfo: { name: 'agent-universe-js', version: '2.5.5' },
        capabilities: {},
      },
    });
    await this._post(
      { jsonrpc: '2.0', method: 'notifications/initialized' },
      false
    );
    this.initialized = true;
    return result;
  }

  async ping() {
    await this._post({ jsonrpc: '2.0', id: ++this._id, method: 'ping' });
    return true;
  }

  async listTools() {
    if (!this.initialized) await this.initialize();
    const result = await this._post({
      jsonrpc: '2.0',
      id: ++this._id,
      method: 'tools/list',
    });
    return result.tools;
  }

  async callTool(name, args = {}) {
    if (!this.initialized) await this.initialize();
    return this._post({
      jsonrpc: '2.0',
      id: ++this._id,
      method: 'tools/call',
      params: { name, arguments: args },
    });
  }
}

module.exports = { McpHttpClient, McpError, MCP_PROTOCOL_VERSION };
