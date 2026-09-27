#!/usr/bin/env node
// conformance/generate.mjs
//
// 用 Node/OpenSSL 的 Ed25519 独立生成跨实现签名向量，
// 与 Rust 实现（gsn-core）不共享任何代码 —— 这是真正的跨实现校验，
// 而不是上游那种"自己验自己"的自洽检查。
//
// 运行：node conformance/generate.mjs
// 产出：conformance/vectors.json
//
// 首条向量取自上游 gsn-core/tests/cross_lang_signature.rs（固定 seed=0x01×32）。

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Ed25519 PKCS#8 DER 固定前缀（其后紧跟 32 字节 seed）
const PKCS8_PREFIX = Buffer.from('302e020100300506032b657004220420', 'hex');
// Ed25519 SubjectPublicKeyInfo DER 固定前缀（其后紧跟 32 字节原始公钥）
const SPKI_PREFIX = Buffer.from('302a300506032b6570032100', 'hex');

function keypairFromSeed(seed) {
  const privDer = Buffer.concat([PKCS8_PREFIX, seed]);
  const privateKey = crypto.createPrivateKey({ key: privDer, format: 'der', type: 'pkcs8' });
  const publicKey = crypto.createPublicKey(privateKey);
  return { privateKey, publicKey };
}

function rawPublic(publicKey) {
  const der = publicKey.export({ format: 'der', type: 'spki' });
  return der.subarray(SPKI_PREFIX.length); // 32 字节原始公钥
}

function fingerprint(rawPub) {
  return crypto.createHash('sha256').update(rawPub).digest('hex').slice(0, 16); // 前 8 字节
}

function generateUpstreamVector() {
  const seed = Buffer.alloc(32, 1); // 0x01 × 32
  const { privateKey, publicKey } = keypairFromSeed(seed);
  const rawPub = rawPublic(publicKey);
  const fp = fingerprint(rawPub);

  const upstreamDid = `did:aip:${fp}`;
  const newDid = `did:nau:${fp}`;

  // canonical 载荷：键序固定为 capabilities, did, name, stake
  const canonical = JSON.stringify({
    capabilities: ['text-generation', 'mcp'],
    did: upstreamDid,
    name: 'CrossLang',
    stake: 100,
  });

  const signature = crypto.sign(null, Buffer.from(canonical), privateKey).toString('hex');

  // 自检：用公钥验证签名
  if (!crypto.verify(null, Buffer.from(canonical), publicKey, Buffer.from(signature, 'hex'))) {
    throw new Error('自校验失败：签名无法被对应公钥验证');
  }
  // 自检：篡改后必须验证失败
  const tampered = canonical.replace('CrossLang', 'Tampered');
  if (crypto.verify(null, Buffer.from(tampered), publicKey, Buffer.from(signature, 'hex'))) {
    throw new Error('自校验失败：篡改载荷竟验证通过');
  }

  return {
    name: 'cross-lang upstream identity (gsn-core cross_lang_signature)',
    source: 'gsn-core/tests/cross_lang_signature.rs:11-14',
    seed: '0x' + seed.toString('hex'),
    public_key: rawPub.toString('hex'),
    upstream_did: upstreamDid,
    new_did: newDid,
    canonical_payload: canonical,
    signature,
    generated_by: 'conformance/generate.mjs (Node/OpenSSL Ed25519, independent of Rust)',
  };
}

function main() {
  const vectors = [generateUpstreamVector()];
  const out = path.join(__dirname, 'vectors.json');
  fs.writeFileSync(out, JSON.stringify(vectors, null, 2) + '\n', 'utf8');
  const v = vectors[0];
  console.log('wrote', out);
  console.log('public_key :', v.public_key);
  console.log('upstream   :', v.upstream_did);
  console.log('new        :', v.new_did);
  console.log('signature  :', v.signature);
}

main();
