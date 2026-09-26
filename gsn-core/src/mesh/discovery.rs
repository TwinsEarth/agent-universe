//! 网络嗅探与自动发现：扫描本地网络，发现 peers

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 发现公告：peer 上线时广播自己的信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryAnnouncement {
    /// 临时会话识别码
    pub session_sn: String,
    /// 永久 DID（可选，首次上线可能未绑定）
    pub permanent_did: Option<String>,
    /// 主机名
    pub hostname: String,
    /// 操作系统
    pub os: String,
    /// 监听地址
    pub listen_addrs: Vec<String>,
    /// 监听端口
    pub port: u16,
    /// 协议版本
    pub protocol_version: String,
}

/// 发现表：记录所有嗅探到的 peers
pub struct DiscoveryTable {
    peers: HashMap<String, DiscoveryAnnouncement>,
}

impl DiscoveryTable {
    pub fn new() -> Self {
        Self { peers: HashMap::new() }
    }

    /// 收到一个 peer 的发现公告
    pub fn ingest(&mut self, ann: DiscoveryAnnouncement) {
        self.peers.insert(ann.session_sn.clone(), ann);
    }

    pub fn get(&self, session_sn: &str) -> Option<&DiscoveryAnnouncement> {
        self.peers.get(session_sn)
    }

    pub fn all(&self) -> Vec<&DiscoveryAnnouncement> {
        let mut v: Vec<_> = self.peers.values().collect();
        v.sort_by(|a, b| a.session_sn.cmp(&b.session_sn));
        v
    }

    pub fn count(&self) -> usize {
        self.peers.len()
    }

    /// 按操作系统筛选
    pub fn by_os(&self, os: &str) -> Vec<&DiscoveryAnnouncement> {
        self.peers.values().filter(|a| a.os == os).collect()
    }

    /// 已绑定永久 DID 的 peers
    pub fn bound_peers(&self) -> Vec<&DiscoveryAnnouncement> {
        self.peers.values().filter(|a| a.permanent_did.is_some()).collect()
    }
}

impl Default for DiscoveryTable {
    fn default() -> Self {
        Self::new()
    }
}

/// 本地网络嗅探器
pub struct LocalSniffer {
    /// 扫描的 CIDR 网段
    pub scan_cidr: String,
    /// 扫描端口列表
    pub scan_ports: Vec<u16>,
    /// 发现结果
    table: DiscoveryTable,
    /// 扫描次数
    scan_count: u32,
}

impl LocalSniffer {
    pub fn new(cidr: &str) -> Self {
        Self {
            scan_cidr: cidr.to_string(),
            scan_ports: vec![4001, 4002, 4101, 4102],
            table: DiscoveryTable::new(),
            scan_count: 0,
        }
    }

    /// 执行一次扫描（模拟：生产环境用 mDNS 或 UDP 广播）
    pub fn scan(&mut self) -> usize {
        self.scan_count += 1;
        self.table.count()
    }

    /// 模拟发现一个 peer（测试用）
    pub fn simulate_discovery(&mut self, ann: DiscoveryAnnouncement) {
        self.table.ingest(ann);
    }

    pub fn table(&self) -> &DiscoveryTable {
        &self.table
    }

    pub fn scan_count(&self) -> u32 {
        self.scan_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ann(sn: &str, os: &str, did: Option<&str>) -> DiscoveryAnnouncement {
        DiscoveryAnnouncement {
            session_sn: sn.into(),
            permanent_did: did.map(|s| s.into()),
            hostname: format!("host-{}", sn),
            os: os.into(),
            listen_addrs: vec![format!("/ip4/192.168.1.{}/tcp/4001", sn)],
            port: 4001,
            protocol_version: "gsn/0.2.53".into(),
        }
    }

    #[test]
    fn ingest_and_retrieve() {
        let mut t = DiscoveryTable::new();
        t.ingest(ann("SN001", "macos", Some("did:mac")));
        assert_eq!(t.count(), 1);
        let a = t.get("SN001").unwrap();
        assert_eq!(a.os, "macos");
        assert_eq!(a.permanent_did, Some("did:mac".to_string()));
    }

    #[test]
    fn three_os_types_discovered() {
        let mut t = DiscoveryTable::new();
        t.ingest(ann("SN001", "macos", Some("did:mac")));
        t.ingest(ann("SN002", "linux", Some("did:linux")));
        t.ingest(ann("SN003", "windows", None));

        assert_eq!(t.count(), 3);
        assert_eq!(t.by_os("macos").len(), 1);
        assert_eq!(t.by_os("linux").len(), 1);
        assert_eq!(t.by_os("windows").len(), 1);
    }

    #[test]
    fn bound_vs_unbound_peers() {
        let mut t = DiscoveryTable::new();
        t.ingest(ann("SN001", "macos", Some("did:mac")));
        t.ingest(ann("SN002", "linux", Some("did:linux")));
        t.ingest(ann("SN003", "windows", None));

        assert_eq!(t.bound_peers().len(), 2);
    }

    #[test]
    fn sniffer_scans_and_discovers() {
        let mut s = LocalSniffer::new("192.168.1.0/24");
        s.scan();
        assert_eq!(s.scan_count(), 1);

        s.simulate_discovery(ann("SN001", "macos", None));
        assert_eq!(s.table().count(), 1);
    }

    #[test]
    fn all_sorted_by_sn() {
        let mut t = DiscoveryTable::new();
        t.ingest(ann("SN003", "windows", None));
        t.ingest(ann("SN001", "macos", None));
        t.ingest(ann("SN002", "linux", None));

        let all = t.all();
        assert_eq!(all[0].session_sn, "SN001");
        assert_eq!(all[1].session_sn, "SN002");
        assert_eq!(all[2].session_sn, "SN003");
    }
}
