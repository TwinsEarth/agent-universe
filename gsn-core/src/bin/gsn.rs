//! gsn - Agent Universe 统一命令行入口
//!
//! 子命令：
//!   gsn version                         版本信息
//!   gsn daemon [选项]                   启动全节点（同 gsn-daemon）
//!   gsn mcp [--transport stdio]         启动 MCP 服务器（默认 stdio）
//!   gsn market <操作> [参数]            通过 HTTP API 操作智能体市场
//!   gsn ledger verify [--data-dir D]   离线校验账本哈希链 + 重放守恒独立审计
//!   gsn doctor [--data-dir D] [--api U] 环境/账本/连通性只读诊断
//!   gsn identity                        生成本地身份（Ed25519）
//!
//! market 子命令通过 --api <url> 或 GSN_API 环境变量连接 daemon，
//! 默认 http://127.0.0.1:4002。写操作在 daemon 配置了 REST_BEARER_TOKEN 时
//! 需要鉴权：用 --token <t> 或 GSN_API_TOKEN 提供 Bearer 令牌（不配置则不发该头）。

use gsn_core::identity::Keypair;
use gsn_core::node;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        print_top_help();
        std::process::exit(0);
    }

    let code = match argv[0].as_str() {
        "version" | "-V" | "--version" => {
            println!("gsn {}", VERSION);
            println!("agent-universe v3.9.10");
            if argv.iter().any(|a| a == "--check") {
                run_version_check(&argv[1..]).await
            } else {
                0
            }
        }
        "help" | "--help" | "-h" => {
            print_top_help();
            0
        }
        "update" => run_update(&argv[1..]).await,
        "daemon" => {
            // Docker 健康探针：`gsn daemon --healthcheck` 只做一次 HTTP GET，不启动节点。
            if node::wants_healthcheck(&argv[1..]) {
                std::process::exit(node::run_healthcheck_now().await);
            }
            let args = node::parse_daemon_args(&argv[1..]);
            match node::run_daemon(args).await {
                Ok(_) => 0,
                Err(e) => {
                    eprintln!("daemon 错误: {e}");
                    1
                }
            }
        }
        "mcp" => run_mcp(&argv[1..]).await,
        "market" => run_market(&argv[1..]).await,
        "ledger" => run_ledger(&argv[1..]).await,
        "doctor" => run_doctor(&argv[1..]).await,
        "identity" => run_identity(),
        other => {
            eprintln!("错误: 未知子命令 '{other}'");
            eprintln!("运行 'gsn help' 查看可用命令");
            1
        }
    };

    std::process::exit(code);
}

fn print_top_help() {
    println!("gsn - Agent Universe 统一命令行 v{}\n", VERSION);
    println!("用法: gsn <命令> [参数]\n");
    println!("命令:");
    println!("  daemon [选项]        启动 GSN 全节点（P2P + API + 市场）");
    println!("  mcp [--transport t]  启动 MCP 服务器（stdio，AI 客户端接入）");
    println!("  market <操作>        操作智能体市场（通过运行中的节点 API）");
    println!("  ledger verify        离线校验账本哈希链与重放守恒（只读）");
    println!("  doctor               环境/账本/daemon 连通性诊断（只读）");
    println!("  identity            生成 Ed25519 本地身份");
    println!("  update [版本]        检查/更新 daemon 与 npm 包（见 `gsn update --help`）");
    println!("  version             显示版本（加 --check 只联网检查更新，不安装）");
    println!("  help                显示本帮助\n");
    println!("market 操作: deposit/register/search/discover/publish/bid/");
    println!("             match/result/verify/settle/dispute/arbitrate/");
    println!("             balance/conservation/leaderboard/stats");
    println!("             （用 --api <url> 或 GSN_API 指定节点，默认 127.0.0.1:4002）");
}

// ───────────────────────── update ─────────────────────────

/// `gsn update` / `gsn version --check` 解析出的选项。
struct UpdateOpts {
    /// 显式目标版本（手动，可跨大版本）；None 表示按通道自动选目标。
    target: Option<String>,
    track: Option<gsn_core::update::Track>,
    check_only: bool,
    no_npm: bool,
}

fn print_update_help() {
    println!("用法:");
    println!("  gsn update                 按本机通道检查并自动更新（默认 minor）");
    println!("  gsn update --check         只检查是否有新版本，不安装");
    println!("  gsn update <X.Y.Z>         手动更新到指定版本（允许跨大版本/降级）");
    println!("  gsn update --track minor   自动跟最新中版本基线 x.Y.0（不追补丁）");
    println!("  gsn update --track patch   自动跟最新小版本 x.Y.Z（先锋/贡献者通道）");
    println!("  gsn update --no-npm        只更新 daemon 二进制，不更新 npm JS 包");
    println!("  gsn version --check        同 --check，只查询不安装");
    println!("\n环境变量:");
    println!("  GSN_AUTO_UPDATE=0          daemon 启动只检查不自动安装");
    println!("  GSN_NO_UPDATE_CHECK=1      完全关闭启动联网检查");
    println!("  GSN_UPDATE_TRACK=minor|patch  显式指定本机通道");
    println!("  GSN_PIONEERS_FILE=<path>   追加一份先锋/贡献者 Peer ID 名单");
    println!("\n说明: 自动更新绝不跨大版本；daemon 更新后下次启动生效。");
}

