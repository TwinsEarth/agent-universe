//! Mesh 自组网管理：组合心跳、发现、会话、NAT 穿透

use super::heartbeat::{HeartbeatConfig, HeartbeatTracker};
use super::discovery::{DiscoveryAnnouncement, LocalSniffer};
use super::session::{PermanentDid, SessionId, SessionRegistry};
use crate::nat::{ConnectionState, NatTraversalManager, NatType};
use serde::{Deserialize, Serialize};

/// Mesh 网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    pub heartbeat: HeartbeatConfig,
    /// 扫描网段
    pub scan_cidr: String,
    /// 本机主机名
    pub hostname: String,
    /// 本机操作系统
    pub os: String,
    /// 监听端口
    pub port: u16,
    /// 协议版本
    pub protocol_version: String,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            heartbeat: HeartbeatConfig::default(),
            scan_cidr: "192.168.1.0/24".into(),
            hostname: "unknown".into(),
            os: std::env::consts::OS.into(),
            port: 4001,
            protocol_version: "gsn/0.2.53".into(),
        }
    }
}

/// Mesh 拓扑中的节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshTopology {
    /// 本机临时 SN
    pub local_sn: String,
    /// 本机永久 DID
    pub local_did: Option<String>,
    /// 已发现的节点
    pub discovered: Vec<DiscoveryAnnouncement>,
    /// 在线节点 SN 列表
    pub online: Vec<String>,
    /// NAT 类型
    pub nat_type: String,
    /// 连接数
    pub connection_count: usize,
}

/// Mesh 节点：自组网的完整运行时
pub struct MeshNode {
    config: MeshConfig,
    heartbeat: HeartbeatTracker,
    sniffer: LocalSniffer,
    sessions: SessionRegistry,
    nat: NatTraversalManager,
    /// 本机会话
    local_session: SessionId,
    /// 本机 DID
    local_did: Option<PermanentDid>,
    /// 是否已启动
    started: bool,
}

impl MeshNode {
    pub fn new(config: MeshConfig, session_code: &str) -> Self {
        let mut sessions = SessionRegistry::new();
        let local_session = sessions.allocate_session(session_code);
        Self {
            heartbeat: HeartbeatTracker::new(config.heartbeat.clone()),
            sniffer: LocalSniffer::new(&config.scan_cidr),
            sessions,
            nat: NatTraversalManager::new(),
            local_session,
            local_did: None,
            config,
            started: false,
        }
    }

    /// 启动 mesh 节点：开始嗅探、心跳、自组网
    pub fn start(&mut self) {
        self.started = true;
        self.sniffer.scan();
    }

    /// 绑定本机永久 DID
    pub fn bind_permanent_did(&mut self, did: PermanentDid) -> Result<(), String> {
        self.sessions.bind(&self.local_session, did.clone())?;
        self.local_did = Some(did);
        Ok(())
    }

    /// 发现一个新 peer（通过嗅探）
    pub fn discover_peer(&mut self, ann: DiscoveryAnnouncement) {
        let sn = ann.session_sn.clone();
        self.sniffer.simulate_discovery(ann);
        self.heartbeat.register(&sn);
    }

    /// 收到 peer 的心跳响应
    pub fn peer_pong(&mut self, sn: &str, rtt_ms: u64) {
        self.heartbeat.record_pong(sn, rtt_ms);
    }

    /// peer 心跳超时
    pub fn peer_timeout(&mut self, sn: &str) {
        self.heartbeat.record_miss(sn);
    }

    /// 检测本机 NAT 类型
    pub fn detect_nat(&mut self) -> NatType {
        self.nat.detect_nat_type()
    }

    /// 尝试与 peer 建立连接（NAT 穿透）
    pub fn connect_peer(&mut self, sn: &str) -> ConnectionState {
        let candidates = self.nat.gather_candidates();
        self.nat.connect(sn.to_string(), candidates)
    }

    /// 推进一个心跳周期
    pub fn tick(&mut self) {
        self.heartbeat.tick();
    }

