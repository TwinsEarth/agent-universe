// lib/keychain.js
// Ed25519 密钥对 + DID 身份（零依赖，使用 node:crypto）
//
// v2.5.7：身份派生口径与 Rust / Python 统一 ——
//   fingerprint = hex(SHA-256(原始 32 字节公钥)[..8])
// 本项目新铸造身份使用 did:nau: 前缀；parseDid 仍接受上游 did:aip:。

const crypto = require('node:crypto');

// Ed25519 SubjectPublicKeyInfo DER 固定前缀（其后紧跟 32 字节原始公钥）
const SPKI_PREFIX = Buffer.from('302a300506032b6570032100', 'hex');

/**
 * 密钥对与去中心化身份
 */
class Keypair {
  constructor(publicKey, privateKey, did) {
    this.publicKey = publicKey;   // SPKI DER hex
    this.privateKey = privateKey; // PKCS8 DER hex
    this.did = did;               // did:nau:<fingerprint>
  }

  /** 从原始 32 字节公钥计算 fingerprint（与 Rust/Python 一致） */
  static fingerprint(rawPubkey) {
    return crypto.createHash('sha256')
      .update(rawPubkey)
      .digest('hex')
      .slice(0, 16); // 前 8 字节 = 16 hex 字符
  }

  /** 解析 DID，接受上游 did:aip: 与本项目 did:nau:；返回 {method, identifier} */
  static parseDid(s) {
    const m = /^did:(aip|nau):([0-9a-f]+)$/.exec(s);
    if (!m) {
      throw new Error(`不支持的 DID（格式应为 did:<aip|nau>:<id>）: ${s}`);
    }
    return { method: m[1], identifier: m[2] };
  }

  /** 生成新的 Ed25519 密钥对与 DID（新身份使用 did:nau:） */
  static generate() {
    const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
    const spkiDer = publicKey.export({ format: 'der', type: 'spki' });
    const pubHex = spkiDer.toString('hex');

    // 去掉 SPKI 固定前缀，得到 32 字节原始公钥
    const rawPub = spkiDer.subarray(SPKI_PREFIX.length);
    const did = `did:nau:${Keypair.fingerprint(rawPub)}`;

    // 私钥以 PKCS8 DER hex 保存，便于重建 KeyObject
    const privHex = privateKey.export({ format: 'der', type: 'pkcs8' })
      .toString('hex');

    const kp = new Keypair(pubHex, privHex, did);
    kp._pubKeyObj = publicKey;
    kp._privKeyObj = privateKey;
    return kp;
  }

  /** 获取可用于签名的私钥 KeyObject */
  _privateKeyObject() {
    if (this._privKeyObj) return this._privKeyObj;
    this._privKeyObj = crypto.createPrivateKey({
      key: Buffer.from(this.privateKey, 'hex'),
      format: 'der',
      type: 'pkcs8',
    });
    return this._privKeyObj;
  }

  /** 获取公钥 KeyObject */
  _publicKeyObject() {
    if (this._pubKeyObj) return this._pubKeyObj;
    this._pubKeyObj = crypto.createPublicKey({
      key: Buffer.from(this.publicKey, 'hex'),
      format: 'der',
      type: 'spki',
    });
    return this._pubKeyObj;
  }

  /** 返回 32 字节原始公钥 */
  rawPublicKey() {
    const spkiDer = this._publicKeyObject().export({ format: 'der', type: 'spki' });
    return spkiDer.subarray(SPKI_PREFIX.length);
  }

  /** 对消息签名，返回 hex */
  sign(message) {
    const data = Buffer.isBuffer(message) ? message : Buffer.from(message);
    return crypto.sign(null, data, this._privateKeyObject()).toString('hex');
  }

  /** 验证签名（默认用本密钥对的公钥） */
  verify(message, signatureHex) {
    const data = Buffer.isBuffer(message) ? message : Buffer.from(message);
    const sig = Buffer.from(signatureHex, 'hex');
    return crypto.verify(null, data, this._publicKeyObject(), sig);
  }
}

module.exports = { Keypair };
