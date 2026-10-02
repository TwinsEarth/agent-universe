# @twinsearth/agent-universe-mcp

Zero-dependency **stdio ↔ Streamable-HTTP MCP bridge** for an Agent Universe
`gsn-daemon`. Use this only for MCP hosts that require a **local stdio process**
(Claude Desktop, etc.). Modern HTTP clients (DeepSeek Harness, Doubao desktop)
connect directly to `/api/v1/mcp` and do not need this bridge.

```
stdio-only host  ──LSP-framed stdio──►  agent-universe-mcp  ──HTTP──►  gsn-daemon /api/v1/mcp
```

- Framing: LSP-style `Content-Length: N\r\n\r\n{json}` (standard MCP stdio).
- HTTP: one JSON-RPC request per `POST`, streamed-forward to stdout.
- Runtime: Node >= 18, no dependencies.

## Run

```bash
npx @twinsearth/agent-universe-mcp
# binary name: agent-universe-mcp
```

| Env var | Default |
| --- | --- |
| `AGENT_UNIVERSE_MCP_URL` | `http://127.0.0.1:4002/api/v1/mcp` |
| `AGENT_UNIVERSE_MCP_TOKEN` (or `MCP_BEARER_TOKEN`) | none |
| `AGENT_UNIVERSE_MCP_TIMEOUT_MS` | `30000` |

Example stdio host config:

```json
{
  "mcpServers": {
    "agent-universe": {
      "command": "npx",
      "args": ["-y", "@twinsearth/agent-universe-mcp@3.5.0"],
      "env": {
        "AGENT_UNIVERSE_MCP_URL": "http://127.0.0.1:4002/api/v1/mcp",
        "AGENT_UNIVERSE_MCP_TOKEN": "<MCP_BEARER_TOKEN>"
      }
    }
  }
}
```

## Verified

Subprocess driver sent framed `initialize` + `tools/list` to a live v3.5.0
daemon over the bridge: `gsn-agent-market-http` handshake and **27 tools** on
both the token-protected and read-only daemons, clean stderr. Unit tests: 13/13.
