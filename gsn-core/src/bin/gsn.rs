//! gsn - Agent Universe 统一命令行入口
//!
//! 子命令：
//!   gsn version                         版本信息
//!   gsn daemon [选项]                   启动全节点（同 gsn-daemon）
//!   gsn mcp [--transport stdio]         启动 MCP 服务器（默认 stdio）
//!   gsn market <操作> [参数]            通过 HTTP API 操作智能体市场
//!   gsn identity                        生成本地身份（Ed25519）
//!   gsn ledger verify [--data-dir D]    离线核验账本哈希链 + 独立审计（v3.6.0）
//!   gsn doctor [--data-dir D] [--api U] 本机运维体检（版本/账本/daemon，v3.6.0）
//!
//! market 子命令通过 --api <url> 或 GSN_API 环境变量连接 daemon，
//! 默认 http://127.0.0.1:4002。
//!
//! 边界：`ledger verify` 是只读离线核验，校验流水自洽与哈希链（tamper-evident，
//! 非 tamper-proof：能整体重写数据库并连同锚点一起改的攻击者不可由本命令发现）。
//! 在线账实交叉归运行中的 daemon 负责。

use gsn_core::identity::Keypair;
use gsn_core::marketplace::{AuditReport, SettlementEngine};
use gsn_core::node;
use gsn_core::storage::PersistentStore;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
            println!("agent-universe v3.6.0");
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
                Err(e) => {
                    eprintln!("daemon 错误: {e}");
                    1
                }
            }
        }
        "mcp" => run_mcp(&argv[1..]).await,
        "market" => run_market(&argv[1..]).await,
        "identity" => run_identity(),
        "ledger" => run_ledger(&argv[1..]),
        "doctor" => run_doctor(&argv[1..]).await,
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
    println!("  ledger verify       离线核验账本哈希链与独立审计（--data-dir）");
    println!("  doctor              本机运维体检：版本/账本/daemon 健康");
    println!("  version             显示版本");
    println!("  help                显示本帮助\n");
    println!("market 操作: deposit/register/search/discover/publish/bid/");
    println!("             match/result/verify/settle/dispute/arbitrate/");
    println!("             balance/conservation/leaderboard/stats");
    println!("             （用 --api <url> 或 GSN_API 指定节点，默认 127.0.0.1:4002）");
}

// ───────────────────────── 公共参数解析 ─────────────────────────

