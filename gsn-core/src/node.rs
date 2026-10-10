//! 节点运行时
//!
//! 把守护进程的启动逻辑封装为库函数，供 `gsn-daemon` 二进制与
//! `gsn daemon` 子命令复用，避免逻辑重复。

use crate::api::market_actor::MarketActorHandle;
use crate::api::rest::url_decode;
use crate::crdt::{CrdtMessage, CrdtOp, CrdtStats, CrdtStore, LwwEntry, CRDT_TOPIC};
use crate::erasure::distributed::{
    DistributedShardNode, ShardEnvelope, ShardTransport, DEFAULT_DATA_SHARDS, DEFAULT_PARITY_SHARDS,
};
use crate::erasure::wire::{GossipShardTransport, ShardWire, SHARD_TOPIC};
use crate::erasure::ErasureCoder;
use crate::net::peer::{extract_relay_peer_id, InboundGossipMessage};
use crate::net::P2pPeer;
use crate::relay_pool::{self, RelayClass, DEFAULT_PARALLEL_RELAYS};
use crate::sandbox::SandboxManager;
use crate::storage::{PersistentStore, StoredRelay};
use crate::NodeMode;
use libp2p::{Multiaddr, PeerId};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};

// ───────────────────────── P2P 节点命令协议 ─────────────────────────

#[allow(dead_code)]
pub enum PeerCommand {
    GetInfo {
        reply: oneshot::Sender<PeerInfo>,
    },
    DhtPut {
        key: String,
        value: Vec<u8>,
    },
    Subscribe {
        topic: String,
    },
    Publish {
        topic: String,
        data: Vec<u8>,
    },
    /// P0-1: 运行 Kademlia 标准自举（迭代 self-lookup），让 kbucket 从“只认识
    /// boot”扩展为“持有全网 K 个邻居”。常在首个 bootstrap 连接建立后触发。
    BootstrapDht,
    /// v2.5.3: 主动连接 bootstrap 节点
    AddBootstrap {
        addr: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// v2.5.3: 列出已连接对等节点 peer_id
    ListPeers {
        reply: oneshot::Sender<Vec<String>>,
    },
    /// v2.5.4: 连接公共 libp2p bootstrap/relay 网络
    BootstrapPublic {
        reply: oneshot::Sender<Vec<String>>,
    },
    /// v2.5.4: 获取 AutoNAT 检测的 NAT 状态
    NatStatus {
        reply: oneshot::Sender<String>,
    },
    /// v2.5.4: 经 Circuit Relay 中继监听（listen_on /p2p-circuit，建立并持有 reservation）
    ListenRelay {
        addr: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// v2.5.5: 探测候选 relay 是否提供 hop（dial + Identify 协议判定）
    ProbeRelay {
        addr: String,
        reply: oneshot::Sender<Result<bool, String>>,
    },
    /// v2.5.5: relay 池报告（列表 + 容量快照 + 当前活跃通道）
    ListRelayPool {
        reply: oneshot::Sender<serde_json::Value>,
    },
    /// v2.5.5: 手动加入 relay
    AddRelay {
        addr: String,
        class: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// v2.5.5: 移除 relay（并关闭其通道）
    RemoveRelayCmd {
        target: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// v2.5.5: 手动扩容（manual_bonus += amount），返回新有效上限
    ExpandCapacity {
        amount: i64,
        reply: oneshot::Sender<Result<i64, String>>,
    },
    /// v2.5.5: 维持/补齐多通道到目标数（自动选择 + listen），返回活跃 relay
    EnsureChannels {
        reply: oneshot::Sender<Vec<String>>,
    },
    /// v2.5.5: 触发 DHT 随机发现（扩充 hop 候选）
    DiscoverRelays,
    /// 跨节点 DHT 查找：对任意 key 发起迭代 Kademlia 查找，返回全局最接近的
    /// peer_id 列表（用于验证跨节点路由 / O(log n) 收敛）。
    FindPeers {
        key: String,
        reply: oneshot::Sender<Result<Vec<String>, String>>,
    },
}

#[derive(Clone, serde::Serialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub connected: usize,
    pub routing_entries: usize,
    pub listen_addrs: Vec<String>,
    pub bootstrapped: Vec<String>,
    /// v2.5.4: AutoNAT 检测的 NAT 状态
    pub nat_status: String,
}

pub type PeerCmdTx = mpsc::Sender<PeerCommand>;

/// 纠删码分片驱动的控制命令（REST → 分片驱动）。
#[allow(dead_code)]
pub enum ShardControl {
    /// 编码并分发一个 blob（成功返回 blob_id）。
    Ingest {
        blob_id: String,
        data: Vec<u8>,
        reply: oneshot::Sender<Result<String, String>>,
    },
    /// 重建一个 blob（不足时返回 pending 错误，由调用方稍后重试）。
    Reconstruct {
        blob_id: String,
        reply: oneshot::Sender<Result<Vec<u8>, String>>,
    },
}

/// 纠删码分片驱动控制句柄。
pub type ShardCtrlTx = mpsc::Sender<ShardControl>;

/// 当前 unix 毫秒墙钟时间（CRDT LWW 时间戳）。
pub(crate) fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 取 CRDT 存储锁；遇毒化锁按仓库既有约定恢复并继续（绝不 panic）。
fn crdt_guard(store: &Arc<std::sync::Mutex<CrdtStore>>) -> std::sync::MutexGuard<'_, CrdtStore> {
    store.lock().unwrap_or_else(|e| {
        eprintln!("⚠️ CRDT: 存储锁曾毒化，恢复后继续（请人工核查）");
        e.into_inner()
    })
}

/// 可 clone 的 CRDT 句柄：持有共享 LWW 存储 + 网络发布通道。
///
/// 由 run_daemon 构造一次，每连接 clone 给 REST/管理面使用；写操作经
/// `local_put` 本地应用后，把 `CrdtMessage::Op` 经 GossipSub 广播给对端。
#[derive(Clone)]
pub struct CrdtHandle {
    store: Arc<std::sync::Mutex<CrdtStore>>,
    peer: PeerCmdTx,
}

impl CrdtHandle {
    pub fn new(store: Arc<std::sync::Mutex<CrdtStore>>, peer: PeerCmdTx) -> Self {
        Self { store, peer }
    }

    /// 本地写入（推进本节点计数器 + LWW 应用），成功后把 Op 广播出去。
    ///
    /// 广播用 `try_send`（非阻塞）：通道满时只告警，不阻塞 REST 请求，也不丢本地写入。
    pub fn put(&self, key: &str, value: &str) -> Result<CrdtOp, String> {
        let now = now_millis();
        let op = crdt_guard(&self.store).local_put(key, value, now)?;
        match serde_json::to_vec(&CrdtMessage::Op(op.clone())) {
            Ok(data) => {
                if let Err(e) = self.peer.try_send(PeerCommand::Publish {
                    topic: CRDT_TOPIC.into(),
                    data,
                }) {
                    tracing::warn!("CRDT Op 广播 try_send 失败（已本地写入）: {e}");
                }
            }
            Err(e) => tracing::warn!("CRDT Op 序列化失败（已本地写入）: {e}"),
        }
        Ok(op)
    }

    /// 立即广播一次全量快照（管理/测试/反熵手动触发用）。
    pub fn broadcast_snapshot(&self) -> Result<(), String> {
        let snap = crdt_guard(&self.store).snapshot(now_millis());
        let data = serde_json::to_vec(&CrdtMessage::Snapshot(snap))
            .map_err(|e| format!("快照序列化失败: {e}"))?;
        self.peer
            .try_send(PeerCommand::Publish {
                topic: CRDT_TOPIC.into(),
                data,
            })
            .map_err(|e| format!("快照广播 try_send 失败: {e}"))?;
        Ok(())
    }

    /// 读取一个键的最新值。
    pub fn get(&self, key: &str) -> Option<String> {
        crdt_guard(&self.store).get(key).map(|s| s.to_string())
    }

    /// 读取一个键的完整物化条目（含 ts/origin）。
    pub fn entry(&self, key: &str) -> Option<LwwEntry> {
        crdt_guard(&self.store).entry(key).cloned()
    }

    /// 当前键数。
    pub fn len(&self) -> usize {
        crdt_guard(&self.store).len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        crdt_guard(&self.store).is_empty()
    }

    /// 全部条目（按 key 排序）。
    pub fn all(&self) -> Vec<LwwEntry> {
        crdt_guard(&self.store)
            .all_entries()
            .into_iter()
            .cloned()
            .collect()
    }

    /// 运行统计快照。
    pub fn stats(&self) -> CrdtStats {
        crdt_guard(&self.store).stats()
    }

    /// 估算状态字节数。
    pub fn size_bytes(&self) -> usize {
        crdt_guard(&self.store).size_bytes()
    }
}

// ───────────────────────── 参数 ─────────────────────────

#[derive(Debug, Clone)]
pub struct DaemonArgs {
    pub listen: String,
    pub port: u16,
    pub api_port: u16,
    pub data_dir: PathBuf,
    pub mode: String,
    pub bootstrap: Vec<String>,
    /// 自建 Relay 中继服务端：`true` 时本节点同时作为 Circuit Relay v2
    /// 中继（hop），为其他节点提供跨 NAT 的 reservation / circuit。
    pub relay_server: bool,
    /// 启动即注入的中继地址（自建/自有中继，分类 dedicated）。可多个。
    pub relay: Vec<String>,
    /// 中继服务端：最多同时承载的 circuit 数。
    pub relay_max_circuits: usize,
    /// 中继服务端：最多 reservation 数。
    pub relay_max_reservations: usize,
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
            relay_server: false,
            relay: Vec::new(),
            relay_max_circuits: 512,
            relay_max_reservations: 1024,
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
            return if rest.is_empty() {
                home
            } else {
                home.join(rest)
            };
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
    println!("  --relay-server      启用自建 Circuit Relay v2 中继服务端 (hop)");
    println!("  --relay <addr>      启动即注入的自有/自建中继 multiaddr (可多个, 分类 dedicated)");
    println!("  --relay-max-circuits <n>   中继最多 circuit 数 (默认: 512)");
    println!("  --relay-max-reservations <n> 中继最多 reservation 数 (默认: 1024)");
    println!("  --help              显示帮助");
}

/// 从 argv（不含程序名）解析
pub fn parse_daemon_args(args: &[String]) -> DaemonArgs {
    let mut d = DaemonArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" => {
                d.listen = require_value(args, i, "--listen");
                i += 2;
            }
            "--port" => {
                d.port = require_value(args, i, "--port").parse().unwrap_or(4001);
                i += 2;
            }
            "--api-port" => {
                d.api_port = require_value(args, i, "--api-port").parse().unwrap_or(4002);
                i += 2;
            }
            "--data-dir" => {
                d.data_dir = PathBuf::from(require_value(args, i, "--data-dir"));
                i += 2;
            }
            "--mode" => {
                d.mode = require_value(args, i, "--mode");
                i += 2;
            }
            "--bootstrap" => {
                while i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    d.bootstrap.push(args[i + 1].clone());
                    i += 1;
                }
                i += 1;
            }
            "--relay-server" => {
                d.relay_server = true;
                i += 1;
            }
            "--relay" => {
                while i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    d.relay.push(args[i + 1].clone());
                    i += 1;
                }
                i += 1;
            }
            "--relay-max-circuits" => {
                d.relay_max_circuits = require_value(args, i, "--relay-max-circuits")
                    .parse()
                    .unwrap_or(512);
                i += 2;
            }
            "--relay-max-reservations" => {
                d.relay_max_reservations = require_value(args, i, "--relay-max-reservations")
                    .parse()
                    .unwrap_or(1024);
                i += 2;
            }
            "--version" | "-V" => {
                // v3.5.5（W-02）：gsn-daemon / `gsn daemon` 支持 --version/-V，
                // 与 `gsn --version` 输出同一权威版本（gsn-core crate 版本）。
                println!("gsn-daemon {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                print_daemon_help();
                std::process::exit(0);
            }
            _ => {
                eprintln!("错误: 未知选项 {}", args[i]);
                std::process::exit(1);
            }
        }
    }
    d.data_dir = expand_tilde(d.data_dir);
    d
}

// ───────────────────────── --healthcheck（Docker 健康探针） ─────────────────────────

/// 是否请求了 `--healthcheck`（只识别标志本身，不解析其余参数）。
pub fn wants_healthcheck(args: &[String]) -> bool {
    args.iter().any(|a| a == "--healthcheck")
}

/// 执行一次健康探针：GET `GSN_HEALTHCHECK_URL`（默认 http://127.0.0.1:4002/health），
/// 3 秒超时；HTTP 2xx 返回退出码 0，否则（含连接失败/超时/非 2xx）返回 1。
///
/// 不启动节点，只做一次轻量 HTTP GET，供 Docker HEALTHCHECK 调用。
/// 用手写 TCP HTTP/1.1（不引新依赖），解析 host:port 后发 `GET <path>`。
pub async fn run_healthcheck_now() -> i32 {
    let url = std::env::var("GSN_HEALTHCHECK_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4002/health".to_string());
    let url = url.trim_end_matches('/').to_string();

    // 解析 scheme://host:port/path
    let after_proto = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => &url,
    };
    let (host_port, path) = match after_proto.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (after_proto, "/health".to_string()),
    };
    let (host, port) = host_port
        .split_once(':')
        .map(|(h, p)| (h.to_string(), p.parse::<u16>().unwrap_or(80)))
        .unwrap_or_else(|| (host_port.to_string(), 80));

    let fut = async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect((host.as_str(), port)).await?;
        let req = format!("GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
        stream.write_all(req.as_bytes()).await?;
        stream.flush().await?;
        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).await?;
        Ok::<u16, std::io::Error>({
            let text = String::from_utf8_lossy(&resp);
            text.lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(0)
        })
    };

    match tokio::time::timeout(std::time::Duration::from_secs(3), fut).await {
        Ok(Ok(status)) if (200..300).contains(&status) => {
            println!("healthcheck OK: {url} -> {status}");
            0
        }
        Ok(Ok(status)) => {
            eprintln!("healthcheck FAIL: {url} -> HTTP {status}");
            1
        }
        Ok(Err(e)) => {
            eprintln!("healthcheck FAIL: {url} 连接/读取失败: {e}");
            1
        }
        Err(_) => {
            eprintln!("healthcheck FAIL: {url} 超时（3s）");
            1
        }
    }
}

// ───────────────────────── swarm actor ─────────────────────────

