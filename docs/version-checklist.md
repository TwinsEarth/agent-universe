# 版本号登记表（Version Checklist）

> **用途**：发版时**照此表逐项核对**，配合 `scripts/bump-version.sh` 一键改值。
> 以后新增任何带版本号的文件，**必须同步补登记到本表**，避免每次发版丢三落四。
>
> 当前版本：**npm 2.5.5** ｜ **Rust gsn-core 0.2.55**
> 对应规则：npm `X.Y.Z` ↔ Rust `0.2.YZ`（Y×10+Z）。例：2.5.6 → 0.2.56；2.6.0 → 0.2.60。

---

## 一、版本声明点（每次发版必须全部同步改值）

### A. npm / JavaScript 包

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| A1 | `package.json`（根） | `"version"` | 2.5.5 | npm 主包 `@twinsearth/agent-universe` |
| A2 | `js/package.json` | `"version"` | 2.5.5 | JS SDK |
| A3 | `js/package-lock.json` | 根 `"version"` + self `"version"`（2 处） | 2.5.5 | lock 自引用 |
| A4 | `js/index.js` | `const version = '...'` | 2.5.5 | 运行时导出版本，**ci.yml 校验它** |
| A5 | `js/lib/aca.js` | `version: opts.version \|\| '...'` | 2.5.5 | ACA envelope 默认版本 |
| A6 | `js/lib/mcp.js` | `clientInfo: {..., version: '...'}` | 2.5.5 | MCP 握手 clientInfo |
| A7 | `js/test/test.js` | `test('版本号为 ...')` + `assert.strictEqual(version, '...')` | 2.5.5 | 版本断言，改版本号必须同步改 |

### B. Tauri 客户端 `client/`

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| B1 | `client/package.json` | `"version"` | 2.5.5 | |
| B2 | `client/package-lock.json` | 根 + self（2 处） | 2.5.5 | |
| B3 | `client/src-tauri/Cargo.toml` | `version = "..."` | 2.5.5 | Tauri 壳 Cargo |
| B4 | `client/src-tauri/Cargo.toml` | `description = "...vX.Y.Z ..."` 里的版本 | v2.5.5 | 描述串 |
| B5 | `client/src-tauri/tauri.conf.json` | `"version"` | 2.5.5 | |
| B6 | `client/src-tauri/tauri.conf.json` | `"title": "Agent Universe vX.Y.Z"` | v2.5.5 | 窗口标题 |
| B7 | `client/src-tauri/src/lib.rs` | `"...".to_string()`（运行时版本） | 2.5.5 | |
| B8 | `client/README.md` | 开头"vX.Y.Z 跨平台客户端" + 下载文件名里的 `_X.Y.Z_` / `-vX.Y.Z.` | 2.5.5 | 当前版本描述与产物名 |
| B9 | `client/platforms/{android,linux,macos,windows}.md` | `releases/tag/vX.Y.Z` 链接 | v2.5.5 | 成品下载指向的 Release |

### C. Tauri 桌面壳 `desktop/`

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| C1 | `desktop/package.json` | `"version"` | 2.5.5 | |
| C2 | `desktop/package-lock.json` | 根 + self（2 处） | 2.5.5 | |
| C3 | `desktop/src-tauri/Cargo.toml` | `version = "..."` | 2.5.5 | |
| C4 | `desktop/src-tauri/Cargo.toml` | `description` 里的版本 | v2.5.5 | |
| C5 | `desktop/src-tauri/tauri.conf.json` | `"version"` | 2.5.5 | |
| C6 | `desktop/src-tauri/tauri.conf.json` | `"title": "Agent Universe vX.Y.Z"` | v2.5.5 | |
| C7 | `desktop/src-tauri/src/lib.rs` | `"...".to_string()` | 2.5.5 | |
| C8 | `desktop/src/main.js` | 头注释 `// Agent Universe vX.Y.Z ...` | v2.5.5 | |

### D. Rust 核心 `gsn-core/`（语义化独立线 0.2.XX）

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| D1 | `gsn-core/Cargo.toml` | `version = "0.2.XX"` | 0.2.55 | **Rust 语义化，不写 2.5.5** |
| D2 | `gsn-core/Cargo.toml` | `description = "...vX.Y.Z: ..."` 里的 npm 版本 | v2.5.5 | 描述串跟 npm |
| D3 | `gsn-core/src/bin/gsn.rs` | `println!("agent-universe vX.Y.Z")` | v2.5.5 | `gsn --version` 输出 |
| D4 | `gsn-core/src/lib.rs` | 头注释 `//! ... vX.Y.Z` | v2.5.5 | 库文档头 |
| D5 | `gsn-core/Cargo.lock` | `[[package]] name="gsn-core"` 下的 `version` | 0.2.55 | **lock 同步**（bump 脚本按包名块改，不误伤依赖） |
| D6 | `client/src-tauri/Cargo.lock` | `[[package]] name="au-client-universal"` 下的 `version` | 2.5.5 | client 壳 lock |
| D7 | `desktop/src-tauri/Cargo.lock` | `[[package]] name="au-client"` 下的 `version` | — | desktop 壳 lock（首次 build 才生成，不存在则跳过） |

### E. Python SDK

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| E1 | `aip-sdk-py/pyproject.toml` | `version = "..."` | 2.5.5 | |

