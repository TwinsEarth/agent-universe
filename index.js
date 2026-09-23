// Agent Universe - 智能体宇宙
// 群众化的 AGI 路线，去中心化的智能体共享 & 开源网络
//
// 本包是 Rust 核心库 gsn-core 的 npm 元数据入口。
// 实际运行需要通过 cargo 编译 gsn-daemon，或使用 Python SDK (aip-sdk-py)。

const pkg = require('./package.json');

module.exports = {
  name: pkg.name,
  version: pkg.version,
  mission: '让科技造福全人类！',
  description: '群众化的 AGI 路线，去中心化的智能体共享 & 开源网络',
  repository: 'https://github.com/TwinsEarth/agent-universe',
  // Rust 核心：cd gsn-core && cargo build --release
  // Python SDK：cd aip-sdk-py && pip install -e .
};
