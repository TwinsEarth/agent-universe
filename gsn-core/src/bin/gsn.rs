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
use gsn_core::memory::{
    Cat7Block, Cat7Field, FieldVerdict, FleetAccess, FleetKind, FleetMemory, LineageGraph,
    SvafAssessment, SvafDecision,
};
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
            println!("agent-universe v5.1.3");
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
        "memory" => run_memory(&argv[1..]).await,
        "vpn" => run_vpn(&argv[1..]).await,
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
    println!("  memory <操作>       读写本地记忆档案（append/list/recall/query/path）");
    println!("  vpn <子命令>         系统代理/PAC/订阅/路由/探活（见 `gsn vpn --help`）");
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

// ───────────────────────── vpn ─────────────────────────

/// `gsn vpn`：系统代理 / PAC / 订阅 / 路由 / 探活（v3.9.14，参照 v2rayN）。
async fn run_vpn(args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_vpn_help();
        return if args.is_empty() { 1 } else { 0 };
    }
    match args[0].as_str() {
        "set" => {
            // gsn vpn set --host H --port P [--bypass a,b]
            let mut host = "127.0.0.1".to_string();
            let mut port: u16 = 10808;
            let mut bypass: Vec<String> = vec!["<local>".to_string()];
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--host" if i + 1 < args.len() => {
                        host = args[i + 1].clone();
                        i += 2;
                    }
                    "--port" if i + 1 < args.len() => {
                        if let Ok(p) = args[i + 1].parse() {
                            port = p;
                        } else {
                            eprintln!("错误: 端口非法 '{}'", args[i + 1]);
                            return 1;
                        }
                        i += 2;
                    }
                    "--bypass" if i + 1 < args.len() => {
                        bypass = args[i + 1]
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        i += 2;
                    }
                    other => {
                        eprintln!("错误: 未知参数 '{other}'");
                        return 1;
                    }
                }
            }
            let cfg =
                gsn_core::net::SystemProxyConfig::new(gsn_core::net::ProxyScheme::Http, host, port)
                    .with_bypass(&bypass.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            let mgr = gsn_core::net::SystemProxyManager::new(gsn_core::net::RealCommandRunner);
            match mgr.set_http_proxy(&cfg) {
                Ok(r) => {
                    println!(
                        "系统 HTTP 代理已设置: {} ({}): {}",
                        cfg.url(),
                        r.platform,
                        r.detail
                    );
                    if !r.ok {
                        return 1;
                    }
                }
                Err(e) => {
                    eprintln!("设置失败: {e}");
                    return 1;
                }
            }
            0
        }
        "clear" => {
            let mgr = gsn_core::net::SystemProxyManager::new(gsn_core::net::RealCommandRunner);
            match mgr.clear_proxy() {
                Ok(r) => {
                    println!("系统代理已清除 ({}): {}", r.platform, r.detail);
                    if !r.ok {
                        return 1;
                    }
                }
                Err(e) => {
                    eprintln!("清除失败: {e}");
                    return 1;
                }
            }
            0
        }
        "pac" => {
            // gsn vpn pac --port 10808 （本地 PAC 服务；Ctrl-C 退出）
            let mut port: u16 = 10808;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--port" if i + 1 < args.len() => {
                        if let Ok(p) = args[i + 1].parse() {
                            port = p;
                        } else {
                            eprintln!("错误: 端口非法 '{}'", args[i + 1]);
                            return 1;
                        }
                        i += 2;
                    }
                    other => {
                        eprintln!("错误: 未知参数 '{other}'");
                        return 1;
                    }
                }
            }
            let listen: std::net::SocketAddr = match format!("127.0.0.1:{port}").parse() {
                Ok(addr) => addr,
                Err(e) => {
                    eprintln!("错误: 监听地址非法: {e}");
                    return 1;
                }
            };
            let cfg = gsn_core::net::PacConfig {
                listen,
                proxy_port: port,
                custom_pac_path: None,
            };
            match gsn_core::net::serve_pac(cfg).await {
                Ok(handle) => {
                    println!("PAC 服务已启动: http://127.0.0.1:{port}/proxy.pac");
                    println!("按 Ctrl-C 停止");
                    tokio::select! {
                        _ = handle => {}
                        _ = tokio::signal::ctrl_c() => {}
                    }
                    0
                }
                Err(e) => {
                    eprintln!("PAC 服务启动失败: {e}");
                    1
                }
            }
        }
        "sub" => {
            // gsn vpn sub <url>  ：校验订阅 URL 并展开节点列表（离线，不联网）。
            if args.len() < 2 {
                eprintln!("用法: gsn vpn sub <订阅URL>");
                return 1;
            }
            let entry = gsn_core::net::SubscriptionEntry {
                id: "cli-sub".to_string(),
                url: args[1].clone(),
                enabled: true,
                more_urls: Vec::new(),
            };
            match entry.validate() {
                Ok(()) => {
                    let urls = entry.all_urls();
                    println!("订阅校验通过: {}", args[1]);
                    println!("  展开 URL 数: {}", urls.len());
                    for u in &urls {
                        println!("   - {u}");
                    }
                    // 离线演示：以 url 本身作为正文占位，展示解析入口；真实下载由调用方执行。
                    let mut dedup = std::collections::HashSet::new();
                    let nodes = gsn_core::net::parse_nodes(&entry.url, &entry.id, &mut dedup);
                    println!("  解析节点数: {}", nodes.len());
                    for n in nodes.iter().take(20) {
                        println!(
                            "   * {}  {}",
                            n.peer_id.as_deref().unwrap_or("-"),
                            n.address
                        );
                    }
                    if nodes.len() > 20 {
                        println!("   ... 其余 {} 条省略", nodes.len() - 20);
                    }
                }
                Err(e) => {
                    eprintln!("订阅校验失败: {e}");
                    return 1;
                }
            }
            0
        }
        "route" => {
            // gsn vpn route <规则文件> <host> [ip]
            if args.len() < 3 {
                eprintln!("用法: gsn vpn route <规则文件> <host> [ip]");
                return 1;
            }
            let path = &args[1];
            let host = &args[2];
            let ip = args.get(3).map(|s| s.as_str());
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("读取规则文件失败: {e}");
                    return 1;
                }
            };
            let mut set = gsn_core::net::RouteSet {
                rules: Vec::new(),
                fallback: gsn_core::net::RouteAction::Direct,
            };
            for line in content.lines() {
                if let Some(rule) = gsn_core::net::parse_rule_line(line) {
                    set.rules.push(rule);
                }
            }
            let gs = std::collections::HashSet::new();
            let action = set.evaluate(host, ip, 0, "", "", "", &gs, &gs);
            println!("路由决策: {} => {:?}", host, action);
            0
        }
        "probe" => {
            // gsn vpn probe --port 10808 [--url U] [--ip-api U]
            let mut cfg = gsn_core::net::ProbeConfig::default();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--port" if i + 1 < args.len() => {
                        if let Ok(p) = args[i + 1].parse() {
                            cfg.proxy_port = p;
                        } else {
                            eprintln!("错误: 端口非法 '{}'", args[i + 1]);
                            return 1;
                        }
                        i += 2;
                    }
                    "--url" if i + 1 < args.len() => {
                        cfg.speed_ping_url = args[i + 1].clone();
                        i += 2;
                    }
                    "--ip-api" if i + 1 < args.len() => {
                        cfg.ip_api_url = args[i + 1].clone();
                        i += 2;
                    }
                    other => {
                        eprintln!("错误: 未知参数 '{other}'");
                        return 1;
                    }
                }
            }
            let handshake = gsn_core::net::socks5_handshake(cfg.proxy_port, cfg.timeout).await;
            match handshake {
                Ok(true) => println!("SOCKS5 握手: 通过 (127.0.0.1:{})", cfg.proxy_port),
                Ok(false) => {
                    eprintln!("SOCKS5 握手: 代理存在但返回非 05 00，可能需认证");
                    return 1;
                }
                Err(e) => {
                    eprintln!("SOCKS5 握手: 失败 - {e}");
                    return 1;
                }
            }
            match gsn_core::net::measure_latency_via_proxy(
                cfg.proxy_port,
                &cfg.speed_ping_url,
                cfg.timeout,
            )
            .await
            {
                Ok(ms) => println!("延迟测量 ({}): {ms} ms", cfg.speed_ping_url),
                Err(e) => eprintln!("延迟测量失败: {e}"),
            }
            match gsn_core::net::query_exit_info(cfg.proxy_port, &cfg.ip_api_url, cfg.timeout).await
            {
                Ok(info) => println!("出口信息: {info}"),
                Err(e) => eprintln!("出口信息查询失败: {e}"),
            }
            0
        }
        other => {
            eprintln!("错误: 未知 vpn 子命令 '{other}'");
            print_vpn_help();
            1
        }
    }
}

