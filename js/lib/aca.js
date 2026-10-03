// lib/aca.js
// ACA（Agent Communication Architecture）：与 Rust gsn-core / Python SDK
// 对齐的身份、规范签名与协议对象构造。零依赖，使用 node:crypto。
//
// 跨语言一致性：canonical 载荷为移除 signature 键后，紧凑、按 key 字典序、
// 不转义非 ASCII 的 JSON 字节，与 Rust serde_json(BTreeMap)、Python
// sort_keys 产出逐字节一致，因此三端可互验签名。

const crypto = require('node:crypto');

// Ed25519 PKCS#8 DER 固定前缀（16 字节），后接 32 字节种子
const PKCS8_PREFIX = Buffer.from('302e020100300506032b657004220420', 'hex');

// ── 枚举标签（与 Rust serde 变体名一致） ──
const MODE_SELF_REPORTED = 'SelfReported';
const LEVEL_L0 = 'L0Sample';
const PRIVACY_PUBLIC = 'Public';
const PRIORITY_NORMAL = 'Normal';
const STATUS_COMPLETED = 'Completed';
const MSG_HANDSHAKE = 'Handshake';
const MSG_TASK_PROPOSAL = 'TaskProposal';
const MSG_RECEIPT = 'Receipt';

function nowSec() {
  return Math.floor(Date.now() / 1000);
}

/** 稳定 JSON 序列化：紧凑、对象键按字典序、非 ASCII 原样输出 */
function stableStringify(value) {
  // v2.7.1（GAP §9.1）：禁止静默产出非法 JSON 或错误载荷。
  // - BigInt：JSON.stringify 会抛晦涩的 "Do not know how to serialize"，这里显式抛类型错误；
  // - Date：此前 typeof object 且 own keys 为空，静默变成 {}，载荷错误却签名看似有效；
  // - 顶层 undefined：JSON.stringify 返回 undefined（非字符串），会拼出裸标记。
  if (typeof value === 'bigint') {
    throw new TypeError('stableStringify: BigInt 不可进入签名载荷（Rust 侧无对应整数类型）');
  }
  if (value instanceof Date) {
    throw new TypeError('stableStringify: Date 不可签名；请先传 epoch 毫秒数或 ISO 字符串');
  }
  if (value === null || typeof value !== 'object') {
    // 顶层 undefined 走到这里：JSON.stringify(undefined) 返回 undefined，
    // 若被拼进对象/数组会变成裸标记。显式拒绝。
    if (value === undefined) {
      throw new TypeError('stableStringify: 顶层 undefined 不可序列化；对象中的 undefined 键会被省略');
    }
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    // 数组空位/undefined 元素转 null（JSON 合法），保持位置。
    return '[' + value.map((v) => (v === undefined ? 'null' : stableStringify(v))).join(',') + ']';
  }
  // v2.6.4（GAP §4.4）：按 Unicode 码点序排序，而非 JS 默认的 UTF-16 码元序。
  // 否则含 astral（补充平面，如 emoji）键名时，JS 与 Rust BTreeMap 的键序分歧，
  // 三端 canonical 载荷不再逐字节一致。
  const keys = Object.keys(value).sort((a, b) => {
    const ca = [...a];
    const cb = [...b];
    const n = Math.min(ca.length, cb.length);
    for (let i = 0; i < n; i++) {
      const diff = ca[i].codePointAt(0) - cb[i].codePointAt(0);
      if (diff !== 0) return diff;
    }
    return ca.length - cb.length;
  });
  return (
    '{' +
    keys
      .filter((k) => value[k] !== undefined)
      .map((k) => JSON.stringify(k) + ':' + stableStringify(value[k]))
      .join(',') +
    '}'
  );
}

/** 规范待签名载荷：移除 signature 键，返回 Buffer */
function canonicalPayload(obj) {
  const copy = { ...obj };
  delete copy.signature;
  return Buffer.from(stableStringify(copy), 'utf8');
}

function rawPublicFromKey(pubKey) {
  return Buffer.from(pubKey.export({ format: 'jwk' }).x, 'base64url');
}

function pubKeyFromRaw(raw) {
  return crypto.createPublicKey({
    key: { kty: 'OKP', crv: 'Ed25519', x: raw.toString('base64url') },
    format: 'jwk',
  });
}

/**
 * 跨语言对齐的节点身份：Ed25519 + did:aip（原始 32 字节公钥指纹）
 */
class AipIdentity {
  constructor(privKey, rawPub) {
    this._priv = privKey;
    this._rawPub = rawPub;
    const hash = crypto.createHash('sha256').update(rawPub).digest('hex');
    this.did = 'did:aip:' + hash.slice(0, 16);
  }

  static generate() {
    const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
    return new AipIdentity(privateKey, rawPublicFromKey(publicKey));
  }

