# iOS 构建

## 前置要求

- macOS（必须）
- Xcode 15+
- iOS 13+
- Rust + iOS target: `rustup target add aarch64-apple-ios x86_64-apple-ios`
- Node.js 20+

## 初始化

```bash
cd client
npm install
npm run tauri ios init
```

## 开发

```bash
npm run tauri ios dev
```

## 构建 Release

```bash
npm run tauri ios build
```

## 产物

- `.ipa`: `src-tauri/gen/apple/build/`

## 注意事项

- iOS 不支持后台 P2P 长连接，客户端以轻节点模式运行
- WKWebView 无 Node crypto，使用固定 DID（市场结算链路不依赖签名）
- 上架 App Store 需 Apple Developer 账号 + 审核
