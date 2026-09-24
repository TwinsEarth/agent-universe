# Agent Universe Client

v2.3.6 跨平台客户端。

## 平台支持

| 平台 | 安装包 | 状态 |
|------|--------|------|
| macOS | `macos/Agent Universe_2.3.6_aarch64.dmg` | ✅ |
| Windows | `windows/Agent Universe_2.3.6_x64_en-US.msi` | ✅ |
| Android | `android/app-universal-release-unsigned.apk` | ✅ |
| Linux | `linux/gsn-daemon-linux-x64-v2.3.6.tar.gz` | ✅ |
| iOS | 见 `ios/README.md` | ⚠️ 需 Apple 证书 |

## 源码

- 前端：`src/`（Vite + HTML/JS）
- Rust 壳：`src-tauri/`（Tauri 2）
- 平台配置：`platforms/`

## 构建

```bash
# 前端
npm install
npm run build

# 桌面
npx tauri build

# Android
npx tauri android build
```