  static fromSeed(seed) {
    const buf = Buffer.isBuffer(seed) ? seed : Buffer.from(seed);
    if (buf.length !== 32) throw new Error('seed 必须为 32 字节');
    const priv = crypto.createPrivateKey({
      key: Buffer.concat([PKCS8_PREFIX, buf]),
      format: 'der',
      type: 'pkcs8',
    });
    const rawPub = rawPublicFromKey(crypto.createPublicKey(priv));
    return new AipIdentity(priv, rawPub);
  }

  /** 32 字节原始公钥 */
  get publicKey() {
    return this._rawPub;
  }

  signObject(obj) {
    return crypto.sign(null, canonicalPayload(obj), this._priv).toString('hex');
  }

  signInto(obj) {
    obj.signature = this.signObject(obj);
    return obj;
  }

  static verifyObject(obj, rawPub) {
    const sigHex = obj && obj.signature;
    if (!sigHex) return false;
    let sig;
    try {
      sig = Buffer.from(sigHex, 'hex');
    } catch (e) {
      return false;
    }
    try {
      return crypto.verify(
        null,
        canonicalPayload(obj),
        pubKeyFromRaw(rawPub),
        sig
      );
    } catch (e) {
      return false;
    }
  }
}

function buildManifest(identity, name, capabilities, opts = {}) {
  const manifest = {
    did: identity.did,
    name,
    version: opts.version || '3.7.3',
    capabilities: [...capabilities],
    endpoints: opts.endpoints || [],
    hardware: {
      cpu_cores: opts.cpuCores || 0,
      memory_mb: opts.memoryMb || 0,
      disk_free_gb: opts.diskFreeGb || 0,
      gpu: null,
      bandwidth_up_mbps: 0,
      bandwidth_down_mbps: 0,
      platform: opts.platform || 'desktop',
    },
    verification_modes: opts.verificationModes || [MODE_SELF_REPORTED],
    stake: opts.stake || 0,
    reputation_ref: null,
    model_hash: null,
    mcp_tool_count: opts.mcpToolCount || 0,
    signature: '',
    timestamp: nowSec(),
  };
  return identity.signInto(manifest);
}

function buildEnvelope(requesterDid, capability, outputSpec, opts = {}) {
  return {
    task_id: crypto.randomUUID(),
    requester_did: requesterDid,
    capability,
    input_cid: opts.inputCid || '',
    output_spec: outputSpec,
    verification_level: opts.verificationLevel || LEVEL_L0,
    privacy: opts.privacy || PRIVACY_PUBLIC,
    budget: opts.budget || 0,
    timeout_secs: opts.timeoutSecs || 300,
    priority: opts.priority || PRIORITY_NORMAL,
    mcp_tool: opts.mcpTool || null,
    created_at: nowSec(),
  };
}

function buildMessage(identity, msgType, toDid, payload) {
  const message = {
    message_id: crypto.randomUUID(),
    msg_type: msgType,
    from_did: identity.did,
    to_did: toDid,
    payload,
    timestamp: nowSec(),
    signature: '',
  };
  return identity.signInto(message);
}

function handshakeMessage(identity, manifest) {
  return buildMessage(identity, MSG_HANDSHAKE, '*', manifest);
}

function proposalMessage(identity, toDid, envelope) {
  return buildMessage(identity, MSG_TASK_PROPOSAL, toDid, envelope);
}

function buildReceipt(identity, taskId, result, opts = {}) {
  const hash = crypto.createHash('sha256').update(result).digest();
  const metering = Object.assign(
    {
      compute_ms: 0,
      memory_peak_mb: 0,
      bandwidth_mb: 0,
      storage_bytes: 0,
      energy_joules: 0,
    },
    opts.metering || {}
  );
  const receipt = {
    task_id: taskId,
    executor_did: identity.did,
    result_cid: 'cid:' + hash.toString('hex').slice(0, 16),
    result_hash: [...hash],
    status: opts.status || STATUS_COMPLETED,
    metering,
    tee_quote: opts.teeQuote || null,
    zk_proof: opts.zkProof || null,
    completed_at: nowSec(),
    signature: '',
  };
  return identity.signInto(receipt);
}

function receiptMessage(identity, toDid, receipt) {
  return buildMessage(identity, MSG_RECEIPT, toDid, receipt);
}

function verifyManifest(manifest, rawPub) {
  return AipIdentity.verifyObject(manifest, rawPub);
}
function verifyMessage(message, rawPub) {
  return AipIdentity.verifyObject(message, rawPub);
}
function verifyReceipt(receipt, rawPub) {
  return AipIdentity.verifyObject(receipt, rawPub);
}
function verifyReceiptResult(receipt, result) {
  const hash = crypto.createHash('sha256').update(result).digest();
  return deepEqualArray(receipt.result_hash, [...hash]);
}

function deepEqualArray(a, b) {
  if (!Array.isArray(a) || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

module.exports = {
  AipIdentity,
  canonicalPayload,
  stableStringify,
  buildManifest,
  buildEnvelope,
  buildMessage,
  buildReceipt,
  handshakeMessage,
  proposalMessage,
  receiptMessage,
  verifyManifest,
  verifyMessage,
  verifyReceipt,
  verifyReceiptResult,
};
