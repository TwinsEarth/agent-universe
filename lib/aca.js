// 根包 /lib 子路径入口（v2.5.7 补）：re-export js/lib，
// 使根发布包暴露 /lib/aca.js 子路径，供浏览器端按需引入（与 js 子包发布结构对齐）。
module.exports = require('../js/lib/aca.js');
