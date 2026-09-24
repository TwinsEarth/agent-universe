// index.js
// Agent Universe（智能体宇宙）主入口
// 群众化的 AGI 路线，去中心化的智能体共享 & 开源网络

const { Keypair } = require('./lib/keychain');
const { AgentCard, Task, TaskStatus } = require('./lib/models');
const { MemoryDHT, ShardedIndex, shardOf } = require('./lib/dht');
const { AgentMarket, Bid, Reputation, MIN_STAKE } = require('./lib/market');
const { McpHttpClient, McpError, MCP_PROTOCOL_VERSION } = require('./lib/mcp');
const aca = require('./lib/aca');

const version = '2.3.6';

/**
 * AgentUniverse 门面：聚合身份、网络、市场能力
 */
class AgentUniverse {
  constructor() {
    this.identity = Keypair.generate();
    this.dht = new MemoryDHT();
    this.market = new AgentMarket();
  }

  get did() {
    return this.identity.did;
  }

  /** 注册一张 AgentCard 到本地网络 */
  register(card) {
    this.dht.put(card.did, card);
    return card;
  }

  /** 发现某能力的 Agent */
  discover(capability) {
    return this.market.discoverBySkill(capability);
  }
}

module.exports = {
  version,
  AgentUniverse,
  Keypair,
  AgentCard,
  Task,
  TaskStatus,
  MemoryDHT,
  ShardedIndex,
  shardOf,
  AgentMarket,
  Bid,
  Reputation,
  MIN_STAKE,
  // MCP
  McpHttpClient,
  McpError,
  MCP_PROTOCOL_VERSION,
  // ACA（跨语言对齐身份与协议对象）
  AipIdentity: aca.AipIdentity,
  buildManifest: aca.buildManifest,
  buildEnvelope: aca.buildEnvelope,
  buildMessage: aca.buildMessage,
  buildReceipt: aca.buildReceipt,
  handshakeMessage: aca.handshakeMessage,
  proposalMessage: aca.proposalMessage,
  receiptMessage: aca.receiptMessage,
  verifyManifest: aca.verifyManifest,
  verifyMessage: aca.verifyMessage,
  verifyReceipt: aca.verifyReceipt,
  verifyReceiptResult: aca.verifyReceiptResult,
};
