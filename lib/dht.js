// 根包 /lib 子路径入口（v2.5.7 补）：re-export js/lib，
// 使根发布包暴露 /lib/dht.js 子路径（与 js 子包发布结构对齐）。
module.exports = require('../js/lib/dht.js');
