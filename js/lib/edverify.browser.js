// lib/edverify.browser.js
// Ed25519 验签 —— 浏览器 / WKWebView 实现（fail-closed 占位）。
//
// market.js 的认证式 BFT QA 投票验签需要同步 Ed25519 验证能力。浏览器端只有
// 异步的 Web Crypto（crypto.subtle.importKey/verify 返回 Promise），无法满足
// 当前同步调用契约；Web Crypto 的 Ed25519 支持也并非所有浏览器齐备。
//
// 安全原则：缺能力时必须显式失败，绝不静默“验签通过”。因此浏览器端不做
// 任何签名验证、不返回 true，而是抛出明确错误，要求把该类认证式投票的
// 签名校验放到 Node / Rust 侧（daemon / MCP / gsn-core）执行。
//
// 该模块不引用任何 Node 内置模块，因此可被安全打包进浏览器产物；package.json
// 的 "browser" 字段会在浏览器构建时用本文件替换 ./edverify.js。
//
// 注意：密码学能力边界，浏览器端不提供验签属设计决策，如需在浏览器内做
// Ed25519 验证，应另行引入经审计的异步 Web Crypto / @noble/ed25519 适配，
// 并把调用链改为异步，需外部安全审计。

/**
 * @throws {Error} 始终抛出：浏览器端不具备同步 Ed25519 验签能力。
 */
function verifyEd25519() {
  throw new Error(
    '当前环境无同步 Ed25519 验签能力（浏览器/WKWebView）；' +
      '认证式投票的签名验证须在 Node/Rust 侧进行',
  );
}

module.exports = { verifyEd25519 };