fn print_vpn_help() {
    println!("用法: gsn vpn <子命令> [参数]\n");
    println!("子命令:");
    println!("  set [--host H] [--port P] [--bypass a,b]   设置系统 HTTP 代理");
    println!("  clear                                      清除系统代理");
    println!("  pac [--port P]                             启动本地 PAC 服务 (http://127.0.0.1:P/proxy.pac)");
    println!("  sub <订阅URL>                              校验并展开订阅节点（离线）");
    println!("  route <规则文件> <host> [ip]               按规则文件求值路由动作");
    println!("  probe [--port P] [--url U] [--ip-api U]    探测本地 SOCKS5：握手/延迟/出口 IP");
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

// ───────────────────────── memory ─────────────────────────

fn print_memory_help() {
    println!("用法: gsn memory <操作> [选项]");
    println!("操作:");
    println!("  append   追加一条记忆（--task-id 必填）");
    println!("  list     列出记忆（当前 + 滚动档案，可过滤）");
    println!("  recall   按条件检索（同 list 的过滤选项）");
    println!("  query    组合条件检索（同 list，末尾给出结论摘要）");
    println!("  status   显示容量状态（当前/滚动文件字节、上限）");
    println!("  profile  节点画像：从记忆聚合节点历史（样本/成功率/耗时/P95/能力）");
    println!("  digest   会话摘要：按 task_id 聚合条目（条目/成功率/耗时/gate/结论要点）");
    println!("  verify-chain  完整性哈希链校验（逐文件，报告旧格式/损坏行/断点）");
    println!("  fleet    集体记忆（facts/decisions/open problems + 角色化读写）");
    println!("  path     显示 memory.jsonl 路径");
    println!("\n选项:");
    println!("  --data-dir <目录>     节点数据目录（默认同 daemon: ~/.gsn/data）");
    println!("  --max-bytes <n>       单文件软上限覆盖（0=不限，仅本次调用）");
    println!("  --keep-rotated <n>    保留滚动文件数覆盖（仅本次调用）");
    println!("  --allow-secret        append 时关闭隐私 fail-closed（默认开启；风险自负）");
    println!("  --redact              append 命中 secret 时字段级脱敏后落盘（保留上下文）");
    println!("  --task-id <id>        任务标识（append 必填）");
    println!("  --node-id <id>        append 标注来源节点；profile 过滤到单节点");
    println!("  --gate <名称>         闸门/来源（如 ci）");
    println!("  --conclusion <文本>   结论");
    println!("  --tag <标签>          标签（可重复，多个为 AND）");
    println!("  --exit-code <n>       退出码（append 写入；list/recall/query 精确过滤）");
    println!("  --ms <n>              耗时毫秒");
    println!("  --stdout <文本>       stdout 末尾片段");
    println!("  --stderr <文本>       stderr 末尾片段");
    println!("  --contains <子串>     结论包含子串（list/recall/query）");
    println!("  --ok                  仅成功 exit=0（list/recall/query）");
    println!("  --failed              仅失败退出码非零（list/recall/query）");
    println!("  --since <RFC3339>     仅返回该时间之后（含边界）");
    println!("  --until <RFC3339>     仅返回该时间之前（不含边界）");
    println!("  --limit <n>           最多返回条数");
    println!("  --json                以 JSON 输出（不含摘要）");
    println!("\nfleet 子操作（v5.1.1 集体记忆）：");
    println!("  gsn memory fleet post --kind fact|decision|open_problem --role <角色> --author <id> --content <文本>");
    println!("  gsn memory fleet list [--kind <k>] [--role <角色>] [--archived] [--json]");
    println!("  gsn memory fleet status [--role <角色>]");
    println!("  gsn memory fleet curate --claim <hash> --role captain|admin");
    println!("  gsn memory fleet purge --role admin");
    println!("  gsn memory fleet export --cat7（导出 CAT7 七字段交换块 JSON）");
    println!("  gsn memory fleet import <blocks.json>（导入 CAT7 交换块，去重+完整性校验）");
    println!("  gsn memory fleet assess --blocks <blocks.json> --assessor <id> --accept <字段,..> --reject <字段,..>");
    println!("  gsn memory fleet lineage link --child <hash> --parent <hash>（另: ancestors/root/children/status）");
    println!("  角色: observer(读) < contributor(读+写) < captain(+策展) < admin(+管理)");
}

fn print_fleet_help() {
    println!("用法: gsn memory fleet <子操作> [选项]（v5.1.1 集体记忆基座 + CAT7 交换）");
    println!("子操作:");
    println!("  post     发布集体记忆（--kind/--author/--content 必填，需 contributor+）");
    println!("  list     列出集体记忆（可 --kind 过滤；--archived 含归档；需 observer+）");
    println!("  status   显示策展统计与文件字节（需 observer+）");
    println!("  curate   策展归档（--claim <hash>，需 captain+）");
    println!("  purge    清空集体记忆（需 admin）");
    println!("  export   导出 CAT7 七字段交换块（--cat7 标记；JSON 数组输出）");
    println!("  import   导入 CAT7 交换块（<blocks.json> 路径；逐块校验+按 claim 去重）");
    println!(
        "  assess   SVAF 字段级评估（--blocks/--assessor/--accept/--reject；逐字段接受/拒绝）"
    );
    println!(
        "  lineage  血缘（link/ancestors/root/children/status；--child/--parent/--claim；写入 lineage.jsonl）"
    );
    println!("选项:");
    println!("  --kind fact|decision|open_problem   策展分类");
    println!("  --role observer|contributor|captain|admin   执行者角色（默认 observer）");
    println!("  --author <id>       发布者 agent_id");
    println!("  --content <文本>    记忆内容");
    println!("  --claim <hash>      curate 目标 claim_hash");
    println!("  --assessor <id>     assess 评估方（接收方）agent_id");
    println!("  --accept <字段,..>  assess 显式接受字段（kind/author/content/source/confidence/created_at）");
    println!("  --reject <字段,..>  assess 显式拒绝字段（reject 覆盖 accept）");
    println!("  --blocks <路径>     assess 待评估交换块文件");
    println!("  --archived          list 时包含已归档条目");
    println!("  --data-dir <目录>   节点数据目录（默认 ~/.gsn/data）");
    println!("  --json              以 JSON 输出");
}

/// 解析 SVAF 逗号分隔字段列表（未知字段 → Err(该字段名)，调用方 fail-closed）。
fn parse_svaf_fields(csv: &str) -> Result<Vec<Cat7Field>, String> {
    let mut out = Vec::new();
    for part in csv.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match Cat7Field::parse(p) {
            Some(f) => {
                if !out.contains(&f) {
                    out.push(f);
                }
            }
            None => return Err(p.to_string()),
        }
    }
    Ok(out)
}

