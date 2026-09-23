# Contributing to Agent Universe

感谢你对 Agent Universe 的兴趣！我们欢迎任何形式的贡献。

## 快速开始

```bash
# 克隆仓库
git clone https://github.com/TwinsEarth/agent-universe.git
cd agent-universe

# Rust 核心库
cd gsn-core
cargo build
cargo test

# Python SDK
cd ../aip-sdk-py
pip install -e .
pytest
```

## 开发流程

1. Fork 本仓库
2. 创建特性分支：`git checkout -b feature/your-feature`
3. 确保所有测试通过：`cargo test && pytest`
4. 提交代码：`git commit -m "feat: 描述你的改动"`
5. 推送到分支：`git push origin feature/your-feature`
6. 创建 Pull Request

## 提交规范

我们使用 [Conventional Commits](https://www.conventionalcommits.org/)：

- `feat:` 新功能
- `fix:` 修复 bug
- `docs:` 文档更新
- `refactor:` 代码重构
- `test:` 测试相关
- `ci:` CI/CD 相关
- `chore:` 杂项

## 版本号

- 大版本：不兼容的 API 变更
- 小版本：向下兼容的功能新增
- 修订号：向下兼容的问题修复

## 报告问题

- Bug 报告：[Issues](https://github.com/TwinsEarth/agent-universe/issues)
- 安全问题：请先参考 [SECURITY.md](SECURITY.md)

## 行为准则

参与本项目即表示同意遵守 [Code of Conduct](CODE_OF_CONDUCT.md)。
