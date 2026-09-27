// 根包 /lib 子路径入口（v2.5.7 补）：re-export js/lib，
// 使根发布包暴露 /lib/keychain.js 子路径（依赖 node:crypto，浏览器端勿直接引）。
module.exports = require('../js/lib/keychain.js');
