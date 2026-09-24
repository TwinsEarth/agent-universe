# Agent Universe Desktop Client (Tauri 2)

v2.3.4 轻桌面客户端，内嵌 `@twinsearth/agent-universe@2.3.4` SDK，演示智能体市场端到端结算链路。

## 架构

```
desktop/
├── package.json        # Vite 前端依赖
├── vite.config.js      # Vite 配置 (port 1420)
├── index.html          # 入口
├── src/
│   ├── main.js         # 前端逻辑：市场端到端演示
│   └── style.css       # 暗色 UI
└── src-tauri/          # Rust 壳
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json # 920×720, com.twinsearth.auclient
    └── src/
        ├── main.rs
        └── lib.rs
```

## 开发

```bash
# 1. 安装前端依赖（走 GitHub Packages）
npm install

# 2. 启动 dev
npm run tauri dev
```

## 打包（需要 macOS）

```bash
npm run tauri build
# 产物: src-tauri/target/release/bundle/macos/Agent Universe Client.app
```

## 说明

- 前端通过 `@twinsearth/agent-universe/lib/market.js` 直接引入 `AgentMarket`，避免 WKWebView 无 `node:crypto` 导致的 Keypair 初始化失败。
- 演示使用固定 DID（市场结算链路本身不依赖 Ed25519 签名）。
- 正式分发需 Apple 开发者证书签名 + 公证。
