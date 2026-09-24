// Type definitions for @twinsearth/agent-universe

export const version: string;
export const MIN_STAKE: number;

export class Keypair {
  publicKey: string;
  privateKey: string;
  did: string;
  static generate(): Keypair;
  sign(message: string | Buffer): string;
  verify(message: string | Buffer, signatureHex: string): boolean;
}

export class AgentCard {
  did: string;
  name: string;
  capabilities: string[];
  skills: string[];
  version: string;
  stake: number;
  reputation: number;
  static new(opts: { did: string; name: string }): AgentCard;
  withCapability(cap: string): this;
  withSkill(skill: string): this;
  hasCapability(cap: string): boolean;
}

export const TaskStatus: Record<string, string>;

export class Task {
  taskId: string;
  goal: string;
  status: string;
  assignee: string | null;
  history: string[];
  constructor(taskId: string, goal: string);
  assign(did: string): this;
  start(): this;
  complete(): this;
  verify(passed: boolean): this;
  settle(): this;
  readonly isTerminal: boolean;
}

export class MemoryDHT {
  put(key: string, value: unknown): void;
  get(key: string): unknown;
  has(key: string): boolean;
  remove(key: string): boolean;
  readonly size: number;
}

export class ShardedIndex {
  constructor(numShards?: number);
  put(key: string, value: unknown): number;
  get(key: string): unknown;
  has(key: string): boolean;
  remove(key: string): boolean;
  readonly size: number;
  distribution(): number[];
}

export function shardOf(key: string, numShards: number): number;

export class Bid {
  agentId: string;
  price: number;
}

export class Reputation {
  quality: number;
  speed: number;
  honesty: number;
  availability: number;
  overall(): number;
}

export class AgentMarket {
  agents: Map<string, unknown>;
  tasks: Map<string, Task>;
  deposit(account: string, amount: number): void;
  balance(account: string): number;
  registerAgent(card: AgentCard, stake: number): boolean;
  publishTask(taskId: string, goal: string, budget: number, requester: string): Task;
  submitBid(taskId: string, agentId: string, price: number): boolean;
  matchTask(taskId: string): Bid;
  completeTask(taskId: string, passed?: boolean): Task;
  settle(taskId: string): { paid: number; reason: string };
  slash(agentId: string, amount: number, reason: string): unknown;
  discoverBySkill(skill: string): AgentCard[];
  leaderboard(limit?: number): unknown[];
  conservationCheck(totalDeposits: number): {
    conserved: boolean;
    balanceSum: number;
    totalSlashed: number;
    expected: number;
  };
}

export class AgentUniverse {
  identity: Keypair;
  dht: MemoryDHT;
  market: AgentMarket;
  readonly did: string;
  register(card: AgentCard): AgentCard;
  discover(capability: string): AgentCard[];
}

// ── MCP ──
export const MCP_PROTOCOL_VERSION: string;

export class McpError extends Error {
  code: number;
}

export class McpHttpClient {
  initialized: boolean;
  constructor(baseUrl?: string, path?: string, timeout?: number);
  initialize(): Promise<{
    protocolVersion: string;
    serverInfo: { name: string; version: string };
  }>;
  ping(): Promise<boolean>;
  listTools(): Promise<
    Array<{
      name: string;
      description?: string;
      inputSchema: Record<string, unknown>;
    }>
  >;
  callTool(
    name: string,
    args?: Record<string, unknown>
  ): Promise<{ content: Array<{ type: string; text: string }>; isError?: boolean }>;
}

// ── ACA ──
export class AipIdentity {
  did: string;
  readonly publicKey: Buffer;
  static generate(): AipIdentity;
  static fromSeed(seed: Buffer | Uint8Array): AipIdentity;
  signObject(obj: Record<string, unknown>): string;
  signInto(obj: Record<string, unknown>): Record<string, unknown>;
  static verifyObject(obj: Record<string, unknown>, rawPub: Buffer): boolean;
}

export function buildManifest(
  identity: AipIdentity,
  name: string,
  capabilities: string[],
  opts?: Record<string, unknown>
): Record<string, unknown>;

export function buildEnvelope(
  requesterDid: string,
  capability: string,
  outputSpec: string,
  opts?: Record<string, unknown>
): Record<string, unknown>;

export function buildMessage(
  identity: AipIdentity,
  msgType: string,
  toDid: string,
  payload: unknown
): Record<string, unknown>;

export function buildReceipt(
  identity: AipIdentity,
  taskId: string,
  result: Buffer,
  opts?: Record<string, unknown>
): Record<string, unknown>;

export function handshakeMessage(
  identity: AipIdentity,
  manifest: Record<string, unknown>
): Record<string, unknown>;

export function proposalMessage(
  identity: AipIdentity,
  toDid: string,
  envelope: Record<string, unknown>
): Record<string, unknown>;

export function receiptMessage(
  identity: AipIdentity,
  toDid: string,
  receipt: Record<string, unknown>
): Record<string, unknown>;

export function verifyManifest(
  manifest: Record<string, unknown>,
  rawPub: Buffer
): boolean;
export function verifyMessage(
  message: Record<string, unknown>,
  rawPub: Buffer
): boolean;
export function verifyReceipt(
  receipt: Record<string, unknown>,
  rawPub: Buffer
): boolean;
export function verifyReceiptResult(
  receipt: Record<string, unknown>,
  result: Buffer
): boolean;
