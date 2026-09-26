//! 节点运行时
//!
//! 把守护进程的启动逻辑封装为库函数，供 `gsn-daemon` 二进制与
//! `gsn daemon` 子命令复用，避免逻辑重复。

use crate::api::market_actor::MarketActorHandle;
use crate::net::P2pPeer;
use crate::storage::{PersistentStore, StoredAgent};
use crate::NodeMode;
use libp2p::Multiaddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};

// ───────────────────────── P2P 节点命令协议 ─────────────────────────

#[allow(dead_code)]
pub enum PeerCommand {
    GetInfo { reply: oneshot::Sender<PeerInfo> },
    DhtPut { key: String, value: Vec<u8> },
    Subscribe { topic: String },
    Publish { topic: String, data: Vec<u8> },
    /// v2.5.3: 主动连接 bootstrap 节点
    AddBootstrap { addr: String, reply: oneshot::Sender<Result<(), String>> },
    /// v2.5.3: 列出已连接对等节点 peer_id
    ListPeers { reply: oneshot::Sender<Vec<String>> },
}

#[derive(Clone, serde::Serialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub connected: usize,
    pub routing_entries: usize,
    pub listen_addrs: Vec<String>,
    pub bootstrapped: Vec<String>,
}

pub type PeerCmdTx = mpsc::Sender<PeerCommand>;

// ───────────────────────── 参数 ─────────────────────────

#[derive(Debug, Clone)]
pub struct DaemonArgs {
    pub listen: String,
    pub port: u16,
    pub api_port: u16,
    pub data_dir: PathBuf,
    pub mode: String,
    pub bootstrap: Vec<String>,
}

impl Default for DaemonArgs {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0".to_string(),
            port: 4001,
            api_port: 4002,
            data_dir: default_data_dir(),
            mode: "full".to_string(),
            bootstrap: Vec::new(),
        }
    }
}

/// 返回真实 home 下的默认数据目录（不使用字面 "~"，PathBuf 不会展开它）。
pub fn default_data_dir() -> PathBuf {
    match std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        Some(home) => PathBuf::from(home).join(".gsn").join("data"),
        // 无 home 环境变量时回退到当前目录，避免创建字面 "~" 目录。
        None => PathBuf::from(".gsn").join("data"),
    }
}

/// 展开路径开头的 "~" 或 "~/" 为真实 home 目录；其余情况原样返回。
pub fn expand_tilde(p: PathBuf) -> PathBuf {
    let s = match p.to_str() {
        Some(s) => s,
        None => return p,
    };
    if let Some(rest) = s.strip_prefix('~') {
        if let Some(home) = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
        {
            let rest = rest.trim_start_matches('/');
            return if rest.is_empty() { home } else { home.join(rest) };
        }
    }
    p
}

pub fn require_value(args: &[String], i: usize, opt: &str) -> String {
    if i + 1 >= args.len() || args[i + 1].starts_with("--") {
        eprintln!("错误: 选项 {} 缺少参数", opt);
        eprintln!("用法: gsn daemon [选项]，运行 gsn daemon --help 查看帮助");
        std::process::exit(1);
    }
    args[i + 1].clone()
}

pub fn print_daemon_help() {
    println!("gsn-daemon - GSN 全节点守护进程\n");
    println!("用法: gsn-daemon [选项]（也可通过 gsn daemon 调用）\n");
    println!("选项:");
    println!("  --listen <addr>     监听地址 (默认: 0.0.0.0)");
    println!("  --port <port>       P2P 端口 (默认: 4001)");
    println!("  --api-port <port>   HTTP API 端口 (默认: 4002)");
    println!("  --data-dir <path>   数据目录 (默认: ~/.gsn/data)");
    println!("  --mode <mode>       节点模式: archive|full|light|edge|browser (默认: full)");
    println!("  --bootstrap <addr>  引导节点 multiaddr (可多个)");
    println!("  --help              显示帮助");
}

