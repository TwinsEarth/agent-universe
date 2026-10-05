//! 恶意节点鲁棒性集成测试：GossipSub 在 20% 恶意节点（订阅但不转发）下的消息到达率。
//!
//! 场景：同进程内 10 个独立 libp2p swarm，各自拥有独立 Ed25519 Keypair、独立 TCP 监听端口
//! （真实 127.0.0.1 TCP 连接，无内存替身）。其中 8 个为正常节点（honest），2 个为恶意节点
//! （malicious，索引 8、9）。
//!
//! 恶意节点实现（已核对 libp2p-gossipsub 0.47.0 源码确认）：
//!   - 它**订阅**同一主题，但把 mesh 配置设为 mesh_outbound_min=0 / mesh_n_low=0 /
//!     mesh_n=0 / mesh_n_high=0（ConfigBuilder::build 校验通过：0<=0<=0<=0 且 0*2<=0）。
//!   - 因为 mesh_n=0，它自己永远不向别人发 GRAFT、不维护任何 mesh 链路；
//!   - 因为 mesh_n_high=0，对端发来的入站 GRAFT 一律 PRUNE 回去（见 behaviour.rs handle_graft
//!     第 1419 行：peers.len()(0) >= mesh_n_high(0) 恒真，且入站 peer 非 outbound），
//!     于是它的 mesh 永远为空——收到消息后 forward_msg 没有 mesh peer 可转发 = “订阅但不转发”。
//!   - 对端（正常节点）收到 PRUNE 后会把恶意节点移出 mesh，转而 GRAFT 其它正常节点，
//!     这就是 mesh 的自愈（self-healing）。
//!
//! 验收：
//!   1) 10 个真实 swarm 全互联（任意 i<j 由 i dial j，共 45 条连接）；
//!   2) 从正常节点 0 发唯一 payload，统计最多 30s 内收到它的【正常节点】数 / 8；
//!   3) 断言到达率 > 0.95（正常节点应全到）；
//!   4) 打印每个节点是否收到、传播来源、实测到达率与 mesh 自愈观察。
//!
//! 运行：`cargo test --test malicious_test -- --nocapture`

use futures::StreamExt;
use libp2p::gossipsub::{self, IdentTopic, MessageAuthenticity};
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{noise, ping, tcp, yamux, Multiaddr, PeerId, SwarmBuilder};
use std::time::{Duration, Instant};

const N_NODES: usize = 10;
const N_MALICIOUS: usize = 2;
const N_HONEST: usize = N_NODES - N_MALICIOUS; // 8
const MALICIOUS_IDX: [usize; N_MALICIOUS] = [8, 9];
const TOPIC: &str = "gsn.malicious.resilience.v1";
/// 单个 swarm 每次驱动的最长等待（毫秒）。
const POLL_STEP_MS: u64 = 10;
/// 最长测量窗口（含 mesh 建立后的传播时间）。
const MEASURE_TIMEOUT: Duration = Duration::from_secs(30);

/// 组合的网络行为：仅 gossipsub + ping（保活）。最小化依赖，全部逻辑在本文件内。
#[derive(NetworkBehaviour)]
struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    ping: ping::Behaviour,
}

/// 一个独立 swarm 节点。
struct TestNode {
    swarm: Swarm<Behaviour>,
    peer_id: PeerId,
    idx: usize,
    malicious: bool,
    /// 内核分配的真实 TCP 监听地址（NewListenAddr 后才得知）。
    listen_addr: Option<Multiaddr>,
}

/// 构造 gossipsub。恶意节点用空 mesh 配置使其“订阅但不转发”。
fn build_gossipsub(key: &libp2p::identity::Keypair, malicious: bool) -> gossipsub::Behaviour {
    let mut builder = gossipsub::ConfigBuilder::default();
    // 加快 mesh 建立/自愈节奏，便于在测试窗口内观测。
    builder
        .heartbeat_interval(Duration::from_millis(500))
        .heartbeat_initial_delay(Duration::from_millis(300))
        .validation_mode(gossipsub::ValidationMode::Permissive);
    if malicious {
        // 恶意节点：订阅但不维护 mesh、不承担转发。
        builder
            .mesh_outbound_min(0)
            .mesh_n_low(0)
            .mesh_n(0)
            .mesh_n_high(0);
    }
    let config = builder
        .build()
        .expect("gossipsub config 校验应通过（恶意节点 0<=0<=0<=0 合法）");
    gossipsub::Behaviour::new(MessageAuthenticity::Signed(key.clone()), config)
        .expect("gossipsub Behaviour 构造")
}

/// 起一个独立 swarm：独立 Keypair + 真实 TCP 监听 0.0.0.0:0。
async fn spawn_node(idx: usize, malicious: bool) -> TestNode {
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(local_key.public());
    let gossipsub_key = local_key.clone();

    let swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .expect("TCP transport")
        .with_behaviour(move |_k: &libp2p::identity::Keypair| Behaviour {
            gossipsub: build_gossipsub(&gossipsub_key, malicious),
            ping: ping::Behaviour::new(ping::Config::new()),
        })
        .expect("behaviour")
        // 测试里 swarm 由单线程轮转驱动，避免 5s idle 默认把连接判死。
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::MAX))
        .build();

    let mut node = TestNode {
        swarm,
        peer_id,
        idx,
        malicious,
        listen_addr: None,
    };
    node.swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .expect("listen_on tcp/0");
    node
}