/// 取 `--name` 后的值。
fn mem_value_after(rest: &[String], i: usize, name: &str) -> Result<String, String> {
    rest.get(i + 1)
        .cloned()
        .ok_or_else(|| format!("{name} 需要一个参数"))
}

/// v5.0.6：从参数列表取 `--name <value>`（无则 None；只取第一次出现）。
fn mem_flag_value(rest: &[String], name: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < rest.len() {
        if rest[i] == name {
            return Some(rest[i + 1].clone());
        }
        i += 1;
    }
    None
}

/// 解析 memory.jsonl 路径（默认 data-dir/memory.jsonl）。
fn resolve_memory_path(rest: &[String]) -> Result<std::path::PathBuf, String> {
    let mut data_dir = node::default_data_dir();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--data-dir" {
            data_dir = std::path::PathBuf::from(mem_value_after(rest, i, "--data-dir")?);
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(node::expand_tilde(data_dir).join("memory.jsonl"))
}

/// 解析 list/recall 的检索条件，返回 (query, json 输出)。
fn parse_memory_query(rest: &[String]) -> Result<(gsn_core::MemoryQuery, bool), String> {
    use gsn_core::MemoryQuery;
    let mut q = MemoryQuery::new();
    let mut json_out = false;
    let mut i = 0;
    while i < rest.len() {
        let name = rest[i].as_str();
        match name {
            "--data-dir" => i += 2,
            "--max-bytes" | "--keep-rotated" => i += 2,
            "--json" => {
                json_out = true;
                i += 1;
            }
            "--all" => i += 1, // query 已含滚动档案；保留兼容
            "--task-id" => {
                q = q.task_id(mem_value_after(rest, i, name)?);
                i += 2;
            }
            "--tag" => {
                q = q.tag(mem_value_after(rest, i, name)?);
                i += 2;
            }
            "--gate" => {
                q = q.gate(mem_value_after(rest, i, name)?);
                i += 2;
            }
            "--contains" => {
                q = q.conclusion_contains(mem_value_after(rest, i, name)?);
                i += 2;
            }
            "--exit-code" => {
                let v = mem_value_after(rest, i, name)?;
                let n = v
                    .parse()
                    .map_err(|_| format!("--exit-code 需要整数，得到 {v}"))?;
                q = q.exit_code(n);
                i += 2;
            }
            "--ok" => {
                q = q.ok(true);
                i += 1;
            }
            "--failed" => {
                q = q.ok(false);
                i += 1;
            }
            "--since" => {
                q = q.since(mem_value_after(rest, i, name)?);
                i += 2;
            }
            "--until" => {
                q = q.until(mem_value_after(rest, i, name)?);
                i += 2;
            }
            "--limit" => {
                let v = mem_value_after(rest, i, name)?;
                let n = v
                    .parse()
                    .map_err(|_| format!("--limit 需要数字，得到 {v}"))?;
                q = q.limit(n);
                i += 2;
            }
            other => return Err(format!("未知选项 '{other}'")),
        }
    }
    Ok((q, json_out))
}

