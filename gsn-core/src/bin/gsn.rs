//! gsn - Agent Universe 统一命令行入口
//!
//! 子命令：
//!   gsn version                         版本信息
//!   gsn daemon [选项]                   启动全节点（同 gsn-daemon）
//!   gsn mcp [--transport stdio]         启动 MCP 服务器（默认 stdio）
//!   gsn market <操作> [参数]            通过 HTTP API 操作智能体市场
//!   gsn identity                        生成本地身份（Ed25519）
//!
//! market 子命令通过 --api <url> 或 GSN_API 环境变量连接 daemon，
//! 默认 http://127.0.0.1:4002。

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
            println!("agent-universe v2.5.5");
            0
        }
        "help" | "--help" | "-h" => {
            print_top_help();
            0
        }
        "daemon" => {
            let args = node::parse_daemon_args(&argv[1..]);
            match node::run_daemon(args).await {
                Ok(_) => 0,
                Err(e) => { eprintln!("daemon 错误: {e}"); 1 }
            }
        }
        "mcp" => run_mcp(&argv[1..]).await,
        "market" => run_market(&argv[1..]).await,
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
    println!("  identity            生成 Ed25519 本地身份");
    println!("  version             显示版本");
    println!("  help                显示本帮助\n");
    println!("market 操作: deposit/register/search/discover/publish/bid/");
    println!("             match/result/verify/settle/dispute/arbitrate/");
    println!("             balance/conservation/leaderboard/stats");
    println!("             （用 --api <url> 或 GSN_API 指定节点，默认 127.0.0.1:4002）");
}

// ───────────────────────── MCP ─────────────────────────

async fn run_mcp(args: &[String]) -> i32 {
    let mut transport = "stdio";
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--transport" => {
                if i + 1 < args.len() { transport = args[i + 1].as_str(); i += 2; } else { i += 1; }
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
            Err(e) => { eprintln!("mcp 错误: {e}"); 1 }
        },
        other => {
            eprintln!("错误: 不支持的 MCP 传输 '{other}'（远程请用节点的 http://<host>:4002/api/v1/mcp）");
            1
        }
    }
}

// ───────────────────────── identity ─────────────────────────

fn run_identity() -> i32 {
    let keypair = Keypair::generate();
    let pubkey_hex: String = keypair.public_key().iter().map(|b| format!("{:02x}", b)).collect();
    // peer id：公钥 SHA256 的前 16 字节 hex
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(keypair.public_key());
    let peer_id: String = digest.iter().take(16).map(|b| format!("{:02x}", b)).collect();
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

    // 解析 --api
    let mut api = std::env::var("GSN_API").unwrap_or_else(|_| "http://127.0.0.1:4002".to_string());
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--api" && i + 1 < args.len() {
            api = args[i + 1].clone();
            i += 2;
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }

    let op = positional[0].as_str();
    let params = &positional[1..];

    // 构造 (method, path, body) 请求
    let request = build_market_request(op, params);
    let (method, path, body) = match request {
        Some(r) => r,
        None => {
            eprintln!("错误: 无法构造 market {op} 请求（参数不足）");
            print_market_help();
            return 1;
        }
    };

    match http_call(&api, &method, &path, &body).await {
        Ok((status, text)) => {
            // 美化输出 JSON
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or(text)),
                Err(_) => println!("{}", text),
            }
            if (200..300).contains(&status) { 0 } else { 1 }
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
    println!("  deposit <account> <amount>          充值");
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
    println!("  verify <task_id> [approvals size]   QA 验证");
    println!("  settle <task_id>                    结算任务");
    println!("  dispute <dispute.json>              发起争议");
    println!("  arbitrate <dispute_id> <guilty> [slash]  仲裁");
    println!("  conservation                        守恒检查");
    println!("  leaderboard [limit]                 信誉排行榜");
    println!("  stats                               市场统计");
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