fn parse_update_opts(args: &[String]) -> Result<UpdateOpts, String> {
    let mut target = None;
    let mut track = None;
    let mut check_only = false;
    let mut no_npm = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--check" => check_only = true,
            "--no-npm" => no_npm = true,
            "--track" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--track 需要值 minor|patch".to_string())?;
                track = Some(match v.as_str() {
                    "minor" => gsn_core::update::Track::Minor,
                    "patch" => gsn_core::update::Track::Patch,
                    other => return Err(format!("未知通道 '{other}'（有效值: minor/patch）")),
                });
                i += 1;
            }
            "--help" | "-h" => {
                print_update_help();
                std::process::exit(0);
            }
            other if !other.starts_with('-') => {
                if target.is_none() {
                    target = Some(other.to_string());
                } else {
                    return Err(format!("多余的位置参数 '{other}'"));
                }
            }
            other => return Err(format!("未知选项 '{other}'（见 gsn update --help）")),
        }
        i += 1;
    }
    Ok(UpdateOpts {
        target,
        track,
        check_only,
        no_npm,
    })
}

/// `gsn version --check`：只查询，不安装。
async fn run_version_check(args: &[String]) -> i32 {
    let track = match parse_update_opts(args) {
        Ok(o) => o.track,
        Err(e) => {
            eprintln!("错误: {e}");
            return 2;
        }
    };
    match tokio::task::spawn_blocking(move || gsn_core::update::check(track)).await {
        Ok(Ok((current, decision, resolved))) => {
            println!("当前版本: v{current}（通道: {:?}）", resolved);
            match decision.target {
                Some(t) => println!("可更新: v{t}（未安装；用 `gsn update {t}` 安装）"),
                None => println!("已是本大版本通道内最新。"),
            }
            if let Some(m) = decision.newer_major {
                println!("另有新大版本 v{m}（不自动跨版本；用 `gsn update {m}` 手动升级）");
            }
            0
        }
        Ok(Err(e)) => {
            eprintln!("检查失败（不影响本地使用）: {e}");
            1
        }
        Err(e) => {
            eprintln!("检查任务异常: {e}");
            1
        }
    }
}

async fn run_update(args: &[String]) -> i32 {
    let opts = match parse_update_opts(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("错误: {e}");
            return 2;
        }
    };

    // ── 只检查 ──
    if opts.check_only {
        return run_version_check(args).await;
    }

    let timeout = gsn_core::update::AutoUpdateConfig::default().timeout;
    let do_npm = !opts.no_npm;

    // ── 确定目标版本 ──
    let target = match opts.target {
        Some(raw) => match gsn_core::update::SemVer::parse(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("错误: {e}");
                return 2;
            }
        },
        None => {
            let track = opts.track;
            let decision =
                tokio::task::spawn_blocking(move || gsn_core::update::check(track)).await;
            match decision {
                Ok(Ok((current, d, _))) => match d.target {
                    Some(t) => t,
                    None => {
                        println!("当前 v{current} 已是本大版本通道内最新。");
                        if let Some(m) = d.newer_major {
                            println!("另有新大版本 v{m}（用 `gsn update {m}` 手动升级）");
                        }
                        return 0;
                    }
                },
                Ok(Err(e)) => {
                    eprintln!("检查失败: {e}");
                    return 1;
                }
                Err(e) => {
                    eprintln!("检查任务异常: {e}");
                    return 1;
                }
            }
        }
    };

    // ── 下载→校验→原子替换→npm 更新 ──
    println!("正在更新到 v{target} …");
    let target_for_task = target.clone();
    match tokio::task::spawn_blocking(move || {
        gsn_core::update::perform_update(&target_for_task, do_npm, timeout)
    })
    .await
    {
        Ok(Ok(npm)) => {
            println!("✅ daemon 已更新到 v{target}（重启 daemon 后生效）。");
            if let Some(n) = npm {
                println!("   {n}");
            }
            0
        }
        Ok(Err(e)) => {
            eprintln!("❌ 更新失败: {e}");
            1
        }
        Err(e) => {
            eprintln!("更新任务异常: {e}");
            1
        }
    }
}

// ───────────────────────── MCP ─────────────────────────

