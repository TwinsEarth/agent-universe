# 版本号登记表（Version Checklist）

> **用途**：发版时**照此表逐项核对**，配合 `scripts/bump-version.sh` 一键改值。
> 以后新增任何带版本号的文件，**必须同步补登记到本表**，避免每次发版丢三落四。
>
> 当前版本：**npm 2.5.7** ｜ **Rust gsn-core 0.2.57**
> 对应规则：npm `X.Y.Z` ↔ Rust `0.2.YZ`（Y×10+Z）。例：2.5.6 → 0.2.56；2.6.0 → 0.2.60。

---

## 一、版本声明点（每次发版必须全部同步改值）

### A. npm / JavaScript 包

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| A1 | `package.json`（根） | `"version"` | 2.5.7 | npm 主包 `@twinsearth/agent-universe` |
| A2 | `js/package.json` | `"version"` | 2.5.7 | JS SDK |
| A3 | `js/package-lock.json` | 根 `"version"` + self `"version"`（2 处） | 2.5.7 | lock 自引用 |
| A4 | `js/index.js` | `const version = '...'` | 2.5.7 | 运行时导出版本，**ci.yml 校验它** |
| A5 | `js/lib/aca.js` | `version: opts.version \|\| '...'` | 2.5.7 | ACA envelope 默认版本 |
| A6 | `js/lib/mcp.js` | `clientInfo: {..., version: '...'}` | 2.5.7 | MCP 握手 clientInfo |
| A7 | `js/test/test.js` | `test('版本号为 ...')` + `assert.strictEqual(version, '...')` | 2.5.7 | 版本断言，改版本号必须同步改 |
| A8 | `lib/{aca,dht,keychain,market,mcp,models}.js`（根，6 个） | `module.exports = require('../js/lib/X.js')` | — | **v2.5.7 新增**：让根发布包暴露 `/lib/*` 子路径（根包此前只含 `js/`，客户端 import `/lib/market.js` 落空，GAP §9.5）；不含版本号，bump 无需改 |

### B. Tauri 客户端 `client/`

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| B1 | `client/package.json` | `"version"` | 2.5.7 | |
| B2 | `client/package-lock.json` | 根 + self（2 处） | 2.5.7 | |
| B3 | `client/src-tauri/Cargo.toml` | `version = "..."` | 2.5.7 | Tauri 壳 Cargo |
| B4 | `client/src-tauri/Cargo.toml` | `description = "...vX.Y.Z ..."` 里的版本 | v2.5.7 | 描述串 |
| B5 | `client/src-tauri/tauri.conf.json` | `"version"` | 2.5.7 | |
| B6 | `client/src-tauri/tauri.conf.json` | `"title": "Agent Universe vX.Y.Z"` | v2.5.7 | 窗口标题 |
| B7 | `client/src-tauri/src/lib.rs` | `"...".to_string()`（运行时版本） | 2.5.7 | |
| B8 | `client/README.md` | 开头"vX.Y.Z 跨平台客户端" + 下载文件名里的 `_X.Y.Z_` / `-vX.Y.Z.` | 2.5.7 | 当前版本描述与产物名 |
| B9 | `client/platforms/{android,linux,macos,windows}.md` | `releases/tag/vX.Y.Z` 链接 | v2.5.7 | 成品下载指向的 Release |
| B10 | `client/platforms/ios.md` | **不含任何版本号 / Release 链接** | — | 仅讲 iOS 构建步骤；bump 时**不期待**它产生 diff（v2.5.7 核实，勿误判为遗漏） |
| B11 | `client/index.html` | `<title>Agent Universe vX.Y.Z` + 版本徽标 `vX.Y.Z · Universal` | v2.5.7 | **v2.5.7 补登记**（GAP §9.3）；bump 脚本 `s/vX.Y.Z/vNEW/g` |
| B12 | `client/package.json` | dependencies `"@twinsearth/agent-universe": "file:.."` | file:.. | **v2.5.7 改**（原 registry `^2.3.4`）：构建直接打包仓库根源码，根治"tag 触发构建时本版本 npm 包尚未发布"的时序竞争；不含版本号 |
| B13 | `client/.npmrc` | `install-links=true` | — | **v2.5.7 新增**：让 `file:..` 按 files 白名单打包成 node_modules 内真实拷贝（默认 false 建 symlink，Vite 解析到 node_modules 外源码、CJS 不被转换致 build 失败） |
| B14 | `client/src/main.js` | `import AU from '.../lib/market.js'; const { AgentMarket, MIN_STAKE } = AU;` | — | **v2.5.7 改**（原命名导入）：default 导入整个 module.exports 再解构，规避 rollup 对 re-export CJS 命名导出的静态识别失败；Vite build 已验证通过 |

