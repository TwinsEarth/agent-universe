# Android 构建

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
