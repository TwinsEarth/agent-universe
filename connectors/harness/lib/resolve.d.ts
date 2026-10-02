import type {
  AgentUniverseHarnessConfig,
  ResolvedMcpClientConfig,
} from './index.d.ts'

export declare const DEFAULT_URL: string
export declare const DEFAULT_SERVER_NAME: string
export declare const DEFAULT_TOOL_CALL_TIMEOUT_MS: number
export declare function resolveClientConfig(
  raw?: AgentUniverseHarnessConfig,
  env?: NodeJS.ProcessEnv,
): ResolvedMcpClientConfig