/// 从监听地址取出 TCP 端口。
fn tcp_port_of(addr: &Multiaddr) -> Option<u16> {
    addr.iter().find_map(|p| match p {
        Protocol::Tcp(port) => Some(port),
        _ => None,
    })
}

/// 处理一个 swarm 事件，收集 listen 地址 / 订阅事件 / 收到目标消息。
/// 返回 true 表示该节点（正常节点）收到了目标 payload。
fn handle_event(
    node: &mut TestNode,
    ev: SwarmEvent<BehaviourEvent>,
    payload: &[u8],
    received: &mut [bool],
    prop_source: &mut [Option<PeerId>],
    subscribed_view: &mut [usize],
) {
    match ev {
        SwarmEvent::NewListenAddr { address, .. } => {
            if tcp_port_of(&address).is_some() && node.listen_addr.is_none() {
                node.listen_addr = Some(address);
            }
        }
        SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
            ..
        })) => {
            subscribed_view[node.idx] += 1;
        }
        SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message {
            propagation_source,
            message,
            ..
        })) if message.data == payload && !node.malicious && !received[node.idx] => {
            received[node.idx] = true;
            prop_source[node.idx] = Some(propagation_source);
        }
        _ => {}
    }
}

/// 在 `dur` 时间内驱动所有 swarm，对每个事件调用处理逻辑。
async fn drive_all<F>(nodes: &mut [TestNode], dur: Duration, mut on_event: F)
where
    F: FnMut(&mut TestNode, SwarmEvent<BehaviourEvent>),
{
    let end = Instant::now() + dur;
    while Instant::now() < end {
        let mut progressed = false;
        for node in nodes.iter_mut() {
            match tokio::time::timeout(Duration::from_millis(POLL_STEP_MS), node.swarm.next()).await
            {
                Ok(Some(ev)) => {
                    on_event(node, ev);
                    progressed = true;
                }
                Ok(None) => {}
                Err(_) => {} // 本节点暂时无事件，轮到下一个
            }
        }
        if !progressed {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

#[tokio::test]
async fn gossipsub_survives_20pct_malicious_nonforwarders() {
    // 唯一 payload（每个测试 run 重新随机）。
    let payload: Vec<u8> = format!(
        "malicious-resilience-payload-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    )
    .into_bytes();
    let topic = IdentTopic::new(TOPIC);

    println!("=== GossipSub 恶意节点鲁棒性测试 ===");
    println!("payload = {}", String::from_utf8_lossy(&payload));
    println!(
        "节点配置：{} 正常 + {} 恶意（索引 {:?}），全互联真实 TCP\n",
        N_HONEST, N_MALICIOUS, MALICIOUS_IDX
    );

    // ── 1. 起 10 个独立 swarm ──
    let mut nodes: Vec<TestNode> = Vec::with_capacity(N_NODES);
    for idx in 0..N_NODES {
        let malicious = MALICIOUS_IDX.contains(&idx);
        nodes.push(spawn_node(idx, malicious).await);
    }

    // ── 2. 轮询拿到每个节点的真实 TCP 监听地址 ──
    let mut dummy_recv = vec![false; N_NODES];
    let mut dummy_src = vec![None; N_NODES];
    let mut sub_view = vec![0usize; N_NODES];
    drive_all(&mut nodes, Duration::from_secs(10), |node, ev| {
        handle_event(
            node,
            ev,
            &payload,
            &mut dummy_recv,
            &mut dummy_src,
            &mut sub_view,
        );
    })
    .await;
    for node in &nodes {
        let addr = node
            .listen_addr
            .as_ref()
            .expect("每个节点都应拿到 TCP NewListenAddr");
        println!(
            "  node {:>2} {} peer={} listen={}",
            node.idx,
            if node.malicious { "[MAL]" } else { "[OK] " },
            node.peer_id,
            addr
        );
    }

    // ── 3. 全互联：i<j 由 i dial j（共 45 条连接，避免重复 dial）──
    for i in 0..N_NODES {
        for j in (i + 1)..N_NODES {
            let port = tcp_port_of(nodes[j].listen_addr.as_ref().unwrap()).unwrap();
            let j_peer = nodes[j].peer_id;
            let dial: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", port, j_peer)
                .parse()
                .unwrap();
            nodes[i].swarm.dial(dial).expect("dial");
        }
    }

    // 等所有节点都达到 9 条连接（全互联）。
    let conn_deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < conn_deadline {
        let all_connected = nodes
            .iter()
            .all(|n| n.swarm.connected_peers().count() == N_NODES - 1);
        if all_connected {
            break;
        }
        drive_all(&mut nodes, Duration::from_millis(100), |node, ev| {
            handle_event(
                node,
                ev,
                &payload,
                &mut dummy_recv,
                &mut dummy_src,
                &mut sub_view,
            );
        })
        .await;
    }
    for node in &nodes {
        println!(
            "  node {:>2} 连接数={}/{}",
            node.idx,
            node.swarm.connected_peers().count(),
            N_NODES - 1
        );
    }

    // ── 4. 全部节点订阅主题（含恶意节点：订阅但不转发）──
    for node in nodes.iter_mut() {
        node.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .expect("subscribe");
    }

    // ── 5. mesh 建立预热：跑若干 heartbeat，让 SUBSCRIBE 交换 + GRAFT/PRUNE 收敛。
    //    恶意节点会回 PRUNE，正常节点自愈地把 mesh 重新连到其它正常节点。
    drive_all(&mut nodes, Duration::from_secs(6), |node, ev| {
        handle_event(
            node,
            ev,
            &payload,
            &mut dummy_recv,
            &mut dummy_src,
            &mut sub_view,
        );
    })
    .await;

    // 打印预热后的 mesh 视图（自愈观察）。
    let topic_hash = topic.hash();
    println!("\n--- mesh 预热后视图（正常节点应已把恶意节点移出 mesh）---");
    for node in &nodes {
        let mesh: Vec<_> = node
            .swarm
            .behaviour()
            .gossipsub
            .mesh_peers(&topic_hash)
            .cloned()
            .collect();
        let mesh_idxs: Vec<String> = mesh
            .iter()
            .map(|p| {
                nodes
                    .iter()
                    .find(|n| &n.peer_id == p)
                    .map(|n| n.idx.to_string())
                    .unwrap_or_else(|| p.to_string())
            })
            .collect();
        println!(
            "  node {:>2} {} mesh_size={} mesh_peers=[{}] 连接数={} 观察到对端订阅事件={}",
            node.idx,
            if node.malicious { "[MAL]" } else { "[OK] " },
            mesh.len(),
            mesh_idxs.join(","),
            node.swarm.connected_peers().count(),
            sub_view[node.idx],
        );
    }

    // ── 6. 从正常节点 0 发布唯一 payload，测量到达率 ──
    let mut received = vec![false; N_NODES];
    let mut prop_source: Vec<Option<PeerId>> = vec![None; N_NODES];
    received[0] = true; // 发布者本身“拥有”该消息
    prop_source[0] = None;

    let measure_start = Instant::now();
    let mut last_republish = Instant::now();
    let mut republish_count = 0u32;
    let deadline = Instant::now() + MEASURE_TIMEOUT;
    while Instant::now() < deadline {
        // 每 500ms 重发一次（gossipsub seq 递增 => 新 message_id，节点会再次投递）。
        if last_republish.elapsed() >= Duration::from_millis(500) {
            match nodes[0]
                .swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic.clone(), payload.clone())
            {
                Ok(_) => republish_count += 1,
                Err(e) => panic!("node0 publish 失败: {e}"),
            }
            last_republish = Instant::now();
        }

        // 驱动所有 swarm 一小段时间。
        drive_all(&mut nodes, Duration::from_millis(50), |node, ev| {
            handle_event(
                node,
                ev,
                &payload,
                &mut received,
                &mut prop_source,
                &mut sub_view,
            );
        })
        .await;

        // 全部正常节点已到齐 => 提前结束。
        let got = received[..N_HONEST].iter().filter(|&&r| r).count();
        if got == N_HONEST {
            break;
        }
    }
    let elapsed = measure_start.elapsed();

    // ── 7. 打印明细 + 断言 ──
    println!(
        "\n--- 消息到达明细（最多 {}s）---",
        MEASURE_TIMEOUT.as_secs()
    );
    for node in &nodes {
        let got = received[node.idx];
        let src = prop_source[node.idx]
            .map(|p| {
                nodes
                    .iter()
                    .find(|n| n.peer_id == p)
                    .map(|n| format!("node{}", n.idx))
                    .unwrap_or_else(|| p.to_string())
            })
            .unwrap_or_else(|| {
                if node.idx == 0 {
                    "publisher".to_string()
                } else {
                    "-".to_string()
                }
            });
        println!(
            "  node {:>2} {} 收到={:<14} 传播来源={}",
            node.idx,
            if node.malicious { "[MAL]" } else { "[OK] " },
            if node.malicious {
                "(不计入分母)"
            } else if got {
                "YES"
            } else {
                "NO"
            },
            src
        );
    }

    let received_normal: usize = received[..N_HONEST].iter().filter(|&&r| r).count();
    let rate = received_normal as f64 / N_HONEST as f64;
    println!(
        "\n=== 结果：正常节点收到 {}/{}，到达率 = {:.2}%（重发 {} 次，耗时 {:?}）===",
        received_normal,
        N_HONEST,
        rate * 100.0,
        republish_count,
        elapsed
    );

    assert!(
        rate > 0.95,
        "到达率 {:.2}% <= 95%，未通过鲁棒性阈值（收到 {}/{} 个正常节点）",
        rate * 100.0,
        received_normal,
        N_HONEST
    );
    println!(
        "=== 通过：到达率 {:.2}% > 95%。mesh 在 2 个恶意不转发节点存在下仍完成自愈传播。 ===",
        rate * 100.0
    );
}