/// 从 argv（不含程序名）解析
pub fn parse_daemon_args(args: &[String]) -> DaemonArgs {
    let mut d = DaemonArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" => { d.listen = require_value(args, i, "--listen"); i += 2; }
            "--port" => { d.port = require_value(args, i, "--port").parse().unwrap_or(4001); i += 2; }
            "--api-port" => { d.api_port = require_value(args, i, "--api-port").parse().unwrap_or(4002); i += 2; }
            "--data-dir" => { d.data_dir = PathBuf::from(require_value(args, i, "--data-dir")); i += 2; }
            "--mode" => { d.mode = require_value(args, i, "--mode"); i += 2; }
            "--bootstrap" => {
                while i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    d.bootstrap.push(args[i + 1].clone());
                    i += 1;
                }
                i += 1;
            }
            "--help" | "-h" => { print_daemon_help(); std::process::exit(0); }
            _ => { eprintln!("错误: 未知选项 {}", args[i]); std::process::exit(1); }
        }
    }
    d.data_dir = expand_tilde(d.data_dir);
    d
}

// ───────────────────────── swarm actor ─────────────────────────

async fn run_swarm_actor(mut peer: P2pPeer, mut cmd_rx: mpsc::Receiver<PeerCommand>) {
    loop {
        tokio::select! {
            event = peer.next_event() => {
                tracing::debug!("libp2p event: {:?}", event);
            }
            maybe_cmd = cmd_rx.recv() => {
                match maybe_cmd {
                    Some(cmd) => handle_peer_command(&mut peer, cmd),
                    None => break,
                }
            }
        }
    }
}

fn handle_peer_command(peer: &mut P2pPeer, cmd: PeerCommand) {
    match cmd {
        PeerCommand::GetInfo { reply } => {
            let info = PeerInfo {
                peer_id: peer.peer_id.to_string(),
                connected: peer.connected_peers(),
                routing_entries: peer.routing_table_size(),
                listen_addrs: peer.listen_addrs(),
                bootstrapped: peer.bootstrapped(),
            };
            let _ = reply.send(info);
        }
        PeerCommand::DhtPut { key, value } => {
            if let Err(e) = peer.dht_put(key.as_bytes(), value) {
                eprintln!("⚠️ DHT put 失败 [{}]: {}", key, e);
            }
        }
        PeerCommand::Subscribe { topic } => {
            if let Err(e) = peer.subscribe(&topic) {
                eprintln!("⚠️ subscribe 失败 [{}]: {}", topic, e);
            }
        }
        PeerCommand::Publish { topic, data } => {
            if let Err(e) = peer.publish(&topic, data) {
                eprintln!("⚠️ publish 失败 [{}]: {}", topic, e);
            }
        }
        PeerCommand::AddBootstrap { addr, reply } => {
            let result = peer.add_bootstrap_from_str(&addr);
            if result.is_ok() {
                eprintln!("✅ bootstrap 连接已发起: {}", addr);
            } else {
                eprintln!("⚠️ bootstrap 失败 [{}]: {}", addr, result.as_ref().err().unwrap());
            }
            let _ = reply.send(result);
        }
        PeerCommand::ListPeers { reply } => {
            let peers = peer.connected_peer_ids();
            let _ = reply.send(peers);
        }
    }
}

// ───────────────────────── HTTP 工具 ─────────────────────────

pub fn http_response(status: u16, status_text: &str, body: String, content_type: &str) -> String {
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nConnection: close\r\n\r\n{}",
        status, status_text, content_type, body.len(), body
    )
}

async fn fetch_peer_info(cmd_tx: &PeerCmdTx) -> Option<PeerInfo> {
    let (reply, rx) = oneshot::channel();
    cmd_tx.send(PeerCommand::GetInfo { reply }).await.ok()?;
    rx.await.ok()
}

