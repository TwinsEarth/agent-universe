# macOS 构建

## 前置要求

- macOS 11+（推荐 Apple Silicon / M 系列）
- Xcode 15+
- Rust stable
- Node.js 20+

## 构建

```bash
cd client
npm install
npm run tauri build
```

## 产物

- `.app`: `src-tauri/target/release/bundle/macos/Agent Universe.app`
- `.dmg`: `src-tauri/target/release/bundle/dmg/`

## 代码签名（分发必需）

```bash
# 未签名时，首次打开需右键 → 打开
# 正式分发需 Apple Developer 证书
codesign --deep --force --sign "Developer ID Application: ..." Agent\ Universe.app
```

## 公证

```bash
xcrun notarytool submit Agent\ Universe.dmg --keychain-profile "AC_PASSWORD" --wait
```
