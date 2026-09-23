//! gsn-daemon - GSN 全节点 headless 守护进程
//!
//! 真实网络节点：
//! - libp2p P2P 节点（Noise + Kademlia DHT + GossipSub），bind P2P 端口
//! - HTTP API（tokio TcpListener + 手写 HTTP/1.1），bind API 端口
//! - SQLite 持久化（agents/tasks 落盘）
//!
//! 节点用 actor 模式：swarm task 独占 P2pPeer，HTTP handler 通过 mpsc 通道发命令，
//! 避免共享 Mutex 死锁。

use gsn_core::net::P2pPeer;
use gsn_core::storage::{PersistentStore, StoredAgent};
use gsn_core::NodeMode;
use libp2p::Multiaddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};

// ───────────────────────── 节点命令协议 ─────────────────────────

/// 发送给 swarm task 的命令
#[allow(dead_code)] // Subscribe/Publish 为节点命令 API 的一部分，预留给后续调用
enum PeerCommand {
    /// 查询节点信息
    GetInfo { reply: oneshot::Sender<PeerInfo> },
    /// DHT 写入
    DhtPut { key: String, value: Vec<u8> },
    /// 订阅主题
    Subscribe { topic: String },
    /// 发布消息
    Publish { topic: String, data: Vec<u8> },
}

/// 节点快照信息
#[derive(Clone)]
struct PeerInfo {
    peer_id: String,
    connected: usize,
    routing_entries: usize,
}

type PeerCmdTx = mpsc::Sender<PeerCommand>;

// ───────────────────────── 命令行参数 ─────────────────────────

#[derive(Debug, Clone)]
struct Args {
    listen: String,
    port: u16,
    api_port: u16,
    data_dir: PathBuf,
    mode: String,
    bootstrap: Vec<String>,
}

fn require_value(args: &[String], i: usize, opt: &str) -> String {
    if i + 1 >= args.len() || args[i + 1].starts_with("--") {
        eprintln!("错误: 选项 {} 缺少参数", opt);
        eprintln!("用法: gsn-daemon [选项]，运行 gsn-daemon --help 查看帮助");
        std::process::exit(1);
    }
    args[i + 1].clone()
}

impl Args {
    fn parse() -> Self {
        let mut listen = "0.0.0.0".to_string();
        let mut port = 4001u16;
        let mut api_port = 4002u16;
        let mut data_dir = PathBuf::from("~/.gsn/data");
        let mut mode = "full".to_string();
        let mut bootstrap = Vec::new();

        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--listen" => { listen = require_value(&args, i, "--listen"); i += 2; }
                "--port" => {
                    port = require_value(&args, i, "--port").parse().unwrap_or(4001);
                    i += 2;
                }
                "--api-port" => {
                    api_port = require_value(&args, i, "--api-port").parse().unwrap_or(4002);
                    i += 2;
                }
                "--data-dir" => { data_dir = PathBuf::from(require_value(&args, i, "--data-dir")); i += 2; }
                "--mode" => { mode = require_value(&args, i, "--mode"); i += 2; }
                "--bootstrap" => {
                    while i + 1 < args.len() && !args[i + 1].starts_with("--") {
                        bootstrap.push(args[i + 1].clone());
                        i += 1;
                    }
                    i += 1;
                }
                "--help" | "-h" => { print_help(); std::process::exit(0); }
                _ => {
                    eprintln!("错误: 未知选项 {}", args[i]);
                    std::process::exit(1);
                }
            }
        }
        Self { listen, port, api_port, data_dir, mode, bootstrap }
    }
}

fn print_help() {
    println!("gsn-daemon - GSN 全节点守护进程\n");
    println!("用法: gsn-daemon [选项]\n");
    println!("选项:");
    println!("  --listen <addr>     监听地址 (默认: 0.0.0.0)");
    println!("  --port <port>       P2P 端口 (默认: 4001)");
    println!("  --api-port <port>   HTTP API 端口 (默认: 4002)");
    println!("  --data-dir <path>   数据目录 (默认: ~/.gsn/data)");
    println!("  --mode <mode>       节点模式: archive|full|light|edge|browser (默认: full)");
    println!("  --bootstrap <addr>  引导节点 multiaddr (可多个)");
    println!("  --help              显示帮助");
}