### C. Tauri 桌面壳 `desktop/`

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| C1 | `desktop/package.json` | `"version"` | 2.5.7 | |
| C2 | `desktop/package-lock.json` | 根 + self（2 处） | 2.5.7 | |
| C3 | `desktop/src-tauri/Cargo.toml` | `version = "..."` | 2.5.7 | |
| C4 | `desktop/src-tauri/Cargo.toml` | `description` 里的版本 | v2.5.7 | |
| C5 | `desktop/src-tauri/tauri.conf.json` | `"version"` | 2.5.7 | |
| C6 | `desktop/src-tauri/tauri.conf.json` | `"title": "Agent Universe vX.Y.Z"` | v2.5.7 | |
| C7 | `desktop/src-tauri/src/lib.rs` | `"...".to_string()` | 2.5.7 | |
| C8 | `desktop/src/main.js` | 头注释 `// Agent Universe vX.Y.Z ...` | v2.5.7 | |
| C9 | `desktop/src-tauri/Cargo.lock` | **不存在 / 被 .gitignore 忽略** | — | desktop 壳 lock 不入库；勿误判为缺口 |
| C10 | `desktop/index.html` | `<title>Agent Universe vX.Y.Z` + `#sdk-version` 徽标 `vX.Y.Z` | v2.5.7 | **v2.5.7 补登记**（GAP §9.3） |
| C11 | `desktop/README.md` | 头部 `vX.Y.Z 轻桌面客户端` + 内嵌 `agent-universe@X.Y.Z` | v2.5.7 | **v2.5.7 补登记**（GAP §9.3） |
| C12 | `desktop/package.json` | dependencies `"@twinsearth/agent-universe": "file:.."` | file:.. | **v2.5.7 改**（原 `^2.3.4`），理由同 B12 |
| C13 | `desktop/.npmrc` | `install-links=true` | — | **v2.5.7 新增**，理由同 B13 |
| C14 | `desktop/src/main.js` | `import AU from '.../lib/market.js'; const { AgentMarket, MIN_STAKE } = AU;` | — | **v2.5.7 改**（原命名导入），理由同 B14；Vite build 已验证通过 |

### D. Rust 核心 `gsn-core/`（语义化独立线 0.2.XX）

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| D1 | `gsn-core/Cargo.toml` | `version = "0.2.XX"` | 0.2.57 | **Rust 语义化，不写 2.5.7** |
| D2 | `gsn-core/Cargo.toml` | `description = "...vX.Y.Z: ..."` 里的 npm 版本 | v2.5.7 | 描述串跟 npm |
| D3 | `gsn-core/src/bin/gsn.rs` | `println!("agent-universe vX.Y.Z")` | v2.5.7 | `gsn --version` 输出 |
| D4 | `gsn-core/src/lib.rs` | 头注释 `//! ... vX.Y.Z` | v2.5.7 | 库文档头 |
| D5 | `gsn-core/Cargo.lock` | `[[package]] name="gsn-core"` 下的 `version` | 0.2.57 | **lock 同步**（bump 脚本按包名块改，不误伤依赖） |
| D6 | `client/src-tauri/Cargo.lock` | `[[package]] name="au-client-universal"` 下的 `version` | 2.5.7 | client 壳 lock |
| D7 | `desktop/src-tauri/Cargo.lock` | `[[package]] name="au-client"` 下的 `version` | — | desktop 壳 lock（不入库 / 不存在则跳过） |

