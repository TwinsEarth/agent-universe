// 根包 /lib 子路径入口（v2.5.7 补）：re-export js/lib，
// 使根发布包暴露 /lib/market.js 子路径。market.js 为纯逻辑（仅依赖 models，不引 node:crypto），
// 浏览器 / WKWebView 端只引此模块即可运行市场演示，避开密钥模块。
module.exports = require('../js/lib/market.js');
