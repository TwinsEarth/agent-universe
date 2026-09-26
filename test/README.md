# 回归测试套件（Regression Suite）

把 Agent Universe 从 v1.0.0 到当前版本开发过程中**踩过、修过的每一个 bug / 遗漏 / 注意事项**固化为测试，每次发版、每次改动都跑一遍，防止同一个问题反复出现。新遇到的 bug，修复时必须同步在此追加用例。

## 一键运行

| 平台 | 命令 |
| --- | --- |
| Linux / macOS | `bash test/run-all.sh` |
| Windows | `powershell -ExecutionPolicy Bypass -File test/run-all.ps1` |
| 仅 JS 回归 | `node test/regression.js` |
| 仅 Rust 全量 | `cd gsn-core && cargo test` |

CI（`.github/workflows/ci.yml`）在每次 push / PR 时自动执行 JS 回归与 Rust 全量测试。

## 用例索引

### A. 版本声明点一致性（防止 bump 版本丢三落四）

| 编号 | 覆盖的历史问题 |
| --- | --- |
| REG-001 | 根 / js / client / desktop 四个 `package.json` 版本必须都等于 SDK 版本（曾出现只改根、漏改子包） |
| REG-002 | `gsn-core/Cargo.toml` 版本必须等于 Rust 线 `0.2.(Y*10+Z)`（npm 与 Cargo 两条版本线曾不同步） |
| REG-003 | 两个 `Cargo.lock` 本包版本必须与 manifest 一致（曾出现 `cargo build --locked` 因 lock 过期失败） |
| REG-004 | `pyproject.toml` / js 运行时 version / `ci.yml` 版本校验串必须一致（Python SDK 与 CI 曾停旧版本） |

### B. 历史编译 & CI bug 守卫

| 编号 | 覆盖的历史问题 |
| --- | --- |
| REG-010 | `gsn-core/src/net/peer.rs` 的 `subscribe()` 曾把参数名误写成 `IdentTopic::new(t)`（实际参数名是 `topic`），导致 `cargo build` E0425 编译失败 |
| REG-011 | `release.yml` 曾缺顶层 `permissions: contents: write`，导致上传 Release 产物时 403 |
| REG-012 | `release.yml` 收集产物的 glob 曾在无匹配时直接报错退出（zsh `no matches found`），需 `shopt -s nullglob` |
| REG-013 | `publish.yml` 曾缺 `npm publish` 步骤，导致打 tag 后 npm 包始终不更新 |

### C. 市场重复防护与非法输入

| 编号 | 覆盖的历史问题 |
| --- | --- |
| REG-020 | 重复注册同一个 Agent 必须被拒绝（曾无防护导致质押状态被覆盖） |
| REG-021 | 同一任务重复结算必须返回 `{paid:0, reason:'already_paid'}`（曾可重复打款，资金凭空产生） |
| REG-022 | 负金额充值、空 goal、非正预算必须抛错（曾可发布零/负预算任务） |
| REG-023 | 未注册 Agent 投标、无投标时匹配必须抛错（曾产生空中标、任务卡死） |

### D. 资金守恒

| 编号 | 覆盖的历史问题 |
| --- | --- |
| REG-030 | 充值→质押→发布→投标→匹配→完成→结算全链路（无罚没）系统总余额必须等于总充值（支付是账户间转账，不得凭空增减） |
| REG-031 | 罚没后系统总余额必须等于总充值减罚没（罚没是唯一允许减少系统总余额的操作） |

### E. Rust 网络层（`gsn-core/tests/regression_net.rs`）

| 编号 | 覆盖的历史问题 |
| --- | --- |
| REG（Rust） | `subscribe()` 后 `publish()` 不得 panic；无 mesh peer 时 GossipSub 返回 `InsufficientPeers` 属正常，其他错误才是 bug |

## 已知、做不通 / 易踩的坑（避免重复排查）

- **libp2p reservation 不会自动续期**：不能依赖内部 `renewal_timeout`，必须在应用层显式重新 `listen_relay`。
- **第三方 kubo relay 的 reservation 通常只有约 120 秒 / 128 KB**，长期可能下线；需要主动重订 + relay 池（见 v2.5.5）。
- **对称 NAT 下 DCUtR TCP 打洞可能失败**（拓扑限制，非代码缺陷），此时应回落到 Circuit Relay。
- **官方 relay 不一定允许建 reservation**；社区节点需经 DHT 实际探测后入池。
- **GossipSub 在没有 mesh peer 时 publish 返回 `InsufficientPeers`** 是正常现象，不是 bug。
- **WKWebView 没有 Node `crypto` 模块**：桌面演示用固定 DID；真实密钥签名需走 Rust 侧 Tauri command。
- **数据目录里的字面 `~` 必须展开**（`dirs::home_dir()` / `std::env::home_dir`），不能直接当路径。
- **Identify 协议里的 `/gsn/0.2.YZ` 只是信息字段**，不参与连通性协商，版本不一致不会导致连不上。
- **npm 自动发布需要有效的 `NPM_TOKEN`**（repo Settings → Secrets and variables → Actions），token 过期则打 tag 也发不出去。

## 维护规则

1. 修复任何新 bug，**先在本目录（或对应语言的 tests 目录）追加一条能复现该 bug 的用例**，确认它在修复前失败、修复后通过，再提交。
2. 新增功能时，把新的版本声明点 / 配置点同步登记进 `docs/version-checklist.md`，并在 `scripts/bump-version.sh` 加对应规则。
3. 用例编号只增不复用；废弃用例保留编号并注明，不删除。