### F. CI / 工作流

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| F1 | `.github/workflows/ci.yml` | `au.version!=='X.Y.Z'` 校验 | 2.5.5 | CI 强制版本一致 |
| F2 | `.github/workflows/client-build.yml` | 注释里的示例 tag `（如 vX.Y.Z）` | v2.5.5 | 注释，不影响构建 |
| F3 | `.github/workflows/publish.yml` | 不写死版本，用 `GITHUB_REF_NAME`；含 npm-publish job | — | 打 tag 自动发 Release+npm；需配 `NPM_TOKEN` secret |

---

## 二、谱系/索引点（每次发版**新增一行/一条**，不改旧值）

| # | 文件 | 动作 |
|---|---|---|
| G1 | `README.md` | 版本谱系表加一行；「已发布版本」列表补号；文档节视情况加新架构文档链接 |
| G2 | `RELEASES.md` | 谱系树加新节点（`← 当前` 移到新版）；大版本详情加新章节；能力清单加 ✅ |
| G3 | `CHANGELOG.md` | **顶部**插入新版本条目（## [vX.Y.Z] - 日期） |
| G4 | `releases/vX.Y.Z.md` | **新建** release note（照 `releases/v2.5.5.md` 模板） |
| G5 | `docs/architecture-vX.Y.Z.md` | 大版本/有架构变化时**新建**架构文档（带分层图） |

---

## 二·补、发布物校验点（打 tag 后逐项核对，防"代码改了但没发布"）

| # | 校验项 | 怎么验 | 通过标准 |
|---|---|---|---|
| H1 | git tag | `git ls-remote --tags origin vX.Y.Z` | 远程存在该 tag |
| H2 | GitHub Release | `GET /releases/tags/vX.Y.Z` | 非 draft，assets 含二进制/wheel |
| H3 | npm registry | `npm view @twinsearth/agent-universe version` | 返回 X.Y.Z（不是旧版） |
| H4 | CI 状态 | Actions 页 ci.yml / publish.yml | 全部 conclusion=success |
| H5 | Release badge | README 的 `shields.io/github/v/release` | 自动显示 vX.Y.Z |
| H6 | Rust lock | `grep -A1 'name="gsn-core"' gsn-core/Cargo.lock` | version=0.2.YZ |

> v2.5.5 起启用；v2.4.0~v2.5.4 历史版本不补 tag/Release/npm，仅以 `releases/` 文档归档。

---

## 三、不要改（历史记录，改了反而失真）

- `CHANGELOG.md` 里旧版本条目（如 `## [v2.3.6]`）；
- `RELEASES.md` 谱系树里的旧节点、旧版本详情、旧能力清单；
- `README.md` 谱系表里的旧版本行、旧版本章节（如 `## v2.3.6 ...`）、指向旧 tag 的历史 release 链接；
- `releases/vX.Y.Z.md` 历史文件整份；
- 源码里**特性引入注释**：`//! v2.3.4:`、`// v2.5.4: ...`、`//! v2.3.6: ...` 等——它们记录"该特性由哪个版本引入"，是历史事实，不是当前版本号。

---

## 四、发版 SOP（标准流程）

```bash
# 0. 前置：GitHub repo Settings -> Secrets 配好 NPM_TOKEN（npmjs granular token）。
#    配一次即可；不配则 publish.yml 的 npm-publish job 会 warning 跳过，不阻塞 Release。

# 1. 一键改所有"版本声明点"（输入 npm 版本号，自动算 Rust 线）
bash scripts/bump-version.sh 2.5.6

# 2. 跑测试验证版本断言与市场闭环
node js/test/test.js
(cd gsn-core && cargo test)

# 3. 手动补谱系点（第二节 G1–G5）：README / RELEASES / CHANGELOG / releases/新文件

# 4. 全仓复扫，确认无旧版本号漏网（下面这条应只命中第三节"不要改"的历史注释）
grep -rn "2\.5\.5" --include='*.json' --include='*.toml' --include='*.rs' --include='*.js' --include='*.yml' . \
  --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git --exclude-dir=docs

# 5. 提交推送
git add -A
git -c user.name="TwinsEarth" -c user.email="dev@twinsearth.local" \
  commit -m "chore(vX.Y.Z): 全仓版本号统一到 vX.Y.Z"
git push origin main

# 6. 打 tag 并推送 —— 触发整条发布流水线：
#    publish.yml: 自动 gh release create + 构建 gsn-daemon(linux/mac) + Python wheel
#                 + npm publish @twinsearth/agent-universe@X.Y.Z
#    release.yml: 三平台 gsn-daemon 二进制上传 Release
git -c user.name="TwinsEarth" -c user.email="dev@twinsearth.local" \
  tag -a vX.Y.Z -m "Agent Universe vX.Y.Z"
git push origin vX.Y.Z

# 7. 验证：GitHub Releases 页出现 vX.Y.Z、npm view @twinsearth/agent-universe version 是新版
```

> **版本线决策（2026-09-26 确认）**：v2.4.0 ~ v2.5.4 不补历史 git tag（代码已演进、补打会触发大量 CI 且产物与版本号不符）；它们的 release note 已在 `releases/` 归档、谱系已在 README/RELEASES 列出。**从 v2.5.5 起启用新流水线：打 tag → 自动建 GitHub Release → 自动构建产物 → 自动发 npm。**

> **新增版本点时**：在新文件/新模块里写了版本号常量、窗口标题、版本打印后，**立即回到本登记表追加一行**，并在 `scripts/bump-version.sh` 里加对应替换规则——这样下一个版本就不会漏。
