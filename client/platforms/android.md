# Android 构建

> **成品下载**：APK 由 CI（`.github/workflows/client-build.yml`）在打版本 tag 时自动构建并发布到 [GitHub Release](https://github.com/TwinsEarth/agent-universe/releases/tag/v2.3.6)，可直接下载，无需本地构建。以下为本地手动构建步骤。

## 前置要求

- JDK 17
- Android SDK + NDK
- Android Studio（推荐）
- Rust + Android target:
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
  ```
- Node.js 20+

## 初始化

```bash
cd client
npm install
npm run tauri android init
```

## 开发（连接设备或模拟器）

```bash
npm run tauri android dev
```

## 构建 Release APK

```bash
npm run tauri android build
```

## 产物

- `.apk`: `src-tauri/gen/android/app/build/outputs/apk/universal/release/`

## 注意事项

- Android 后台限制：App 退到后台后 P2P 连接可能被系统杀死
- 建议使用 WorkManager 定期唤醒同步
- 上架 Google Play 需签名 + 审核
