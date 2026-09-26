# Android 构建

> **成品下载**：APK 由 CI（`.github/workflows/client-build.yml`）在打版本 tag 时自动构建并发布到 [GitHub Release](https://github.com/TwinsEarth/agent-universe/releases/tag/v2.5.5)，可直接下载，无需本地构建。以下为本地手动构建步骤。

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

## 关于签名（重要）

Android 只允许安装**已签名** APK。

- **CI 发布到 Release 的 APK 为 debug 签名**（`tauri android build --apk --debug`，AGP 用调试密钥自动签名），可直接下载、侧载安装，`android:debuggable=true`。
- 不加 `--debug` 的 release 变体在未配置 keystore 时产出 `*-unsigned.apk`，**无法安装**。
- 上架 Google Play 需生成自有 release keystore 并配置 `signingConfigs`（将 keystore(base64) 与密码放入 GitHub Secrets，在 CI 中解码签名）。

```bash
keytool -genkey -v -keystore twinsearth.jks -keyalg RSA -keysize 2048 \
  -validity 10000 -alias twinsearth
```

## 构建 Release APK

```bash
npm run tauri android build          # 需已配置签名，否则产出 unsigned
npm run tauri android build --apk --debug   # 调试签名、可直接安装
```

## 产物

- debug：`src-tauri/gen/android/app/build/outputs/apk/universal/debug/`
- release：`src-tauri/gen/android/app/build/outputs/apk/universal/release/`

## 注意事项

- Android 后台限制：App 退到后台后 P2P 连接可能被系统杀死
- 建议使用 WorkManager 定期唤醒同步
- 上架 Google Play 需 release 签名 + 审核
