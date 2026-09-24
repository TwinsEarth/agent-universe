# iOS 安装包

iOS 版本需要 Apple Developer 证书和 Xcode 构建。
当前环境无 Apple Developer 证书，无法直接构建 .ipa。

源码位置：`client/src-tauri/gen/apple/`

构建步骤（在 macOS 上）：
1. 安装 Xcode
2. 配置 Apple Developer 证书
3. 运行 `npx tauri build --target universal-apple-darwin`