    /// 生成当前拓扑快照
    pub fn topology(&mut self) -> MeshTopology {
        let nat_type = format!("{:?}", self.nat.detect_nat_type());
        let connection_count = self.nat.connected_count();
        MeshTopology {
            local_sn: self.local_session.display(),
            local_did: self.local_did.as_ref().map(|d| d.0.clone()),
            discovered: self.sniffer.table().all().into_iter().cloned().collect(),
            online: self.heartbeat.available_peers(),
            nat_type,
            connection_count,
        }
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn local_sn(&self) -> String {
        self.local_session.display()
    }

    pub fn online_count(&self) -> usize {
        self.heartbeat.available_peers().len()
    }

    pub fn discovered_count(&self) -> usize {
        self.sniffer.table().count()
    }

    pub fn config(&self) -> &MeshConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(os: &str, hostname: &str) -> MeshConfig {
        MeshConfig {
            os: os.into(),
            hostname: hostname.into(),
            ..Default::default()
        }
    }

    fn ann_for(sn: &str, os: &str) -> DiscoveryAnnouncement {
        DiscoveryAnnouncement {
            session_sn: sn.into(),
            permanent_did: None,
            hostname: format!("host-{}", sn),
            os: os.into(),
            listen_addrs: vec![],
            port: 4001,
            protocol_version: "gsn/0.2.53".into(),
        }
    }

    #[test]
    fn node_starts_and_gets_sn() {
        let node = MeshNode::new(test_config("linux", "cloud"), "cloud01");
        assert!(!node.is_started());
        assert!(node.local_sn().contains("SN-000001"));
    }

    #[test]
    fn bind_did_after_start() {
        let mut node = MeshNode::new(test_config("macos", "macmini"), "mac01");
        node.start();
        node.bind_permanent_did(PermanentDid("did:mac:root".into())).unwrap();
        let topo = node.topology();
        assert_eq!(topo.local_did, Some("did:mac:root".to_string()));
    }

    #[test]
    fn discover_and_track_three_peers() {
        let mut node = MeshNode::new(test_config("linux", "cloud"), "cloud01");
        node.start();

        node.discover_peer(ann_for("SN-mac", "macos"));
        node.discover_peer(ann_for("SN-win", "windows"));
        node.discover_peer(ann_for("SN-lin", "linux"));

        assert_eq!(node.discovered_count(), 3);
        // 初始发现后默认在线
        node.peer_pong("SN-mac", 30);
        node.peer_pong("SN-win", 80);
        node.peer_pong("SN-lin", 5);
        assert_eq!(node.online_count(), 3);
    }

    #[test]
    fn peer_goes_offline_after_misses() {
        let mut node = MeshNode::new(test_config("linux", "cloud"), "cloud01");
        node.start();
        node.discover_peer(ann_for("SN-win", "windows"));

        node.peer_pong("SN-win", 50);
        node.peer_timeout("SN-win");
        node.peer_timeout("SN-win");
        assert_eq!(node.online_count(), 1); // suspicious 仍算非 offline

        node.peer_timeout("SN-win");
        assert_eq!(node.online_count(), 0); // offline
    }

    #[test]
    fn nat_detection_and_connect() {
        let mut node = MeshNode::new(test_config("windows", "winpc"), "win01");
        let nat = node.detect_nat();
        assert_eq!(nat, NatType::PortRestrictedCone);

        let state = node.connect_peer("SN-mac");
        assert_eq!(state, ConnectionState::Connected);
    }

    #[test]
    fn topology_snapshot_contains_all_info() {
        let mut node = MeshNode::new(test_config("macos", "macmini"), "mac01");
        node.start();
        node.bind_permanent_did(PermanentDid("did:mac".into())).unwrap();
        node.discover_peer(ann_for("SN-win", "windows"));
        node.peer_pong("SN-win", 100);

        let topo = node.topology();
        assert!(topo.local_sn.contains("SN-"));
        assert_eq!(topo.local_did, Some("did:mac".to_string()));
        assert_eq!(topo.discovered.len(), 1);
        assert_eq!(topo.online.len(), 1);
        assert!(!topo.nat_type.is_empty());
    }

    #[test]
    fn three_machine_mesh_scenario() {
        // 模拟云电脑视角：发现 mac mini 和 windows
        let mut cloud = MeshNode::new(test_config("linux", "cloud-linux"), "cloud");
        cloud.start();
        cloud.bind_permanent_did(PermanentDid("did:cloud".into())).unwrap();

        cloud.discover_peer(DiscoveryAnnouncement {
            session_sn: "SN-mac".into(),
            permanent_did: Some("did:mac".into()),
            hostname: "mac-mini".into(),
            os: "macos".into(),
            listen_addrs: vec!["/ip4/192.168.1.10/tcp/4001".into()],
            port: 4001,
            protocol_version: "gsn/0.2.53".into(),
        });
        cloud.discover_peer(DiscoveryAnnouncement {
            session_sn: "SN-win".into(),
            permanent_did: None,
            hostname: "win-pc".into(),
            os: "windows".into(),
            listen_addrs: vec!["/ip4/192.168.1.20/tcp/4001".into()],
            port: 4001,
            protocol_version: "gsn/0.2.53".into(),
        });

        cloud.peer_pong("SN-mac", 25);
        cloud.peer_pong("SN-win", 90);

        let topo = cloud.topology();
        assert_eq!(topo.discovered.len(), 2);
        assert_eq!(topo.online, vec!["SN-mac", "SN-win"]);
        assert_eq!(topo.local_did, Some("did:cloud".to_string()));
    }
}
