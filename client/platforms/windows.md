# Windows 构建

> **成品下载**：`.msi` / NSIS `.exe` 由 CI（`.github/workflows/client-build.yml`）在打版本 tag 时自动构建并发布到 [GitHub Release](https://github.com/TwinsEarth/agent-universe/releases/tag/v2.5.5)，可直接下载。以下为本地手动构建步骤。

## 前置要求

- Windows 10/11
- Visual Studio 2022（含 C++ 桌面开发）
- WebView2 Runtime
- Rust stable
- Node.js 20+
- WiX Toolset（生成 .msi）

## 构建

```powershell
cd client
npm install
npm run tauri build
```

## 产物

- `.msi`: `src-tauri/target/release/bundle/msi/`
- `.exe` (NSIS): `src-tauri/target/release/bundle/nsis/`

## 注意事项

- WebView2 默认嵌入 bootstrapper（见 tauri.conf.json）
- 首次分发建议用 NSIS installer（自动安装 WebView2）
- 代码签名需 Authenticode 证书