/// v2.5.3: 网络增强 API
///
/// 端点：
/// - GET  /peers       列出已连接对等节点
/// - GET  /info        本机 peer 信息
/// - POST /bootstrap   添加 bootstrap 节点（body: {"addr":"/ip4/.../tcp/..."}）
async fn handle_network_api(
    method: &str,
    net_path: &str,
    body: &str,
    cmd_tx: &PeerCmdTx,
    _start: &Instant,
) -> (u16, String) {
    let path = net_path.trim_end_matches('/');

    // GET /peers
    if method == "GET" && (path == "/peers" || path == "/peers/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::ListPeers { reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(peers) => (200, serde_json::json!({"peers": peers, "count": peers.len()}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // GET /info
    else if method == "GET" && (path == "/info" || path.is_empty() || path == "/") {
        match fetch_peer_info(cmd_tx).await {
            Some(info) => (200, serde_json::to_string(&info).unwrap_or_else(|_| "{}".into())),
            None => (500, serde_json::json!({"error":"peer_unavailable"}).to_string()),
        }
    }
    // POST /bootstrap
    else if method == "POST" && (path == "/bootstrap" || path == "/bootstrap/") {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(body);
        let addr = match parsed {
            Ok(v) => v.get("addr").and_then(|a| a.as_str()).map(|s| s.to_string()),
            Err(_) => None,
        };
        let addr = match addr {
            Some(a) => a,
            None => return (400, serde_json::json!({"error":"missing_or_invalid_addr"}).to_string()),
        };
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::AddBootstrap { addr: addr.clone(), reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(Ok(())) => (201, serde_json::json!({"status":"bootstrap_initiated","addr":addr}).to_string()),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    else {
        (404, serde_json::json!({"error":"not_found","path":path}).to_string())
    }
}

// ───────────────────────── HTTP API 服务器 ─────────────────────────

async fn run_api_server(
    listen: String,
    api_port: u16,
    mode: String,
    p2p_port: u16,
    start: Instant,
    store: Arc<PersistentStore>,
    peer_cmd_tx: PeerCmdTx,
    market: MarketActorHandle,
) -> anyhow::Result<()> {
    use crate::api::rest::{route, NodeInfo};
    use crate::mcp::sse;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let addr = format!("{}:{}", listen, api_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("✅ HTTP API 监听: http://{}", addr);

    loop {
        let (mut stream, _remote) = listener.accept().await?;
        let mode = mode.clone();
        let store = store.clone();
        let peer_cmd_tx = peer_cmd_tx.clone();
        let market = market.clone();

        tokio::spawn(async move {
            // 读取完整请求
            let mut all = Vec::new();
            let mut buf = vec![0u8; 16384];
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        all.extend_from_slice(&buf[..n]);
                        if let Ok(s) = std::str::from_utf8(&all) {
                            if let Some(header_end) = s.find("\r\n\r\n") {
                                let headers = &s[..header_end];
                                let len: usize = headers
                                    .lines()
                                    .find(|l| l.to_lowercase().starts_with("content-length:"))
                                    .and_then(|l| l.split(':').nth(1))
                                    .and_then(|v| v.trim().parse().ok())
                                    .unwrap_or(0);
                                let body_bytes = s.len() - header_end - 4;
                                if body_bytes >= len || len == 0 { break; }
                            } else { break; }
                        } else { break; }
                    }
                    Err(_) => return,
                }
            }

            let request = String::from_utf8_lossy(&all);
            let request_line = request.lines().next().unwrap_or("");
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("GET").to_string();
            let raw_path = parts.next().unwrap_or("/").to_string();
            let (path_part, _) = raw_path.split_once('?').map(|(p, q)| (p, q)).unwrap_or((&raw_path, ""));
            let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
            let body = request[body_start..].to_string();

            // ───── MCP over HTTP 端点（优先拦截） ─────
            if path_part == "/api/v1/mcp" {
                let mcp = if method == "GET" {
                    sse::handle_get()
                } else if method == "POST" {
                    sse::handle_post(&body, &market).await
                } else {
                    let response = http_response(405, "Method Not Allowed", String::new(), "text/plain");
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                };
                let response = http_response(mcp.status, mcp.status_text, mcp.body, &mcp.content_type);
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (MCP {})", method, path_part, mcp.status);
                return;
            }

            // ───── v2.5.3 网络增强端点 ─────
            if path_part.starts_with("/api/v1/network/") || path_part.starts_with("/network/") {
                let net_path = path_part.trim_start_matches("/api/v1").trim_start_matches("/network").to_string();
                let net_resp = handle_network_api(&method, &net_path, &body, &peer_cmd_tx, &start).await;
                let ct = if net_resp.1 == "application/json" { "application/json" } else { "text/plain" };
                let resp = http_response(net_resp.0, if net_resp.0 == 200 { "OK" } else if net_resp.0 == 201 { "Created" } else if net_resp.0 == 400 { "Bad Request" } else { "Not Found" }, net_resp.1, ct);
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (network {})", method, path_part, net_resp.0);
                return;
            }

            let peer_info = fetch_peer_info(&peer_cmd_tx).await;
            let connected = peer_info.as_ref().map(|i| i.connected).unwrap_or(0);
            let info = NodeInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                mode: mode.clone(),
                p2p_port,
                connected_peers: connected,
                uptime_ms: start.elapsed().as_millis(),
            };

            let routed = route(&method, &raw_path, &body, &market, &info).await;

            // 注册 agent 落 SQLite + DHT
            if method == "POST" && (path_part == "/api/v1/agents" || path_part == "/agents") && routed.status == 201 {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                    let agent_id = v.get("agent_id").and_then(|x| x.as_str()).unwrap_or("");
                    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("");
                    let skills = v.get("skills").and_then(|x| x.as_array())
                        .map(|a| a.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>().join(","))
                        .unwrap_or_default();
                    let stake = v.get("stake").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let stored = StoredAgent {
                        agent_id: agent_id.to_string(), name: name.to_string(),
                        skills, stake, reputation: 0.0,
                        created_at: chrono::Utc::now().to_rfc3339(),
                    };
                    let _ = store.upsert_agent(&stored);
                    let _ = peer_cmd_tx.send(PeerCommand::DhtPut {
                        key: format!("/aip/agent/{}", agent_id),
                        value: body.as_bytes().to_vec(),
                    }).await;
                }
            }

            let response_body = if routed.body.is_null() {
                String::new()
            } else {
                serde_json::to_string(&routed.body).unwrap_or_default()
            };
            let content_type = if routed.body.is_null() { "text/plain" } else { "application/json" };
            let response = http_response(routed.status, routed.status_text, response_body, content_type);
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
            eprintln!("← {} {} ({} {})", method, path_part, routed.status, routed.status_text);
        });
    }
}