async fn run_mcp(args: &[String]) -> i32 {
    let mut transport = "stdio";
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--transport" => {
                if i + 1 < args.len() {
                    transport = args[i + 1].as_str();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("用法: gsn mcp [--transport stdio]");
                println!("  stdio: 标准输入输出（Claude Desktop / Cursor 本地接入，默认）");
                return 0;
            }
            _ => i += 1,
        }
    }

    match transport {
        "stdio" => match gsn_core::mcp::stdio::run_stdio().await {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("mcp 错误: {e}");
                1
            }
        },
        other => {
            eprintln!(
                "错误: 不支持的 MCP 传输 '{other}'（远程请用节点的 http://<host>:4002/api/v1/mcp）"
            );
            1
        }
    }
}

// ───────────────────────── identity ─────────────────────────

fn run_identity() -> i32 {
    let keypair = Keypair::generate();
    let pubkey_hex: String = keypair
        .public_key()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    // peer id：公钥 SHA256 的前 16 字节 hex
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(keypair.public_key());
    let peer_id: String = digest
        .iter()
        .take(16)
        .map(|b| format!("{:02x}", b))
        .collect();
    println!("✅ 已生成新身份（Ed25519）");
    println!("   Peer ID (短): {}", peer_id);
    println!("   公钥 (hex): {}", pubkey_hex);
    println!("   提示: 私钥种子请自行安全保存（Keypair::seed），切勿入库或泄露");
    0
}

// ───────────────────────── market ─────────────────────────

async fn run_market(args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_market_help();
        return if args.is_empty() { 1 } else { 0 };
    }

    // 解析 --api / --token（token 默认取 GSN_API_TOKEN；不设置则不发 Authorization）
    let mut api = std::env::var("GSN_API").unwrap_or_else(|_| "http://127.0.0.1:4002".to_string());
    let mut token = std::env::var("GSN_API_TOKEN").ok();
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--api" if i + 1 < args.len() => {
                api = args[i + 1].clone();
                i += 2;
            }
            "--token" if i + 1 < args.len() => {
                token = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                positional.push(args[i].clone());
                i += 1;
            }
        }
    }

    let op = positional[0].as_str();
    let params = &positional[1..];

    // 构造 (method, path, body) 请求
    let request = build_market_request(op, params);
    let (method, path, body) = match request {
        Ok(r) => r,
        Err(msg) => {
            eprintln!("错误: {msg}");
            print_market_help();
            return 2;
        }
    };

    let token = token.filter(|t| !t.trim().is_empty());
    match http_call(&api, &method, &path, &body, token.as_deref()).await {
        Ok((status, text)) => {
            // 美化输出 JSON
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or(text)),
                Err(_) => println!("{}", text),
            }
            if (200..300).contains(&status) {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("请求节点 {} 失败: {}", api, e);
            eprintln!("（节点是否已用 'gsn daemon' 启动？）");
            1
        }
    }
}

fn print_market_help() {
    println!("用法: gsn market <操作> [参数] [--api url]\n");
    println!("操作:");
    println!("  deposit @signed-envelope.json      授信充值（提交治理成员签名的 governance:credit 信封，CLI 不替签）");
    println!("  balance <account>                   查询余额");
    println!("  register <card.json|inline-json>    注册智能体");
    println!("  get <agent_id>                      查询智能体");
    println!("  discover <skill>                    按技能发现");
    println!("  search <keyword>                    搜索智能体");
    println!("  publish <task.json>                 发布任务");
    println!("  task <task_id>                      查询任务");
    println!("  bid <bid.json>                      提交投标");
    println!("  match <task_id>                     匹配智能体");
    println!("  result <envelope.json>              提交结果");
    println!("  verify <task_id> @verify.json      认证式 QA 验证（v2.5.9）");
    println!("  settle <task_id>                    结算任务");
    println!("  dispute <dispute.json>              发起争议");
    println!("  arbitrate <dispute_id> <guilty> <arbitrator>  仲裁（罚没由服务端规则决定）");
    println!("  conservation                        守恒检查");
    println!("  audit                               独立审计（v2.5.9）");
    println!("  leaderboard [limit]                 信誉排行榜");
    println!("  stats                               市场统计");
    println!("\n鉴权：daemon 配置 REST_BEARER_TOKEN 时，写操作需 --token <t> 或 GSN_API_TOKEN。");
}

/// 读取参数：若是 @file 则读文件内容，否则原样（JSON 字符串）
fn read_json_arg(arg: &str) -> String {
    if let Some(path) = arg.strip_prefix('@') {
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("读取 {} 失败: {}", path, e);
            std::process::exit(1);
        })
    } else {
        arg.to_string()
    }
}