fn opt_value(args: &[String], flag: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < args.len() {
        if args[i] == flag {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn resolve_db_path(data_dir: Option<&str>) -> PathBuf {
    let dir = match data_dir {
        Some(d) => node::expand_tilde(PathBuf::from(d)),
        None => node::default_data_dir(),
    };
    dir.join("gsn.db")
}

// ───────────────────────── ledger verify（v3.6.0）─────────────────────────

#[derive(Debug)]
struct LedgerReport {
    records: usize,
    head: String,
    audit: AuditReport,
}

#[derive(Debug, PartialEq, Eq)]
enum LedgerIssue {
    /// 数据库文件不存在（全新数据目录 / 离线）；不创建空库
    MissingDb,
    /// 存在无法解析的流水行（seq, payload 损坏），参数为坏行数
    CorruptRows(usize),
    /// 哈希链断链：参数为首个断链 seq；u64::MAX 表示锚定 head 不一致
    ChainBroken(u64),
    /// 存储 / 引擎层错误
    Engine(String),
}

/// 纯函数式离线核验：坏行 → 断链 → 重放独立审计。不产生任何写入。
fn evaluate_ledger(db: &Path) -> Result<LedgerReport, LedgerIssue> {
    if !db.exists() {
        return Err(LedgerIssue::MissingDb);
    }
    let store = PersistentStore::open(db).map_err(|e| LedgerIssue::Engine(e.to_string()))?;
    let loaded = store
        .load_ledger_records_checked()
        .map_err(|e| LedgerIssue::Engine(e.to_string()))?;
    if !loaded.corrupt.is_empty() {
        return Err(LedgerIssue::CorruptRows(loaded.corrupt.len()));
    }
    let head = store
        .verify_ledger_chain()
        .map_err(LedgerIssue::ChainBroken)?;
    let engine = SettlementEngine::restore(loaded.records.clone()).map_err(LedgerIssue::Engine)?;
    let audit = engine.independent_audit();
    Ok(LedgerReport {
        records: loaded.records.len(),
        head,
        audit,
    })
}

fn run_ledger(args: &[String]) -> i32 {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    if sub != "verify" {
        eprintln!("用法: gsn ledger verify [--data-dir <目录>]");
        return 2;
    }
    let db = resolve_db_path(opt_value(&args[1..], "--data-dir").as_deref());
    match evaluate_ledger(&db) {
        Ok(rep) => {
            println!("LEDGER       PASS");
            println!("DB           {}", db.display());
            println!("RECORDS      {}", rep.records);
            println!("HEAD         {}", rep.head);
            println!(
                "AUDIT        {}",
                if rep.audit.passed { "passed" } else { "FAILED" }
            );
            println!("EXPECTED     {}", rep.audit.expected_total.as_i64());
            println!("ACTUAL       {}", rep.audit.actual_total.as_i64());
            println!(
                "AGGREGATE    {}",
                if rep.audit.aggregate_matches {
                    "matches"
                } else {
                    "mismatch"
                }
            );
            println!("MISMATCHES   {}", rep.audit.mismatches.len());
            for m in &rep.audit.mismatches {
                println!(
                    "  - {} expected={} actual={}",
                    m.account,
                    m.expected.as_i64(),
                    m.actual.as_i64()
                );
            }
            i32::from(!rep.audit.passed)
        }
        Err(issue) => {
            match issue {
                LedgerIssue::MissingDb => {
                    println!("LEDGER       MISSING");
                    println!("DB           {} （不存在；未创建空库）", db.display());
                    return 2;
                }
                LedgerIssue::CorruptRows(n) => {
                    eprintln!("LEDGER       FAIL  存在 {n} 条无法解析的流水行（CorruptRows）");
                }
                LedgerIssue::ChainBroken(seq) => {
                    if seq == u64::MAX {
                        eprintln!(
                            "LEDGER       FAIL  锚定 head 与链末不一致（ChainBroken: anchor）"
                        );
                    } else {
                        eprintln!("LEDGER       FAIL  哈希链断链（首个断链 seq={seq}）");
                    }
                }
                LedgerIssue::Engine(e) => eprintln!("LEDGER       ERROR {e}"),
            }
            1
        }
    }
}

// ───────────────────────── doctor（v3.6.0）─────────────────────────

struct DoctorItem {
    name: &'static str,
    ok: bool,
    fatal: bool,
    detail: String,
}

async fn run_doctor(args: &[String]) -> i32 {
    let data_dir = opt_value(args, "--data-dir");
    let api = opt_value(args, "--api")
        .or_else(|| std::env::var("GSN_API").ok())
        .unwrap_or_else(|| "http://127.0.0.1:4002".to_string());
    let db = resolve_db_path(data_dir.as_deref());

    let mut items: Vec<DoctorItem> = Vec::new();

    // 1) CLI 版本（唯一版本来源：编译期 CARGO_PKG_VERSION）
    items.push(DoctorItem {
        name: "cli-version",
        ok: true,
        fatal: false,
        detail: format!("gsn {VERSION}"),
    });

    // 2) 本地账本（硬检查；缺库为非致命 WARN）
    match evaluate_ledger(&db) {
        Ok(rep) => items.push(DoctorItem {
            name: "local-ledger",
            ok: rep.audit.passed,
            fatal: true,
            detail: format!(
                "{} records, head {} (audit {})",
                rep.records,
                &rep.head[..rep.head.len().min(16)],
                if rep.audit.passed { "passed" } else { "FAILED" }
            ),
        }),
        Err(LedgerIssue::MissingDb) => items.push(DoctorItem {
            name: "local-ledger",
            ok: true,
            fatal: false,
            detail: format!("{} 不存在（全新/离线，WARN）", db.display()),
        }),
        Err(other) => items.push(DoctorItem {
            name: "local-ledger",
            ok: false,
            fatal: true,
            detail: format!("{other:?}"),
        }),
    }

    // 3) daemon /health（离线非致命 WARN；900ms 超时）
    match tokio::time::timeout(
        Duration::from_millis(900),
        http_call(&api, "GET", "/health", ""),
    )
    .await
    {
        Ok(Ok((status, _))) if (200..300).contains(&status) => items.push(DoctorItem {
            name: "daemon-health",
            ok: true,
            fatal: false,
            detail: format!("{api}/health -> {status}"),
        }),
        Ok(Ok((status, _))) => items.push(DoctorItem {
            name: "daemon-health",
            ok: false,
            fatal: true,
            detail: format!("{api}/health -> HTTP {status}"),
        }),
        Ok(Err(e)) => items.push(DoctorItem {
            name: "daemon-health",
            ok: true,
            fatal: false,
            detail: format!("daemon 不可达（WARN，离线）: {e}"),
        }),
        Err(_) => items.push(DoctorItem {
            name: "daemon-health",
            ok: true,
            fatal: false,
            detail: "daemon 900ms 超时（WARN，离线）".to_string(),
        }),
    }

    let mut hard_fail = false;
    for it in &items {
        let tag = if it.ok {
            "OK  "
        } else if it.fatal {
            "FAIL"
        } else {
            "WARN"
        };
        if !it.ok && it.fatal {
            hard_fail = true;
        }
        println!("[{tag}] {:<14} {}", it.name, it.detail);
    }
    if hard_fail {
        1
    } else {
        0
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

    // v2.8.6（GAP §4.1）/ v3.6.0：金额必须是合法非负整数，非法即本地拒绝，
    // 绝不再用 parse().unwrap_or(0) 把 "lots"/"10.5" 静默存成 0。
    if op == "deposit" {
        if params.len() < 2 {
            eprintln!("错误: deposit 需要 <account> <amount>");
            return 1;
        }
        match params[1].parse::<i64>() {
            Ok(amount) if amount >= 0 => {}
            _ => {
                eprintln!(
                    "错误: 充值金额必须是非负整数（收到 {:?}）；不接受浮点或非数字，未发送请求",
                    params[1]
                );
                return 1;
            }
        }
    }

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
    println!("  deposit <account> <amount>          充值（金额为非负整数）");
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
    println!("  arbitrate <id> <guilty> [arbitrator] 仲裁（罚没由服务端规则决定）");
    println!("  conservation                        守恒检查");
    println!("  audit                               独立审计（v2.5.9）");
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
        // 金额已在 run_market 校验为非负 i64，这里 parse 必成功。
        "deposit" if p.len() >= 2 => Some((
            "POST".into(),
            format!("/api/v1/accounts/{}/deposit", p[0]),
            json!({ "amount": p[1].parse::<i64>().ok()? }).to_string(),
        )),
        "balance" if !p.is_empty() => Some((
            "GET".into(),
            format!("/api/v1/accounts/{}/balance", p[0]),
            String::new(),
        )),
        "register" if !p.is_empty() => {
            Some(("POST".into(), "/api/v1/agents".into(), read_json_arg(&p[0])))
        }
        "get" if !p.is_empty() => Some((
            "GET".into(),
            format!("/api/v1/agents/{}", p[0]),
            String::new(),
        )),
        "discover" if !p.is_empty() => Some((
            "GET".into(),
            format!("/api/v1/agents?skill={}", p[0]),
            String::new(),
        )),
        "search" if !p.is_empty() => Some((
            "GET".into(),
            format!("/api/v1/agents?q={}", p[0]),
            String::new(),
        )),
        "publish" if !p.is_empty() => {
            Some(("POST".into(), "/api/v1/tasks".into(), read_json_arg(&p[0])))
        }
        "task" if !p.is_empty() => Some((
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
            task_id.map(|tid| ("POST".into(), format!("/api/v1/tasks/{}/bids", tid), body))
        }
        "match" if !p.is_empty() => Some((
            "POST".into(),
            format!("/api/v1/tasks/{}/match", p[0]),
            String::new(),
        )),
        "result" if !p.is_empty() => {
            let body = read_json_arg(&p[0]);
            let task_id = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("task_id").and_then(|x| x.as_str()).map(String::from));
            task_id.map(|tid| {
                (
                    "POST".into(),
                    format!("/api/v1/tasks/{}/results", tid),
                    body,
                )
            })
        }
        "verify" if !p.is_empty() => {
            // v2.5.9 认证式：载荷文件含 members（固定委员集）与 signed_votes（签名票）
            if p.len() < 2 {
                eprintln!("verify 需要认证载荷：verify <task_id> @verify.json");
                std::process::exit(1);
            }
            let body = read_json_arg(&p[1]);
            Some((
                "POST".into(),
                format!("/api/v1/tasks/{}/verify", p[0]),
                body,
            ))
        }
        "settle" if !p.is_empty() => Some((
            "POST".into(),
            format!("/api/v1/tasks/{}/settle", p[0]),
            String::new(),
        )),
        "dispute" if !p.is_empty() => Some((
            "POST".into(),
            "/api/v1/disputes".into(),
            read_json_arg(&p[0]),
        )),
        // v2.8.5：arbitrator 必填、罚没由服务端规则决定（不再发送 slash_amount）。
        "arbitrate" if p.len() >= 2 => {
            let guilty = p[1] == "guilty" || p[1] == "true";
            let arbitrator = p.get(2).cloned().unwrap_or_else(|| "gsn-cli".to_string());
            Some((
                "POST".into(),
                format!("/api/v1/disputes/{}/arbitrate", p[0]),
                json!({ "guilty": guilty, "arbitrator": arbitrator }).to_string(),
            ))
        }
        "conservation" => Some(("GET".into(), "/api/v1/conservation".into(), String::new())),
        "audit" => Some(("GET".into(), "/api/v1/audit".into(), String::new())),
        "leaderboard" => {
            let limit = p.first().map(|s| s.as_str()).unwrap_or("10");
            Some((
                "GET".into(),
                format!("/api/v1/leaderboard?limit={}", limit),
                String::new(),
            ))
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
    let (host, port) = host_port
        .split_once(':')
        .map(|(h, p)| (h, p.parse::<u16>().unwrap_or(80)))
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
    let status = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let body_start = text.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    Ok((status, text[body_start..].to_string()))
}

// ───────────────────────── ledger verify 回归测试 ─────────────────────────
// 五条用例在旧实现（无 ledger verify / 坏行静默跳过 / 无锚定校验）下均无法通过。
#[cfg(test)]
mod tests {
    use super::*;
    use gsn_core::marketplace::{Money, SettlementReason, SettlementRecord};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d =
            std::env::temp_dir().join(format!("gsn-ledger-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn deposit_rec(account: &str, amount: i64, seq: u64) -> SettlementRecord {
        SettlementRecord {
            task_id: format!("dep-{seq}"),
            from_account: "__external__".to_string(),
            to_account: account.to_string(),
            amount: Money::new(amount),
            reason: SettlementReason::Deposited,
            timestamp: 1_700_000_000_000 + seq,
        }
    }

    #[test]
    fn ledger_intact_chain_passes() {
        let dir = unique_dir("intact");
        let db = dir.join("gsn.db");
        {
            let store = PersistentStore::open(&db).unwrap();
            store
                .append_ledger_record(&deposit_rec("alice", 1000, 1))
                .unwrap();
            store
                .append_ledger_record(&deposit_rec("bob", 250, 2))
                .unwrap();
        }
        let rep = evaluate_ledger(&db).expect("intact ledger verifies");
        assert_eq!(rep.records, 2);
        assert!(rep.audit.passed);
        assert_eq!(rep.audit.expected_total.as_i64(), 1250);
        assert_eq!(rep.audit.actual_total.as_i64(), 1250);
        assert!(rep.audit.aggregate_matches);
        assert!(rep.audit.mismatches.is_empty());
    }

    #[test]
    fn ledger_missing_db_is_missing_and_not_created() {
        let dir = unique_dir("missing");
        let db = dir.join("gsn.db");
        assert!(!db.exists());
        let issue = evaluate_ledger(&db).expect_err("missing db is an error");
        assert_eq!(issue, LedgerIssue::MissingDb);
        // 缺库不得副作用建空库
        assert!(!db.exists(), "verify 不得创建空数据库");
    }

    #[test]
    fn ledger_tampered_payload_breaks_chain_at_seq1() {
        let dir = unique_dir("tamper");
        let db = dir.join("gsn.db");
        {
            let store = PersistentStore::open(&db).unwrap();
            store
                .append_ledger_record(&deposit_rec("alice", 1000, 1))
                .unwrap();
            store
                .append_ledger_record(&deposit_rec("bob", 250, 2))
                .unwrap();
        }
        // 把 seq=1 的 payload 改成仍可解析但金额不同的记录 -> 哈希失配
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            let payload: String = conn
                .query_row("SELECT payload FROM ledger_entries WHERE seq=1", [], |r| {
                    r.get(0)
                })
                .unwrap();
            let mut rec: serde_json::Value = serde_json::from_str(&payload).unwrap();
            rec["amount"] = serde_json::json!(9999);
            conn.execute(
                "UPDATE ledger_entries SET payload=?1 WHERE seq=1",
                [serde_json::to_string(&rec).unwrap()],
            )
            .unwrap();
        }
        let issue = evaluate_ledger(&db).expect_err("tampered payload must fail");
        assert_eq!(issue, LedgerIssue::ChainBroken(1));
    }

    #[test]
    fn ledger_corrupt_row_reported() {
        let dir = unique_dir("corrupt");
        let db = dir.join("gsn.db");
        {
            let store = PersistentStore::open(&db).unwrap();
            store
                .append_ledger_record(&deposit_rec("alice", 1000, 1))
                .unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute(
                "UPDATE ledger_entries SET payload='{not-json' WHERE seq=1",
                [],
            )
            .unwrap();
        }
        let issue = evaluate_ledger(&db).expect_err("corrupt row must fail");
        assert_eq!(issue, LedgerIssue::CorruptRows(1));
    }

    #[test]
    fn ledger_anchor_mismatch_is_u64_max() {
        let dir = unique_dir("anchor");
        let db = dir.join("gsn.db");
        {
            let store = PersistentStore::open(&db).unwrap();
            store
                .append_ledger_record(&deposit_rec("alice", 1000, 1))
                .unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute(
                "UPDATE kv_meta SET value='deadbeef' WHERE key='ledger_head_hash'",
                [],
            )
            .unwrap();
        }
        let issue = evaluate_ledger(&db).expect_err("anchor mismatch must fail");
        assert_eq!(issue, LedgerIssue::ChainBroken(u64::MAX));
    }
}
