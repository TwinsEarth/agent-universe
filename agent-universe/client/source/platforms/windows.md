# Windows 构建

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
