//! 节点运行时
//!
//! 把守护进程的启动逻辑封装为库函数，供 `gsn-daemon` 二进制与
//! `gsn daemon` 子命令复用，避免逻辑重复。

use crate::api::market_actor::MarketActorHandle;
use crate::net::P2pPeer;
use crate::relay_pool::{self, RelayClass, DEFAULT_PARALLEL_RELAYS};
use crate::storage::{PersistentStore, StoredAgent, StoredRelay};
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
    GetInfo { reply: oneshot::Sender<PeerInfo> },
    DhtPut { key: String, value: Vec<u8> },
    Subscribe { topic: String },
    Publish { topic: String, data: Vec<u8> },
    /// v2.5.3: 主动连接 bootstrap 节点
    AddBootstrap { addr: String, reply: oneshot::Sender<Result<(), String>> },
    /// v2.5.3: 列出已连接对等节点 peer_id
    ListPeers { reply: oneshot::Sender<Vec<String>> },
    /// v2.5.4: 连接公共 libp2p bootstrap/relay 网络
    BootstrapPublic { reply: oneshot::Sender<Vec<String>> },
    /// v2.5.4: 获取 AutoNAT 检测的 NAT 状态
    NatStatus { reply: oneshot::Sender<String> },
    /// v2.5.4: 经 Circuit Relay 中继监听（listen_on /p2p-circuit，建立并持有 reservation）
    ListenRelay { addr: String, reply: oneshot::Sender<Result<(), String>> },
    /// v2.5.5: 探测候选 relay 是否提供 hop（dial + Identify 协议判定）
    ProbeRelay { addr: String, reply: oneshot::Sender<Result<bool, String>> },
    /// v2.5.5: relay 池报告（列表 + 容量快照 + 当前活跃通道）
    ListRelayPool { reply: oneshot::Sender<serde_json::Value> },
    /// v2.5.5: 手动加入 relay
    AddRelay { addr: String, class: String, reply: oneshot::Sender<Result<(), String>> },
    /// v2.5.5: 移除 relay（并关闭其通道）
    RemoveRelayCmd { target: String, reply: oneshot::Sender<Result<(), String>> },
    /// v2.5.5: 手动扩容（manual_bonus += amount），返回新有效上限
    ExpandCapacity { amount: i64, reply: oneshot::Sender<Result<i64, String>> },
    /// v2.5.5: 维持/补齐多通道到目标数（自动选择 + listen），返回活跃 relay
    EnsureChannels { reply: oneshot::Sender<Vec<String>> },
    /// v2.5.5: 触发 DHT 随机发现（扩充 hop 候选）
    DiscoverRelays,
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