/// 构造市场请求三元组。返回 Err(可读原因) 表示参数不合法（调用方以退出码 2 终止，
/// 绝不发出会被服务端误解为 0 的请求）。
#[allow(clippy::type_complexity)]
fn build_market_request(op: &str, p: &[String]) -> Result<(String, String, String), String> {
    use serde_json::json;
    match op {
        // P0-4：授信（credit）是特权写，必须提交受权治理成员已签名的
        // SignedGovernanceCommand 信封（capability=governance:credit、
        // target=受信账户、claim.amount>=0）。CLI 不持有治理私钥，绝不替用户
        // 签名；生产授信必须由链上支付凭证支撑（纯协议内核，需外部审计）。
        // 用法：gsn market deposit @deposit-envelope.json
        "deposit" if p.len() == 1 => {
            let body = read_json_arg(&p[0]);
            let env: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
                format!("deposit 需提交签名治理信封 JSON 文件（SignedGovernanceCommand）: {e}")
            })?;
            let target = env
                .get("target")
                .and_then(|x| x.as_str())
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| "签名治理信封缺少非空字符串字段 target".to_string())?;
            match env.get("capability").and_then(|x| x.as_str()) {
                Some("governance:credit") => {}
                other => {
                    return Err(format!(
                        "deposit 要求 capability=governance:credit，当前为 {:?}；CLI 不替治理成员签名，请提交受权信封",
                        other.unwrap_or("")
                    ));
                }
            }
            Ok((
                "POST".into(),
                format!("/api/v1/accounts/{}/deposit", target),
                body,
            ))
        }
        "deposit" => {
            Err(
                "deposit 用法已变更（授信为特权写，需治理成员签名，CLI 不替签）：\n\
                 正确：gsn market deposit @deposit-envelope.json\n\
                 信封由受权治理成员生成（参考 examples/market_demo.rs / examples/mac_gov_fixture.rs）；\n\
                 不再支持 gsn market deposit <account> <amount>（裸 amount 必被服务端拒绝）。"
                    .to_string(),
            )
        }
        "balance" if !p.is_empty() => Ok((
            "GET".into(),
            format!("/api/v1/accounts/{}/balance", p[0]),
            String::new(),
        )),
        "register" if !p.is_empty() => {
            Ok(("POST".into(), "/api/v1/agents".into(), read_json_arg(&p[0])))
        }
        "get" if !p.is_empty() => Ok((
            "GET".into(),
            format!("/api/v1/agents/{}", p[0]),
            String::new(),
        )),
        "discover" if !p.is_empty() => Ok((
            "GET".into(),
            format!("/api/v1/agents?skill={}", p[0]),
            String::new(),
        )),
        "search" if !p.is_empty() => Ok((
            "GET".into(),
            format!("/api/v1/agents?q={}", p[0]),
            String::new(),
        )),
        "publish" if !p.is_empty() => {
            Ok(("POST".into(), "/api/v1/tasks".into(), read_json_arg(&p[0])))
        }
        "task" if !p.is_empty() => Ok((
            "GET".into(),
            format!("/api/v1/tasks/{}", p[0]),
            String::new(),
        )),
        "bid" if !p.is_empty() => {
            // bid JSON 需含 task_id；若只给 agent/task/price 则组装
            let body = read_json_arg(&p[0]);
            let task_id = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("task_id").and_then(|x| x.as_str()).map(String::from));
            task_id
                .map(|tid| ("POST".into(), format!("/api/v1/tasks/{}/bids", tid), body))
                .ok_or_else(|| "bid JSON 缺少字符串字段 task_id".to_string())
        }
        "match" if !p.is_empty() => Ok((
            "POST".into(),
            format!("/api/v1/tasks/{}/match", p[0]),
            String::new(),
        )),
        "result" if !p.is_empty() => {
            let body = read_json_arg(&p[0]);
            let task_id = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("task_id").and_then(|x| x.as_str()).map(String::from));
            task_id
                .map(|tid| {
                    (
                        "POST".into(),
                        format!("/api/v1/tasks/{}/results", tid),
                        body,
                    )
                })
                .ok_or_else(|| "result JSON 缺少字符串字段 task_id".to_string())
        }
        "verify" if !p.is_empty() => {
            // v2.5.9 认证式：载荷文件含 members（固定委员集）与 signed_votes（签名票）
            if p.len() < 2 {
                return Err("verify 需要认证载荷：verify <task_id> @verify.json".into());
            }
            let body = read_json_arg(&p[1]);
            Ok((
                "POST".into(),
                format!("/api/v1/tasks/{}/verify", p[0]),
                body,
            ))
        }
        "settle" if !p.is_empty() => Ok((
            "POST".into(),
            format!("/api/v1/tasks/{}/settle", p[0]),
            String::new(),
        )),
        "dispute" if !p.is_empty() => Ok((
            "POST".into(),
            "/api/v1/disputes".into(),
            read_json_arg(&p[0]),
        )),
        "arbitrate" if p.len() >= 3 => {
            // v3.5.5：罚没金额由服务端规则决定（服务端已不读 slash_amount），
            // CLI 不再发送会被忽略的 f64 slash；改为必填非空仲裁者身份（服务端强制）。
            let guilty = matches!(p[1].as_str(), "guilty" | "true");
            let arbitrator = p[2].trim();
            if arbitrator.is_empty() {
                return Err("arbitrate 需要非空仲裁者身份 <arbitrator>".into());
            }
            Ok((
                "POST".into(),
                format!("/api/v1/disputes/{}/arbitrate", p[0]),
                json!({"guilty": guilty, "arbitrator": arbitrator}).to_string(),
            ))
        }
        "conservation" => Ok(("GET".into(), "/api/v1/conservation".into(), String::new())),
        "audit" => Ok(("GET".into(), "/api/v1/audit".into(), String::new())),
        "leaderboard" => {
            let limit = p.first().map(|s| s.as_str()).unwrap_or("10");
            Ok((
                "GET".into(),
                format!("/api/v1/leaderboard?limit={}", limit),
                String::new(),
            ))
        }
        "stats" => Ok(("GET".into(), "/api/v1/stats".into(), String::new())),
        _ => Err(format!("无法构造 market {op} 请求（操作未知或参数不足）")),
    }
}