async fn run_swarm_actor(
    mut peer: P2pPeer,
    mut cmd_rx: mpsc::Receiver<PeerCommand>,
    store: Arc<PersistentStore>,
    inbound_tx: mpsc::Sender<InboundGossipMessage>,
) {
    // 周期性驱动网络栈：Kademlia 路由刷新、AutoNAT 重测、dnsaddr 解析与连接
    // 状态机都依赖被反复 poll；select! 在命令到达时会取消 next_event future，
    // 部分子系统的 waker 无法保证唤醒，因此用一个静默 idle tick 兜底 poll。
    let mut idle = tokio::time::interval(std::time::Duration::from_secs(5));
    idle.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // P0-2: 冷启动 publish 重试。节点刚启动时 gossipsub mesh 尚未组建，
    // `publish` 会返回 InsufficientPeers；若不重试，这条本地写入/快照就永远
    // 发不出去（周期快照 15s 虽能兜底，但太慢且不稳定）。这里把失败消息
    // 放入 pending，每 2s 重试，最多 6 次（约 12s，覆盖 mesh 组建窗口）。
    let mut pending_publish: Vec<(String, Vec<u8>, u8)> = Vec::new();
    let mut retry_tick = tokio::time::interval(std::time::Duration::from_secs(2));

    // v2.5.4/5: relay reservation 应用层续期（真机实测 relay client 内部
    // renewal_timeout 在本机始终不自动续期）。每 80s（<120s 到期）对**所有**
    // 活跃 relay 通道显式续期；listen_via_relay 按 relay 先 remove 旧 listener 再建。
    let mut renew = tokio::time::interval(std::time::Duration::from_secs(80));
    renew.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // v2.5.5: 多通道状态
    //   active_relay_addrs: relay_id → addr（持有/待确认的通道，用于续期与切换）
    //   pending_probes:     probe dial 后等待 Identify 上报协议的回调
    //   connecting:         已发起 listen、尚未收到 ReservationReqAccepted 的 relay
    let mut active_relay_addrs: HashMap<String, String> = HashMap::new();
    let mut pending_probes: HashMap<PeerId, oneshot::Sender<Result<bool, String>>> = HashMap::new();
    let mut connecting: HashSet<PeerId> = HashSet::new();
    // 跨节点 DHT 查找：QueryId → 等待结果的回调（在 kad GetClosestPeers 结果
    // 事件到达时 resolve）。
    let mut pending_finds: HashMap<
        libp2p::kad::QueryId,
        oneshot::Sender<Result<Vec<String>, String>>,
    > = HashMap::new();

    loop {
        tokio::select! {
            event = peer.next_event() => {
                let event = match event {
                    Some(e) => e,
                    None => continue, // swarm 事件流结束（理论上不可达）
                };
                log_swarm_event(&event);
                let need_ensure = process_swarm_event(
                    &mut peer, &event, &store,
                    &mut active_relay_addrs, &mut pending_probes, &mut connecting,
                    &inbound_tx, &mut pending_finds,
                );
                if need_ensure {
                    let held = ensure_channels(&mut peer, &store, &mut active_relay_addrs, &mut connecting);
                    eprintln!("🔁 自动切换后当前通道: {:?}", held);
                }
            }
            maybe_cmd = cmd_rx.recv() => {
                match maybe_cmd {
                    Some(cmd) => handle_peer_command(
                        &mut peer, cmd, &store,
                        &mut active_relay_addrs, &mut pending_probes, &mut connecting,
                        &mut pending_publish, &mut pending_finds,
                    ),
                    None => break,
                }
            }
            _ = idle.tick() => {
                // 仅用于唤醒并重新 poll swarm，无额外动作
            }
            _ = retry_tick.tick() => {
                // 重试冷启动期间未能发出的 publish（mesh 未就绪）。
                if pending_publish.is_empty() { continue; }
                let mut still: Vec<(String, Vec<u8>, u8)> = Vec::new();
                for (topic, data, attempts) in pending_publish.drain(..) {
                    match peer.try_publish(&topic, data.clone()) {
                        Ok(()) => eprintln!("✅ 重试 publish 成功 [{topic}]（第{attempts}次）"),
                        Err(libp2p::gossipsub::PublishError::InsufficientPeers) => {
                            if attempts < 6 {
                                still.push((topic, data, attempts + 1));
                            } else {
                                eprintln!("⚠️ publish 重试 {attempts} 次仍 InsufficientPeers，放弃 [{topic}]");
                            }
                        }
                        Err(e) => eprintln!("⚠️ publish 重试失败 [{topic}]: {e}"),
                    }
                }
                pending_publish = still;
            }
            _ = renew.tick() => {
                for (id, addr) in &active_relay_addrs {
                    match peer.listen_via_relay(addr) {
                        Ok(_) => eprintln!("🔄 续期 relay reservation 已发起: {}", id),
                        Err(e) => eprintln!("⚠️ 续期失败 [{}]: {}", id, e),
                    }
                }
            }
        }
    }
}

// ─────────────── v2.5.5 事件处理：probe 结果 / hop 自动入池 / 掉线切换 ───────────────

/// 当前 UTC RFC3339
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// v3.5.6（AU-持久化）：中继池遥测/容量写入 SQLite 失败时**显式告警**而非静默丢弃。
///
/// 这些写入是尽力而为的运维状态（健康位/失败计数/容量元数据），不能让一次落盘失败
/// 打断 P2P 主路径（否则一次磁盘错误会拖垮整个 swarm 事件循环），因此控制流不中断；
/// 但旧实现用 `let _ =` 完全吞掉错误，运维无从察觉中继池状态可能正在与磁盘漂移。
/// 这里统一记录操作、对象 id 与根因，使「持久化失败」可观测（与 market_actor 的
/// `warn_persist` 同一原则：降级路径必须留痕）。
fn log_relay_persist_err(op: &str, id: &str, err: &anyhow::Error) {
    eprintln!("⚠️ [relay-persist] {op}({id}) 落盘失败（本次运行仍按内存状态继续）: {err}");
}

