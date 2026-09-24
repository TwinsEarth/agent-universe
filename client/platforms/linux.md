# Linux 构建

## 前置要求

- Ubuntu 22.04+ / Fedora 38+ / Arch
- WebKit2GTK 4.1
- GTK 3
- Rust stable
- Node.js 20+

```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev
```

## 构建

```bash
cd client
npm install
npm run tauri build
```

## 产物

- `.deb`: `src-tauri/target/release/bundle/deb/`
- `.AppImage`: `src-tauri/target/release/bundle/appimage/`

## 注意事项

- CI 在 Ubuntu runner 上验证 Rust 编译，但 Tauri 打包需本地图形依赖
- .deb 依赖已在 tauri.conf.json 中声明
