import type { Context } from '@deepseek-ai/cordis'

/** User-facing Agent Universe Harness bundle configuration (all fields optional). */
export interface AgentUniverseHarnessConfig {
  /** Agent Universe MCP endpoint. Default http://127.0.0.1:4002/api/v1/mcp. */
  url?: string
  /** Bearer token. Falls back to MCP_BEARER_TOKEN / AGENT_UNIVERSE_MCP_TOKEN env. */
  token?: string
  /** Local tool namespace. Default "agent-universe" (^[A-Za-z0-9_-]{1,32}$). */
  serverName?: string
  /** Per-tool-call timeout in ms. Default 60000. */
  toolCallTimeoutMs?: number
  /** Reject activation if initial connection / tool sync fails. Default false. */
  failOnStartupError?: boolean
  /** Extra HTTP headers; an explicit Authorization overrides the token. */
  headers?: Record<string, string>
}

/** Exact Streamable HTTP config consumed by @deepseek-ai/dsh-mcp-client (rc.1). */
export interface ResolvedMcpClientConfig {
  transport: 'streamable-http'
  serverName: string
  url: string
  headers: Record<string, string>
  toolCallTimeoutMs: number
  failOnStartupError: boolean
}

export declare const name: string
export declare const inject: string[]
export declare const Config: unknown
export declare function apply(ctx: Context, config: AgentUniverseHarnessConfig): Promise<void>
export declare function resolveClientConfig(
  raw?: AgentUniverseHarnessConfig,
  env?: NodeJS.ProcessEnv,
): ResolvedMcpClientConfig
export declare const DEFAULT_URL: string
export declare const DEFAULT_SERVER_NAME: string