fn print_memory_entries(entries: &[gsn_core::MemoryEntry], json_out: bool) {
    if json_out {
        println!(
            "{}",
            serde_json::to_string(entries).unwrap_or_else(|_| "[]".to_string())
        );
        return;
    }
    if entries.is_empty() {
        println!("（无匹配记忆）");
        return;
    }
    for e in entries {
        println!(
            "● [{}] {} ({})",
            e.at,
            e.task_id,
            e.gate.as_deref().unwrap_or("-")
        );
        if let Some(c) = &e.conclusion {
            println!("    结论: {c}");
        }
        if !e.tags.is_empty() {
            println!("    标签: {}", e.tags.join(", "));
        }
        if let Some(c) = e.exit_code {
            println!("    退出码: {c}");
        }
        if let Some(m) = e.ms {
            println!("    耗时: {m} ms");
        }
    }
}

/// 输出结论摘要：总数 / 成功 / 失败 / 无退出码 / 总耗时。
fn print_memory_summary(entries: &[gsn_core::MemoryEntry]) {
    let total = entries.len();
    let ok = entries.iter().filter(|e| e.exit_code == Some(0)).count();
    let failed = entries
        .iter()
        .filter(|e| matches!(e.exit_code, Some(c) if c != 0))
        .count();
    let no_code = entries.iter().filter(|e| e.exit_code.is_none()).count();
    let total_ms: u64 = entries.iter().filter_map(|e| e.ms).sum();
    println!("── 摘要 ──");
    println!("匹配 {total} 条：成功 {ok}、失败 {failed}、无退出码 {no_code}；总耗时 {total_ms} ms");
}

/// 扫描容量覆盖参数 `--max-bytes <n>` / `--keep-rotated <n>`。
fn parse_memory_capacity(rest: &[String]) -> Result<(Option<u64>, Option<usize>), String> {
    let mut max_bytes: Option<u64> = None;
    let mut keep_rotated: Option<usize> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--max-bytes" => {
                let v = mem_value_after(rest, i, "--max-bytes")?;
                max_bytes = Some(
                    v.parse()
                        .map_err(|_| format!("--max-bytes 需要数字，得到 {v}"))?,
                );
                i += 2;
            }
            "--keep-rotated" => {
                let v = mem_value_after(rest, i, "--keep-rotated")?;
                keep_rotated = Some(
                    v.parse()
                        .map_err(|_| format!("--keep-rotated 需要数字，得到 {v}"))?,
                );
                i += 2;
            }
            _ => i += 1,
        }
    }
    Ok((max_bytes, keep_rotated))
}

/// 以人类可读方式展示记忆存储容量状态。
fn print_memory_status(s: &gsn_core::MemoryStorageStatus) {
    println!("记忆存储容量");
    println!("路径: {}", s.path);
    println!(
        "当前文件: {} 字节 / {} 条（单文件上限 {}）",
        s.current_bytes, s.current_entries, s.max_bytes
    );
    if s.rotated.is_empty() {
        println!("滚动文件: 无（保留 {} 个）", s.keep_rotated);
    } else {
        println!("滚动文件（保留 {} 个）：", s.keep_rotated);
        for r in &s.rotated {
            println!("  memory.{}.jsonl: {} 字节", r.index, r.bytes);
        }
    }
    match s.disk_bound_bytes {
        Some(bound) => println!("磁盘合计 {} 字节 / 硬上限 {}", s.total_bytes, bound),
        None => println!("磁盘合计 {} 字节（未设上限）", s.total_bytes),
    }
    println!("tail 字段最大字符: {}", s.tail_max_chars);
}

