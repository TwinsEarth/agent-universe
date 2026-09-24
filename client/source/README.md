# Agent Universe 跨平台客户端（v2.3.4）

基于 **Tauri 2** 的统一跨平台客户端，一套源码覆盖五大平台：

| 平台 | 产物 | 构建环境 | 状态 |
|------|------|----------|------|
| 🖥️ **macOS** | `.app` / `.dmg` (Apple Silicon + Intel) | macOS + Xcode | ✅ 已验证 |
| 🪟 **Windows** | `.msi` / `.exe` | Windows + MSVC | 🔶 待构建 |
| 🐧 **Linux** | `.deb` / `.AppImage` | Linux + WebKit2GTK | 🔶 待构建 |
| 📱 **iOS** | `.ipa` | macOS + Xcode | 🔶 待构建 |
| 🤖 **Android** | `.apk` | Linux/macOS + Android SDK | 🔶 待构建 |

## 快速开始

```bash
cd client
npm install

# 桌面端开发
npm run tauri dev

# 桌面端打包（当前平台）
npm run tauri build

# Android
npm run tauri android init
npm run tauri android dev

# iOS（需 macOS）
npm run tauri ios init
npm run tauri ios dev
```

## 目录结构

```
client/
├── src/               # 前端（Vite + HTML/JS）
├── src-tauri/         # Rust 壳（Tauri 2）
├── platforms/         # 各平台构建说明
│   ├── macos.md
│   ├── windows.md
│   ├── linux.md
│   ├── ios.md
│   └── android.md
└── README.md          # 本文件
```

## 与 desktop/ 的区别

- **desktop/**：macOS 专用轻量演示客户端（920×720，市场端到端 Demo）
- **client/**：五平台统一客户端，含移动端支持，面向正式分发