// ───────────────────────── 极简 HTTP 客户端 ─────────────────────────

// ───────────────────────── ledger verify ─────────────────────────

/// 解析 `--data-dir`，得到账本数据库路径（`<data-dir>/gsn.db`）。
/// 缺省用 daemon 的默认数据目录；`~` 与 daemon 保持同一展开规则。
fn resolve_db_path(args: &[String]) -> Result<std::path::PathBuf, String> {
    let mut data_dir = node::default_data_dir();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--data-dir" {
            let v = args
                .get(i + 1)
                .ok_or_else(|| "--data-dir 需要一个路径参数".to_string())?;
            data_dir = std::path::PathBuf::from(v);
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(node::expand_tilde(data_dir).join("gsn.db"))
}

/// 离线账本核验结果。注意边界（如实陈述，不夸大）：哈希链是**篡改检测**手段；
/// 离线重放 `independent_audit` 验证的是**流水自身重放后的守恒**（累计充值 vs 账户余额
/// 之和），离线 CLI 没有另一份在线余额可交叉比对——在线交叉比对由运行中的 daemon
/// （REST `/audit`、结算前审计）承担。
#[derive(Debug, PartialEq, Eq)]
struct LedgerReport {
    records: usize,
    corrupt: usize,
    head: String,
    passed: bool,
    expected_total: i64,
    actual_total: i64,
    aggregate_matches: bool,
    mismatches: usize,
}

/// 离线核验可能的失败类型（供 CLI 决定退出码与措辞，也便于测试）。
#[derive(Debug, PartialEq, Eq)]
enum LedgerIssue {
    /// 数据库文件不存在（核验不得顺手创建空库）。
    MissingDb,
    /// 哈希链在某 seq 断链/哈希不符；`u64::MAX` 表示锚定 head 不一致。
    ChainBroken(u64),
    /// 存在无法解析的损坏流水行。
    CorruptRows(usize),
    /// 打开/读取数据库失败。
    OpenFailed(String),
    /// 流水重放或审计本身失败（如守恒不成立）。
    AuditFailed(String),
}

/// 对给定 db 文件做离线核验，纯计算、不打印、不产生副作用。
fn evaluate_ledger(db: &std::path::Path) -> Result<LedgerReport, LedgerIssue> {
    use gsn_core::marketplace::SettlementEngine;
    use gsn_core::storage::PersistentStore;

    if !db.exists() {
        return Err(LedgerIssue::MissingDb);
    }
    let store = PersistentStore::open(db).map_err(|e| LedgerIssue::OpenFailed(e.to_string()))?;

    let load = store
        .load_ledger_records_checked()
        .map_err(|e| LedgerIssue::OpenFailed(e.to_string()))?;
    if !load.corrupt.is_empty() {
        return Err(LedgerIssue::CorruptRows(load.corrupt.len()));
    }

    // 1) 哈希链 + 锚定 head：这是离线可做的篡改检测。
    let head = store
        .verify_ledger_chain()
        .map_err(LedgerIssue::ChainBroken)?;

    // 2) 真实生产路径重放 + independent_audit（不另造校验逻辑）。
    let engine = SettlementEngine::restore(load.records).map_err(LedgerIssue::AuditFailed)?;
    let audit = engine.independent_audit();

    Ok(LedgerReport {
        records: audit.replayed_records,
        corrupt: 0,
        head,
        passed: audit.passed,
        expected_total: audit.expected_total.0,
        actual_total: audit.actual_total.0,
        aggregate_matches: audit.aggregate_matches,
        mismatches: audit.mismatches.len(),
    })
}

fn print_ledger_report(db: &std::path::Path, r: &LedgerReport) {
    let short_head: String = r.head.chars().take(16).collect();
    println!("账本文件: {}", db.display());
    println!("流水条数: {}", r.records);
    println!(
        "链 head : {}",
        if short_head.is_empty() {
            "(genesis / 空链)".to_string()
        } else {
            short_head
        }
    );
    println!(
        "重放守恒: expected={} actual={} aggregate_matches={}",
        r.expected_total, r.actual_total, r.aggregate_matches
    );
    println!("账实不符账户: {}", r.mismatches);
    if r.passed && r.mismatches == 0 {
        println!("结果: PASS — 哈希链完整，流水重放守恒，审计通过");
    } else {
        println!("结果: FAIL — 审计未通过（见上方账实不符明细）");
    }
}

