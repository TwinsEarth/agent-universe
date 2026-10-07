// lib/edverify.js
// Ed25519 同步验签 —— Node 实现。
//
// 该文件显式依赖 node:crypto / Buffer，只能在 Node 运行时加载；
// 浏览器 / WKWebView 构建时由 package.json 的 "browser" 字段替换为
// ./edverify.browser.js（fail-closed）。market.js 等浏览器可安全引用的
// 纯逻辑模块只允许 require('./edverify')，禁止直接 require('node:crypto')，
// 以免把 Node 内置模块静态外链进浏览器包（Vite externalized 警告）。
//
// 验签规则与 Rust 侧 SignedQaVote 校验逐字一致：
// 公钥为 32 字节原始 Ed25519 公钥（hex），按 SPKI DER 包装后验签。
//
// 注意：Ed25519 属于密码学原语，本实现仅做平台适配，算法安全性需外部审计。

const crypto = require('node:crypto');

// Ed25519 SubjectPublicKeyInfo 的固定 DER 前缀（hex），后接 32 字节公钥。
const ED25519_SPKI_PREFIX = '302a300506032b6570032100';

/**
 * 同步验证一条 Ed25519 签名。
 * @param {string} pubkeyHex 32 字节公钥（64 个 hex 字符）
 * @param {string} message 原始消息（UTF-8 字符串，验签前不做额外哈希）
 * @param {string} signatureHex 64 字节签名（128 个 hex 字符）
 * @returns {boolean} 签名是否有效
 */
function verifyEd25519(pubkeyHex, message, signatureHex) {
  const der = Buffer.from(ED25519_SPKI_PREFIX + pubkeyHex, 'hex');
  const key = crypto.createPublicKey({ key: der, format: 'der', type: 'spki' });
  return crypto.verify(
    null,
    Buffer.from(message, 'utf8'),
    key,
    Buffer.from(signatureHex, 'hex'),
  );
}

module.exports = { verifyEd25519 };