// ───────────────────────── 启动 ─────────────────────────

/// 启动节点（核心入口，gsn-daemon 与 gsn daemon 共用）
pub async fn run_daemon(args: DaemonArgs) -> anyhow::Result<()> {
    println!("=== GSN Daemon v{} ===", env!("CARGO_PKG_VERSION"));

    let node_mode = match args.mode.as_str() {
        "archive" => NodeMode::Archive,
        "full" => NodeMode::Full,
        "light" => NodeMode::Light,
        "edge" => NodeMode::Edge,
        "browser" => NodeMode::Browser,
        _ => NodeMode::Full,
    };

    std::fs::create_dir_all(&args.data_dir)?;
    std::fs::create_dir_all(args.data_dir.join("logs"))?;

    let db_path = args.data_dir.join("gsn.db");
    let store = Arc::new(PersistentStore::open(&db_path)?);
    println!(
        "✅ SQLite: {:?}（{} agents / {} tasks）",
        db_path,
        store.agent_count().unwrap_or(0),
        store.task_count().unwrap_or(0)
    );

    let mut peer = P2pPeer::new()?;
    match peer.listen_on_port(args.port) {
        Ok(_) => println!("✅ libp2p P2P 端口: {}", args.port),
        Err(e) => eprintln!("⚠️ P2P 端口 {} 绑定失败: {}", args.port, e),
    }
    for addr_str in &args.bootstrap {
        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
            match peer.add_bootstrap(addr) {
                Ok(_) => println!("→ bootstrap: {}", addr_str),
                Err(e) => eprintln!("⚠️ bootstrap {} 失败: {}", addr_str, e),
            }
        }
    }

    let (peer_cmd_tx, peer_cmd_rx) = mpsc::channel::<PeerCommand>(64);
    println!("   Peer ID: {}", peer.peer_id);
    println!("   模式: {:?}", node_mode);
    let _ = peer.subscribe("gsn/agents");
    let _ = peer.subscribe("gsn/tasks");
    tokio::spawn(run_swarm_actor(peer, peer_cmd_rx));

    let market = MarketActorHandle::spawn();
    println!("✅ Agent Market actor 已启动");
    println!("✅ gsn-daemon 启动完成");

    run_api_server(
        args.listen.clone(), args.api_port, args.mode.clone(),
        args.port, Instant::now(), store, peer_cmd_tx, market,
    )
    .await?;

    Ok(())
}