async fn run_ledger(args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("用法: gsn ledger verify [--data-dir <目录>]");
        println!(
            "  离线校验本地账本：哈希链完整性（报首个断链 seq）+ 锚定 head + 重放守恒独立审计。"
        );
        println!("  默认数据目录同 daemon；不修改任何数据，也不会创建数据库。");
        return if args.is_empty() { 1 } else { 0 };
    }
    if args[0] != "verify" {
        eprintln!("错误: 未知 ledger 操作 '{}'（支持: verify）", args[0]);
        return 1;
    }
    let db = match resolve_db_path(&args[1..]) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("参数错误: {e}");
            return 2;
        }
    };
    match evaluate_ledger(&db) {
        Ok(r) => {
            print_ledger_report(&db, &r);
            if r.passed && r.mismatches == 0 {
                0
            } else {
                1
            }
        }
        Err(LedgerIssue::MissingDb) => {
            eprintln!("账本数据库不存在: {}", db.display());
            eprintln!("提示: 用 --data-dir 指定节点数据目录，或先启动一次 gsn daemon。");
            2
        }
        Err(LedgerIssue::ChainBroken(seq)) if seq == u64::MAX => {
            eprintln!("结果: FAIL — 锚定 head 与链末不一致（元数据可能被篡改）");
            1
        }
        Err(LedgerIssue::ChainBroken(seq)) => {
            eprintln!("结果: FAIL — 哈希链断链/哈希不符，首个异常 seq = {seq}");
            1
        }
        Err(LedgerIssue::CorruptRows(n)) => {
            eprintln!("结果: FAIL — 存在 {n} 条无法解析的损坏流水行（见 daemon 恢复告警）");
            1
        }
        Err(LedgerIssue::OpenFailed(e)) => {
            eprintln!("无法打开/读取账本: {e}");
            2
        }
        Err(LedgerIssue::AuditFailed(e)) => {
            eprintln!("结果: FAIL — 流水重放/审计失败: {e}");
            1
        }
    }
}

// ───────────────────────── doctor ─────────────────────────

struct DoctorItem {
    name: &'static str,
    ok: bool,
    hard: bool,
    detail: String,
}

