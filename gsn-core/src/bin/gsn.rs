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
            println!("agent-universe v3.5.6");
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
    println!("  deposit <account> <amount>          充值（amount 必须为整数，非法即报错）");
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

/// 严格解析金额：只接受十进制整数（可选前导 +/-），拒绝浮点、空串、"lots" 等。
/// 与服务端 deposit 契约一致（body amount 必须是 JSON 整数 i64），非法即报错，
/// 绝不静默退化为 0（v3.5.5 / GAP §8.1：旧实现 `parse::<i64>().unwrap_or(0)`
/// 会把 `market_deposit {"amount":"lots"}` 存成 0 却返回成功）。
fn parse_amount(raw: &str) -> Result<i64, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("amount 为空".into());
    }
    // 拒绝任何含小数点/指数/非数字的写法，避免 f64 截断。
    let digits = s
        .strip_prefix('+')
        .or_else(|| s.strip_prefix('-'))
        .unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("amount 必须是十进制整数，收到非法值 {raw:?}"));
    }
    s.parse::<i64>()
        .map_err(|_| format!("amount 超出 i64 范围: {raw:?}"))
}

/// 构造市场请求三元组。返回 Err(可读原因) 表示参数不合法（调用方以退出码 2 终止，
/// 绝不发出会被服务端误解为 0 的请求）。
#[allow(clippy::type_complexity)]
fn build_market_request(op: &str, p: &[String]) -> Result<(String, String, String), String> {
    use serde_json::json;
    match op {
        "deposit" if p.len() >= 2 => {
            let amount = parse_amount(&p[1])?;
            Ok((
                "POST".into(),
                format!("/api/v1/accounts/{}/deposit", p[0]),
                json!({"amount": amount}).to_string(),
            ))
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
    use super::{build_market_request, parse_amount};

    #[test]
    fn amount_accepts_plain_integers_and_signs() {
        assert_eq!(parse_amount("100").unwrap(), 100);
        assert_eq!(parse_amount("  42 ").unwrap(), 42);
        assert_eq!(parse_amount("+7").unwrap(), 7);
        assert_eq!(parse_amount("-5").unwrap(), -5);
        assert_eq!(parse_amount("0").unwrap(), 0);
    }

    #[test]
    fn amount_rejects_non_integer_instead_of_defaulting_to_zero() {
        // 旧实现 parse::<i64>().unwrap_or(0) 会把这些静默存成 0。
        for bad in [
            "lots", "", "  ", "10.5", "10.0", "1e3", "0x10", "1,000", "nan",
        ] {
            assert!(parse_amount(bad).is_err(), "应拒绝 {bad:?}");
        }
        let overflow = format!("{}", i64::MAX as u128 + 1);
        assert!(parse_amount(&overflow).is_err(), "应拒绝 i64 溢出值");
    }

    #[test]
    fn deposit_body_carries_parsed_integer_and_rejects_bad_amount() {
        let (method, path, body) =
            build_market_request("deposit", &["acct".into(), "250".into()]).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(path, "/api/v1/accounts/acct/deposit");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["amount"], serde_json::json!(250));

        assert!(build_market_request("deposit", &["acct".into(), "lots".into()]).is_err());
        assert!(build_market_request("deposit", &["acct".into(), "10.5".into()]).is_err());
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
