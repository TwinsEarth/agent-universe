//! 路由表持久化：把 kbucket 快照落盘（serde_json），重启后回填 + 再 bootstrap。
//!
//! v3.9.17：DHT 路由持久化。解决的问题：节点重启后路由表清空，只能重新
//! 从 bootstrap 种子出发扩散（冷启动慢、依赖种子可达性）。本模块把
//! `peer → 地址列表` 快照写到 `data_dir/dht-routing.json`，重启后：
//! 1. `load()` 读回快照；
//! 2. 对每条记录 `add_address` 回填 kbucket（不发起连接，只登记地址）；
//! 3. 再调 `bootstrap()`，使路由表以「旧邻居 + 种子」为起点快速收敛。
//!
//! 写入是原子的（临时文件 + rename），损坏/缺失文件返回具名错误，由调用方
//! 降级为「本次运行不持久化」并留痕——绝不阻断节点启动。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 一条路由表快照记录：peer id → 已知地址列表。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingEntry {
    /// PeerId 的 Base58 表示。
    pub peer_id: String,
    /// 已知 Multiaddr 的字符串列表（按距离/新鲜度排序不做保证，仅原样回填）。
    pub addrs: Vec<String>,
}

/// 路由表持久化存储（单文件、原子写）。
#[derive(Debug, Clone)]
pub struct RoutingTableStore {
    path: PathBuf,
}

impl RoutingTableStore {
    /// 以指定路径创建存储。
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// 快照文件是否存在。
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// 原子写入快照：先写 `path.tmp` 再 rename，避免半写文件。
    pub fn save(&self, entries: &[RoutingEntry]) -> Result<(), String> {
        let tmp = self.path.with_extension("json.tmp");
        let bytes =
            serde_json::to_vec_pretty(entries).map_err(|e| format!("序列化路由快照失败: {e}"))?;
        fs::write(&tmp, bytes).map_err(|e| format!("写临时快照失败 {tmp:?}: {e}"))?;
        fs::rename(&tmp, &self.path)
            .map_err(|e| format!("快照原子替换失败 {tmp:?} -> {:?}: {e}", self.path))?;
        Ok(())
    }

    /// 读取快照；文件不存在返回空列表（调用方按「无历史」处理）。
    pub fn load(&self) -> Result<Vec<RoutingEntry>, String> {
        if !self.exists() {
            return Ok(Vec::new());
        }
        let bytes =
            fs::read(&self.path).map_err(|e| format!("读路由快照失败 {:?}: {e}", self.path))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("解析路由快照失败 {:?}: {e}", self.path))
    }
}

/// 把快照条目从文本形式解析成可回填的结构；逐条容错，坏的条目跳过。
pub fn sanitize_entries(raw: Vec<RoutingEntry>, max_per_peer: usize) -> Vec<RoutingEntry> {
    raw.into_iter()
        .filter(|e| !e.peer_id.is_empty())
        .map(|mut e| {
            e.addrs.truncate(max_per_peer);
            e.addrs.retain(|a| !a.is_empty());
            e
        })
        .filter(|e| !e.addrs.is_empty())
        .collect()
}

/// 校验文件路径父目录存在（用于 save 前的快速失败）。
pub fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建路由快照目录失败 {parent:?}: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gsn-routing-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("create tmp");
        d
    }

    #[test]
    fn roundtrip_save_load() {
        let dir = tmpdir("roundtrip");
        let store = RoutingTableStore::new(dir.join("dht-routing.json"));
        let entries = vec![
            RoutingEntry {
                peer_id: "12D3KooWabc".into(),
                addrs: vec!["/ip4/1.2.3.4/tcp/4001".into()],
            },
            RoutingEntry {
                peer_id: "12D3KooWdef".into(),
                addrs: vec![
                    "/ip4/5.6.7.8/tcp/4001".into(),
                    "/ip6/2001:db8::1/tcp/4001".into(),
                ],
            },
        ];
        store.save(&entries).expect("save ok");
        let loaded = store.load().expect("load ok");
        assert_eq!(loaded, entries);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_returns_empty() {
        let dir = tmpdir("missing");
        let store = RoutingTableStore::new(dir.join("dht-routing.json"));
        assert!(!store.exists());
        assert_eq!(store.load().expect("no err"), Vec::<RoutingEntry>::new());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_overwrites_atomically() {
        let dir = tmpdir("overwrite");
        let store = RoutingTableStore::new(dir.join("dht-routing.json"));
        store
            .save(&[RoutingEntry {
                peer_id: "A".into(),
                addrs: vec!["/ip4/1.1.1.1/tcp/1".into()],
            }])
            .expect("first save");
        store
            .save(&[RoutingEntry {
                peer_id: "B".into(),
                addrs: vec!["/ip4/2.2.2.2/tcp/1".into()],
            }])
            .expect("second save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].peer_id, "B");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_file_errors() {
        let dir = tmpdir("corrupt");
        let p = dir.join("dht-routing.json");
        fs::write(&p, b"not-json{{{").expect("write corrupt");
        let store = RoutingTableStore::new(p);
        assert!(store.load().is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_filters_bad_entries() {
        let raw = vec![
            RoutingEntry {
                peer_id: "ok".into(),
                addrs: vec!["/ip4/1.2.3.4/tcp/1".into(), "".into()],
            },
            RoutingEntry {
                peer_id: "".into(),
                addrs: vec!["/ip4/1.2.3.4/tcp/1".into()],
            },
            RoutingEntry {
                peer_id: "no-addr".into(),
                addrs: vec![],
            },
        ];
        let cleaned = sanitize_entries(raw, 4);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0].peer_id, "ok");
        assert_eq!(cleaned[0].addrs.len(), 1);
    }

    #[test]
    fn sanitize_truncates_addrs() {
        let raw = vec![RoutingEntry {
            peer_id: "p".into(),
            addrs: (0..10).map(|i| format!("/ip4/1.2.3.{i}/tcp/1")).collect(),
        }];
        let cleaned = sanitize_entries(raw, 2);
        assert_eq!(cleaned[0].addrs.len(), 2);
    }

    #[test]
    fn ensure_parent_dir_creates() {
        let dir = tmpdir("parent");
        let deep = dir.join("a/b/c/dht-routing.json");
        ensure_parent_dir(&deep).expect("create parents");
        assert!(deep.parent().unwrap().exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