async fn run_doctor(args: &[String]) -> i32 {
    let mut items: Vec<DoctorItem> = Vec::new();

    // 1) 版本
    items.push(DoctorItem {
        name: "CLI 版本",
        ok: true,
        hard: false,
        detail: format!("gsn {VERSION}"),
    });

    // 2) 数据目录与账本（硬检查：链断或损坏即不健康）
    let db = resolve_db_path(args).unwrap_or_else(|_| node::default_data_dir().join("gsn.db"));
    if !db.exists() {
        items.push(DoctorItem {
            name: "本地账本",
            ok: false,
            hard: false,
            detail: format!("未找到 {}（尚未初始化，不影响启动）", db.display()),
        });
    } else {
        match evaluate_ledger(&db) {
            Ok(r) => items.push(DoctorItem {
                name: "本地账本",
                ok: r.passed && r.mismatches == 0,
                hard: true,
                detail: format!(
                    "{} 条流水，链完整，重放守恒 expected=actual={}",
                    r.records, r.actual_total
                ),
            }),
            Err(LedgerIssue::ChainBroken(seq)) if seq == u64::MAX => items.push(DoctorItem {
                name: "本地账本",
                ok: false,
                hard: true,
                detail: "锚定 head 与链末不一致".to_string(),
            }),
            Err(LedgerIssue::ChainBroken(seq)) => items.push(DoctorItem {
                name: "本地账本",
                ok: false,
                hard: true,
                detail: format!("哈希链断链，首个异常 seq = {seq}"),
            }),
            Err(LedgerIssue::CorruptRows(n)) => items.push(DoctorItem {
                name: "本地账本",
                ok: false,
                hard: true,
                detail: format!("{n} 条损坏流水行"),
            }),
            Err(other) => items.push(DoctorItem {
                name: "本地账本",
                ok: false,
                hard: true,
                detail: format!("无法核验: {other:?}"),
            }),
        }
    }

    // 3) daemon 连通性（软检查：daemon 可能本来就没在跑）
    let mut api = std::env::var("GSN_API").unwrap_or_else(|_| "http://127.0.0.1:4002".to_string());
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--api" {
            if let Some(v) = args.get(i + 1) {
                api = v.clone();
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    let probe = tokio::time::timeout(
        std::time::Duration::from_millis(900),
        http_call(&api, "GET", "/health", "", None),
    )
    .await;
    match probe {
        Ok(Ok((200, _))) => items.push(DoctorItem {
            name: "daemon 连通性",
            ok: true,
            hard: false,
            detail: format!("{api} /health 200"),
        }),
        Ok(Ok((code, _))) => items.push(DoctorItem {
            name: "daemon 连通性",
            ok: false,
            hard: false,
            detail: format!("{api} 有响应但 HTTP {code}"),
        }),
        Ok(Err(e)) => items.push(DoctorItem {
            name: "daemon 连通性",
            ok: false,
            hard: false,
            detail: format!("{api} 连接失败: {e}（daemon 未运行则可忽略）"),
        }),
        Err(_) => items.push(DoctorItem {
            name: "daemon 连通性",
            ok: false,
            hard: false,
            detail: format!("{api} 探测超时（daemon 未运行或被防火墙拦截）"),
        }),
    }

    println!("gsn doctor — 环境与数据诊断\n");
    let mut hard_fail = false;
    for it in &items {
        let mark = if it.ok {
            "OK  "
        } else if it.hard {
            "FAIL"
        } else {
            "WARN"
        };
        if !it.ok && it.hard {
            hard_fail = true;
        }
        println!("[{mark}] {:<14} {}", it.name, it.detail);
    }
    println!();
    if hard_fail {
        println!("结论: 存在硬错误（账本完整性），建议先备份数据目录再排查。");
        1
    } else {
        println!("结论: 无硬错误（WARN 项按需要处理）。");
        0
    }
}

// doctor 只用 CARGO_PKG_VERSION，避免新增会与 VERSION 漂移的声明点。

async fn http_call(
    base: &str,
    method: &str,
    path: &str,
    body: &str,
    token: Option<&str>,
) -> anyhow::Result<(u16, String)> {
    let base = base.trim_end_matches('/');
    // 解析 host:port
    let after_proto = base.split("://").nth(1).unwrap_or(base);
    let host_port = after_proto.split('/').next().unwrap_or("127.0.0.1:4002");
    let (host, port) = host_port
        .split_once(':')
        .map(|(h, p)| (h, p.parse::<u16>().unwrap_or(80)))
        .unwrap_or((host_port, 80));

    let mut stream = tokio::net::TcpStream::connect((host, port)).await?;
    stream.set_nodelay(true)?;

    // v3.5.5（W-03）：daemon 配置 REST_BEARER_TOKEN 时，写操作需要 Bearer。
    // 旧手写 HTTP 客户端从不发 Authorization，导致配了 token 后所有写操作 401。
    let auth = match token {
        Some(t) => format!("Authorization: Bearer {t}\r\n"),
        None => String::new(),
    };
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\n{auth}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        method, path, host_port, body.len(), body
    );
    tokio::io::AsyncWriteExt::write_all(&mut stream, req.as_bytes()).await?;
    tokio::io::AsyncWriteExt::flush(&mut stream).await?;

    // 读取完整响应（服务器 Connection: close，读到 EOF 为止）
    let mut response = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response).await?;

    let text = String::from_utf8_lossy(&response).to_string();
    let status = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    Ok((status, text[body_start..].to_string()))
}

#[cfg(test)]
mod tests {
    use super::{build_market_request, evaluate_ledger, LedgerIssue};
    use gsn_core::marketplace::{Money, SettlementReason, SettlementRecord};
    use gsn_core::storage::PersistentStore;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("gsn-cli-{tag}-{pid}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn deposit(acct: &str, amount: i64, ts: u64) -> SettlementRecord {
        SettlementRecord {
            task_id: format!("deposit:{acct}"),
            from_account: String::new(),
            to_account: acct.to_string(),
            amount: Money::new(amount),
            reason: SettlementReason::Deposited,
            timestamp: ts,
        }
    }

    #[test]
    fn ledger_verify_passes_on_intact_chain() {
        let dir = unique_dir("ok");
        let db = dir.join("gsn.db");
        {
            let store = PersistentStore::open(&db).expect("open");
            store
                .append_ledger_record(&deposit("a1", 100, 1))
                .expect("a1");
            store
                .append_ledger_record(&deposit("a2", 250, 2))
                .expect("a2");
        }
        let r = evaluate_ledger(&db).expect("intact ledger must verify");
        assert_eq!(r.records, 2);
        assert!(r.passed, "replay conservation must hold");
        assert_eq!(r.expected_total, 350);
        assert_eq!(r.mismatches, 0);
    }

    #[test]
    fn ledger_verify_missing_db_is_named_not_created() {
        let dir = unique_dir("missing");
        let db = dir.join("gsn.db");
        // 核验不得为了“通过”而创建空库。
        assert_eq!(evaluate_ledger(&db), Err(LedgerIssue::MissingDb));
        assert!(!db.exists(), "verify 不得创建数据库文件");
    }