### E. Python SDK

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| E1 | `aip-sdk-py/pyproject.toml` | `version = "..."` | 2.5.7 | |
| E2 | `aip-sdk-py/aip/__init__.py` | `__version__ = "..."` | 2.5.7 | **v2.5.7 补登记**（GAP §9.3），长期漂移 |
| E3 | `aip-sdk-py/aip/mcp_client.py` | `_SDK_VERSION = "..."` | 2.5.7 | **v2.5.7 补登记**（GAP §9.3），MCP 握手 clientInfo |
| E4 | `aip-sdk-py/aip/aca.py` | `version: str = "..."` 默认值 | 2.5.7 | **v2.5.7 补登记**（GAP §9.3） |

### F. CI / 工作流

| # | 文件 | 字段 / 位置 | 当前值 | 说明 |
|---|---|---|---|---|
| F1 | `.github/workflows/ci.yml` | `au.version!=='X.Y.Z'` 校验 | 2.5.7 | CI 强制版本一致 |
| F2 | `.github/workflows/client-build.yml` | 注释里的示例 tag `（如 vX.Y.Z）` | v2.5.7 | 注释，不影响构建 |
| F3 | `.github/workflows/publish.yml` | 不写死版本，用 `GITHUB_REF_NAME`；含 npm-publish job | — | 打 tag 自动发 Release+npm；**必须配 `NPM_TOKEN` secret，缺失则 npm-publish job 红灯失败（v2.5.7 起不再静默跳过）** |

---

## 二、谱系/索引点（每次发版**新增一行/一条**，不改旧值）

| # | 文件 | 动作 |
|---|---|---|
| G1 | `README.md` | 版本谱系表加一行；「已发布版本」列表补号；文档节视情况加新架构文档链接 |
| G2 | `RELEASES.md` | 谱系树加新节点（`← 当前` 移到新版）；大版本详情加新章节；能力清单加 ✅ |
| G3 | `CHANGELOG.md` | **顶部**插入新版本条目（## [vX.Y.Z] - 日期） |
| G4 | `releases/vX.Y.Z.md` | **新建** release note（照 `releases/v2.5.6.md` 模板） |
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
- 本文件第四节 SOP 里 grep 示例所用的"上一版本号"，以及版本线决策里"从 v2.5.5 起"等历史表述；
- 源码里**特性引入注释**：`//! v2.3.4:`、`// v2.5.4: ...`、`//! v2.3.6: ...` 等——它们记录"该特性由哪个版本引入"，是历史事实，不是当前版本号。

---

## 四、发版 SOP（标准流程）

```bash
# 0. 前置：GitHub repo Settings -> Secrets and variables -> Actions 配好 NPM_TOKEN
#    （npmjs.com 的 Automation / Granular token，仅需 publish 权限）。配一次即可。
#    v2.5.6 起：缺失 NPM_TOKEN 时 npm-publish job 直接 ::error:: + exit 1（红灯暴露，不再静默跳过）；
#    Release 产物由 build/release job 独立完成，不受 npm job 影响。

# 1. 一键改所有"版本声明点"（输入 npm 版本号，自动算 Rust 线；含本清单顶部与表格当前值）
bash scripts/bump-version.sh 2.5.6

# 2. 一键全量回归（JS 单测 + 历史 bug 回归 test/regression.js + Rust 全量）
bash test/run-all.sh            # Windows: powershell -ExecutionPolicy Bypass -File test/run-all.ps1
#    必须全绿；任何一条失败都禁止进入下一步、禁止发版（历史 bug 回归见 test/README.md）

# 3. 手动补谱系点（第二节 G1–G5）：README / RELEASES / CHANGELOG / releases/新文件

# 4. 全仓复扫，确认无旧版本号漏网（把下面 grep 的版本号换成"上一版本"；
#    应只命中第三节"不要改"的历史注释 / SOP 示例）
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

> **新增版本点时**：在新文件/新模块里写了版本常量、窗口标题、版本打印后，**立即回到本登记表追加一行**，并在 `scripts/bump-version.sh` 里加对应替换规则——这样下一个版本就不会漏。