// ───────────────────────── swarm actor task ─────────────────────────

/// 拥有 P2pPeer，同时处理 swarm 事件和命令通道
async fn run_swarm_actor(mut peer: P2pPeer, mut cmd_rx: mpsc::Receiver<PeerCommand>) {
    loop {
        tokio::select! {
            // 处理 libp2p 网络事件
            event = peer.next_event() => {
                tracing::debug!("libp2p event: {:?}", event);
            }
            // 处理来自 HTTP handler 的命令
            maybe_cmd = cmd_rx.recv() => {
                match maybe_cmd {
                    Some(cmd) => handle_peer_command(&mut peer, cmd),
                    // 命令通道关闭，退出
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
    }
}

// ───────────────────────── HTTP 工具 ─────────────────────────

fn http_response(status: u16, status_text: &str, body: String, content_type: &str) -> String {
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        status, status_text, content_type, body.len(), body
    )
}

fn json_response(status: u16, status_text: &str, value: serde_json::Value) -> String {
    http_response(
        status,
        status_text,
        serde_json::to_string(&value).unwrap_or_default(),
        "application/json",
    )
}

/// 通过命令通道查询节点信息
async fn fetch_peer_info(cmd_tx: &PeerCmdTx) -> Option<PeerInfo> {
    let (reply, rx) = oneshot::channel();
    cmd_tx
        .send(PeerCommand::GetInfo { reply })
        .await
        .ok()?;
    rx.await.ok()
}

// ───────────────────────── HTTP API 服务器 ─────────────────────────

async fn run_api_server(
    listen: String,
    api_port: u16,
    mode: String,
    p2p_port: u16,
    start: Instant,
    store: Arc<PersistentStore>,
    cmd_tx: PeerCmdTx,
) -> anyhow::Result<()> {
    let addr = format!("{}:{}", listen, api_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("✅ HTTP API 监听: http://{}", addr);

    loop {
        let (mut stream, _remote) = listener.accept().await?;
        let mode = mode.clone();
        let store = store.clone();
        let cmd_tx = cmd_tx.clone();

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let mut buf = vec![0u8; 16384];
            let n = match stream.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };
            let request = String::from_utf8_lossy(&buf[..n]);

            let request_line = request.lines().next().unwrap_or("");
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("GET");
            let raw_path = parts.next().unwrap_or("/");
            let path = raw_path.split('?').next().unwrap_or("/");

            let response = match (method, path) {
                ("GET", "/") | ("GET", "/health") => {
                    let info = fetch_peer_info(&cmd_tx).await;
                    let value = serde_json::json!({
                        "status": "ok",
                        "service": "gsn-daemon",
                        "version": env!("CARGO_PKG_VERSION"),
                        "mode": mode,
                        "p2p_port": p2p_port,
                        "connected_peers": info.as_ref().map(|i| i.connected).unwrap_or(0),
                        "uptime_ms": start.elapsed().as_millis(),
                    });
                    json_response(200, "OK", value)
                }
                ("GET", "/version") => json_response(
                    200,
                    "OK",
                    serde_json::json!({"name": "gsn-daemon", "version": env!("CARGO_PKG_VERSION")}),
                ),
                ("GET", "/agents") => {
                    let agents = store.load_agents().unwrap_or_default();
                    let list: Vec<serde_json::Value> = agents
                        .iter()
                        .map(|a| serde_json::json!({
                            "agent_id": a.agent_id, "name": a.name,
                            "skills": a.skills, "stake": a.stake, "reputation": a.reputation,
                        }))
                        .collect();
                    json_response(200, "OK", serde_json::json!({"agents": list}))
                }
                ("GET", "/tasks") => {
                    let tasks = store.load_tasks().unwrap_or_default();
                    let list: Vec<serde_json::Value> = tasks
                        .iter()
                        .map(|t| serde_json::json!({
                            "task_id": t.task_id, "goal": t.goal, "state": t.state,
                            "owner": t.owner, "budget": t.budget,
                        }))
                        .collect();
                    json_response(200, "OK", serde_json::json!({"tasks": list}))
                }
                ("GET", "/peers") => {
                    if let Some(info) = fetch_peer_info(&cmd_tx).await {
                        json_response(200, "OK", serde_json::json!({
                            "local_peer_id": info.peer_id,
                            "connected_peers": info.connected,
                            "routing_table_entries": info.routing_entries,
                        }))
                    } else {
                        json_response(503, "Service Unavailable", serde_json::json!({"error": "peer actor 不可用"}))
                    }
                }
                ("POST", "/agents") => {
                    let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
                    let body = &request[body_start..];
                    match serde_json::from_str::<serde_json::Value>(body) {
                        Ok(v) => {
                            let agent_id = v.get("agent_id").and_then(|x| x.as_str()).unwrap_or("");
                            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("");
                            let skills = v.get("skills").and_then(|x| x.as_array())
                                .map(|a| a.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>().join(","))
                                .unwrap_or_default();
                            let stake = v.get("stake").and_then(|x| x.as_f64()).unwrap_or(0.0);

                            if agent_id.is_empty() || name.is_empty() {
                                json_response(400, "Bad Request",
                                    serde_json::json!({"error": "agent_id 和 name 必填"}))
                            } else {
                                let stored = StoredAgent {
                                    agent_id: agent_id.to_string(), name: name.to_string(),
                                    skills, stake, reputation: 0.0,
                                    created_at: chrono::Utc::now().to_rfc3339(),
                                };
                                match store.upsert_agent(&stored) {
                                    Ok(_) => {
                                        // 同步写入 DHT（发命令，不阻塞等待结果）
                                        let _ = cmd_tx.send(PeerCommand::DhtPut {
                                            key: format!("/aip/agent/{}", agent_id),
                                            value: body.as_bytes().to_vec(),
                                        }).await;
                                        json_response(201, "Created",
                                            serde_json::json!({"status": "registered", "agent_id": agent_id}))
                                    }
                                    Err(e) => json_response(500, "Internal Server Error",
                                        serde_json::json!({"error": e.to_string()})),
                                }
                            }
                        }
                        Err(_) => json_response(400, "Bad Request",
                            serde_json::json!({"error": "请求体不是合法 JSON"})),
                    }
                }
                ("OPTIONS", _) => http_response(204, "No Content", String::new(), "text/plain"),
                _ => {
                    // 404：serde_json::json! 自动转义 path 中的引号/反斜杠等特殊字符
                    json_response(404, "Not Found", serde_json::json!({
                        "error": "not_found", "path": path,
                    }))
                }
            };

            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;

            let status_line = response.lines().next().unwrap_or("");
            eprintln!("← {} {} ({})", method, path, status_line);
        });
    }
}