async fn run_memory(args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_memory_help();
        return if args.is_empty() { 1 } else { 0 };
    }
    let op = args[0].as_str();
    let rest: &[String] = &args[1..];

    let path = match resolve_memory_path(rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("参数错误: {e}");
            return 2;
        }
    };
    let mut store = gsn_core::PersistentMemoryStore::open(&path);
    // 容量覆盖（仅影响本次 CLI 调用）：--max-bytes / --keep-rotated。
    let (max_bytes_override, keep_rotated_override) = match parse_memory_capacity(rest) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("参数错误: {e}");
            return 2;
        }
    };
    if let Some(mb) = max_bytes_override {
        store = store.max_bytes(mb);
    }
    if let Some(kr) = keep_rotated_override {
        store = store.keep_rotated(kr);
    }
    // 隐私审核默认开启（fail-closed）；仅当显式传入 --allow-secret 才关闭（风险自负）。
    if rest.iter().any(|a| a == "--allow-secret") {
        store = store.privacy_audit(false);
    }
    // v5.0.5：--redact 命中 secret 改为字段级脱敏后落盘（保留上下文，加 audit-redacted）。
    // 若同时给 --allow-secret（已关闭审核），则 --allow-secret 优先，redact 不生效。
    if rest.iter().any(|a| a == "--redact") {
        store = store.redact_secrets(true);
    }

    match op {
        "path" => {
            println!("{}", path.display());
            0
        }
        "status" => {
            let json_out = rest.iter().any(|a| a == "--json");
            match store.status() {
                Ok(s) => {
                    if json_out {
                        println!("{}", serde_json::to_string_pretty(&s).unwrap_or_default());
                    } else {
                        print_memory_status(&s);
                    }
                    0
                }
                Err(err) => {
                    eprintln!("读取状态失败: {err}");
                    1
                }
            }
        }
        "append" => {
            let mut task_id: Option<String> = None;
            let mut node_id: Option<String> = None;
            let mut gate: Option<String> = None;
            let mut conclusion: Option<String> = None;
            let mut tags: Vec<String> = Vec::new();
            let mut exit_code: Option<i32> = None;
            let mut ms: Option<u64> = None;
            let mut stdout: Option<String> = None;
            let mut stderr: Option<String> = None;
            let mut parse_err: Option<String> = None;
            let mut i = 0;
            while i < rest.len() {
                let name = rest[i].as_str();
                match name {
                    "--data-dir" => i += 2,
                    "--max-bytes" | "--keep-rotated" => i += 2,
                    "--allow-secret" | "--redact" => i += 1,
                    "--task-id" => match mem_value_after(rest, i, name) {
                        Ok(v) => {
                            task_id = Some(v);
                            i += 2;
                        }
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    "--node-id" => match mem_value_after(rest, i, name) {
                        Ok(v) => {
                            node_id = Some(v);
                            i += 2;
                        }
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    "--gate" => match mem_value_after(rest, i, name) {
                        Ok(v) => {
                            gate = Some(v);
                            i += 2;
                        }
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    "--conclusion" => match mem_value_after(rest, i, name) {
                        Ok(v) => {
                            conclusion = Some(v);
                            i += 2;
                        }
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    "--tag" => match mem_value_after(rest, i, name) {
                        Ok(v) => {
                            tags.push(v);
                            i += 2;
                        }
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    "--exit-code" => match mem_value_after(rest, i, name) {
                        Ok(v) => match v.parse() {
                            Ok(n) => {
                                exit_code = Some(n);
                                i += 2;
                            }
                            Err(_) => {
                                parse_err = Some(format!("--exit-code 需要整数，得到 {v}"));
                                i += 1;
                            }
                        },
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    "--ms" => match mem_value_after(rest, i, name) {
                        Ok(v) => match v.parse() {
                            Ok(n) => {
                                ms = Some(n);
                                i += 2;
                            }
                            Err(_) => {
                                parse_err = Some(format!("--ms 需要数字，得到 {v}"));
                                i += 1;
                            }
                        },
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    "--stdout" => match mem_value_after(rest, i, name) {
                        Ok(v) => {
                            stdout = Some(v);
                            i += 2;
                        }
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    "--stderr" => match mem_value_after(rest, i, name) {
                        Ok(v) => {
                            stderr = Some(v);
                            i += 2;
                        }
                        Err(e) => {
                            parse_err = Some(e);
                            i += 1;
                        }
                    },
                    other => {
                        parse_err = Some(format!("未知选项 '{other}'"));
                        i += 1;
                    }
                }
                if parse_err.is_some() {
                    break;
                }
            }
            if let Some(e) = parse_err {
                eprintln!("参数错误: {e}");
                return 2;
            }
            let tid = match task_id {
                Some(t) => t,
                None => {
                    eprintln!("参数错误: append 需要 --task-id <id>");
                    return 2;
                }
            };
            let mut e = gsn_core::MemoryEntry::new(tid);
            if let Some(n) = node_id {
                e = e.node_id(n);
            }
            if let Some(g) = gate {
                e = e.gate(g);
            }
            if let Some(c) = exit_code {
                e = e.exit_code(c);
            }
            if let Some(m) = ms {
                e = e.ms(m);
            }
            if let Some(s) = stdout {
                e = e.stdout_tail(s);
            }
            if let Some(s) = stderr {
                e = e.stderr_tail(s);
            }
            if let Some(c) = conclusion {
                e = e.conclusion(c);
            }
            if !tags.is_empty() {
                e = e.tags(tags);
            }
            match store.append(e) {
                Ok(()) => {
                    println!("已追加到 {}", path.display());
                    0
                }
                Err(gsn_core::MemoryError::SecretRefused { field, kind }) => {
                    eprintln!(
                        "拒绝写入（隐私审核 fail-closed）：{field} 命中疑似凭据（{kind}）。\n\
                         明文未落盘。若你确认内容安全并坚持写入，可显式加 --allow-secret（风险自负）。"
                    );
                    3
                }
                Err(err) => {
                    eprintln!("追加失败: {err}");
                    1
                }
            }
        }
        "list" | "recall" | "query" => match parse_memory_query(rest) {
            Ok((q, json_out)) => match store.query(&q) {
                Ok(entries) => {
                    print_memory_entries(&entries, json_out);
                    if !json_out {
                        print_memory_summary(&entries);
                    }
                    0
                }
                Err(err) => {
                    eprintln!("读取失败: {err}");
                    1
                }
            },
            Err(e) => {
                eprintln!("参数错误: {e}");
                2
            }
        },
        // v5.0.6：节点画像——从记忆聚合节点历史（样本/成功率/耗时/P95/观察能力）。
        "profile" => match store.load_all() {
            Ok(entries) => {
                let filter = mem_flag_value(rest, "--node-id");
                let mut nodes: Vec<(String, gsn_core::NodeProfile)> = Vec::new();
                for e in &entries {
                    let Some(nid) = e.node_id.as_deref() else {
                        continue;
                    };
                    if let Some(f) = &filter {
                        if nid != f {
                            continue;
                        }
                    }
                    if !nodes.iter().any(|(id, _)| id == nid) {
                        let p = gsn_core::NodeProfile::new(nid, gsn_core::FleetRole::Unknown)
                            .aggregate_from_entries(&entries);
                        nodes.push((nid.to_string(), p));
                    }
                }
                nodes.sort_by(|a, b| a.0.cmp(&b.0));
                if nodes.is_empty() {
                    let hint = "（无带 node_id 的记忆条目；可 append --node-id <id> 或由 daemon --node-id 自动标注）";
                    println!("{}", hint);
                } else {
                    let hdr = format!(
                        "{:<16} {:>6} {:>8} {:>10} {:>8}  {}",
                        "node_id", "样本数", "成功率", "avg_ms", "p95_ms", "观察能力"
                    );
                    println!("{}", hdr);
                    for (nid, p) in &nodes {
                        let rate = p
                            .success_rate
                            .map(|r| format!("{:.0}%", r * 100.0))
                            .unwrap_or_else(|| "-".to_string());
                        let avg = p
                            .avg_ms
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let p95 = p
                            .p95_ms
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let caps = p.observed_capabilities.join(",");
                        println!(
                            "{:<16} {:>6} {:>8} {:>10} {:>8}  {}",
                            nid, p.history_n, rate, avg, p95, caps
                        );
                    }
                }
                0
            }
            Err(err) => {
                eprintln!("读取失败: {err}");
                1
            }
        },
        "digest" => match store.load_all() {
            Ok(entries) => {
                let tid_filter = mem_flag_value(rest, "--task-id");
                let nid_filter = mem_flag_value(rest, "--node-id");
                let limit = match mem_flag_value(rest, "--limit") {
                    Some(v) => match v.parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            eprintln!("参数错误: --limit 需要数字，得到 {v}");
                            return 2;
                        }
                    },
                    None => None,
                };
                let filtered: Vec<gsn_core::MemoryEntry> = entries
                    .into_iter()
                    .filter(|e| {
                        if let Some(t) = &tid_filter {
                            if e.task_id != *t {
                                return false;
                            }
                        }
                        if let Some(n) = &nid_filter {
                            if e.node_id.as_deref() != Some(n.as_str()) {
                                return false;
                            }
                        }
                        true
                    })
                    .collect();
                let digests = gsn_core::digest_sessions(&filtered);
                let json_out = rest.iter().any(|a| a == "--json");
                if json_out {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&digests).unwrap_or_default()
                    );
                } else if digests.is_empty() {
                    println!("（无匹配的记忆条目，无法生成会话摘要）");
                } else {
                    let hdr = format!(
                        "{:<20} {:>5} {:>7} {:>7} {:>7}  {:>6}  {}",
                        "task_id", "条目", "成功率", "avg_ms", "max_ms", "gates", "结论要点"
                    );
                    println!("{hdr}");
                    let shown = limit.map_or(digests.len(), |l| l.min(digests.len()));
                    for d in &digests[..shown] {
                        let rate = d
                            .success_rate
                            .map(|r| format!("{:.0}%", r * 100.0))
                            .unwrap_or_else(|| "-".to_string());
                        let avg = d
                            .avg_ms
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let max = d
                            .max_ms
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let gates = if d.gates.is_empty() {
                            "-".to_string()
                        } else {
                            d.gates.join(",")
                        };
                        let concl = if d.conclusions.is_empty() {
                            "-".to_string()
                        } else {
                            d.conclusions.join(" | ")
                        };
                        println!(
                            "{:<20} {:>5} {:>7} {:>7} {:>7}  {:>6}  {}",
                            d.task_id, d.entries, rate, avg, max, gates, concl
                        );
                    }
                }
                0
            }
            Err(err) => {
                eprintln!("读取失败: {err}");
                1
            }
        },
        "verify-chain" => match store.verify_chain() {
            Ok(reports) => {
                let json_out = rest.iter().any(|a| a == "--json");
                if json_out {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&reports).unwrap_or_default()
                    );
                } else {
                    let header = format!(
                        "{:<44} {:>6} {:>6} {:>6}  {:>6}  {}",
                        "file", "条目", "旧格式", "损坏行", "断点", "状态"
                    );
                    println!("{header}");
                    let mut all_ok = true;
                    for r in &reports {
                        if !r.ok {
                            all_ok = false;
                        }
                        let status = if r.ok { "OK" } else { "损坏" };
                        let broken = r
                            .broken_at
                            .map(|b| b.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        println!(
                            "{:<44} {:>6} {:>6} {:>6}  {:>6}  {}",
                            r.path, r.entries, r.unsigned_entries, r.corrupt_lines, broken, status
                        );
                    }
                    if all_ok {
                        let ok_line = "哈希链完整：未发现篡改/损坏证据";
                        println!("{ok_line}");
                    } else {
                        eprintln!("哈希链校验未通过：存在损坏行或链断点（见上表）");
                    }
                }
                if reports.iter().all(|r| r.ok) {
                    0
                } else {
                    1
                }
            }
            Err(err) => {
                eprintln!("读取失败: {err}");
                1
            }
        },
        "fleet" => {
            // v5.1.0 集体记忆基座：facts/decisions/open problems + 角色化读写。
            // 注意：run_memory 已把 op（"fleet"）从 rest 去除，rest[0] 即子操作。
            if rest.is_empty() || rest[0] == "--help" || rest[0] == "help" {
                print_fleet_help();
                return 0;
            }
            let data_dir = match mem_flag_value(rest, "--data-dir") {
                Some(d) => std::path::PathBuf::from(d),
                None => std::env::home_dir().unwrap_or_default().join(".gsn/data"),
            };
            let fleet_path = data_dir.join("fleet-memory.jsonl");
            let role = match mem_flag_value(rest, "--role")
                .map(|s| FleetAccess::parse(&s))
                .unwrap_or(Some(FleetAccess::Observer))
            {
                Some(r) => r,
                None => {
                    eprintln!("错误: 未知角色（支持: observer/contributor/captain/admin）");
                    return 1;
                }
            };
            let sub = rest[0].as_str();
            let mut fm = match FleetMemory::open(&fleet_path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("打开集体记忆失败: {e}");
                    return 1;
                }
            };
            match sub {
                "post" => {
                    let kind =
                        match mem_flag_value(rest, "--kind").and_then(|s| FleetKind::parse(&s)) {
                            Some(k) => k,
                            None => {
                                eprintln!("错误: --kind 必填（fact|decision|open_problem）");
                                return 1;
                            }
                        };
                    let author = match mem_flag_value(rest, "--author") {
                        Some(a) => a,
                        None => {
                            eprintln!("错误: --author 必填");
                            return 1;
                        }
                    };
                    let content = match mem_flag_value(rest, "--content") {
                        Some(c) => c,
                        None => {
                            eprintln!("错误: --content 必填");
                            return 1;
                        }
                    };
                    match fm.post(role, kind, &author, &content) {
                        Ok(e) => {
                            if rest.iter().any(|a| a == "--json") {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&e).unwrap_or_default()
                                );
                            } else {
                                println!(
                                    "已发布集体记忆: kind={} claim={}",
                                    e.kind.as_str(),
                                    &e.claim_hash[..16]
                                );
                            }
                            0
                        }
                        Err(e) => {
                            eprintln!("发布失败: {e}");
                            1
                        }
                    }
                }
                "list" => {
                    let kind = mem_flag_value(rest, "--kind").and_then(|s| FleetKind::parse(&s));
                    let include_archived = rest.iter().any(|a| a == "--archived");
                    match fm.list(role, kind, include_archived) {
                        Ok(rows) => {
                            if rest.iter().any(|a| a == "--json") {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&rows).unwrap_or_default()
                                );
                            } else {
                                let header = format!(
                                    "{:<12} {:<22} {:<9} {:<18}  {}",
                                    "kind", "author", "status", "claim(前16)", "content"
                                );
                                println!("{header}");
                                for e in &rows {
                                    println!(
                                        "{:<12} {:<22} {:<9} {:<18}  {}",
                                        e.kind.as_str(),
                                        e.author,
                                        format!("{:?}", e.status),
                                        &e.claim_hash[..16],
                                        e.content
                                    );
                                }
                            }
                            0
                        }
                        Err(e) => {
                            eprintln!("读取失败: {e}");
                            1
                        }
                    }
                }
                "status" => match fm.status(role) {
                    Ok(st) => {
                        if rest.iter().any(|a| a == "--json") {
                            println!("{}", serde_json::to_string_pretty(&st).unwrap_or_default());
                        } else {
                            println!("集体记忆文件: {}", st.file);
                            println!(
                                "条目: {}（活跃 {} / 归档 {}）",
                                st.entries, st.active, st.archived
                            );
                            println!(
                                "策展: facts={} decisions={} open_problems={}",
                                st.facts, st.decisions, st.open_problems
                            );
                            println!("字节: {}（损坏行: {}）", st.file_bytes, st.corrupt_lines);
                        }
                        0
                    }
                    Err(e) => {
                        eprintln!("读取失败: {e}");
                        1
                    }
                },
                "curate" => {
                    let claim = match mem_flag_value(rest, "--claim") {
                        Some(c) => c,
                        None => {
                            eprintln!("错误: --claim 必填");
                            return 1;
                        }
                    };
                    match fm.curate(role, &claim) {
                        Ok(e) => {
                            println!(
                                "已归档: kind={} claim={}",
                                e.kind.as_str(),
                                &e.claim_hash[..16]
                            );
                            0
                        }
                        Err(e) => {
                            eprintln!("策展失败: {e}");
                            1
                        }
                    }
                }
                "purge" => match fm.purge(role) {
                    Ok(()) => {
                        println!("集体记忆已清空");
                        0
                    }
                    Err(e) => {
                        eprintln!("清空失败: {e}");
                        1
                    }
                },
                "export" => {
                    // v5.1.1：导出 CAT7 七字段交换块（跨 Agent 记忆交换标准单元）。
                    let blocks = fm.export_cat7();
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&blocks).unwrap_or_default()
                    );
                    0
                }
                "import" => {
                    // v5.1.1：导入 CAT7 交换块（--data-dir 指向本机集体记忆）。
                    if rest.len() < 2 {
                        eprintln!("错误: import 需提供 <blocks.json> 路径");
                        return 1;
                    }
                    let path = rest[1].to_string();
                    let text = match std::fs::read_to_string(&path) {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("读取交换块文件失败: {e}");
                            return 1;
                        }
                    };
                    let blocks: Vec<Cat7Block> = match serde_json::from_str(&text) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("解析 CAT7 交换块失败: {e}");
                            return 1;
                        }
                    };
                    match fm.import_cat7(&blocks) {
                        Ok((added, skipped)) => {
                            println!("导入完成: 新增 {added} 条，跳过 {skipped} 条（已存在）");
                            0
                        }
                        Err(e) => {
                            eprintln!("导入失败: {e}");
                            1
                        }
                    }
                }
                "assess" => {
                    // v5.1.2：SVAF 字段级评估——逐字段接受/拒绝，而非整条接受/拒绝。
                    let blocks_path = match mem_flag_value(rest, "--blocks") {
                        Some(p) => p,
                        None => {
                            eprintln!("错误: assess 需用 --blocks <blocks.json> 指定交换块");
                            return 1;
                        }
                    };
                    let assessor = match mem_flag_value(rest, "--assessor") {
                        Some(a) => a,
                        None => {
                            eprintln!("错误: assess 需用 --assessor <id> 指定评估方");
                            return 1;
                        }
                    };
                    let accept_fields = match mem_flag_value(rest, "--accept")
                        .map(|s| parse_svaf_fields(&s))
                        .transpose()
                    {
                        Ok(v) => v.unwrap_or_default(),
                        Err(bad) => {
                            eprintln!(
                                "错误: --accept 含未知字段 '{bad}'（支持: kind/author/content/source/confidence/created_at）"
                            );
                            return 1;
                        }
                    };
                    let reject_fields = match mem_flag_value(rest, "--reject")
                        .map(|s| parse_svaf_fields(&s))
                        .transpose()
                    {
                        Ok(v) => v.unwrap_or_default(),
                        Err(bad) => {
                            eprintln!("错误: --reject 含未知字段 '{bad}'");
                            return 1;
                        }
                    };
                    let text = match std::fs::read_to_string(&blocks_path) {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("读取交换块文件失败: {e}");
                            return 1;
                        }
                    };
                    let raw_blocks: Vec<Cat7Block> = match serde_json::from_str(&text) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("解析 CAT7 交换块失败: {e}");
                            return 1;
                        }
                    };
                    let mut full = 0usize;
                    let mut partial = 0usize;
                    let mut rejected = 0usize;
                    let mut adopted_blocks: Vec<Cat7Block> = Vec::new();
                    for b in &raw_blocks {
                        // 前置完整性校验（篡改块不能进入评估，整批 fail-closed）。
                        let mut assessment = match SvafAssessment::for_block(b, &assessor) {
                            Ok(a) => a,
                            Err(e) => {
                                eprintln!("评估失败（块未通过完整性校验，fail-closed）: {e}");
                                return 1;
                            }
                        };
                        for f in &accept_fields {
                            assessment.set(*f, FieldVerdict::Accept);
                        }
                        for f in &reject_fields {
                            // reject 后设，覆盖 accept。
                            assessment.set(*f, FieldVerdict::Reject);
                        }
                        let decision = assessment.decision();
                        if decision == SvafDecision::Reject {
                            rejected += 1;
                            continue;
                        }
                        match assessment.apply(b) {
                            Ok(Some(adopted)) => {
                                // 以 SvafDecision 为准计数（PartialAccept 即使字段值因
                                // None/同秒巧合相同，也仍是部分接受）。
                                if decision == SvafDecision::AcceptAll {
                                    full += 1;
                                } else {
                                    partial += 1;
                                }
                                adopted_blocks.push(adopted);
                            }
                            Ok(None) => {
                                // decision 非 reject 时不应为 None：防御性 fail-closed。
                                eprintln!("评估内部不一致，拒绝整批");
                                return 1;
                            }
                            Err(e) => {
                                eprintln!("应用评估失败: {e}");
                                return 1;
                            }
                        }
                    }
                    // 导入处理后块（剥离块仍通过 verify；import_cat7 再做一次去重）。
                    let (added, skipped) = if adopted_blocks.is_empty() {
                        (0usize, 0usize)
                    } else {
                        match fm.import_cat7(&adopted_blocks) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("导入失败: {e}");
                                return 1;
                            }
                        }
                    };
                    println!(
                        "字段级评估完成（评估方 {assessor}）：全接受 {full}，部分接受 {partial}（已剥离未接受字段），拒绝 {rejected}"
                    );
                    println!("导入完成: 新增 {added} 条，跳过 {skipped} 条（已存在）");
                    0
                }
                "lineage" => {
                    // v5.1.3：内容哈希键的父/祖先链（lineage.jsonl），独立于 fleet-memory。
                    // 调用形如 gsn memory fleet lineage <link|ancestors|root|children|status>。
                    let lineage_path = data_dir.join("lineage.jsonl");
                    let lsub = match rest.get(1) {
                        Some(s) => s.as_str(),
                        None => {
                            eprintln!(
                                "错误: lineage 需指定子操作（link/ancestors/root/children/status）"
                            );
                            return 1;
                        }
                    };
                    let mut g = match LineageGraph::open(&lineage_path) {
                        Ok(g) => g,
                        Err(e) => {
                            eprintln!("打开血缘库失败: {e}");
                            return 1;
                        }
                    };
                    match lsub {
                        "link" => {
                            let child = match mem_flag_value(rest, "--child") {
                                Some(c) => c,
                                None => {
                                    eprintln!("错误: link 需 --child <claim_hash>");
                                    return 1;
                                }
                            };
                            let parent = match mem_flag_value(rest, "--parent") {
                                Some(p) => p,
                                None => {
                                    eprintln!("错误: link 需 --parent <claim_hash>");
                                    return 1;
                                }
                            };
                            if let Err(e) = g.link(&child, &parent) {
                                eprintln!("建立血缘失败: {e}");
                                return 1;
                            }
                            if let Err(e) = g.save() {
                                eprintln!("保存血缘失败: {e}");
                                return 1;
                            }
                            println!("已建立血缘: {child} 派生自 {parent}");
                            0
                        }
                        "ancestors" => {
                            let claim = match mem_flag_value(rest, "--claim") {
                                Some(c) => c,
                                None => {
                                    eprintln!("错误: ancestors 需 --claim <hash>");
                                    return 1;
                                }
                            };
                            match g.ancestors(&claim) {
                                Ok(anc) => {
                                    if mem_flag_value(rest, "--json").is_some() {
                                        println!("{}", serde_json::to_string(&anc).unwrap());
                                    } else {
                                        println!("祖先链（最近在前，共 {} 代）：", anc.len());
                                        for (i, h) in anc.iter().enumerate() {
                                            println!("  {}. {h}", i + 1);
                                        }
                                    }
                                    0
                                }
                                Err(e) => {
                                    eprintln!("查询祖先失败: {e}");
                                    1
                                }
                            }
                        }
                        "root" => {
                            let claim = match mem_flag_value(rest, "--claim") {
                                Some(c) => c,
                                None => {
                                    eprintln!("错误: root 需 --claim <hash>");
                                    return 1;
                                }
                            };
                            match g.root(&claim) {
                                Ok(r) => {
                                    println!("根节点: {r}");
                                    0
                                }
                                Err(e) => {
                                    eprintln!("查询根失败: {e}");
                                    1
                                }
                            }
                        }
                        "children" => {
                            let claim = match mem_flag_value(rest, "--claim") {
                                Some(c) => c,
                                None => {
                                    eprintln!("错误: children 需 --claim <hash>");
                                    return 1;
                                }
                            };
                            let kids = g.children_of(&claim);
                            println!("直接子节点（{} 个）：", kids.len());
                            for k in &kids {
                                println!("  {k}");
                            }
                            0
                        }
                        "status" => {
                            let recs = g.records();
                            let mut nodes = std::collections::HashSet::new();
                            for r in &recs {
                                nodes.insert(r.child.clone());
                                nodes.insert(r.parent.clone());
                            }
                            println!("血缘库: {}", lineage_path.display());
                            println!("边数: {}，涉及节点: {}", recs.len(), nodes.len());
                            0
                        }
                        other_lsub => {
                            eprintln!(
                                "错误: 未知 lineage 子操作 '{other_lsub}'（link/ancestors/root/children/status）"
                            );
                            1
                        }
                    }
                }
                other_sub => {
                    eprintln!(
                        "错误: 未知 fleet 操作 '{other_sub}'（支持: post/list/status/curate/purge/export/import/help）"
                    );
                    1
                }
            }
        }
        other => {
            eprintln!(
                "错误: 未知 memory 操作 '{other}'（支持: append/list/recall/query/status/profile/digest/verify-chain/fleet/path）"
            );
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