    #[test]
    fn ledger_verify_reports_first_broken_seq_on_payload_tamper() {
        let dir = unique_dir("broken");
        let db = dir.join("gsn.db");
        {
            let store = PersistentStore::open(&db).expect("open");
            store
                .append_ledger_record(&deposit("a1", 100, 1))
                .expect("a1");
            store
                .append_ledger_record(&deposit("a2", 250, 2))
                .expect("a2");
        }
        // 凭空造币的等价手法：把 seq=1 的 payload 改成更大金额（合法 JSON，但哈希对不上）。
        let forged = serde_json::to_string(&deposit("a1", 99_999, 1)).expect("serialize");
        {
            let conn = rusqlite::Connection::open(&db).expect("raw conn");
            conn.execute(
                "UPDATE ledger_entries SET payload = ?1 WHERE seq = 1",
                [forged],
            )
            .expect("tamper");
        }
        assert_eq!(evaluate_ledger(&db), Err(LedgerIssue::ChainBroken(1)));
    }

    #[test]
    fn ledger_verify_reports_corrupt_rows_explicitly() {
        let dir = unique_dir("corrupt");
        let db = dir.join("gsn.db");
        {
            let store = PersistentStore::open(&db).expect("open");
            store
                .append_ledger_record(&deposit("a1", 100, 1))
                .expect("a1");
            let conn = rusqlite::Connection::open(&db).expect("raw conn");
            conn.execute(
                "INSERT INTO ledger_entries (payload) VALUES ('{not json')",
                [],
            )
            .expect("insert corrupt");
        }
        // 损坏行必须被显式报告，不能静默跳过然后 PASS（GAP §3.6）。
        assert_eq!(evaluate_ledger(&db), Err(LedgerIssue::CorruptRows(1)));
    }

    #[test]
    fn ledger_verify_reports_anchor_mismatch_as_max() {
        let dir = unique_dir("anchor");
        let db = dir.join("gsn.db");
        {
            let store = PersistentStore::open(&db).expect("open");
            store
                .append_ledger_record(&deposit("a1", 100, 1))
                .expect("a1");
            let conn = rusqlite::Connection::open(&db).expect("raw conn");
            conn.execute(
                "UPDATE kv_meta SET value = 'deadbeef' WHERE key = 'ledger_head_hash'",
                [],
            )
            .expect("tamper anchor");
        }
        assert_eq!(
            evaluate_ledger(&db),
            Err(LedgerIssue::ChainBroken(u64::MAX))
        );
    }

    #[test]
    fn deposit_forwards_signed_governance_envelope_and_derives_target() {
        // P0-4：deposit 必须提交治理成员已签名的授信信封；CLI 只转发、不替签。
        let envelope = serde_json::json!({
            "id": "cmd-1",
            "sender_did": "did:nau:gov",
            "sender_pubkey": "00".repeat(32),
            "capability": "governance:credit",
            "target": "did:nau:beneficiary",
            "claim": { "amount": 250 },
            "nonce": "n-1",
            "issued_at": 1000,
            "expires_at": 2000,
            "signature": "ab".repeat(64)
        })
        .to_string();
        let (method, path, body) =
            build_market_request("deposit", std::slice::from_ref(&envelope)).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(path, "/api/v1/accounts/did:nau:beneficiary/deposit");
        // 信封原样转发，CLI 不重排/不篡改字段，签名才可继续验证。
        assert_eq!(body, envelope);
    }

    #[test]
    fn deposit_rejects_legacy_positional_and_invalid_envelope() {
        // 旧的 deposit <account> <amount>（裸 amount）必被拒绝，不再构造请求。
        assert!(build_market_request("deposit", &["acct".into(), "250".into()]).is_err());
        assert!(build_market_request("deposit", &[]).is_err());

        let no_target = serde_json::json!({
            "capability": "governance:credit",
            "claim": { "amount": 1 }
        })
        .to_string();
        assert!(build_market_request("deposit", &[no_target]).is_err());

        let bad_cap = serde_json::json!({
            "capability": "governance:arbitrate",
            "target": "did:nau:x",
            "claim": {}
        })
        .to_string();
        assert!(build_market_request("deposit", &[bad_cap]).is_err());

        assert!(build_market_request("deposit", &["not-json".into()]).is_err());
    }

    #[test]
    fn arbitrate_requires_arbitrator_and_omits_client_slash_amount() {
        // 旧签名 arbitrate <id> <guilty> [slash]（f64，服务端已忽略 slash）必须不再成立。
        assert!(build_market_request("arbitrate", &["d1".into(), "guilty".into()]).is_err());

        let (method, path, body) = build_market_request(
            "arbitrate",
            &["d1".into(), "guilty".into(), "did:nau:judge".into()],
        )
        .unwrap();
        assert_eq!(method, "POST");
        assert_eq!(path, "/api/v1/disputes/d1/arbitrate");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["guilty"], serde_json::json!(true));
        assert_eq!(v["arbitrator"], "did:nau:judge");
        // 罚没由服务端规则决定：CLI 不再发送会被忽略的 slash_amount（f64）。
        assert!(v.get("slash_amount").is_none());
    }
}