// ───────────────────────── main ─────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
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

    // SQLite 真实落盘
    let db_path = args.data_dir.join("gsn.db");
    let store = Arc::new(PersistentStore::open(&db_path)?);
    println!(
        "✅ SQLite: {:?}（{} agents / {} tasks）",
        db_path,
        store.agent_count().unwrap_or(0),
        store.task_count().unwrap_or(0)
    );

    // 真实 libp2p 节点
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

    // 节点命令通道（actor 模式）
    let (cmd_tx, cmd_rx) = mpsc::channel::<PeerCommand>(64);
    let local_peer_id = peer.peer_id.to_string();
    println!("   Peer ID: {}", local_peer_id);
    println!("   模式: {:?}", node_mode);

    // 订阅默认主题（直接在 actor 启动前做）
    let _ = peer.subscribe("gsn/agents");
    let _ = peer.subscribe("gsn/tasks");

    // 启动 swarm actor（独占 peer）
    tokio::spawn(run_swarm_actor(peer, cmd_rx));

    println!("✅ gsn-daemon 启动完成");

    // HTTP API（阻塞主线）
    run_api_server(
        args.listen.clone(),
        args.api_port,
        args.mode.clone(),
        args.port,
        Instant::now(),
        store,
        cmd_tx,
    )
    .await?;

    Ok(())
}