/// 构造市场请求三元组
#[allow(clippy::type_complexity)]
fn build_market_request(op: &str, p: &[String]) -> Option<(String, String, String)> {
    use serde_json::json;
    match op {
        "deposit" if p.len() >= 2 => Some((
            "POST".into(),
            format!("/api/v1/accounts/{}/deposit", p[0]),
            json!({"amount": p[1].parse::<f64>().unwrap_or(0.0)}).to_string(),
        )),
        "balance" if p.len() >= 1 => Some((
            "GET".into(), format!("/api/v1/accounts/{}/balance", p[0]), String::new(),
        )),
        "register" if p.len() >= 1 => Some((
            "POST".into(), "/api/v1/agents".into(), read_json_arg(&p[0]),
        )),
        "get" if p.len() >= 1 => Some((
            "GET".into(), format!("/api/v1/agents/{}", p[0]), String::new(),
        )),
        "discover" if p.len() >= 1 => Some((
            "GET".into(), format!("/api/v1/agents?skill={}", p[0]), String::new(),
        )),
        "search" if p.len() >= 1 => Some((
            "GET".into(), format!("/api/v1/agents?q={}", p[0]), String::new(),
        )),
        "publish" if p.len() >= 1 => Some((
            "POST".into(), "/api/v1/tasks".into(), read_json_arg(&p[0]),
        )),
        "task" if p.len() >= 1 => Some((
            "GET".into(), format!("/api/v1/tasks/{}", p[0]), String::new(),
        )),
        "bid" if p.len() >= 1 => {
            // bid JSON 需含 task_id；若只给 agent/task/price 则组装
            let body = read_json_arg(&p[0]);
            let task_id = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("task_id").and_then(|x| x.as_str()).map(String::from));
            if let Some(tid) = task_id {
                Some(("POST".into(), format!("/api/v1/tasks/{}/bids", tid), body))
            } else {
                None
            }
        }
        "match" if p.len() >= 1 => Some((
            "POST".into(), format!("/api/v1/tasks/{}/match", p[0]), String::new(),
        )),
        "result" if p.len() >= 1 => {
            let body = read_json_arg(&p[0]);
            let task_id = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("task_id").and_then(|x| x.as_str()).map(String::from));
            if let Some(tid) = task_id {
                Some(("POST".into(), format!("/api/v1/tasks/{}/results", tid), body))
            } else { None }
        }
        "verify" if p.len() >= 1 => {
            let approvals = p.get(1).map(|s| s.as_str()).unwrap_or("3");
            let size = p.get(2).map(|s| s.as_str()).unwrap_or("4");
            Some((
                "POST".into(),
                format!("/api/v1/tasks/{}/verify?approvals={}&committee_size={}", p[0], approvals, size),
                String::new(),
            ))
        }
        "settle" if p.len() >= 1 => Some((
            "POST".into(), format!("/api/v1/tasks/{}/settle", p[0]), String::new(),
        )),
        "dispute" if p.len() >= 1 => Some((
            "POST".into(), "/api/v1/disputes".into(), read_json_arg(&p[0]),
        )),
        "arbitrate" if p.len() >= 2 => {
            let guilty = p[1] == "guilty" || p[1] == "true";
            let slash = p.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            Some((
                "POST".into(),
                format!("/api/v1/disputes/{}/arbitrate", p[0]),
                json!({"guilty": guilty, "slash_amount": slash}).to_string(),
            ))
        }
        "conservation" => Some(("GET".into(), "/api/v1/conservation".into(), String::new())),
        "leaderboard" => {
            let limit = p.first().map(|s| s.as_str()).unwrap_or("10");
            Some(("GET".into(), format!("/api/v1/leaderboard?limit={}", limit), String::new()))
        }
        "stats" => Some(("GET".into(), "/api/v1/stats".into(), String::new())),
        _ => None,
    }
}

// ───────────────────────── 极简 HTTP 客户端 ─────────────────────────

async fn http_call(
    base: &str,
    method: &str,
    path: &str,
    body: &str,
) -> anyhow::Result<(u16, String)> {
    let base = base.trim_end_matches('/');
    // 解析 host:port
    let after_proto = base.split("://").nth(1).unwrap_or(base);
    let host_port = after_proto.split('/').next().unwrap_or("127.0.0.1:4002");
    let (host, port) = host_port.split_once(':').map(|(h, p)| (h, p.parse::<u16>().unwrap_or(80)))
        .unwrap_or((host_port, 80));

    let mut stream = tokio::net::TcpStream::connect((host, port)).await?;
    stream.set_nodelay(true)?;

    let req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        method, path, host_port, body.len(), body
    );
    tokio::io::AsyncWriteExt::write_all(&mut stream, req.as_bytes()).await?;
    tokio::io::AsyncWriteExt::flush(&mut stream).await?;

    // 读取完整响应（服务器 Connection: close，读到 EOF 为止）
    let mut response = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response).await?;

    let text = String::from_utf8_lossy(&response).to_string();
    let status = text.lines().next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    Ok((status, text[body_start..].to_string()))
}
