// lib/keychain.js
// Ed25519 密钥对 + DID 身份（零依赖，使用 node:crypto）

const crypto = require('node:crypto');

/**
 * 密钥对与去中心化身份
 */
class Keypair {
  constructor(publicKey, privateKey, did) {
    this.publicKey = publicKey;   // hex
    this.privateKey = privateKey; // hex（node KeyObject 可导出的形式）
    this.did = did;               // did:au:<hash>
  }

  /** 生成新的 Ed25519 密钥对与 DID */
  static generate() {
    const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
    const pubHex = publicKey.export({ format: 'der', type: 'spki' })
      .toString('hex');
    // DID 由公钥的 SHA-256 哈希（取前 32 字符）派生
    const didHash = crypto.createHash('sha256')
      .update(publicKey.export({ format: 'der', type: 'spki' }))
      .digest('hex')
      .slice(0, 32);
    const did = `did:au:${didHash}`;

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