async fn run_swarm_actor(
    mut peer: P2pPeer,
    mut cmd_rx: mpsc::Receiver<PeerCommand>,
    store: Arc<PersistentStore>,
) {
    // 周期性驱动网络栈：Kademlia 路由刷新、AutoNAT 重测、dnsaddr 解析与连接
    // 状态机都依赖被反复 poll；select! 在命令到达时会取消 next_event future，
    // 部分子系统的 waker 无法保证唤醒，因此用一个静默 idle tick 兜底 poll。
    let mut idle = tokio::time::interval(std::time::Duration::from_secs(5));
    idle.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

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

    loop {
        tokio::select! {
            event = peer.next_event() => {
                log_swarm_event(&event);
                let need_ensure = process_swarm_event(
                    &mut peer, &event, &store,
                    &mut active_relay_addrs, &mut pending_probes, &mut connecting,
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
                    ),
                    None => break,
                }
            }
            _ = idle.tick() => {
                // 仅用于唤醒并重新 poll swarm，无额外动作
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
    info.listen_addrs.first().map(|a| format!("{}/p2p/{}", a, peer_id))
}

/// Identify 发现支持 hop 的公网节点：自动纳入 relay 池（DHT 发现扩充）
fn auto_adopt_hop_relay(peer_id: &PeerId, info: &libp2p::identify::Info, store: &PersistentStore) {
    let id = peer_id.to_string();
    let exists = store.load_relays().map(|v| v.iter().any(|r| r.relay_id == id)).unwrap_or(false);
    if exists {
        let _ = store.set_relay_status(&id, true, "healthy", 0, 0, &now_iso());
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
fn process_swarm_event(
    peer: &mut P2pPeer,
    event: &libp2p::swarm::SwarmEvent<crate::net::peer::PeerEvent>,
    store: &PersistentStore,
    active: &mut HashMap<String, String>,
    pending: &mut HashMap<PeerId, oneshot::Sender<Result<bool, String>>>,
    connecting: &mut HashSet<PeerId>,
) -> bool {
    use libp2p::swarm::SwarmEvent::*;
    let mut need_ensure = false;
    match event {
        ConnectionClosed { peer_id, .. } => {
            let id = peer_id.to_string();
            if active.remove(&id).is_some() || connecting.remove(peer_id) {
                peer.remove_relay(&id);
                let _ = store.mark_relay_failed(&id, &now_iso());
                eprintln!("🔌 中继通道掉线 {}，准备自动切换", id);
                need_ensure = true;
            }
        }
        OutgoingConnectionError { peer_id, .. } => {
            if let Some(pid) = peer_id {
                if connecting.remove(pid) {
                    let id = pid.to_string();
                    active.remove(&id);
                    peer.remove_relay(&id);
                    let _ = store.mark_relay_failed(&id, &now_iso());
                    eprintln!("❌ relay {} 连接失败，准备自动切换", id);
                    need_ensure = true;
                }
            }
        }
        Behaviour(bev) => {
            use crate::net::peer::PeerEvent::*;
            match bev {
                RelayClient(rc) => {
                    use libp2p::relay::client::Event as RcEvent;
                    if let RcEvent::ReservationReqAccepted { relay_peer_id, renewal, limit } = rc {
                        let id = relay_peer_id.to_string();
                        connecting.remove(relay_peer_id);
                        let (dur, data) = match limit {
                            Some(l) => (
                                l.duration().map(|d| d.as_secs() as i64).unwrap_or(0),
                                l.data_in_bytes().map(|x| x as i64).unwrap_or(0),
                            ),
                            None => (0, 0),
                        };
                        let _ = store.set_relay_status(
                            &id,
                            true,
                            "active",
                            dur,
                            data,
                            &now_iso(),
                        );
                        eprintln!("✅ relay reservation 已建立 {} (renewal={}, {}s/{}B)", id, renewal, dur, data);
                    }
                }
                Identify(ie) => {
                    if let libp2p::identify::Event::Received { peer_id, info, .. } = ie {
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
                    }
                }
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
            Some(r) => {
                match peer.listen_via_relay(&r.multiaddr) {
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
                        let _ = store.mark_relay_failed(&r.relay_id, &now_iso());
                        in_use.insert(r.relay_id.clone());
                    }
                }
            }
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
        if cmd_tx.send(PeerCommand::ProbeRelay { addr: r.multiaddr.clone(), reply: tx }).await.is_err() {
            continue;
        }
        match tokio::time::timeout(std::time::Duration::from_secs(15), rx).await {
            Ok(Ok(Ok(true))) => {
                let _ = store.set_relay_status(&r.relay_id, true, "healthy", 0, 0, &now_iso());
                eprintln!("✅ probe 确认 hop relay: {}", r.relay_id);
            }
            Ok(Ok(Ok(false))) => {
                let _ = store.mark_relay_failed(&r.relay_id, &now_iso());
            }
            _ => {
                let _ = store.mark_relay_failed(&r.relay_id, &now_iso());
            }
        }
    }

    // 3. 补齐多通道
    let (tx, rx) = oneshot::channel();
    if cmd_tx.send(PeerCommand::EnsureChannels { reply: tx }).await.is_ok() {
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
        ConnectionEstablished { peer_id, endpoint, established_in, .. } => {
            eprintln!("已连接 {peer_id} ({endpoint:?}, {established_in:?})");
        }
        ConnectionClosed { peer_id, endpoint, .. } => {
            eprintln!("连接关闭 {peer_id} ({endpoint:?})");
        }
        OutgoingConnectionError { peer_id, error, .. } => {
            eprintln!("出站连接失败 peer={peer_id:?}: {error:?}");
        }
        IncomingConnectionError { send_back_addr, error, .. } => {
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
                Dcutr(e) => eprintln!("⛏️ DCUtR {e:?}"),
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
fn handle_peer_command(
    peer: &mut P2pPeer,
    cmd: PeerCommand,
    store: &PersistentStore,
    active: &mut HashMap<String, String>,
    pending: &mut HashMap<PeerId, oneshot::Sender<Result<bool, String>>>,
    connecting: &mut HashSet<PeerId>,
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
                        if let Some(pid) =
                            crate::net::peer::extract_relay_peer_id(&base).and_then(|s| s.parse::<PeerId>().ok())
                        {
                            connecting.insert(pid);
                        }
                    }
                }
            } else {
                eprintln!("⚠️ relay listen 失败 [{}]: {}", addr, result.as_ref().err().unwrap());
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
            });
            let _ = reply.send(v);
        }
        PeerCommand::AddRelay { addr, class, reply } => {
            let base: Result<Multiaddr, _> = addr.parse();
            match base {
                Ok(base) => {
                    match crate::net::peer::extract_relay_peer_id(&base) {
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
                                RelayClass::from_str(&class)
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
                    }
                }
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
                let _ = store.delete_relay(&id);
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
            let _ = store.set_meta("relay_pool:manual_bonus", &new.to_string());
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
    // v2.5.4: POST /bootstrap-public — 连接公共 libp2p 中继网络
    else if method == "POST" && (path == "/bootstrap-public" || path == "/bootstrap-public/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::BootstrapPublic { reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(initiated) => (201, serde_json::json!({
                "status":"public_network_bootstrap_initiated",
                "initiated": initiated,
                "count": initiated.len()
            }).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.4: GET /nat — 获取 AutoNAT 检测的 NAT 状态
    else if method == "GET" && (path == "/nat" || path == "/nat/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::NatStatus { reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
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
            Ok(v) => v.get("addr").and_then(|a| a.as_str()).map(|s| s.to_string()),
            Err(_) => None,
        };
        let addr = match addr {
            Some(a) => a,
            None => return (400, serde_json::json!({"error":"missing_or_invalid_addr"}).to_string()),
        };
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::ListenRelay { addr: addr.clone(), reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(Ok(())) => (201, serde_json::json!({"status":"relay_listen_initiated","addr":addr}).to_string()),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: GET /relays — relay 池报告（列表 + 容量快照 + 活跃通道）
    else if method == "GET" && (path == "/relays" || path == "/relays/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::ListRelayPool { reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(v) => (200, v.to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: POST /relays/add — 手动加入 relay（body: {"addr":"...","class":"general"}）
    else if method == "POST" && (path == "/relays/add" || path == "/relays/add/") {
        let v: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let addr = v.get("addr").and_then(|a| a.as_str()).unwrap_or("").to_string();
        let class = v.get("class").and_then(|a| a.as_str()).unwrap_or("general").to_string();
        if addr.is_empty() {
            return (400, serde_json::json!({"error":"missing_addr"}).to_string());
        }
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::AddRelay { addr: addr.clone(), class, reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(Ok(())) => (201, serde_json::json!({"status":"relay_added","addr":addr}).to_string()),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: POST /relays/remove — 移除 relay 并关闭通道（body: {"target":"<peer_id 或 addr>"}）
    else if method == "POST" && (path == "/relays/remove" || path == "/relays/remove/") {
        let v: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let target = v.get("target").and_then(|a| a.as_str()).unwrap_or("").to_string();
        if target.is_empty() {
            return (400, serde_json::json!({"error":"missing_target"}).to_string());
        }
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::RemoveRelayCmd { target: target.clone(), reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(Ok(())) => (200, serde_json::json!({"status":"relay_removed","target":target}).to_string()),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: POST /relays/expand — 手动扩容（body: {"amount":50000}）
    else if method == "POST" && (path == "/relays/expand" || path == "/relays/expand/") {
        let v: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let amount = v.get("amount").and_then(|a| a.as_i64()).unwrap_or(0);
        if amount == 0 {
            return (400, serde_json::json!({"error":"missing_or_zero_amount"}).to_string());
        }
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::ExpandCapacity { amount, reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(Ok(cap)) => (200, serde_json::json!({"status":"capacity_expanded","effective_cap":cap}).to_string()),
            Ok(Err(e)) => (400, serde_json::json!({"error":e}).to_string()),
            Err(_) => (500, serde_json::json!({"error":"no_response"}).to_string()),
        }
    }
    // v2.5.5: POST /relays/discover — 触发 DHT relay 候选发现
    else if method == "POST" && (path == "/relays/discover" || path == "/relays/discover/") {
        if cmd_tx.send(PeerCommand::DiscoverRelays).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        (202, serde_json::json!({"status":"relay_discovery_initiated"}).to_string())
    }
    // v2.5.5: POST /relays/ensure — 立即补齐多通道
    else if method == "POST" && (path == "/relays/ensure" || path == "/relays/ensure/") {
        let (reply, rx) = oneshot::channel();
        if cmd_tx.send(PeerCommand::EnsureChannels { reply }).await.is_err() {
            return (500, serde_json::json!({"error":"peer_actor_unavailable"}).to_string());
        }
        match rx.await {
            Ok(chans) => (200, serde_json::json!({"active":chans,"count":chans.len()}).to_string()),
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
    use crate::api::rest;
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
            let info = rest::NodeInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                mode: mode.clone(),
                p2p_port,
                connected_peers: connected,
                uptime_ms: start.elapsed().as_millis(),
            };

            let routed = rest::route(&method, &raw_path, &body, &market, &info).await;

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
    if store.get_meta("relay_pool:start_time").ok().flatten().is_none() {
        let _ = store.set_meta("relay_pool:start_time", &now_iso());
    }
    if store.get_meta("relay_pool:manual_bonus").ok().flatten().is_none() {
        let _ = store.set_meta("relay_pool:manual_bonus", "0");
    }
    if store.relay_count().unwrap_or(0) == 0 {
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

/// 启动节点（核心入口，gsn-daemon 与 gsn daemon 共用）
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

    let mut peer = P2pPeer::new().await?;
    match peer.listen_on_port(args.port) {
        Ok(_) => println!("✅ libp2p P2P 端口: {}", args.port),
        Err(e) => eprintln!("⚠️ P2P 端口 {} 绑定失败: {}", args.port, e),
    }
    // v2.5.5 方案3: QUIC/UDP 监听（与 TCP 同端口），用于 UDP 打洞与直连
    match peer.listen_quic_port(args.port) {
        Ok(a) => println!("✅ libp2p QUIC 端口: {}", a),
        Err(e) => eprintln!("⚠️ QUIC 端口 {} 监听失败: {}", args.port, e),
    }
    for addr_str in &args.bootstrap {
        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
            match peer.add_bootstrap(addr) {
                Ok(_) => println!("→ bootstrap: {}", addr_str),
                Err(e) => eprintln!("⚠️ bootstrap {} 失败: {}", addr_str, e),
            }
        }
    }
    // v2.5.5: dial 社区 relay 候选（无 /p2p），Identify 发现 hop 即自动入池
    for cand in COMMUNITY_RELAY_CANDIDATES {
        if let Err(e) = peer.probe_relay(cand) {
            eprintln!("⚠️ 社区候选 {} dial 失败: {}", cand, e);
        }
    }

    let (peer_cmd_tx, peer_cmd_rx) = mpsc::channel::<PeerCommand>(64);
    println!("   Peer ID: {}", peer.peer_id);
    println!("   模式: {:?}", node_mode);
    let _ = peer.subscribe("gsn/agents");
    let _ = peer.subscribe("gsn/tasks");
    tokio::spawn(run_swarm_actor(peer, peer_cmd_rx, store.clone()));

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