/// 从 start_time(RFC3339) 计算已运行天数
fn elapsed_days_since(start_iso: &str) -> i64 {
    if start_iso.is_empty() {
        return 0;
    }
    match chrono::DateTime::parse_from_rfc3339(start_iso) {
        Ok(t) => (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_days(),
        Err(_) => 0,
    }
}

/// 计算当前 relay 池容量快照
pub fn current_capacity(store: &PersistentStore) -> relay_pool::CapacitySnapshot {
    let start = store
        .get_meta("relay_pool:start_time")
        .ok()
        .flatten()
        .unwrap_or_default();
    let manual: i64 = store
        .get_meta("relay_pool:manual_bonus")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let days = elapsed_days_since(&start);
    let size = store.relay_count().unwrap_or(0) as i64;
    relay_pool::capacity_snapshot(days, manual, size)
}

/// 从 Identify 上报的 listen_addrs 中构造一个可用的 relay multiaddr（追加 /p2p）
fn build_relay_multiaddr(peer_id: &PeerId, info: &libp2p::identify::Info) -> Option<String> {
    let public_ip4 = |s: &str| -> bool {
        s.contains("/ip4/")
            && !s.contains("127.0.0.1")
            && !s.contains("/ip4/192.168.")
            && !s.contains("/ip4/10.")
            && !s.contains("/ip4/172.")
    };
    for a in &info.listen_addrs {
        let s = a.to_string();
        if public_ip4(&s) {
            return Some(format!("{}/p2p/{}", s, peer_id));
        }
    }
    // 域名 / AutoTLS / wss 地址（抗网络干扰）
    for a in &info.listen_addrs {
        let s = a.to_string();
        if s.contains("/dns") || s.contains("/tls/ws") || s.contains("/wss") {
            return Some(format!("{}/p2p/{}", s, peer_id));
        }
    }
    info.listen_addrs
        .first()
        .map(|a| format!("{}/p2p/{}", a, peer_id))
}

/// Identify 发现支持 hop 的公网节点：自动纳入 relay 池（DHT 发现扩充）
fn auto_adopt_hop_relay(peer_id: &PeerId, info: &libp2p::identify::Info, store: &PersistentStore) {
    let id = peer_id.to_string();
    let exists = store
        .load_relays()
        .map(|v| v.iter().any(|r| r.relay_id == id))
        .unwrap_or(false);
    if exists {
        if let Err(e) = store.set_relay_status(&id, true, "healthy", 0, 0, &now_iso()) {
            log_relay_persist_err("set_relay_status", &id, &e);
        }
        return;
    }
    let cap = current_capacity(store);
    if cap.remaining <= 0 {
        return; // 池满
    }
    if let Some(addr) = build_relay_multiaddr(peer_id, info) {
        let relay = StoredRelay {
            relay_id: id.clone(),
            multiaddr: addr,
            class: RelayClass::ThirdParty.as_str().to_string(),
            status: "healthy".to_string(),
            healthy: true,
            fail_count: 0,
            limit_sec: 0,
            data_bytes: 0,
            last_check: now_iso(),
            created_at: now_iso(),
        };
        if store.upsert_relay(&relay).is_ok() {
            eprintln!("➕ 自动纳入新 hop relay: {}", id);
        }
    }
}

/// 处理 swarm 事件中的关键状态变更；返回 true 表示有通道掉线、需要重新 ensure
///
/// 参数为各子系统的共享状态表（连接/重放/pending 集合等），显式允许较多参数：
/// 这些是 actor 内独立状态，强行合并会降低可读性。
#[allow(clippy::too_many_arguments)]
fn process_swarm_event(
    peer: &mut P2pPeer,
    event: &libp2p::swarm::SwarmEvent<crate::net::peer::PeerEvent>,
    store: &PersistentStore,
    active: &mut HashMap<String, String>,
    pending: &mut HashMap<PeerId, oneshot::Sender<Result<bool, String>>>,
    connecting: &mut HashSet<PeerId>,
    inbound_tx: &mpsc::Sender<InboundGossipMessage>,
    pending_finds: &mut HashMap<libp2p::kad::QueryId, oneshot::Sender<Result<Vec<String>, String>>>,
) -> bool {
    use libp2p::swarm::SwarmEvent::*;
    let mut need_ensure = false;
    match event {
        // P0-1: 连接建立后把对端地址登记进 Kademlia kbucket。
        // dial 成功本身不会让 Kademlia 认识对端；不 add_address 则 kbucket 恒空。
        ConnectionEstablished {
            peer_id, endpoint, ..
        } => {
            if endpoint.is_dialer() {
                // 本节点是拨号方：被 dial 的地址就是对端的确定可用地址，加入 kbucket。
                let addr = endpoint.get_remote_address().clone();
                peer.add_kad_address(*peer_id, addr);
            }
            // 本节点是监听方（对端主动连入）：连接的对端地址是“回送地址”
            // （send_back_addr，即对端发起本次连接的临时出站端口，不是它的
            // 监听端口）。若把它加入 kbucket，后续 FIND_NODE 会让别的节点去
            // dial 一个不通的临时端口（实测 "protocol not supported /
            // ConnectionRefused"）。监听方的正确可 dial 地址由随后
            // Identify.Received 用 info.listen_addrs 提供（见下方 Identify 臂）。
            // 连接建立后重放订阅，解决 GossipSub 冷启动死锁（详见
            // P2pPeer::refresh_gossipsub_subscriptions 的说明）。
            peer.refresh_gossipsub_subscriptions();
        }
        ConnectionClosed { peer_id, .. } => {
            let id = peer_id.to_string();
            if active.remove(&id).is_some() || connecting.remove(peer_id) {
                peer.remove_relay(&id);
                if let Err(e) = store.mark_relay_failed(&id, &now_iso()) {
                    log_relay_persist_err("mark_relay_failed", &id, &e);
                }
                eprintln!("🔌 中继通道掉线 {}，准备自动切换", id);
                need_ensure = true;
            }
        }
        OutgoingConnectionError {
            peer_id: Some(pid), ..
        } => {
            if connecting.remove(pid) {
                let id = pid.to_string();
                active.remove(&id);
                peer.remove_relay(&id);
                if let Err(e) = store.mark_relay_failed(&id, &now_iso()) {
                    log_relay_persist_err("mark_relay_failed", &id, &e);
                }
                eprintln!("❌ relay {} 连接失败，准备自动切换", id);
                need_ensure = true;
            }
        }
        OutgoingConnectionError { peer_id: None, .. } => {}
        Behaviour(bev) => {
            use crate::net::peer::PeerEvent::*;
            match bev {
                RelayServer(relay_server_ev) => {
                    // v3.9.13: 自建 Relay 中继服务端事件。仅 `--relay-server`
                    // 节点会产生；普通节点不产生该事件。仅记录，无需业务处理。
                    use libp2p::relay::Event as SrvEvent;
                    match relay_server_ev {
                        SrvEvent::ReservationReqAccepted { .. } => {
                            eprintln!("🔌 relay server: reservation 接受");
                        }
                        SrvEvent::CircuitReqAccepted { .. } => {
                            eprintln!("🔌 relay server: circuit 接受");
                        }
                        _ => {}
                    }
                }
                RelayClient(rc) => {
                    use libp2p::relay::client::Event as RcEvent;
                    if let RcEvent::ReservationReqAccepted {
                        relay_peer_id,
                        renewal,
                        limit,
                    } = rc
                    {
                        let id = relay_peer_id.to_string();
                        connecting.remove(relay_peer_id);
                        let (dur, data) = match limit {
                            Some(l) => (
                                l.duration().map(|d| d.as_secs() as i64).unwrap_or(0),
                                l.data_in_bytes().map(|x| x as i64).unwrap_or(0),
                            ),
                            None => (0, 0),
                        };
                        if let Err(e) =
                            store.set_relay_status(&id, true, "active", dur, data, &now_iso())
                        {
                            log_relay_persist_err("set_relay_status", &id, &e);
                        }
                        eprintln!(
                            "✅ relay reservation 已建立 {} (renewal={}, {}s/{}B)",
                            id, renewal, dur, data
                        );
                    }
                }
                Identify(libp2p::identify::Event::Received { peer_id, info, .. }) => {
                    let supports_hop = info.protocols.iter().any(|p| {
                        let s = p.to_string();
                        s.contains("circuit/relay") && s.ends_with("/hop")
                    });
                    if let Some(tx) = pending.remove(peer_id) {
                        let _ = tx.send(Ok(supports_hop));
                    }
                    if supports_hop {
                        auto_adopt_hop_relay(peer_id, info, store);
                    }
                    // P0-1: Identify 上报的 listen_addrs 全部登记进 DHT（种子候选地址）。
                    // 这些是对端声明的可被 dial 的地址，比 ConnectionEstablished 的
                    // 对端 socket 地址更全（含公网/中继地址），尽力而为、不报错。
                    for a in &info.listen_addrs {
                        peer.add_kad_address(*peer_id, a.clone());
                    }
                }
                Identify(_) => {}
                // v2.7.5: DCUtR 直连升级结果——成功则纳入 direct_peers
                Dcutr(e) => {
                    if peer.note_dcutr_event(e) {
                        eprintln!("⛏️ DCUtR 直连升级成功: {}", e.remote_peer_id);
                    } else if e.result.is_err() {
                        eprintln!("⛏️ DCUtR 直连失败（继续走 relay）: {}", e.remote_peer_id);
                    }
                }
                // P0-2: 入站 GossipSub 消息——先按 message id 去重，首次放行投递到应用层。
                // （libp2p 0.54 gossipsub Event::Message 字段为 propagation_source/message_id/message）
                Gossipsub(libp2p::gossipsub::Event::Message {
                    propagation_source,
                    message_id,
                    message,
                }) => {
                    if peer.gossip_dedup_first_seen(message_id) {
                        let msg = InboundGossipMessage {
                            topic: message.topic.as_str().to_string(),
                            source: propagation_source.to_string(),
                            data: message.data.clone(),
                        };
                        if inbound_tx.try_send(msg).is_err() {
                            tracing::warn!("inbound gossip channel 已满/已关闭，丢弃消息");
                        }
                    } else {
                        tracing::debug!(%message_id, "重复 gossip 消息，已丢弃");
                    }
                }
                Gossipsub(_) => {}
                // 跨节点 DHT 查找结果到达：resolve 等待中的 FindPeers 回调。
                Kademlia(libp2p::kad::Event::OutboundQueryProgressed {
                    id,
                    result: libp2p::kad::QueryResult::GetClosestPeers(outcome),
                    ..
                }) => {
                    if let Some(tx) = pending_finds.remove(id) {
                        match outcome {
                            Ok(ok) => {
                                let peers: Vec<String> =
                                    ok.peers.iter().map(|p| p.peer_id.to_string()).collect();
                                let _ = tx.send(Ok(peers));
                            }
                            Err(err) => {
                                let _ = tx.send(Err(format!("{err:?}")));
                            }
                        }
                    }
                }
                Kademlia(_) => {}
                _ => {}
            }
        }
        _ => {}
    }
    need_ensure
}

/// v2.5.5: 维持/补齐多通道到目标数（DEFAULT_PARALLEL_RELAYS）。
/// 从健康池按分类优先级选 relay，listen_via_relay 发起，返回当前持有通道。
fn ensure_channels(
    peer: &mut P2pPeer,
    store: &PersistentStore,
    active: &mut HashMap<String, String>,
    connecting: &mut HashSet<PeerId>,
) -> Vec<String> {
    let target = DEFAULT_PARALLEL_RELAYS;
    let mut held: HashSet<String> = peer.active_relay_ids().into_iter().collect();
    let relays = store.load_relays().unwrap_or_default();
    let mut in_use = held.clone();
    for p in connecting.iter() {
        in_use.insert(p.to_string());
    }

    let mut guard = 0;
    while held.len() < target && guard < 20 {
        guard += 1;
        match relay_pool::select_replacement(&relays, &in_use) {
            Some(r) => match peer.listen_via_relay(&r.multiaddr) {
                Ok(_) => {
                    eprintln!("🛰️ 发起 relay 通道: {}", r.relay_id);
                    held.insert(r.relay_id.clone());
                    in_use.insert(r.relay_id.clone());
                    active.insert(r.relay_id.clone(), r.multiaddr.clone());
                    if let Ok(pid) = r.relay_id.parse::<PeerId>() {
                        connecting.insert(pid);
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ relay {} listen 失败: {}", r.relay_id, e);
                    if let Err(e) = store.mark_relay_failed(&r.relay_id, &now_iso()) {
                        log_relay_persist_err("mark_relay_failed", &r.relay_id, &e);
                    }
                    in_use.insert(r.relay_id.clone());
                }
            },
            None => break, // 无更多健康候选
        }
    }
    held.into_iter().collect()
}

/// v2.5.5: 主动维护一轮（独立 task 调用）：发现 → probe 未知 → 补齐通道 → 清理 dead
async fn run_relay_maintenance(store: Arc<PersistentStore>, cmd_tx: PeerCmdTx) {
    // 1. DHT 随机发现（hop 节点会在 Identify 时自动入池）
    let _ = cmd_tx.send(PeerCommand::DiscoverRelays).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 2. probe 池中不健康/未知候选
    let candidates = store.load_relays().unwrap_or_default();
    for r in candidates.iter().filter(|x| !x.healthy) {
        let (tx, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::ProbeRelay {
                addr: r.multiaddr.clone(),
                reply: tx,
            })
            .await
            .is_err()
        {
            continue;
        }
        match tokio::time::timeout(std::time::Duration::from_secs(15), rx).await {
            Ok(Ok(Ok(true))) => {
                if let Err(e) =
                    store.set_relay_status(&r.relay_id, true, "healthy", 0, 0, &now_iso())
                {
                    log_relay_persist_err("set_relay_status", &r.relay_id, &e);
                }
                eprintln!("✅ probe 确认 hop relay: {}", r.relay_id);
            }
            Ok(Ok(Ok(false))) => {
                if let Err(e) = store.mark_relay_failed(&r.relay_id, &now_iso()) {
                    log_relay_persist_err("mark_relay_failed", &r.relay_id, &e);
                }
            }
            _ => {
                if let Err(e) = store.mark_relay_failed(&r.relay_id, &now_iso()) {
                    log_relay_persist_err("mark_relay_failed", &r.relay_id, &e);
                }
            }
        }
    }

    // 3. 补齐多通道
    let (tx, rx) = oneshot::channel();
    if cmd_tx
        .send(PeerCommand::EnsureChannels { reply: tx })
        .await
        .is_ok()
    {
        if let Ok(chans) = tokio::time::timeout(std::time::Duration::from_secs(20), rx).await {
            eprintln!("🛰️ 当前多通道: {:?}", chans.unwrap_or_default());
        }
    }

    // 4. 清理失效（dead）节点
    let removed = store.delete_dead_relays().unwrap_or(0);
    if removed > 0 {
        eprintln!("🧹 清理 dead relay {} 个", removed);
    }
}

/// v2.5.4: 简洁记录关键 swarm 事件（连接建立/关闭/dial 错误/外部地址候选）
fn log_swarm_event(event: &libp2p::swarm::SwarmEvent<crate::net::peer::PeerEvent>) {
    use libp2p::swarm::SwarmEvent::*;
    match event {
        ConnectionEstablished {
            peer_id,
            endpoint,
            established_in,
            ..
        } => {
            eprintln!("已连接 {peer_id} ({endpoint:?}, {established_in:?})");
        }
        ConnectionClosed {
            peer_id, endpoint, ..
        } => {
            eprintln!("连接关闭 {peer_id} ({endpoint:?})");
        }
        OutgoingConnectionError { peer_id, error, .. } => {
            eprintln!("出站连接失败 peer={peer_id:?}: {error:?}");
        }
        IncomingConnectionError {
            send_back_addr,
            error,
            ..
        } => {
            eprintln!("入站连接失败 {send_back_addr}: {error:?}");
        }
        NewExternalAddrCandidate { address } => {
            eprintln!("外部地址候选 {address}");
        }
        NewListenAddr { address, .. } => {
            eprintln!("✅ 新监听地址 {address}");
        }
        ExpiredListenAddr { address, .. } => {
            eprintln!("监听地址过期 {address}");
        }
        Behaviour(bev) => {
            use crate::net::peer::PeerEvent::*;
            match bev {
                RelayClient(e) => eprintln!("🔌 RelayClient {e:?}"),
                Identify(e) => eprintln!("🏷️ Identify {e:?}"),
                AutoNat(e) => eprintln!("🧭 AutoNat {e:?}"),
                Dcutr(e) => {
                    // v2.7.5: 日志在 process_swarm_event 里记录（那里有 &mut peer）
                    let _ = &e;
                }
                Kademlia(e) => {
                    use libp2p::kad::Event::*;
                    if let RoutingUpdated { peer, .. } = e {
                        eprintln!("📡 DHT 路由更新 {peer}");
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}
// 内部命令分发器：参数为各类共享状态（peer / store / 连接与重放/pending 集合）。
// 显式允许较多参数：这些都是 actor 内的独立状态表，强行合并会降低可读性。
#[allow(clippy::too_many_arguments)]
fn handle_peer_command(
    peer: &mut P2pPeer,
    cmd: PeerCommand,
    store: &PersistentStore,
    active: &mut HashMap<String, String>,
    pending: &mut HashMap<PeerId, oneshot::Sender<Result<bool, String>>>,
    connecting: &mut HashSet<PeerId>,
    pending_publish: &mut Vec<(String, Vec<u8>, u8)>,
    pending_finds: &mut HashMap<libp2p::kad::QueryId, oneshot::Sender<Result<Vec<String>, String>>>,
) {
    match cmd {
        PeerCommand::GetInfo { reply } => {
            let info = PeerInfo {
                peer_id: peer.peer_id.to_string(),
                connected: peer.connected_peers(),
                routing_entries: peer.routing_table_size(),
                listen_addrs: peer.listen_addrs(),
                bootstrapped: peer.bootstrapped(),
                nat_status: peer.nat_status(),
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
            match peer.try_publish(&topic, data.clone()) {
                Ok(()) => {}
                // mesh 尚未组建 → 排队重试，而不是丢弃（冷启动关键路径）。
                Err(libp2p::gossipsub::PublishError::InsufficientPeers) => {
                    eprintln!("⚠️ publish 暂不可发 [{topic}]：mesh 未就绪，已排队重试");
                    pending_publish.push((topic, data, 1));
                }
                Err(e) => eprintln!("⚠️ publish 失败 [{topic}]: {e}"),
            }
        }
        PeerCommand::BootstrapDht => match peer.dht_bootstrap() {
            Ok(()) => eprintln!("🔁 Kademlia 自举已发起（迭代填充路由表）"),
            Err(e) => eprintln!("⚠️ Kademlia 自举暂不可用（{e}），稍后重试"),
        },
        PeerCommand::AddBootstrap { addr, reply } => {
            let result = peer.add_bootstrap_from_str(&addr);
            match &result {
                Ok(_) => eprintln!("✅ bootstrap 连接已发起: {}", addr),
                Err(e) => eprintln!("⚠️ bootstrap 失败 [{}]: {}", addr, e),
            }
            let _ = reply.send(result);
        }
        PeerCommand::ListPeers { reply } => {
            let peers = peer.connected_peer_ids();
            let _ = reply.send(peers);
        }
        PeerCommand::BootstrapPublic { reply } => {
            let initiated = peer.bootstrap_public_network();
            let _ = reply.send(initiated);
        }
        PeerCommand::NatStatus { reply } => {
            let status = peer.nat_status();
            let _ = reply.send(status);
        }
        PeerCommand::ListenRelay { addr, reply } => {
            let result = peer.listen_via_relay(&addr);
            if result.is_ok() {
                eprintln!("✅ relay reservation 监听已发起: {}", addr);
                if let Ok(base) = addr.parse::<Multiaddr>() {
                    if let Some(id) = crate::net::peer::extract_relay_peer_id(&base) {
                        active.insert(id, addr.clone());
                        if let Some(pid) = crate::net::peer::extract_relay_peer_id(&base)
                            .and_then(|s| s.parse::<PeerId>().ok())
                        {
                            connecting.insert(pid);
                        }
                    }
                }
            } else if let Err(e) = &result {
                eprintln!("⚠️ relay listen 失败 [{}]: {}", addr, e);
            }
            let _ = reply.send(result);
        }
        PeerCommand::ProbeRelay { addr, reply } => {
            let base: Result<Multiaddr, _> = addr.parse();
            match base {
                Ok(base) => {
                    let pid = crate::net::peer::extract_relay_peer_id(&base)
                        .and_then(|s| s.parse::<PeerId>().ok());
                    match peer.probe_relay(&addr) {
                        Ok(_) => match pid {
                            Some(pid) => {
                                pending.insert(pid, reply);
                            }
                            // 无 /p2p：dial 后由 Identify 自动入池，无法关联本次 probe
                            None => {
                                let _ = reply.send(Ok(false));
                            }
                        },
                        Err(e) => {
                            let _ = reply.send(Err(e));
                        }
                    }
                }
                Err(e) => {
                    let _ = reply.send(Err(format!("addr 解析失败: {}", e)));
                }
            }
        }
        PeerCommand::ListRelayPool { reply } => {
            let relays = store.load_relays().unwrap_or_default();
            let cap = current_capacity(store);
            let active_ids = peer.active_relay_ids();
            let v = serde_json::json!({
                "relays": relays,
                "capacity": cap,
                "active": active_ids,
                "target_channels": DEFAULT_PARALLEL_RELAYS,
                "relay_count": relays.len(),
                "direct_peers": peer.direct_peer_ids(),
            });
            let _ = reply.send(v);
        }
        PeerCommand::AddRelay { addr, class, reply } => {
            let base: Result<Multiaddr, _> = addr.parse();
            match base {
                Ok(base) => match crate::net::peer::extract_relay_peer_id(&base) {
                    Some(id) => {
                        let cap = current_capacity(store);
                        let exists = store
                            .load_relays()
                            .map(|v| v.iter().any(|r| r.relay_id == id))
                            .unwrap_or(false);
                        if !exists && cap.remaining <= 0 {
                            let _ = reply.send(Err("relay_pool_full".to_string()));
                            return;
                        }
                        let cls = if class.is_empty() {
                            RelayClass::General
                        } else {
                            class.parse().unwrap_or(RelayClass::General)
                        };
                        let relay = StoredRelay {
                            relay_id: id,
                            multiaddr: addr,
                            class: cls.as_str().to_string(),
                            status: "unknown".to_string(),
                            healthy: false,
                            fail_count: 0,
                            limit_sec: 0,
                            data_bytes: 0,
                            last_check: now_iso(),
                            created_at: now_iso(),
                        };
                        let res = store.upsert_relay(&relay).map_err(|e| e.to_string());
                        let _ = reply.send(res);
                    }
                    None => {
                        let _ = reply.send(Err("addr 缺少 /p2p/<peer_id>".to_string()));
                    }
                },
                Err(e) => {
                    let _ = reply.send(Err(format!("addr 解析失败: {}", e)));
                }
            }
        }
        PeerCommand::RemoveRelayCmd { target, reply } => {
            peer.remove_relay(&target);
            if let Ok(pid) = target.parse::<PeerId>() {
                let id = pid.to_string();
                active.remove(&id);
                connecting.remove(&pid);
                if let Err(e) = store.delete_relay(&id) {
                    log_relay_persist_err("delete_relay", &id, &e);
                }
            }
            let _ = reply.send(Ok(()));
        }
        PeerCommand::ExpandCapacity { amount, reply } => {
            if amount == 0 {
                let _ = reply.send(Err("amount_zero".to_string()));
                return;
            }
            let cur: i64 = store
                .get_meta("relay_pool:manual_bonus")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let new = (cur + amount).max(0);
            if let Err(e) = store.set_meta("relay_pool:manual_bonus", &new.to_string()) {
                log_relay_persist_err("set_meta", "relay_pool:manual_bonus", &e);
            }
            let cap = current_capacity(store);
            let _ = reply.send(Ok(cap.effective_cap));
        }
        PeerCommand::EnsureChannels { reply } => {
            let held = ensure_channels(peer, store, active, connecting);
            let _ = reply.send(held);
        }
        PeerCommand::DiscoverRelays => {
            peer.discover_random_peers();
            eprintln!("🔍 触发 DHT relay 候选发现");
        }
        PeerCommand::FindPeers { key, reply } => {
            let qid = peer.find_closest_peers(&key);
            pending_finds.insert(qid, reply);
        }
    }
}

// ───────────────────────── HTTP 工具 ─────────────────────────

/// v2.8.5（GAP §3.5）：CORS 不再使用通配 `Access-Control-Allow-Origin: *`。
///
/// 从环境变量 `REST_ALLOWED_ORIGINS`（逗号分隔）读取白名单；请求 `Origin`
/// 命中白名单时回显该 Origin 并附 `Vary: Origin`，否则不回显任何跨域许可
/// （默认空 = 不允许任何跨域）。
fn compute_cors(origin: Option<&str>) -> String {
    let allowed = std::env::var("REST_ALLOWED_ORIGINS").unwrap_or_default();
    let mut out = String::new();
    if let Some(origin) = origin {
        if !origin.is_empty() && allowed.split(',').map(|s| s.trim()).any(|s| s == origin) {
            out.push_str(&format!("Access-Control-Allow-Origin: {origin}\r\n"));
            out.push_str("Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n");
            out.push_str("Access-Control-Allow-Headers: Content-Type, Authorization\r\n");
            out.push_str("Vary: Origin\r\n");
        }
    }
    out
}

/// v2.8.5（GAP §3.5）：REST 变更类接口 fail-closed 认证。
///
/// - GET/HEAD/OPTIONS 放行（只读）；
/// - 配置了 `REST_BEARER_TOKEN` 时，POST/PUT/PATCH/DELETE 必须携带
///   `Authorization: Bearer <token>` 且匹配，否则 401；
/// - 未配置令牌时默认 401（fail-closed），仅当显式设置
///   `REST_ALLOW_UNAUTHENTICATED=1` 才放行（受信网络的逃生口，打印警告）。
///
/// 返回 `Err(状态码, 状态文本, 响应体)`，调用方据此返回 401。
/// 从认证头提取稳定主体标识（v2.8.7 沙箱所有权用）。
///
/// - 有效 Bearer → `sub:<token 的 sha256 前 16 hex>`，不泄露明文；
/// - 无 Bearer 但显式 `REST_ALLOW_UNAUTHENTICATED=1` → `sub:anonymous`
///   （无主体隔离，仅限本地受信开发）；
/// - 其余 → None（配置了 token 的变更类在 rest_authorize 已被挡；
///   handle_api 变更类再以 401 兜底）。
pub(crate) fn extract_caller(auth_header: Option<&str>) -> Option<String> {
    use sha2::Digest;
    let token = auth_header
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(token) = token {
        let digest = sha2::Sha256::digest(token.as_bytes());
        let hex = hex::encode(digest);
        return Some(format!("sub:{}", &hex[..16]));
    }
    if std::env::var("REST_ALLOW_UNAUTHENTICATED")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return Some("sub:anonymous".to_string());
    }
    None
}

fn rest_authorize(
    method: &str,
    auth_header: Option<&str>,
) -> Result<(), (u16, &'static str, String)> {
    let m = method.to_uppercase();
    if matches!(m.as_str(), "GET" | "HEAD" | "OPTIONS") {
        return Ok(());
    }
    let expected = std::env::var("REST_BEARER_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    if let Some(expected) = expected {
        let provided = auth_header
            .and_then(|h| {
                h.strip_prefix("Bearer ")
                    .or_else(|| h.strip_prefix("bearer "))
            })
            .map(|s| s.trim())
            .unwrap_or("");
        // v3.5.1（AU-32）：常量时间比较，避免逐字节短路泄露 token 前缀（时序侧信道）
        if crate::security::constant_time_eq_str(provided, &expected) {
            return Ok(());
        }
        return Err((
            401,
            "Unauthorized",
            serde_json::json!({ "error": "认证失败：Bearer 令牌缺失或不匹配" }).to_string(),
        ));
    }
    // 未配置令牌：默认 fail-closed；仅显式逃生口放行。
    if std::env::var("REST_ALLOW_UNAUTHENTICATED")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        eprintln!("⚠️ REST_ALLOW_UNAUTHENTICATED=1：变更接口在无认证下开放（仅限受信网络）");
        return Ok(());
    }
    Err((
        401,
        "Unauthorized",
        serde_json::json!({
            "error": "变更接口默认拒绝：请配置 REST_BEARER_TOKEN，或显式设置 REST_ALLOW_UNAUTHENTICATED=1"
        })
        .to_string(),
    ))
}

pub fn http_response(
    status: u16,
    status_text: &str,
    body: String,
    content_type: &str,
    extra: &str,
) -> String {
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{}",
        status, status_text, content_type, body.len(), body
    )
}

async fn fetch_peer_info(cmd_tx: &PeerCmdTx) -> Option<PeerInfo> {
    let (reply, rx) = oneshot::channel();
    cmd_tx.send(PeerCommand::GetInfo { reply }).await.ok()?;
    rx.await.ok()
}

/// 拉取已连接 peer_id 列表（分片驱动周期性刷新确定性放置的节点集合）。
async fn fetch_peer_ids(cmd_tx: &PeerCmdTx) -> Option<Vec<String>> {
    let (reply, rx) = oneshot::channel();
    cmd_tx.send(PeerCommand::ListPeers { reply }).await.ok()?;
    let ids = tokio::time::timeout(std::time::Duration::from_secs(2), rx)
        .await
        .ok()?
        .ok()?;
    Some(ids)
}

/// 纠删码 REST 端点：ingest / reconstruct。
///
/// - `POST /erasure/ingest`：body `{"blob_id":"...(可选)","data":"UTF-8 文本"}`，
///   成功返回 blob_id；blob_id 缺省时用时间戳生成。
/// - `POST /erasure/reconstruct`：body `{"blob_id":"..."}`，成功返回重建数据（hex +
///   UTF-8 文本，若可解码）；数据不足时返回 409 + pending 提示。
async fn handle_erasure_api(
    method: &str,
    path: &str,
    body: &str,
    shard_ctrl: &ShardCtrlTx,
) -> (u16, String) {
    use serde_json::Value;
    fn json_err(status: u16, code: &str, msg: impl Into<String>) -> (u16, String) {
        (
            status,
            serde_json::json!({"error": code, "message": msg.into()}).to_string(),
        )
    }
    if method != "POST" {
        return json_err(404, "NOT_FOUND", format!("仅支持 POST: {method} {path}"));
    }
    let req: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return json_err(400, "BAD_BODY", format!("请求体非 JSON: {e}")),
    };
    let p = path.split('?').next().unwrap_or(path);
    match p {
        "/ingest" => {
            // 支持 data(UTF-8) 或 data_hex(十六进制)。
            let data: Vec<u8> = if let Some(s) = req.get("data_hex").and_then(|v| v.as_str()) {
                let h = s.strip_prefix("0x").unwrap_or(s);
                match hex::decode(h) {
                    Ok(b) => b,
                    Err(e) => return json_err(400, "BAD_BODY", format!("data_hex 非法: {e}")),
                }
            } else {
                req.get("data")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .as_bytes()
                    .to_vec()
            };
            if data.is_empty() {
                return json_err(400, "BAD_BODY", "data 不能为空");
            }
            let blob_id = req
                .get("blob_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("blob-{}", now_millis()));
            let (tx, rx) = oneshot::channel();
            if shard_ctrl
                .send(ShardControl::Ingest {
                    blob_id: blob_id.clone(),
                    data,
                    reply: tx,
                })
                .await
                .is_err()
            {
                return json_err(500, "DRIVER", "分片驱动不可用");
            }
            match tokio::time::timeout(std::time::Duration::from_secs(3), rx).await {
                Ok(Ok(Ok(id))) => (
                    200,
                    serde_json::json!({
                        "status": "ingested",
                        "blob_id": id,
                        "note": "已编码并按确定性放置分发；重建请 POST /erasure/reconstruct。",
                    })
                    .to_string(),
                ),
                Ok(Ok(Err(e))) => json_err(500, "INGEST", e),
                Ok(Err(_)) => json_err(500, "INGEST", "驱动回执丢失"),
                Err(_) => json_err(500, "INGEST", "ingest 超时"),
            }
        }
        "/reconstruct" => {
            let blob_id = match req.get("blob_id").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => return json_err(400, "BAD_BODY", "缺少 blob_id"),
            };
            let (tx, rx) = oneshot::channel();
            if shard_ctrl
                .send(ShardControl::Reconstruct {
                    blob_id: blob_id.clone(),
                    reply: tx,
                })
                .await
                .is_err()
            {
                return json_err(500, "DRIVER", "分片驱动不可用");
            }
            match tokio::time::timeout(std::time::Duration::from_secs(3), rx).await {
                Ok(Ok(Ok(bytes))) => (
                    200,
                    serde_json::json!({
                        "blob_id": blob_id,
                        "data_hex": format!("0x{}", hex::encode(&bytes)),
                        "data_utf8": String::from_utf8_lossy(&bytes).to_string(),
                    })
                    .to_string(),
                ),
                Ok(Ok(Err(e))) => {
                    if e.contains("pending") {
                        json_err(409, "PENDING", format!("{e}；请稍后重试。"))
                    } else {
                        json_err(500, "RECONSTRUCT", e)
                    }
                }
                Ok(Err(_)) => json_err(500, "RECONSTRUCT", "驱动回执丢失"),
                Err(_) => json_err(500, "RECONSTRUCT", "reconstruct 超时"),
            }
        }
        _ => json_err(404, "NOT_FOUND", format!("未知 erasure 路径: {path}")),
    }
}

/// 渲染 Prometheus 文本 exposition（# HELP/#TYPE + 值）。纯函数，便于单测。
pub(crate) fn render_prometheus_metrics(
    uptime_seconds: f64,
    connected_peers: usize,
    dht_routing_entries: usize,
    s: &CrdtStats,
    crdt_keys: usize,
    crdt_size_bytes: usize,
) -> String {
    format!(
        "# HELP gsn_uptime_seconds 节点运行时长（秒）\n# TYPE gsn_uptime_seconds gauge\ngsn_uptime_seconds {uptime:.3}\n\
         # HELP gsn_connected_peers 当前已连接对等节点数\n# TYPE gsn_connected_peers gauge\ngsn_connected_peers {connected}\n\
         # HELP gsn_dht_routing_entries Kademlia 路由表条目数\n# TYPE gsn_dht_routing_entries gauge\ngsn_dht_routing_entries {routing}\n\
         # HELP gsn_crdt_keys CRDT 当前键数\n# TYPE gsn_crdt_keys gauge\ngsn_crdt_keys {keys}\n\
         # HELP gsn_crdt_size_bytes CRDT 估算状态字节数\n# TYPE gsn_crdt_size_bytes gauge\ngsn_crdt_size_bytes {size}\n\
         # HELP gsn_crdt_inbound_ops_total CRDT 入站同步报文累计数\n# TYPE gsn_crdt_inbound_ops_total counter\ngsn_crdt_inbound_ops_total {inbound}\n\
         # HELP gsn_crdt_applied_ops_total CRDT 实际应用的写操作累计数\n# TYPE gsn_crdt_applied_ops_total counter\ngsn_crdt_applied_ops_total {applied}\n\
         # HELP gsn_crdt_dropped_capped_total CRDT 因达键数上限被丢弃的入站新键累计数\n# TYPE gsn_crdt_dropped_capped_total counter\ngsn_crdt_dropped_capped_total {dropped}\n",
        uptime = uptime_seconds,
        connected = connected_peers,
        routing = dht_routing_entries,
        keys = crdt_keys,
        size = crdt_size_bytes,
        inbound = s.inbound_ops,
        applied = s.applied_ops,
        dropped = s.dropped_capped,
    )
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
    query_string: &str,
    body: &str,
    cmd_tx: &PeerCmdTx,
    _start: &Instant,
) -> (u16, String) {
    let path = net_path.trim_end_matches('/');

    // GET /peers
    if method == "GET" && (path == "/peers" || path == "/peers/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::ListPeers { reply }).await.is_err() {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(peers) => (
                200,
                serde_json::json!({"peers": peers, "count": peers.len()}).to_string(),
            ),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // GET /info
    else if method == "GET" && (path == "/info" || path.is_empty() || path == "/") {
        match fetch_peer_info(cmd_tx).await {
            Some(info) => (
                200,
                serde_json::to_string(&info).unwrap_or_else(|_| "{}".into()),
            ),
            None => (
                500,
                serde_json::json!({"error":"peer_unavailable"}).to_string(),
            ),
        }
    }
    // POST /bootstrap
    else if method == "POST" && (path == "/bootstrap" || path == "/bootstrap/") {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(body);
        let addr = match parsed {
            Ok(v) => v
                .get("addr")
                .and_then(|a| a.as_str())
                .map(|s| s.to_string()),
            Err(_) => None,
        };
        let addr = match addr {
            Some(a) => a,
            None => {
                return (
                    400,
                    serde_json::json!({"error":"missing_or_invalid_addr"}).to_string(),
                )
            }
        };
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::AddBootstrap {
                addr: addr.clone(),
                reply,
            })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(Ok(())) => (
                201,
                serde_json::json!({"status":"bootstrap_initiated","addr":addr}).to_string(),
            ),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.4: POST /bootstrap-public — 连接公共 libp2p 中继网络
    else if method == "POST" && (path == "/bootstrap-public" || path == "/bootstrap-public/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::BootstrapPublic { reply })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(initiated) => (
                201,
                serde_json::json!({
                    "status":"public_network_bootstrap_initiated",
                    "initiated": initiated,
                    "count": initiated.len()
                })
                .to_string(),
            ),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.4: GET /nat — 获取 AutoNAT 检测的 NAT 状态
    else if method == "GET" && (path == "/nat" || path == "/nat/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::NatStatus { reply }).await.is_err() {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(status) => (200, serde_json::json!({"nat_status": status}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.4: POST /relay-listen — 经 Circuit Relay 中继监听（建立并持有 reservation）
    // body: {"addr":"/ip4/15.235.144.210/tcp/4001/p2p/QmcZf59b..."}
    else if method == "POST" && (path == "/relay-listen" || path == "/relay-listen/") {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(body);
        let addr = match parsed {
            Ok(v) => v
                .get("addr")
                .and_then(|a| a.as_str())
                .map(|s| s.to_string()),
            Err(_) => None,
        };
        let addr = match addr {
            Some(a) => a,
            None => {
                return (
                    400,
                    serde_json::json!({"error":"missing_or_invalid_addr"}).to_string(),
                )
            }
        };
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::ListenRelay {
                addr: addr.clone(),
                reply,
            })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(Ok(())) => (
                201,
                serde_json::json!({"status":"relay_listen_initiated","addr":addr}).to_string(),
            ),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: GET /relays — relay 池报告（列表 + 容量快照 + 活跃通道）
    else if method == "GET" && (path == "/relays" || path == "/relays/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::ListRelayPool { reply })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(v) => (200, v.to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: POST /relays/add — 手动加入 relay（body: {"addr":"...","class":"general"}）
    else if method == "POST" && (path == "/relays/add" || path == "/relays/add/") {
        let v: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let addr = v
            .get("addr")
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .to_string();
        let class = v
            .get("class")
            .and_then(|a| a.as_str())
            .unwrap_or("general")
            .to_string();
        if addr.is_empty() {
            return (400, serde_json::json!({"error":"missing_addr"}).to_string());
        }
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::AddRelay {
                addr: addr.clone(),
                class,
                reply,
            })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(Ok(())) => (
                201,
                serde_json::json!({"status":"relay_added","addr":addr}).to_string(),
            ),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: POST /relays/remove — 移除 relay 并关闭通道（body: {"target":"<peer_id 或 addr>"}）
    else if method == "POST" && (path == "/relays/remove" || path == "/relays/remove/") {
        let v: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let target = v
            .get("target")
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .to_string();
        if target.is_empty() {
            return (
                400,
                serde_json::json!({"error":"missing_target"}).to_string(),
            );
        }
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::RemoveRelayCmd {
                target: target.clone(),
                reply,
            })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(Ok(())) => (
                200,
                serde_json::json!({"status":"relay_removed","target":target}).to_string(),
            ),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: POST /relays/expand — 手动扩容（body: {"amount":50000}）
    else if method == "POST" && (path == "/relays/expand" || path == "/relays/expand/") {
        let v: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let amount = v.get("amount").and_then(|a| a.as_i64()).unwrap_or(0);
        if amount == 0 {
            return (
                400,
                serde_json::json!({"error":"missing_or_zero_amount"}).to_string(),
            );
        }
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::ExpandCapacity { amount, reply })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(Ok(cap)) => (
                200,
                serde_json::json!({"status":"capacity_expanded","effective_cap":cap}).to_string(),
            ),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: POST /relays/discover — 触发 DHT relay 候选发现
    else if method == "POST" && (path == "/relays/discover" || path == "/relays/discover/") {
        if cmd_tx.send(PeerCommand::DiscoverRelays).await.is_err() {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        (
            202,
            serde_json::json!({"status":"relay_discovery_initiated"}).to_string(),
        )
    }
    // v2.5.5: POST /relays/ensure — 立即补齐多通道
    else if method == "POST" && (path == "/relays/ensure" || path == "/relays/ensure/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::EnsureChannels { reply })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match rx.await {
            Ok(chans) => (
                200,
                serde_json::json!({"active":chans,"count":chans.len()}).to_string(),
            ),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // GET /find?key=... — 跨节点迭代 Kademlia 查找，返回全局最接近的 peers
    // （用于验证跨节点路由；Kademlia 迭代查找的跳数对 n 有 O(log n) 上界）。
    else if method == "GET" && (path == "/find" || path == "/find/") {
        let key = query_string
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == "key")
            .map(|(_, v)| url_decode(v))
            .unwrap_or_default();
        if key.is_empty() {
            return (400, serde_json::json!({"error":"missing_key"}).to_string());
        }
        let (reply, rx) = oneshot::channel();
        if cmd_tx
            .send(PeerCommand::FindPeers {
                key: key.clone(),
                reply,
            })
            .await
            .is_err()
        {
            return (
                500,
                serde_json::json!({"error":"peer_actor_unavailable"}).to_string(),
            );
        }
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(Ok(peers))) => (
                200,
                serde_json::json!({
                    "key": key,
                    "closest_peers": peers,
                    "count": peers.len(),
                })
                .to_string(),
            ),
            Ok(Ok(Err(e))) => (502, serde_json::json!({"error": e}).to_string()),
            Ok(Err(_)) => (500, serde_json::json!({"error":"no_response"}).to_string()),
            Err(_) => (504, serde_json::json!({"error":"find_timeout"}).to_string()),
        }
    } else {
        (
            404,
            serde_json::json!({"error":"not_found","path":path}).to_string(),
        )
    }
}

// ───────────────────────── HTTP API 服务器 ─────────────────────────

/// v3.0.0 插件 REST 处理：list / call / install / reload / uninstall。
///
/// 调用方已在路由层通过 `rest_authorize` 认证闸门；这里只做插件操作。
/// 全部失败路径返回类型化的 `(状态码, JSON)`，不 panic。
fn handle_plugin_api(
    method: &str,
    path: &str,
    body: &str,
    host: &mut crate::plugin::PluginHost,
) -> (u16, serde_json::Value) {
    let path = path.split('?').next().unwrap_or(path);
    let sub = path
        .trim_start_matches("/api/v1/plugins")
        .trim_start_matches('/');
    let segments: Vec<&str> = if sub.is_empty() {
        Vec::new()
    } else {
        sub.split('/').collect()
    };

    match (method, segments.as_slice()) {
        // GET /plugins：列出全部插件（id / version / tier / state）。
        ("GET", []) => {
            let mut list: Vec<serde_json::Value> = Vec::new();
            for (id, state) in host.route_table() {
                let version = host
                    .registry()
                    .get(&id)
                    .map(|r| r.manifest.plugin.version.clone())
                    .unwrap_or_default();
                let tier = crate::plugin::tier::Tier::from_name(&id);
                list.push(serde_json::json!({
                    "id": id,
                    "version": version,
                    "tier": format!("{tier:?}"),
                    "state": format!("{state:?}"),
                }));
            }
            (
                200,
                serde_json::json!({ "host_version": host.version(), "plugins": list }),
            )
        }
        // POST /plugins：安装（body 为完整清单）。
        ("POST", []) => {
            match serde_json::from_str::<crate::plugin::manifest::PluginManifest>(body) {
                Ok(m) => match host.install(m) {
                    Ok(id) => (201, serde_json::json!({ "installed": id })),
                    Err(e) => (400, serde_json::json!({ "error": e.to_string() })),
                },
                Err(e) => (
                    400,
                    serde_json::json!({ "error": format!("清单解析失败: {e}") }),
                ),
            }
        }
        // GET /plugins/blacklist：黑名单查询（取证/生命周期管理）。
        // 必须在通用 `("GET", [id])` 详情路由之前，否则会被当作 id="blacklist"。
        ("GET", ["blacklist"]) => {
            let entries: Vec<serde_json::Value> = host
                .blacklist()
                .entries()
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "plugin_name": e.plugin_name,
                        "module_sha256": e.module_sha256,
                        "reason": e.reason.as_str(),
                        "blacklisted_at": e.blacklisted_at,
                        "evidence": e.evidence,
                        "appeal": e.appeal.as_ref().map(|a| serde_json::json!({
                            "note": a.note,
                            "filed_at": a.filed_at,
                            "replacement_module_sha256": a.replacement_module_sha256,
                        })),
                    })
                })
                .collect();
            (200, serde_json::json!({ "blacklist": entries }))
        }
        // POST /plugins/trust：信任第三方发布者（body: {"publisher_key":"hex"}）。
        // 补齐 T3 安装前置：默认信任库为空（fail-closed），运维需显式信任。
        ("POST", ["trust"]) => {
            let parsed: serde_json::Value =
                serde_json::from_str(body).unwrap_or_else(|_| serde_json::json!({}));
            match parsed.get("publisher_key").and_then(|x| x.as_str()) {
                Some(key) => {
                    host.trust_publisher(key);
                    (200, serde_json::json!({ "trusted": key }))
                }
                None => (
                    400,
                    serde_json::json!({ "error": "缺少 publisher_key（Ed25519 公钥 hex）" }),
                ),
            }
        }
        // GET /plugins/{id}：单个插件详情。
        ("GET", [id]) => match host.route_table().get(*id).cloned() {
            Some(state) => {
                let version = host
                    .registry()
                    .get(id)
                    .map(|r| r.manifest.plugin.version.clone())
                    .unwrap_or_default();
                let tier = crate::plugin::tier::Tier::from_name(id);
                (
                    200,
                    serde_json::json!({
                        "id": *id,
                        "version": version,
                        "tier": format!("{tier:?}"),
                        "state": format!("{state:?}"),
                    }),
                )
            }
            None => (404, serde_json::json!({ "error": "插件未找到" })),
        },
        // POST /plugins/{id}/call：调用方法（body: {method, payload}）。
        ("POST", [id, "call"]) => {
            let parsed: serde_json::Value =
                serde_json::from_str(body).unwrap_or_else(|_| serde_json::json!({}));
            let method_name = parsed
                .get("method")
                .and_then(|x| x.as_str())
                .unwrap_or("status");
            let args = parsed
                .get("payload")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let arg_bytes = serde_json::to_vec(&args).unwrap_or_default();
            match host.call(id, method_name, &arg_bytes) {
                Ok(bytes) => {
                    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_else(
                        |_| serde_json::json!({ "raw": String::from_utf8_lossy(&bytes) }),
                    );
                    (200, serde_json::json!({ "result": value }))
                }
                Err(e) => (400, serde_json::json!({ "error": e.to_string() })),
            }
        }
        // POST /plugins/{id}/reload：热更新（body 为新版本清单）。
        ("POST", [id, "reload"]) => {
            match serde_json::from_str::<crate::plugin::manifest::PluginManifest>(body) {
                Ok(m) => match host.hot_reload(m) {
                    Ok(_) => (200, serde_json::json!({ "reloaded": *id })),
                    Err(e) => (400, serde_json::json!({ "error": e.to_string() })),
                },
                Err(e) => (
                    400,
                    serde_json::json!({ "error": format!("清单解析失败: {e}") }),
                ),
            }
        }
        // POST /plugins/{id}/stop：暂停（热插拔中间态：保留注册，不卸载）。
        ("POST", [id, "stop"]) => match host.stop(id) {
            Ok(()) => (200, serde_json::json!({ "stopped": *id })),
            Err(e) => (400, serde_json::json!({ "error": e.to_string() })),
        },
        // POST /plugins/{id}/start：恢复（把暂停的插件重新置为 Running）。
        ("POST", [id, "start"]) => match host.start(id) {
            Ok(()) => (200, serde_json::json!({ "started": *id })),
            Err(e) => (400, serde_json::json!({ "error": e.to_string() })),
        },
        // POST /plugins/{id}/unblock：黑名单解封（body: {"new_module_sha256":"hex"}）。
        // 唯一解封路径：新版本模块摘要必须不同于被拉黑的旧摘要（通过完整审核）。
        ("POST", [id, "unblock"]) => {
            let parsed: serde_json::Value =
                serde_json::from_str(body).unwrap_or_else(|_| serde_json::json!({}));
            match parsed.get("new_module_sha256").and_then(|x| x.as_str()) {
                Some(new_digest) => {
                    if host.blacklist_mut().unblock_with_new_module(id, new_digest) {
                        (200, serde_json::json!({ "unblocked": *id }))
                    } else {
                        (
                            400,
                            serde_json::json!({ "error": "解封失败：未找到条目，或新模块摘要与被拉黑旧摘要相同（需发布新版本并通过完整审核）" }),
                        )
                    }
                }
                None => (
                    400,
                    serde_json::json!({ "error": "缺少 new_module_sha256（修复后新版本的模块摘要）" }),
                ),
            }
        }
        // DELETE /plugins/{id}：卸载（T0 不可卸载 → 400）。
        ("DELETE", [id]) => match host.uninstall(id) {
            Ok(()) => (200, serde_json::json!({ "uninstalled": *id })),
            Err(e) => (400, serde_json::json!({ "error": e.to_string() })),
        },
        _ => (404, serde_json::json!({ "error": "未知插件操作" })),
    }
}

// 顶层 HTTP 服务器的依赖注入参数（10 个），聚合为 struct 反而割裂可读性，允许多参数。
#[allow(clippy::too_many_arguments)]
async fn run_api_server(
    listen: String,
    api_port: u16,
    mode: String,
    p2p_port: u16,
    start: Instant,
    peer_cmd_tx: PeerCmdTx,
    market: MarketActorHandle,
    sandbox_mgr: Arc<std::sync::Mutex<SandboxManager>>,
    plugin_host: Arc<std::sync::Mutex<crate::plugin::PluginHost>>,
    crdt: CrdtHandle,
    shard_ctrl: ShardCtrlTx,
) -> anyhow::Result<()> {
    use crate::api::rest;
    use crate::mcp::sse;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // B1：主数据面编排器——绑定插件宿主与 market，在 register/match/settle 三点
    // 走「插件决策 → 宿主应用」。循环外构造一次，每连接 clone。
    let orchestrator =
        crate::plugin::PluginOrchestratorHandle::new(plugin_host.clone(), market.clone());

    let addr = format!("{}:{}", listen, api_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("✅ HTTP API 监听: http://{}", addr);

    loop {
        // v2.6.6：accept 的瞬时错误（EMFILE/ECONNABORTED 等）记录并退避继续，
        // 不再一次错误就从 run_daemon 返回 Err、杀掉整个进程。
        let mut stream = match listener.accept().await {
            Ok((s, _remote)) => s,
            Err(e) => {
                eprintln!("⚠️ accept 错误（已忽略并退避继续）: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };
        let mode = mode.clone();
        // v3.5.3（AU-14）：本连接处理不再直接写 store（agent 落盘统一由 market actor 快照负责），
        // 故不再 clone store 进此闭包。
        let peer_cmd_tx = peer_cmd_tx.clone();
        let market = market.clone();
        let sandbox_mgr = sandbox_mgr.clone();
        let plugin_host = plugin_host.clone();
        let orchestrator = orchestrator.clone();
        let crdt = crdt.clone();
        let shard_ctrl = shard_ctrl.clone();

        tokio::spawn(async move {
            // 读取完整请求（v2.8.5：加读超时与请求体上限，防 slow-loris / 内存 DoS）
            let read_timeout_secs: u64 = std::env::var("REST_READ_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30);
            let max_body_bytes: usize = std::env::var("REST_MAX_BODY_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10 * 1024 * 1024);

            let mut all = Vec::new();
            let mut buf = vec![0u8; 16384];
            let mut early_err: Option<(u16, &'static str, String)> = None;
            loop {
                let read_fut = stream.read(&mut buf);
                match tokio::time::timeout(
                    std::time::Duration::from_secs(read_timeout_secs),
                    read_fut,
                )
                .await
                {
                    Err(_) => {
                        // 单次读超时：408
                        early_err = Some((
                            408,
                            "Request Timeout",
                            serde_json::json!({ "error": "请求读取超时" }).to_string(),
                        ));
                        break;
                    }
                    Ok(Ok(0)) => break,
                    Ok(Ok(n)) => {
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
                                if len > max_body_bytes {
                                    early_err = Some((
                                        413,
                                        "Payload Too Large",
                                        serde_json::json!({ "error": "请求体超过上限" })
                                            .to_string(),
                                    ));
                                    break;
                                }
                                let body_bytes = s.len() - header_end - 4;
                                if body_bytes > max_body_bytes {
                                    early_err = Some((
                                        413,
                                        "Payload Too Large",
                                        serde_json::json!({ "error": "请求体超过上限" })
                                            .to_string(),
                                    ));
                                    break;
                                }
                                if body_bytes >= len || len == 0 {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    Ok(Err(_)) => return,
                }
            }

            // 提前提取请求头（方法/路径/Origin/Authorization）
            let request = String::from_utf8_lossy(&all);
            let request_line = request.lines().next().unwrap_or("");
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("GET").to_string();
            let raw_path = parts.next().unwrap_or("/").to_string();
            let (path_part, query_string) = raw_path.split_once('?').unwrap_or((&raw_path, ""));
            let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
            let body = request[body_start..].to_string();

            let header_value = |name: &str| -> Option<String> {
                request.lines().find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    if k.trim().eq_ignore_ascii_case(name) {
                        Some(v.trim().to_string())
                    } else {
                        None
                    }
                })
            };
            let origin_header = header_value("origin");
            let auth_header = header_value("authorization");
            // CORS 头（白名单命中才有，否则为空）
            let cors_headers = compute_cors(origin_header.as_deref());

            // 读取阶段错误（408/413）：直接返回
            if let Some((st, stt, pl)) = early_err {
                let resp = http_response(st, stt, pl, "application/json", &cors_headers);
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                return;
            }

            // ───── MCP over HTTP 端点（优先拦截） ─────
            if path_part == "/api/v1/mcp" {
                let mcp = if method == "GET" {
                    sse::handle_get()
                } else if method == "POST" {
                    // v2.6.8（GAP §8.8）：提取 Authorization，按 MCP_BEARER_TOKEN 认证。
                    // 未配置令牌时，sse::handle_post 默认拒绝所有写/动钱工具。
                    let auth_header = request.lines().find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        if k.trim().eq_ignore_ascii_case("authorization") {
                            Some(v.trim().to_string())
                        } else {
                            None
                        }
                    });
                    let expected_token = std::env::var("MCP_BEARER_TOKEN")
                        .ok()
                        .filter(|s| !s.is_empty());
                    sse::handle_post(
                        &body,
                        &market,
                        &sandbox_mgr,
                        auth_header.as_deref(),
                        expected_token.as_deref(),
                    )
                    .await
                } else {
                    let response = http_response(
                        405,
                        "Method Not Allowed",
                        String::new(),
                        "text/plain",
                        &cors_headers,
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                };
                let response = http_response(
                    mcp.status,
                    mcp.status_text,
                    mcp.body,
                    &mcp.content_type,
                    &cors_headers,
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (MCP {})", method, path_part, mcp.status);
                return;
            }

            // ───── REST 变更类接口认证闸门（v2.8.5，GAP §3.5；MCP 已独立认证） ─────
            // CORS 预检（OPTIONS）直接返回 204 与 CORS 头。
            if method == "OPTIONS" {
                let resp = http_response(
                    204,
                    "No Content",
                    String::new(),
                    "text/plain",
                    &cors_headers,
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                return;
            }
            if let Err((st, stt, pl)) = rest_authorize(&method, auth_header.as_deref()) {
                let resp = http_response(st, stt, pl, "application/json", &cors_headers);
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (REST 认证拒绝 {})", method, path_part, st);
                return;
            }

            // ───── Prometheus /metrics（纯文本 exposition，GET 只读，已过认证闸门） ─────
            if method == "GET" && (path_part == "/metrics" || path_part == "/api/v1/metrics") {
                let peer_info = fetch_peer_info(&peer_cmd_tx).await;
                let uptime = start.elapsed().as_secs_f64();
                let body = render_prometheus_metrics(
                    uptime,
                    peer_info.as_ref().map(|i| i.connected).unwrap_or(0),
                    peer_info.as_ref().map(|i| i.routing_entries).unwrap_or(0),
                    &crdt.stats(),
                    crdt.len(),
                    crdt.size_bytes(),
                );
                let resp =
                    http_response(200, "OK", body, "text/plain; version=0.0.4", &cors_headers);
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (metrics 200)", method, path_part);
                return;
            }

            // ───── v2.5.3 网络增强端点 ─────
            if path_part.starts_with("/api/v1/network/") || path_part.starts_with("/network/") {
                let net_path = path_part
                    .trim_start_matches("/api/v1")
                    .trim_start_matches("/network")
                    .to_string();
                let net_resp = handle_network_api(
                    &method,
                    &net_path,
                    query_string,
                    &body,
                    &peer_cmd_tx,
                    &start,
                )
                .await;
                let ct = if net_resp.1 == "application/json" {
                    "application/json"
                } else {
                    "text/plain"
                };
                let resp = http_response(
                    net_resp.0,
                    if net_resp.0 == 200 {
                        "OK"
                    } else if net_resp.0 == 201 {
                        "Created"
                    } else if net_resp.0 == 400 {
                        "Bad Request"
                    } else {
                        "Not Found"
                    },
                    net_resp.1,
                    ct,
                    &cors_headers,
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (network {})", method, path_part, net_resp.0);
                return;
            }

            // ───── 链上信任锚端点（/api/v1/chain/*，写操作已过 rest_authorize 闸门） ─────
            // 真实广播在未配置凭据/主网开关时 fail-closed（由 handle_chain_api 内部保证）。
            if path_part.starts_with("/api/v1/chain/") || path_part.starts_with("/chain/") {
                let chain_path = path_part
                    .trim_start_matches("/api/v1")
                    .trim_start_matches("/chain")
                    .to_string();
                // 内部模块约定 path 形如 /chain/status；这里重新拼回 /chain 前缀。
                let chain_path_full = format!("/chain{}", chain_path);
                let (chain_status, chain_body) =
                    crate::chain::handle_chain_api(&method, &chain_path_full, &body).await;
                let resp = http_response(
                    chain_status,
                    match chain_status {
                        200 => "OK",
                        201 => "Created",
                        400 => "Bad Request",
                        403 => "Forbidden",
                        404 => "Not Found",
                        500 => "Internal Server Error",
                        502 => "Bad Gateway",
                        _ => "Error",
                    },
                    chain_body,
                    "application/json",
                    &cors_headers,
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (chain {})", method, path_part, chain_status);
                return;
            }

            // ───── 纠删码分布式存储端点（/api/v1/erasure/*，已过 rest_authorize 写闸门） ─────
            if path_part.starts_with("/api/v1/erasure/") {
                let erasure_path = path_part.trim_start_matches("/api/v1/erasure").to_string();
                let (er_status, er_body) =
                    handle_erasure_api(&method, &erasure_path, &body, &shard_ctrl).await;
                let resp = http_response(
                    er_status,
                    match er_status {
                        200 => "OK",
                        201 => "Created",
                        400 => "Bad Request",
                        404 => "Not Found",
                        500 => "Internal Server Error",
                        _ => "Error",
                    },
                    er_body,
                    "application/json",
                    &cors_headers,
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (erasure {})", method, path_part, er_status);
                return;
            }

            // ───── Agent Sandbox 端点（v2.8.7：认证 + 所有权 + 审计） ─────
            // 认证闸门已由上方 rest_authorize（v2.8.5）覆盖；这里提取认证身份
            // 作为 caller 传入，用于 create 绑定 owner 与后续操作的所有权校验。
            if crate::sandbox::is_sandbox_route(path_part) {
                let mgr = sandbox_mgr.clone();
                let method_c = method.clone();
                let path_c = raw_path.clone();
                let body_c = body.clone();
                let caller = extract_caller(auth_header.as_deref());
                let result = tokio::task::spawn_blocking(move || {
                    let mut guard = mgr.lock().unwrap_or_else(|e| {
                        eprintln!("⚠️ sandbox: 管理器锁曾毒化，恢复后继续（请人工核查）");
                        e.into_inner()
                    });
                    crate::sandbox::handle_sandbox_api(
                        &method_c,
                        &path_c,
                        &body_c,
                        caller.as_deref(),
                        &mut guard,
                    )
                })
                .await;
                let (status, payload) = match result {
                    Ok(x) => x,
                    Err(_) => (500, serde_json::json!({ "error": "sandbox 任务异常" })),
                };
                let status_text = match status {
                    200 => "OK",
                    201 => "Created",
                    400 => "Bad Request",
                    401 => "Unauthorized",
                    403 => "Forbidden",
                    404 => "Not Found",
                    422 => "Unprocessable Entity",
                    429 => "Too Many Requests",
                    _ => "Internal Server Error",
                };
                let resp = http_response(
                    status,
                    status_text,
                    payload.to_string(),
                    "application/json",
                    &cors_headers,
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (sandbox {})", method, path_part, status);
                return;
            }

            // ───── v3.0.0 插件端点（/api/v1/plugins：list/call/install/reload/uninstall）─────
            if path_part == "/api/v1/plugins" || path_part.starts_with("/api/v1/plugins/") {
                let ph = plugin_host.clone();
                let method_c = method.clone();
                let path_c = raw_path.clone();
                let body_c = body.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let mut guard = ph.lock().unwrap_or_else(|e| {
                        eprintln!("⚠️ plugin: host 锁曾毒化，恢复后继续（请人工核查）");
                        e.into_inner()
                    });
                    handle_plugin_api(&method_c, &path_c, &body_c, &mut guard)
                })
                .await;
                let (status, payload) = match result {
                    Ok(x) => x,
                    Err(_) => (500, serde_json::json!({ "error": "plugin 任务异常" })),
                };
                let status_text = match status {
                    200 => "OK",
                    201 => "Created",
                    400 => "Bad Request",
                    404 => "Not Found",
                    _ => "Internal Server Error",
                };
                let resp = http_response(
                    status,
                    status_text,
                    payload.to_string(),
                    "application/json",
                    &cors_headers,
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
                eprintln!("← {} {} (plugin {})", method, path_part, status);
                return;
            }

            let peer_info = fetch_peer_info(&peer_cmd_tx).await;
            let connected = peer_info.as_ref().map(|i| i.connected).unwrap_or(0);
            let dht_routing_entries = peer_info.as_ref().map(|i| i.routing_entries).unwrap_or(0);
            let info = rest::NodeInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                mode: mode.clone(),
                p2p_port,
                connected_peers: connected,
                dht_routing_entries,
                uptime_ms: start.elapsed().as_millis(),
            };

            let routed = rest::route(
                &method,
                &raw_path,
                &body,
                &market,
                Some(&orchestrator),
                Some(&crdt),
                &info,
            )
            .await;

            // 注册 agent：DHT 索引（SQLite 落盘由 market actor 快照负责，见下）。
            // v3.5.3（AU-14）：旧实现这里在 201 后直接 `store.upsert_agent`，与 actor 的
            // 写后快照（market_actor.rs：dispatch 后对同一 agent_id upsert 完整 MarketAgentCard）
            // 形成同键双写、last-writer-wins，且此处 reputation 硬编码 0.0，会与 actor 的权威快照
            // 竞态、可能覆盖 actor 刚写入的真实信誉/状态。现删除冗余直写，以 market actor 为唯一
            // SQLite 写者（每次写命令后 upsert 完整、权威的 agent 行）；注册后立即读回走内存 market，
            // 不受此异步快照时序影响。
            if method == "POST"
                && (path_part == "/api/v1/agents" || path_part == "/agents")
                && routed.status == 201
            {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                    let agent_id = v.get("agent_id").and_then(|x| x.as_str()).unwrap_or("");
                    let _ = peer_cmd_tx
                        .send(PeerCommand::DhtPut {
                            key: format!("/aip/agent/{}", agent_id),
                            value: body.as_bytes().to_vec(),
                        })
                        .await;
                }
            }

            let response_body = if routed.body.is_null() {
                String::new()
            } else {
                serde_json::to_string(&routed.body).unwrap_or_default()
            };
            let content_type = if routed.body.is_null() {
                "text/plain"
            } else {
                "application/json"
            };
            let response = http_response(
                routed.status,
                routed.status_text,
                response_body,
                content_type,
                &cors_headers,
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
            eprintln!(
                "← {} {} ({} {})",
                method, path_part, routed.status, routed.status_text
            );
        });
    }
}

// ───────────────────────── 启动 ─────────────────────────

/// v2.5.5: 社区 relay 候选（DHT 提取的公网 IP，启动时 dial，hop 即自动入池；
/// 可用性实时验证，不保证长期在线）
const COMMUNITY_RELAY_CANDIDATES: &[&str] = &[
    "/ip4/103.6.150.240/tcp/35317",
    "/ip4/148.135.195.208/tcp/4001",
    "/ip4/157.180.13.174/tcp/4001",
    "/ip4/160.119.251.84/tcp/4001",
    "/ip4/181.47.9.240/tcp/4001",
    "/ip4/188.241.98.55/tcp/4001",
    "/ip4/198.96.88.176/tcp/4001",
    "/ip4/202.181.177.191/tcp/4001",
    "/ip4/207.148.5.75/tcp/4001",
    "/ip4/23.145.40.189/tcp/4001",
];

/// v2.5.5: 初始化 relay 池——元数据（start_time/manual_bonus）+ 种子 relay
fn init_relay_pool(store: &PersistentStore) {
    if store
        .get_meta("relay_pool:start_time")
        .ok()
        .flatten()
        .is_none()
    {
        if let Err(e) = store.set_meta("relay_pool:start_time", &now_iso()) {
            log_relay_persist_err("set_meta", "relay_pool:start_time", &e);
        }
    }
    if store
        .get_meta("relay_pool:manual_bonus")
        .ok()
        .flatten()
        .is_none()
    {
        if let Err(e) = store.set_meta("relay_pool:manual_bonus", "0") {
            log_relay_persist_err("set_meta", "relay_pool:manual_bonus", &e);
        }
    }
    if store.relay_count().unwrap_or(0) == 0 {
        // GSN_DISABLE_PUBLIC_RELAY=1 时，连“种子 relay”也不写入（否则它只是
        // 从显式 probe 移到了 ensure_channels/run_relay_maintenance 里被拨号，
        // 名义上“禁用公共 relay”却仍连公网）。本地纯净组网应无任何公网 relay。
        if std::env::var("GSN_DISABLE_PUBLIC_RELAY").as_deref() == Ok("1") {
            println!("⏭️ 本地模式：不写入种子 relay（relay 池为空，仅本地组网）");
            return;
        }
        // 已真机验证的社区 relay（kubo，hop+stop+dcutr，reservation 120s/128KB）
        let seed_id = "12D3KooWJFBbD3czz9bpC4escx5izFKwaBj87XJ5r1rUpzPUu4WE";
        let seed = StoredRelay {
            relay_id: seed_id.to_string(),
            multiaddr: format!("/ip4/148.113.166.44/tcp/4001/p2p/{}", seed_id),
            class: RelayClass::ThirdParty.as_str().to_string(),
            status: "active".to_string(),
            healthy: true,
            fail_count: 0,
            limit_sec: 120,
            data_bytes: 131072,
            last_check: now_iso(),
            created_at: now_iso(),
        };
        match store.upsert_relay(&seed) {
            Ok(_) => println!("✅ Relay 池已初始化（种子 relay 148.113.166.44，初始上限 1 万）"),
            Err(e) => eprintln!("⚠️ 种子 relay 写入失败: {}", e),
        }
    } else {
        let cap = current_capacity(store);
        println!(
            "✅ Relay 池已存在（{} 节点 / 有效上限 {}）",
            store.relay_count().unwrap_or(0),
            cap.effective_cap
        );
    }
}

/// v3.5.3（AU-30）：root 启动闸门（纯函数，便于单测）。
///
/// euid==0（root）且未显式允许时拒绝启动；非 root 或显式 `GSN_ALLOW_ROOT=1` 放行。
/// 沙箱/插件子系统无 setuid 降权，以 root 跑一旦逃逸即获得 root，故默认 fail-closed。
pub fn ensure_not_root(allow_root: bool, euid: u32) -> Result<(), String> {
    if euid == 0 && !allow_root {
        return Err(
            "REFUSAL_RUN_AS_ROOT: daemon 检测到以 root(euid=0) 运行。沙箱/插件子系统无 setuid 降权，\
             不应以 root 运行；如确需，显式设置 GSN_ALLOW_ROOT=1 后重启。"
                .to_string(),
        );
    }
    Ok(())
}

/// 读取当前进程有效 UID（不引 libc）。Linux 下解析 /proc/self/status 的 `Uid:` 行
/// （格式：`Uid:\teff\teuid...`，第二列为 euid）；其它平台或读取失败返回 None。
pub fn current_euid() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            let mut parts = rest.split_whitespace();
            // 列：real effective saved fs。取第二列（effective）。
            let _real = parts.next()?;
            let euid = parts.next()?;
            return euid.parse::<u32>().ok();
        }
    }
    None
}

/// 启动节点（核心入口，gsn-daemon 与 gsn daemon 共用）
/// 接管终止信号（P1-ops，真机根因修复）。
///
/// 现象：守护进程对 `kill -TERM`/`docker stop` 无反应，只能 `kill -9`。
///
/// 根因（真机 + 最小复现双重实证）：在部分监督 shell / 编排环境里，子进程在
/// fork-exec 前同时继承了两样东西，且二者都会跨 exec 保留：
/// 1. SIGTERM/SIGINT 的处置位被置为 `SIG_IGN`；
/// 2. 更关键的是 SIGTERM/SIGINT 被放进了**信号阻塞掩码（blocked mask）**。
///
/// Rust 运行时不会在启动时重置这两者；即便用 tokio 注册了信号处理器（其内部
/// sigaction 会覆盖 handler），信号仍因处于 blocked 状态而永远无法投递，表现为
/// 进程“活着但收不到 SIGTERM”。仅把 handler 复位为 SIG_DFL 也无效——阻塞位仍在。
///
/// 修复（顺序不可省）：
/// 1. 启动最早期用 `pthread_sigmask(SIG_UNBLOCK)` 解除 SIGTERM/SIGINT/SIGHUP 的
///    继承阻塞；
/// 2. 用 `signal(.., SIG_DFL)` 复位可能继承来的 SIG_IGN；
/// 3. 再交给 tokio 注册异步处理器，收到首个终止信号即优雅退出。
///
/// SQLite 采用 WAL 且每笔提交均落盘，崩溃安全；kill -9 后账本可重放，因此信号
/// 即时退出不破坏一致性。
fn install_shutdown_handler() {
    #[cfg(unix)]
    {
        // 第 1、2 步：解除继承阻塞并复位继承忽略。必须在 tokio 接管之前完成。
        // SAFETY: 下面只通过 libc FFI 修改“本进程自身”的信号掩码与处置位，不做任何
        // 裸内存或外来指针解引用：(1) libc::sigset_t 是只含整数/数组的 POD 类型，
        // zeroed() 的全零位模式是其合法表示，且 sigemptyset 会在任何 addset/
        // pthread_sigmask 使用前把它完整初始化，故不会读取未初始化内存；(2) 传入的
        // 集合指针仅来自 &mut set（非空、对齐、生命周期覆盖整次调用），
        // pthread_sigmask 的 old-set 显式传 null_mut()，这是 POSIX 明确允许的；
        // (3) SIGTERM/SIGINT/SIGHUP 均为合法信号编号，signal() 返回值被有意忽略
        // （此处只需复位处置为 SIG_DFL，随后由 tokio 覆盖）。满足这些 FFI 前置条件即
        // 保持健全；信号 FFI 的正确性仍属 REQUIRE EXTERNAL AUDIT 范围。
        unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            if libc::sigemptyset(&mut set) == 0 {
                libc::sigaddset(&mut set, libc::SIGTERM);
                libc::sigaddset(&mut set, libc::SIGINT);
                libc::sigaddset(&mut set, libc::SIGHUP);
                // 非致命：解除失败时退回下方注册流程，由告警暴露。
                let rc = libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
                if rc != 0 {
                    eprintln!(
                        "⚠️ pthread_sigmask(SIG_UNBLOCK) 失败 rc={rc}，终止信号可能仍被继承阻塞"
                    );
                }
            }
            // 复位可能继承自父进程的 SIG_IGN；tokio 随后会覆盖为自己的处理器。
            libc::signal(libc::SIGTERM, libc::SIG_DFL);
            libc::signal(libc::SIGINT, libc::SIG_DFL);
        }

        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("⚠️ 无法接管 SIGTERM（将沿用系统默认处置）: {e}");
                return;
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("⚠️ 无法接管 SIGINT（将沿用系统默认处置）: {e}");
                return;
            }
        };
        tokio::spawn(async move {
            tokio::select! {
                _ = sigterm.recv() => {
                    eprintln!("🛑 收到 SIGTERM，gsn-daemon 退出。");
                    std::process::exit(0);
                }
                _ = sigint.recv() => {
                    eprintln!("🛑 收到 SIGINT，gsn-daemon 退出。");
                    std::process::exit(0);
                }
            }
        });
    }
    #[cfg(not(unix))]
    {
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                std::process::exit(0);
            }
        });
    }
}

pub async fn run_daemon(args: DaemonArgs) -> anyhow::Result<()> {
    // v2.5.4: 初始化 tracing，使 libp2p 内部（relay/identify/autonat/dcutr/swarm）的
    // warn/error/trace 不再被静默丢弃；可用 RUST_LOG 控制粒度（如 libp2p_relay=trace）。
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_target(true)
        .try_init();

    println!("=== GSN Daemon v{} ===", env!("CARGO_PKG_VERSION"));

    // P1-ops（真机复现）：必须在启动最早期接管 SIGTERM/SIGINT。见函数说明。
    install_shutdown_handler();

    // v3.6.1 自动更新：每次启动后台联网到 npm registry 检查版本（非阻塞、带超时、
    // 失败只告警不影响启动）。默认 minor 通道；白名单先锋/贡献者走 patch 通道。
    // GSN_NO_UPDATE_CHECK=1 完全关闭；GSN_AUTO_UPDATE=0 只检查不自动安装。
    if !crate::update::check_disabled() {
        tokio::task::spawn_blocking(|| {
            let cfg = crate::update::AutoUpdateConfig::default();
            let outcome = crate::update::run_startup_check(&cfg);
            let line = crate::update::summarize(&outcome);
            if matches!(outcome, crate::update::CheckOutcome::Failed(_)) {
                tracing::warn!("{line}");
            } else {
                println!("📦 {line}");
                tracing::info!("{line}");
            }
        });
    }

    // v3.5.3（AU-30）：沙箱/插件子系统不应以 root 运行（代码无 setuid 降权，一旦逃逸即 root）。
    // 默认 fail-closed：检测到 euid==0 且未显式设置 GSN_ALLOW_ROOT=1 则拒绝启动。
    // 不引 libc：Linux 下从 /proc/self/status 的 Uid 行解析 euid；非 Linux / 无法读取则跳过
    // （无法判定时不臆断 root，保持可启动）。
    if let Some(euid) = current_euid() {
        let allow_root = std::env::var("GSN_ALLOW_ROOT").as_deref() == Ok("1");
        if let Err(e) = ensure_not_root(allow_root, euid) {
            eprintln!("🚨 CRITICAL: {e}");
            return Err(anyhow::anyhow!("{e}"));
        }
    }

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

    // v2.5.5: 初始化 relay 池（start_time / manual_bonus / 种子 relay）
    init_relay_pool(&store);

    // P0-5: 身份随 --data-dir 持久化（不同 data-dir → 不同 PeerId）。
    // v3.9.13: 自建 Relay 中继服务端。`--relay-server` 时构造带
    // relay::Behaviour（Circuit Relay v2 hop）的节点对外服务。
    let relay_cfg = if args.relay_server {
        let cfg = libp2p::relay::Config {
            max_circuits: args.relay_max_circuits,
            max_circuits_per_peer: args.relay_max_circuits.max(1),
            max_reservations: args.relay_max_reservations,
            max_reservations_per_peer: args.relay_max_reservations.max(1),
            ..Default::default()
        };
        Some(cfg)
    } else {
        None
    };
    let mut peer = if relay_cfg.is_some() {
        P2pPeer::with_data_dir_relay(&args.data_dir, relay_cfg).await?
    } else {
        P2pPeer::with_data_dir(&args.data_dir).await?
    };
    match peer.listen_on_port(args.port) {
        Ok(_) => println!("✅ libp2p P2P 端口: {}", args.port),
        Err(e) => eprintln!("⚠️ P2P 端口 {} 绑定失败: {}", args.port, e),
    }
    // v2.5.5 方案3: QUIC/UDP 监听（与 TCP 同端口），用于 UDP 打洞与直连
    match peer.listen_quic_port(args.port) {
        Ok(a) => println!("✅ libp2p QUIC 端口: {}", a),
        Err(e) => eprintln!("⚠️ QUIC 端口 {} 监听失败: {}", args.port, e),
    }
    // v3.9.13: 中继服务端提示（对外通告地址由部署方固定映射；本进程只做 hop）
    if args.relay_server {
        println!(
            "🔄 Relay Server 已启用: max_circuits={} max_reservations={}",
            args.relay_max_circuits, args.relay_max_reservations
        );
        println!(
            "   其他节点请以 --relay /ip4/<本机公网IP>/tcp/{}/p2p/{} 加入",
            args.port, peer.peer_id
        );
    }
    // v3.9.13: 启动即注入自建/自有中继（分类 dedicated，最高优先级）。
    for addr_str in &args.relay {
        // relay_id 优先取 multiaddr 中的 /p2p/<PeerId>；解析失败时回退整串。
        let relay_id = addr_str
            .parse::<Multiaddr>()
            .ok()
            .as_ref()
            .and_then(extract_relay_peer_id)
            .unwrap_or_else(|| addr_str.clone());
        let stored = crate::storage::StoredRelay {
            relay_id: relay_id.clone(),
            multiaddr: addr_str.clone(),
            class: "dedicated".to_string(),
            status: "unknown".to_string(),
            healthy: true,
            fail_count: 0,
            limit_sec: 0,
            data_bytes: 0,
            last_check: String::new(),
            created_at: String::new(),
        };
        match store.upsert_relay(&stored) {
            Ok(_) => {
                println!("→ 注入自建 relay: {} ({})", relay_id, addr_str);
                if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                    if let Err(e) = peer.add_bootstrap(addr) {
                        eprintln!("⚠️ relay {} 发起 dial 失败: {}", addr_str, e);
                    }
                }
            }
            Err(e) => eprintln!("⚠️ relay {} 落库失败: {}", addr_str, e),
        }
    }
    for addr_str in &args.bootstrap {
        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
            match peer.add_bootstrap(addr) {
                Ok(_) => println!("→ bootstrap: {}", addr_str),
                Err(e) => eprintln!("⚠️ bootstrap {} 失败: {}", addr_str, e),
            }
        }
    }
    // v2.5.5: dial 社区 relay 候选（无 /p2p），Identify 发现 hop 即自动入池。
    // GSN_DISABLE_PUBLIC_RELAY=1：本地/规模化测试只走本地 bootstrap，不冲击公共 relay。
    if std::env::var("GSN_DISABLE_PUBLIC_RELAY").as_deref() == Ok("1") {
        println!("⏭️ GSN_DISABLE_PUBLIC_RELAY=1：跳过公共社区 relay 候选 probe");
    } else {
        for cand in COMMUNITY_RELAY_CANDIDATES {
            if let Err(e) = peer.probe_relay(cand) {
                eprintln!("⚠️ 社区候选 {} dial 失败: {}", cand, e);
            }
        }
    }

    let (peer_cmd_tx, peer_cmd_rx) = mpsc::channel::<PeerCommand>(64);
    println!("   Peer ID: {}", peer.peer_id);
    println!("   模式: {:?}", node_mode);
    // P0-1: 全节点 / 归档节点显式以 Kademlia Server 模式运行——否则节点默认
    // Client 模式（无“已确认外部地址”），拒绝入站 kad 查询、不向 Identify
    // 注册 kad，DHT 无法在节点间扩散。轻/边缘/浏览器节点保持 Client（只查询）。
    if matches!(node_mode, NodeMode::Archive | NodeMode::Full) {
        peer.set_kad_server_mode();
    }
    // P0-2: 入站 GossipSub 消息通道（swarm actor 投递 → 此处消费，后续可路由到 market）
    let (inbound_tx, mut inbound_rx) = mpsc::channel::<InboundGossipMessage>(1024);
    if let Err(e) = peer.subscribe("gsn/agents") {
        eprintln!("⚠️ subscribe gsn/agents 失败: {e}");
    }
    if let Err(e) = peer.subscribe("gsn/tasks") {
        eprintln!("⚠️ subscribe gsn/tasks 失败: {e}");
    }
    // CRDT：订阅同步主题（Op 低延迟 + Snapshot 反熵）。失败必须告警，绝不静默。
    if let Err(e) = peer.subscribe(CRDT_TOPIC) {
        eprintln!("⚠️ subscribe {CRDT_TOPIC} 失败: {e}");
    } else {
        println!("✅ CRDT 同步主题已订阅: {CRDT_TOPIC}");
    }
    // 纠删码：订阅分片主题（分片分发 StoreShard / 拉取 NeedShards / ShardReply）。
    if let Err(e) = peer.subscribe(crate::erasure::wire::SHARD_TOPIC) {
        eprintln!(
            "⚠️ subscribe {} 失败: {e}",
            crate::erasure::wire::SHARD_TOPIC
        );
    } else {
        println!(
            "✅ 纠删码分片主题已订阅: {}",
            crate::erasure::wire::SHARD_TOPIC
        );
    }

    // 纠删码分片：原始报文通道（inbound consumer 转发）+ 控制通道（REST ingest/reconstruct）。
    // 元组首项为经 GossipSub 认证的真实发布者 PeerId（msg.source）。协议子网成员
    // 发现只信任该 source，不信任报文 JSON 自报 from（可被任意伪造）。
    let (shard_raw_tx, mut shard_raw_rx) = mpsc::channel::<(String, Vec<u8>)>(512);
    let (shard_ctrl_tx, mut shard_ctrl_rx) = mpsc::channel::<ShardControl>(32);

    // CRDT 状态存储（LWW-Map），origin = 本节点 PeerId。
    // GSN_CRDT_MAX_KEYS：测试用小上限验证膨胀走平；解析失败回退默认 100_000。
    let crdt_store: Arc<std::sync::Mutex<CrdtStore>> = {
        let origin = peer.peer_id.to_string();
        let store = match std::env::var("GSN_CRDT_MAX_KEYS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
        {
            Some(n) if n >= 1 => {
                println!("✅ CRDT 键数上限（GSN_CRDT_MAX_KEYS）: {n}");
                CrdtStore::with_max_keys(origin, n)
            }
            _ => CrdtStore::new(origin),
        };
        Arc::new(std::sync::Mutex::new(store))
    };

    // 本节点 PeerId 字符串（peer 随后 move 进 swarm actor，这里先捕获）。
    let self_peer_id = peer.peer_id.to_string();

    tokio::spawn(run_swarm_actor(
        peer,
        peer_cmd_rx,
        store.clone(),
        inbound_tx,
    ));

    // 应用层消费者：除既有 info 日志外，把 CRDT_TOPIC 报文路由到 LWW 存储。
    // 解析失败只 warn 并 continue——绝不能 panic/杀消费者（单个坏包不能拖垮节点）。
    tokio::spawn({
        let crdt_store = crdt_store.clone();
        let shard_raw_tx = shard_raw_tx.clone();
        async move {
            while let Some(msg) = inbound_rx.recv().await {
                if msg.topic == CRDT_TOPIC {
                    match serde_json::from_slice::<CrdtMessage>(&msg.data) {
                        Ok(CrdtMessage::Op(op)) => match crdt_guard(&crdt_store).apply_op(op) {
                            Ok(changed) => tracing::debug!(
                                changed,
                                source = %msg.source,
                                "📥 CRDT op applied"
                            ),
                            Err(e) => {
                                tracing::warn!(error = %e, source = %msg.source, "CRDT apply_op 失败")
                            }
                        },
                        Ok(CrdtMessage::Snapshot(snap)) => {
                            let changed = crdt_guard(&crdt_store).merge_snapshot(snap);
                            tracing::debug!(changed, source = %msg.source, "📥 CRDT snapshot merged");
                        }
                        Err(e) => tracing::warn!(
                            topic = %msg.topic,
                            source = %msg.source,
                            len = msg.data.len(),
                            error = %e,
                            "CRDT 报文解析失败，已跳过"
                        ),
                    }
                } else if msg.topic == crate::erasure::wire::SHARD_TOPIC {
                    // 原始报文转发给分片驱动；通道满只告警丢弃（不阻塞/不 panic）。
                    if let Err(e) =
                        shard_raw_tx.try_send((msg.source.to_string(), msg.data.clone()))
                    {
                        tracing::warn!(source = %msg.source, "分片报文转发失败: {e}");
                    }
                } else {
                    tracing::info!(
                        topic = %msg.topic,
                        source = %msg.source,
                        len = msg.data.len(),
                        "📥 inbound gossip delivered to application layer"
                    );
                }
            }
        }
    });

    // 周期快照广播（反熵）：间隔由 env GSN_CRDT_SNAPSHOT_INTERVAL_SECS 决定，默认 15s。
    {
        let crdt_store = crdt_store.clone();
        let peer_cmd_tx = peer_cmd_tx.clone();
        tokio::spawn(async move {
            let secs = std::env::var("GSN_CRDT_SNAPSHOT_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(15);
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(secs.max(1)));
            loop {
                ticker.tick().await;
                let snap = crdt_guard(&crdt_store).snapshot(now_millis());
                match serde_json::to_vec(&CrdtMessage::Snapshot(snap)) {
                    Ok(data) => {
                        if let Err(e) = peer_cmd_tx.try_send(PeerCommand::Publish {
                            topic: CRDT_TOPIC.into(),
                            data,
                        }) {
                            tracing::warn!("CRDT 周期快照广播 try_send 失败: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("CRDT 周期快照序列化失败: {e}"),
                }
            }
        });
    }

    // 纠删码分片驱动：把 GossipSub 报文（shard_raw_rx）与控制命令（shard_ctrl_rx）
    // 桥接到 DistributedShardNode，按 500ms tick 推进：投递入站 → 处理 → 重建重试 → 发布出站。
    {
        let self_id = self_peer_id.clone();
        let peer_cmd = peer_cmd_tx.clone();
        tokio::spawn(async move {
            let coder = match ErasureCoder::new(DEFAULT_DATA_SHARDS, DEFAULT_PARITY_SHARDS) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("⚠️ 分片驱动初始化失败（纠删码构造）: {e}");
                    return;
                }
            };
            let mut shard_node = DistributedShardNode::new(self_id.clone(), coder);
            let mut transport = GossipShardTransport::new(self_id.clone(), vec![self_id.clone()]);
            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(500));
            let mut pending_recon: Vec<String> = Vec::new();
            let mut refresh_ticks: u32 = 0;
            loop {
                ticker.tick().await;

                // 1) 排空原始入站报文，解析后投递（目标匹配才入队）。
                while let Ok((source, bytes)) = shard_raw_rx.try_recv() {
                    match ShardWire::from_bytes(&bytes) {
                        Ok(mut wire) => {
                            // 用 GossipSub 认证的真实发布者覆盖自报 from：成员发现只
                            // 信任传输层 source，防止伪造 from 污染纠删码放置集合。
                            wire.from = source;
                            transport.deliver(&wire);
                        }
                        Err(e) => tracing::warn!(error = %e, "分片报文解析失败，已跳过"),
                    }
                }

                // 2) 每 ~2s（4 ticks）刷新已知 peer 列表。
                refresh_ticks = refresh_ticks.saturating_add(1);
                if refresh_ticks.is_multiple_of(4) {
                    if let Some(ids) = fetch_peer_ids(&peer_cmd).await {
                        transport.set_peers(ids);
                    }
                    // 周期广播 Hello 心跳：对端据此把本节点记入协议子网 observed 集合。
                    // Hello 为广播(to="*")，直接 publish，不经确定性放置出站队列。
                    let hello = ShardWire::direct(
                        "*",
                        self_id.clone(),
                        ShardEnvelope::Hello {
                            node: self_id.clone(),
                        },
                    );
                    if let Err(e) = peer_cmd.try_send(PeerCommand::Publish {
                        topic: SHARD_TOPIC.into(),
                        data: hello.to_bytes(),
                    }) {
                        tracing::warn!("分片 Hello 心跳 publish 失败: {e}");
                    }
                }

                // 3) 处理控制命令。
                while let Ok(ctrl) = shard_ctrl_rx.try_recv() {
                    match ctrl {
                        ShardControl::Ingest {
                            blob_id,
                            data,
                            reply,
                        } => {
                            let r = shard_node
                                .ingest_blob(&blob_id, &data, &mut transport)
                                .map(|_| blob_id.clone());
                            if let Err(e) = reply.send(r) {
                                tracing::warn!("ingest 回执丢失: {e:?}");
                            }
                        }
                        ShardControl::Reconstruct { blob_id, reply } => {
                            let r = shard_node.reconstruct_blob(&blob_id, &mut transport);
                            if r.is_err()
                                && !pending_recon.contains(&blob_id)
                                && !r.as_ref().unwrap_err().contains("硬失败")
                            {
                                pending_recon.push(blob_id.clone());
                            }
                            if let Err(e) = reply.send(r) {
                                tracing::warn!("reconstruct 回执丢失: {e:?}");
                            }
                        }
                    }
                }

                // 4) 处理入站信封（可能产生出站，如 NeedShards→ShardReply）。
                while let Some((from, env)) = transport.next() {
                    shard_node.handle(env, &mut transport);
                    let _ = from;
                }

                // 5) 自动重试 pending 重建；硬失败的放弃（不再重试）。
                let mut still_pending = Vec::new();
                for bid in pending_recon.drain(..) {
                    match shard_node.reconstruct_blob(&bid, &mut transport) {
                        Ok(_) => {}
                        Err(e) => {
                            if e.contains("硬失败") {
                                tracing::warn!("blob {bid} 重建硬失败: {e}");
                            } else {
                                still_pending.push(bid);
                            }
                        }
                    }
                }
                pending_recon = still_pending;

                // 6) 排空出站，经 GossipSub 发布（目标由 ShardWire.to 携带）。
                while let Some((to, env)) = transport.take_outbound() {
                    let wire = ShardWire::direct(to, self_id.clone(), env);
                    let bytes = wire.to_bytes();
                    if let Err(e) = peer_cmd.try_send(PeerCommand::Publish {
                        topic: SHARD_TOPIC.into(),
                        data: bytes,
                    }) {
                        tracing::warn!("分片报文 publish 失败: {e}");
                    }
                }
            }
        });
    }

    // P0-1: 首个 bootstrap 连接建立后触发 Kademlia 自举（等 5s 让连接+add_address），
    // 随后每 5 分钟重跑一次 self-lookup，保证 churn 下路由表持续被刷新。
    {
        let t = peer_cmd_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let _ = t.send(PeerCommand::BootstrapDht).await;
            let mut refresh = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                refresh.tick().await;
                let _ = t.send(PeerCommand::BootstrapDht).await;
            }
        });
    }

    // v2.5.5: 启动后初始化维护（等 8s 让 bootstrap 连接），随后每小时巡检一轮
    {
        let (s, t) = (store.clone(), peer_cmd_tx.clone());
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(8)).await;
            run_relay_maintenance(s, t).await;
        });
    }
    {
        let (s, t) = (store.clone(), peer_cmd_tx.clone());
        tokio::spawn(async move {
            let mut hourly = tokio::time::interval(std::time::Duration::from_secs(3600));
            hourly.tick().await; // 首次立即触发跳过（已由启动维护完成）
            loop {
                hourly.tick().await;
                run_relay_maintenance(s.clone(), t.clone()).await;
            }
        });
    }

    let market = MarketActorHandle::spawn_with_store(store.clone());
    println!("✅ Agent Market actor 已启动（账本持久化，重启可恢复）");

    // v2.8.0：Agent Sandbox 管理器（沙箱目录在 data-dir 下）
    // v2.9.1：后端选择。进程后端无法强制网络/FS/磁盘边界：
    //   - GSN_SANDBOX_BACKEND=process（默认）：operator 显式承认这些局限，
    //     用 trusted_local（带理由 waiver，写入审计），沙箱开箱即用；
    //   - GSN_SANDBOX_BACKEND=none：沙箱禁用（严格默认，create 会被拒绝）；
    //   - 任何拼错的值：拒绝（安全配置的 typo 绝不能静默选中更弱的后端）。
    let sb_dir = crate::sandbox::default_sandbox_dir(&args.data_dir);
    let backend = std::env::var("GSN_SANDBOX_BACKEND").unwrap_or_else(|_| "process".to_string());
    let sb_cfg = match backend.as_str() {
        "process" | "real" => {
            let cfg = crate::sandbox::config::SandboxConfig::trusted_local(
                "daemon operator selected process backend; host has no egress/FS/quota primitive",
            );
            println!("✅ Agent Sandbox：process 后端（已显式承认 网络/FS/磁盘 边界由宿主承担）");
            cfg
        }
        "none" | "off" | "null" | "disabled" => {
            println!("⚠️ Agent Sandbox：已禁用（GSN_SANDBOX_BACKEND=none），create 将被拒绝");
            crate::sandbox::config::SandboxConfig::default()
        }
        other => {
            eprintln!(
                "⚠️ 未知 GSN_SANDBOX_BACKEND={other}（有效值: process/none）；沙箱按禁用处理"
            );
            crate::sandbox::config::SandboxConfig::default()
        }
    };
    let sandbox_mgr = Arc::new(std::sync::Mutex::new(SandboxManager::new(
        sb_dir, sb_cfg, 0,
    )));
    println!("✅ Agent Sandbox manager 已启动（/api/v1/sandboxes）");

    // v3.0.0：一切插件化内核（PluginHost）。装配随内核的 T0 系统插件，
    // 并通过 /api/v1/plugins 暴露 list / call / install / reload / uninstall。
    // 进程沙箱工作目录放在 data-dir 下。
    // B3：T0 系统插件接线真实 daemon 句柄（存储 + 网络）。
    let system_handles = crate::plugin::system::SystemHandles::default()
        .with_store(store.clone())
        .with_peer(peer_cmd_tx.clone());
    let mut plugin_host = crate::plugin::PluginHost::new(
        env!("CARGO_PKG_VERSION"),
        Some(args.data_dir.join("plugins")),
    );
    // v3.5.2（AU-07/AU-24 生产接线）：构造 host 后、装配/启动任何插件之前，从
    // GSN_BLACKLIST_FILE 播种操作员黑名单。未设置/空路径/文件缺失 = 空黑名单（现状 no-op）；
    // 但一旦操作员显式设置了该文件却读取/解析失败，属"显式配置被无视"，危险——打印 CRITICAL
    // 并 fail-closed 拒绝启动插件子系统，绝不静默按空表继续。
    if let Err(e) = plugin_host.seed_blacklist_from_env() {
        eprintln!(
            "CRITICAL: GSN_BLACKLIST_FILE 黑名单播种失败，拒绝以不完整黑名单启动插件子系统: {e}"
        );
        return Err(anyhow::anyhow!("plugin blacklist seed failed: {e}"));
    }
    match plugin_host.boot_system(&system_handles) {
        Ok(started) => println!(
            "✅ Plugin Host 已启动：{} 个 T0 系统插件（{}）",
            started.len(),
            started.join(", ")
        ),
        Err(e) => eprintln!("⚠️ Plugin Host 系统插件装配失败: {e}"),
    }
    // v3.1.0：随内核装配 T1 官方插件（进程隔离 + entry 业务模块）。
    match plugin_host.boot_official() {
        Ok(started) => println!(
            "✅ 官方插件已启动：{} 个 T1（{}）",
            started.len(),
            started.join(", ")
        ),
        Err(e) => eprintln!("⚠️ 官方插件装配失败: {e}"),
    }
    let plugin_host = Arc::new(std::sync::Mutex::new(plugin_host));
    println!("✅ gsn-daemon 启动完成");

    // CRDT REST/管理面句柄：持有共享存储 + 网络发布通道（每连接 clone）。
    let crdt_handle = CrdtHandle::new(crdt_store, peer_cmd_tx.clone());

    run_api_server(
        args.listen.clone(),
        args.api_port,
        args.mode.clone(),
        args.port,
        Instant::now(),
        peer_cmd_tx,
        market,
        sandbox_mgr,
        plugin_host,
        crdt_handle,
        shard_ctrl_tx,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod http_security_tests {
    use super::{compute_cors, handle_plugin_api, rest_authorize};

    // 环境变量是进程全局的；把所有依赖环境变量的断言放进单个测试函数
    // 串行执行，避免与其它测试并行运行时相互污染。
    #[test]
    fn test_compute_cors_and_rest_authorize() {
        // ── 保存现场 ──
        let keys = [
            "REST_ALLOWED_ORIGINS",
            "REST_BEARER_TOKEN",
            "REST_ALLOW_UNAUTHENTICATED",
        ];
        let before: Vec<Option<String>> = keys.iter().map(|k| std::env::var(k).ok()).collect();

        // ── compute_cors：白名单命中才回显，否则空 ──
        std::env::set_var(
            "REST_ALLOWED_ORIGINS",
            "https://app.example.com, http://localhost:3000",
        );
        let h = compute_cors(Some("https://app.example.com"));
        assert!(h.contains("Access-Control-Allow-Origin: https://app.example.com"));
        assert!(h.contains("Vary: Origin"));
        // 白名单外的来源 → 空（不回显，消除通配 *）
        assert_eq!(compute_cors(Some("https://evil.com")), "");
        // 无 Origin → 空
        assert_eq!(compute_cors(None), "");
        std::env::remove_var("REST_ALLOWED_ORIGINS");

        // ── rest_authorize：只读方法放行 ──
        std::env::remove_var("REST_BEARER_TOKEN");
        std::env::remove_var("REST_ALLOW_UNAUTHENTICATED");
        assert!(rest_authorize("GET", None).is_ok());
        assert!(rest_authorize("HEAD", None).is_ok());
        assert!(rest_authorize("OPTIONS", None).is_ok());
        // 未配置 token：变更类默认 fail-closed 401
        assert_eq!(rest_authorize("POST", None).unwrap_err().0, 401);
        // 逃生口放行
        std::env::set_var("REST_ALLOW_UNAUTHENTICATED", "1");
        assert!(rest_authorize("POST", None).is_ok());
        std::env::remove_var("REST_ALLOW_UNAUTHENTICATED");

        // ── 配置 token：Bearer 匹配才放行 ──
        std::env::set_var("REST_BEARER_TOKEN", "secret-token");
        assert_eq!(rest_authorize("POST", None).unwrap_err().0, 401);
        assert_eq!(
            rest_authorize("POST", Some("Bearer wrong")).unwrap_err().0,
            401
        );
        assert!(rest_authorize("POST", Some("Bearer secret-token")).is_ok());
        assert!(rest_authorize("PUT", Some("Bearer secret-token")).is_ok());
        assert!(rest_authorize("DELETE", Some("Bearer secret-token")).is_ok());
        std::env::remove_var("REST_BEARER_TOKEN");

        // ── 恢复现场 ──
        for (k, v) in keys.iter().zip(before) {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }

    // ── v3.4.2 全局审核：补齐 handle_plugin_api 生命周期/信任/黑名单接线 ──
    use crate::plugin::blacklist::{BlacklistEntry, BlacklistReason};
    use crate::plugin::PluginHost;

    #[test]
    fn plugin_api_stop_start_routes_reachable() {
        let mut host = PluginHost::new("3.4.2", None);
        host.boot_system(&crate::plugin::system::SystemHandles::default())
            .expect("boot system plugins");
        let idp = "/api/v1/plugins/com.twinsearth.sys.identity";

        // 初始 Running
        let (st, body) = handle_plugin_api("GET", idp, "", &mut host);
        assert_eq!(st, 200);
        assert_eq!(body["state"], "Running");

        // POST stop → Stopped（暂停，不卸载）
        let (st, body) = handle_plugin_api("POST", &format!("{idp}/stop"), "", &mut host);
        assert_eq!(st, 200, "stop 应可达: {body}");
        let (_, body) = handle_plugin_api("GET", idp, "", &mut host);
        assert_eq!(body["state"], "Stopped");

        // POST start → Running（恢复）
        let (st, body) = handle_plugin_api("POST", &format!("{idp}/start"), "", &mut host);
        assert_eq!(st, 200, "start 应可达: {body}");
        let (_, body) = handle_plugin_api("GET", idp, "", &mut host);
        assert_eq!(body["state"], "Running");
    }

    #[test]
    fn plugin_api_trust_route_fail_closed() {
        let mut host = PluginHost::new("3.4.2", None);

        // 缺 publisher_key → 400
        let (st, _) = handle_plugin_api("POST", "/api/v1/plugins/trust", "{}", &mut host);
        assert_eq!(st, 400);

        // 提供 key → 200（信任第三方，补齐 T3 安装前置）
        let (st, body) = handle_plugin_api(
            "POST",
            "/api/v1/plugins/trust",
            r#"{"publisher_key":"abcd1234"}"#,
            &mut host,
        );
        assert_eq!(st, 200, "trust 应可达: {body}");
        assert_eq!(body["trusted"], "abcd1234");
    }

    #[test]
    fn plugin_api_blacklist_query_and_unblock() {
        let mut host = PluginHost::new("3.4.2", None);
        let bad = "com.twinsearth.third-party.bad";
        host.blacklist_mut().add(BlacklistEntry {
            plugin_name: bad.to_string(),
            module_sha256: Some("old-digest".to_string()),
            reason: BlacklistReason::Malware,
            blacklisted_at: 0,
            evidence: "test evidence".to_string(),
            appeal: None,
        });

        // GET blacklist → 1 条
        let (st, body) = handle_plugin_api("GET", "/api/v1/plugins/blacklist", "", &mut host);
        assert_eq!(st, 200);
        assert_eq!(body["blacklist"].as_array().unwrap().len(), 1);
        assert_eq!(body["blacklist"][0]["plugin_name"], bad);

        // 用相同摘要解封 → 400（必须发布新版本，摘要须不同）
        let (st, _) = handle_plugin_api(
            "POST",
            "/api/v1/plugins/com.twinsearth.third-party.bad/unblock",
            r#"{"new_module_sha256":"old-digest"}"#,
            &mut host,
        );
        assert_eq!(st, 400);

        // 缺 new_module_sha256 → 400
        let (st, _) = handle_plugin_api(
            "POST",
            "/api/v1/plugins/com.twinsearth.third-party.bad/unblock",
            "{}",
            &mut host,
        );
        assert_eq!(st, 400);

        // 用不同摘要解封 → 200，黑名单清空
        let (st, body) = handle_plugin_api(
            "POST",
            "/api/v1/plugins/com.twinsearth.third-party.bad/unblock",
            r#"{"new_module_sha256":"new-digest"}"#,
            &mut host,
        );
        assert_eq!(st, 200, "unblock 应可达: {body}");
        let (_, body) = handle_plugin_api("GET", "/api/v1/plugins/blacklist", "", &mut host);
        assert_eq!(body["blacklist"].as_array().unwrap().len(), 0);
    }

    // v3.5.3（AU-30）：root 启动闸门纯函数用例。
    #[test]
    fn au30_root_gate() {
        use super::ensure_not_root;
        // root + 未允许 → 拒绝。
        assert!(ensure_not_root(false, 0).is_err());
        // root + 显式允许 → 放行。
        assert!(ensure_not_root(true, 0).is_ok());
        // 非 root → 放行（无论 allow 与否）。
        assert!(ensure_not_root(false, 1000).is_ok());
        assert!(ensure_not_root(true, 1000).is_ok());
    }
}

#[cfg(test)]
mod crdt_wiring_tests {
    //! CRDT 接线单测：用一个未连接的 PeerCmd channel + 真实 CrdtStore，
    //! 验证 CrdtHandle 的本地写入 → Op 广播、快照广播、以及 metrics 渲染。
    use super::*;
    use crate::crdt::{CrdtMessage, CrdtStore, CRDT_TOPIC};

    fn test_handle() -> (CrdtHandle, mpsc::Receiver<PeerCommand>) {
        let (tx, rx) = mpsc::channel::<PeerCommand>(8);
        let store = Arc::new(std::sync::Mutex::new(CrdtStore::new("peer-test")));
        (CrdtHandle::new(store, tx), rx)
    }

    #[test]
    fn crdt_put_locally_applies_and_broadcasts_op() {
        let (h, mut rx) = test_handle();
        assert_eq!(h.len(), 0);
        let op = h.put("greeting", "hello").expect("put ok");
        assert_eq!(op.origin, "peer-test");
        assert_eq!(op.counter, 1);
        assert_eq!(h.get("greeting").as_deref(), Some("hello"));
        assert_eq!(h.len(), 1);

        // 通道里应收到一条 Publish 到 CRDT_TOPIC，载荷可解析为 CrdtMessage::Op。
        let cmd = rx.try_recv().expect("expected a published command");
        match cmd {
            PeerCommand::Publish { topic, data } => {
                assert_eq!(topic, CRDT_TOPIC);
                let msg: CrdtMessage = serde_json::from_slice(&data).expect("parse envelope");
                match msg {
                    CrdtMessage::Op(o) => {
                        assert_eq!(o.key, "greeting");
                        assert_eq!(o.value, "hello");
                    }
                    CrdtMessage::Snapshot(_) => panic!("expected Op, got Snapshot"),
                }
            }
            _ => panic!("expected Publish command"),
        }
    }

    #[test]
    fn crdt_broadcast_snapshot_publishes_snapshot_envelope() {
        let (h, mut rx) = test_handle();
        h.put("k1", "v1").unwrap();
        // put 已产生一条 Op；先 drained 掉
        let _ = rx.try_recv();
        h.broadcast_snapshot().unwrap();
        let cmd = rx.try_recv().expect("expected snapshot publish");
        match cmd {
            PeerCommand::Publish { topic, data } => {
                assert_eq!(topic, CRDT_TOPIC);
                let msg: CrdtMessage = serde_json::from_slice(&data).unwrap();
                match msg {
                    CrdtMessage::Snapshot(snap) => {
                        assert_eq!(snap.entries.len(), 1);
                        assert_eq!(snap.entries[0].key, "k1");
                        assert!(snap.seq >= 1);
                    }
                    CrdtMessage::Op(_) => panic!("expected Snapshot, got Op"),
                }
            }
            _ => panic!("expected Publish command"),
        }
    }

    #[test]
    fn crdt_entry_reports_ts_and_origin_and_missing_is_none() {
        let (h, _rx) = test_handle();
        h.put("k", "v").unwrap();
        let e = h.entry("k").expect("entry exists");
        assert_eq!(e.value, "v");
        assert_eq!(e.origin, "peer-test");
        assert!(e.ts > 0);
        assert!(h.entry("missing").is_none());
        assert_eq!(h.stats().applied_ops, 1);
        assert!(h.size_bytes() > 0);
    }

    #[test]
    fn prometheus_metrics_renders_all_metric_lines() {
        let s = CrdtStats {
            inbound_ops: 3,
            applied_ops: 2,
            dropped_capped: 1,
            snapshot_seq: 4,
        };
        let body = render_prometheus_metrics(12.5, 5, 7, &s, 9, 1024);
        for name in [
            "gsn_uptime_seconds",
            "gsn_connected_peers",
            "gsn_dht_routing_entries",
            "gsn_crdt_keys",
            "gsn_crdt_size_bytes",
            "gsn_crdt_inbound_ops_total",
            "gsn_crdt_applied_ops_total",
            "gsn_crdt_dropped_capped_total",
        ] {
            assert!(
                body.contains(&format!("# TYPE {name}")),
                "missing TYPE for {name}"
            );
            assert!(
                body.contains(&format!("# HELP {name}")),
                "missing HELP for {name}"
            );
        }
        assert!(body.contains("gsn_connected_peers 5"));
        assert!(body.contains("gsn_crdt_keys 9"));
        assert!(body.contains("gsn_crdt_inbound_ops_total 3"));
        assert!(body.contains("gsn_crdt_applied_ops_total 2"));
        assert!(body.contains("gsn_crdt_dropped_capped_total 1"));
    }
}
