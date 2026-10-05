//! CRDT 状态存储（LWW 键值映射）+ 同步消息定义
//!
//! 本模块实现一个**状态型 Last-Writer-Wins Register Map（LWW-Map）**：
//! - 每个键是一个 LWW 寄存器，写入携带 `(ts, origin)`：
//!   先比较物理时间戳 `ts`（unix 毫秒）；相等时以 `origin`（节点 PeerId）做
//!   确定性的全局 tiebreak。
//! - 该裁决满足交换律、结合律、幂等律，因此任意节点以任意顺序收到同一组操作，
//!   最终都收敛到**逐字节相同**的状态（状态型 CRDT）。
//!
//! 收敛/防膨胀保障：
//! - 只保存**物化状态**（`entries`），不保留无限增长的操作日志——重复/旧操作被
//!   LWW 裁决直接丢弃，不会堆积；
//! - `max_keys` 为键数硬上限（安全护栏，默认 100_000）：本地写入新键达上限会
//!   明确返回 Err；网络新键达上限被丢弃并计数（覆盖既有键的更新仍接受）；
//! - 周期性全量快照（`snapshot` / `merge_snapshot`）作为反熵（anti-entropy）机制，
//!   保证迟到/分区节点最终能追平，且快照只被合并、不累积。
//!
//! 边界（诚实标注）：
//! - 本 CRDT 依赖各节点本地墙钟 `ts` 近似单调；恶意节点可虚报 `ts`（自抬为最新）
//!   来覆盖键值——这属于 LWW CRDT 的固有信任假设，需要上层结合版本向量/信誉
//!   约束写入来源；纯 LWW 不防御恶意时钟。
//! - 当前不支持删除（删除需要保留有界墓碑，列入后续版本）。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CRDT 同步主题（GossipSub）。
pub const CRDT_TOPIC: &str = "gsn/crdt";

/// 默认键数上限（安全护栏）。
pub const DEFAULT_MAX_KEYS: usize = 100_000;

/// 一次写操作（op-based 同步报文，也可作为 LWW 裁决输入）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrdtOp {
    pub key: String,
    pub value: String,
    /// 物理时间戳（unix 毫秒）。
    pub ts: u64,
    /// 来源节点标识（PeerId）。
    pub origin: String,
    /// 来源节点单调递增的逻辑计数器（用于版本向量与去重元信息）。
    pub counter: u64,
}

/// 一个键的物化 LWW 状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LwwEntry {
    pub key: String,
    pub value: String,
    pub ts: u64,
    pub origin: String,
}

/// 全量快照（state-based 反熵报文）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrdtSnapshot {
    pub entries: Vec<LwwEntry>,
    /// 快照生成时的时间戳（unix 毫秒）。
    pub ts: u64,
    /// 生成该快照的节点。
    pub origin: String,
    /// 生成节点的快照序号。
    pub seq: u32,
}

/// CRDT 网络消息（统一信封）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CrdtMessage {
    /// 单条写操作（低延迟）。
    Op(CrdtOp),
    /// 全量快照（反熵，周期性广播）。
    Snapshot(CrdtSnapshot),
}

/// LWW 比较：`(ts, origin)` 字典序严格大于。
///
/// 注意：相等返回 false（幂等——同一条操作重复应用不应判定为“更新”）。
fn newer(ts: u64, origin: &str, other_ts: u64, other_origin: &str) -> bool {
    if ts != other_ts {
        ts > other_ts
    } else {
        origin > other_origin
    }
}

/// 存储运行统计（供 metrics 导出）。
#[derive(Debug, Clone, Default)]
pub struct CrdtStats {
    pub inbound_ops: u64,
    pub applied_ops: u64,
    pub dropped_capped: u64,
    pub snapshot_seq: u32,
}

/// CRDT 状态存储。
#[derive(Debug, Clone)]
pub struct CrdtStore {
    origin: String,
    entries: HashMap<String, LwwEntry>,
    /// 版本向量：origin → 该来源已见的最大逻辑计数器。
    version: HashMap<String, u64>,
    max_keys: usize,
    snapshot_seq: u32,
    inbound_ops: u64,
    applied_ops: u64,
    dropped_capped: u64,
}

impl CrdtStore {
    /// 以来源节点标识创建，使用默认上限。
    pub fn new(origin: impl Into<String>) -> Self {
        Self::with_max_keys(origin, DEFAULT_MAX_KEYS)
    }

    /// 以来源节点标识 + 自定义键数上限创建。
    pub fn with_max_keys(origin: impl Into<String>, max_keys: usize) -> Self {
        Self {
            origin: origin.into(),
            entries: HashMap::new(),
            version: HashMap::new(),
            max_keys: max_keys.max(1),
            snapshot_seq: 0,
            inbound_ops: 0,
            applied_ops: 0,
            dropped_capped: 0,
        }
    }

    /// 来源节点标识。
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// 键数上限。
    pub fn max_keys(&self) -> usize {
        self.max_keys
    }

    /// 当前键数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 读取一个键的值。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|e| e.value.as_str())
    }

    /// 读取一个键的完整物化条目。
    pub fn entry(&self, key: &str) -> Option<&LwwEntry> {
        self.entries.get(key)
    }

    /// 全部条目（按 key 排序，便于确定性比较）。
    pub fn all_entries(&self) -> Vec<&LwwEntry> {
        let mut v: Vec<&LwwEntry> = self.entries.values().collect();
        v.sort_by(|a, b| a.key.cmp(&b.key));
        v
    }

    /// 版本向量（origin → 计数器）。
    pub fn version_vector(&self) -> &HashMap<String, u64> {
        &self.version
    }

    /// 某来源已见的逻辑计数器。
    pub fn version_of(&self, origin: &str) -> u64 {
        self.version.get(origin).copied().unwrap_or(0)
    }

    /// 估算当前状态序列化后的字节数（供 metrics/膨胀观察）。
    pub fn size_bytes(&self) -> usize {
        let mut total = 0usize;
        for e in self.entries.values() {
            total += e.key.len() + e.value.len() + 32;
        }
        total
    }

    /// 运行统计快照。
    pub fn stats(&self) -> CrdtStats {
        CrdtStats {
            inbound_ops: self.inbound_ops,
            applied_ops: self.applied_ops,
            dropped_capped: self.dropped_capped,
            snapshot_seq: self.snapshot_seq,
        }
    }

    /// 本地写入：推进本节点计数器、本地应用 LWW，并返回可发布的操作。
    ///
    /// 新键且已达上限时返回 Err（明确告知，而非静默丢弃）。
    pub fn local_put(&mut self, key: &str, value: &str, now_ms: u64) -> Result<CrdtOp, String> {
        if !self.entries.contains_key(key) && self.entries.len() >= self.max_keys {
            return Err(format!(
                "CRDT 已达键数上限 {}，拒绝写入新键（覆盖既有键仍允许）",
                self.max_keys
            ));
        }
        let counter = self
            .version
            .get(&self.origin)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| "版本计数器溢出（异常，拒绝写入）".to_string())?;
        self.version.insert(self.origin.clone(), counter);
        let op = CrdtOp {
            key: key.to_string(),
            value: value.to_string(),
            ts: now_ms,
            origin: self.origin.clone(),
            counter,
        };
        self.insert_op(op.clone());
        Ok(op)
    }

    /// 应用一条网络操作：返回状态是否发生变化。
    ///
    /// 幂等：同一条操作重复应用返回 false，不改变状态。
    pub fn apply_op(&mut self, op: CrdtOp) -> Result<bool, String> {
        self.inbound_ops = self.inbound_ops.saturating_add(1);
        Ok(self.insert_op(op))
    }

    /// 内部插入：更新版本向量 + LWW 裁决。
    fn insert_op(&mut self, op: CrdtOp) -> bool {
        // 版本向量只增不减。
        let seen = self.version.get(&op.origin).copied().unwrap_or(0);
        if op.counter > seen {
            self.version.insert(op.origin.clone(), op.counter);
        }

        let changed = match self.entries.get(&op.key) {
            None => {
                if self.entries.len() >= self.max_keys {
                    // 安全护栏：新键达上限，丢弃并计数（不 panic）。
                    self.dropped_capped = self.dropped_capped.saturating_add(1);
                    false
                } else {
                    true
                }
            }
            Some(existing) => newer(op.ts, &op.origin, existing.ts, &existing.origin),
        };

        if changed {
            self.entries.insert(
                op.key.clone(),
                LwwEntry {
                    key: op.key,
                    value: op.value,
                    ts: op.ts,
                    origin: op.origin,
                },
            );
            self.applied_ops = self.applied_ops.saturating_add(1);
        }
        changed
    }

    /// 生成全量快照（推进快照序号）。
    pub fn snapshot(&mut self, now_ms: u64) -> CrdtSnapshot {
        self.snapshot_seq = self.snapshot_seq.saturating_add(1);
        CrdtSnapshot {
            entries: self.entries.values().cloned().collect(),
            ts: now_ms,
            origin: self.origin.clone(),
            seq: self.snapshot_seq,
        }
    }

    /// 合并对端全量快照（反熵）：返回本次合并中实际改变的键数。
    pub fn merge_snapshot(&mut self, snap: CrdtSnapshot) -> usize {
        let mut changed = 0usize;
        for e in snap.entries {
            let op = CrdtOp {
                key: e.key,
                value: e.value,
                ts: e.ts,
                origin: e.origin,
                counter: 0,
            };
            if self.insert_op(op) {
                changed = changed.saturating_add(1);
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_ms() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    #[test]
    fn local_put_then_get() {
        let mut s = CrdtStore::new("peerA");
        let op = s.local_put("greeting", "hello", 1000).unwrap();
        assert_eq!(op.origin, "peerA");
        assert_eq!(op.counter, 1);
        assert_eq!(s.get("greeting"), Some("hello"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn higher_ts_wins_same_ts_higher_origin_wins() {
        let mut s = CrdtStore::new("peerZ");
        // 初值
        s.local_put("k", "v1", 1000).unwrap();
        assert_eq!(s.get("k"), Some("v1"));
        // 更低 ts 不覆盖
        let old = CrdtOp {
            key: "k".into(),
            value: "stale".into(),
            ts: 900,
            origin: "peerZ".into(),
            counter: 99,
        };
        assert!(!s.apply_op(old).unwrap());
        assert_eq!(s.get("k"), Some("v1"));
        // 更高 ts 覆盖
        let newer_op = CrdtOp {
            key: "k".into(),
            value: "v2".into(),
            ts: 2000,
            origin: "peerA".into(),
            counter: 1,
        };
        assert!(s.apply_op(newer_op).unwrap());
        assert_eq!(s.get("k"), Some("v2"));
        // 相同 ts，origin 字典序更大者胜
        let tie = CrdtOp {
            key: "k".into(),
            value: "v3".into(),
            ts: 2000,
            origin: "peerB".into(),
            counter: 1,
        };
        assert!(s.apply_op(tie).unwrap());
        assert_eq!(s.get("k"), Some("v3"));
        // origin 更小的同 ts 不胜
        let tie_lose = CrdtOp {
            key: "k".into(),
            value: "v4".into(),
            ts: 2000,
            origin: "peerA".into(),
            counter: 1,
        };
        assert!(!s.apply_op(tie_lose).unwrap());
        assert_eq!(s.get("k"), Some("v3"));
    }

    #[test]
    fn apply_is_idempotent() {
        let mut s = CrdtStore::new("peerA");
        let op = CrdtOp {
            key: "k".into(),
            value: "v".into(),
            ts: 1000,
            origin: "peerA".into(),
            counter: 1,
        };
        assert!(s.apply_op(op.clone()).unwrap());
        assert!(!s.apply_op(op).unwrap());
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn concurrent_writes_converge_regardless_of_order() {
        // 构造 3 个对同一键的不同操作，两个独立 store 以相反顺序应用，结果必须一致。
        let ops = vec![
            CrdtOp {
                key: "k".into(),
                value: "a".into(),
                ts: 1000,
                origin: "peerA".into(),
                counter: 1,
            },
            CrdtOp {
                key: "k".into(),
                value: "b".into(),
                ts: 3000,
                origin: "peerB".into(),
                counter: 1,
            },
            CrdtOp {
                key: "k".into(),
                value: "c".into(),
                ts: 2000,
                origin: "peerC".into(),
                counter: 1,
            },
        ];
        let mut s1 = CrdtStore::new("s1");
        let mut s2 = CrdtStore::new("s2");
        for o in &ops {
            s1.apply_op(o.clone()).unwrap();
        }
        for o in ops.iter().rev() {
            s2.apply_op(o.clone()).unwrap();
        }
        assert_eq!(s1.get("k"), s2.get("k"));
        // 最高 ts=3000 的 "b" 胜出
        assert_eq!(s1.get("k"), Some("b"));
    }

    #[test]
    fn snapshot_roundtrip_catches_up_new_store() {
        let mut src = CrdtStore::new("peerA");
        src.local_put("k1", "v1", 1000).unwrap();
        src.local_put("k2", "v2", 2000).unwrap();
        let snap = src.snapshot(3000);

        // 新节点（空）只收快照即可追平
        let mut dst = CrdtStore::new("peerB");
        let changed = dst.merge_snapshot(snap);
        assert_eq!(changed, 2);
        assert_eq!(dst.get("k1"), Some("v1"));
        assert_eq!(dst.get("k2"), Some("v2"));

        // 再合并一次（幂等）：无变化
        let snap2 = src.snapshot(4000);
        assert_eq!(dst.merge_snapshot(snap2), 0);
    }

    #[test]
    fn cap_rejects_local_new_key_and_drops_inbound_but_allows_overwrite() {
        let mut s = CrdtStore::with_max_keys("peerA", 2);
        s.local_put("k1", "v1", 1000).unwrap();
        s.local_put("k2", "v2", 1000).unwrap();
        // 本地新键达上限：Err
        assert!(s.local_put("k3", "v3", 1000).is_err());
        // 网络新键达上限：丢弃（false），统计 +1
        let inbound_new = CrdtOp {
            key: "k3".into(),
            value: "v3".into(),
            ts: 1000,
            origin: "peerX".into(),
            counter: 1,
        };
        assert!(!s.apply_op(inbound_new).unwrap());
        assert_eq!(s.stats().dropped_capped, 1);
        // 覆盖既有键（更高 ts）：接受
        let overwrite = CrdtOp {
            key: "k1".into(),
            value: "v1b".into(),
            ts: 5000,
            origin: "peerX".into(),
            counter: 2,
        };
        assert!(s.apply_op(overwrite).unwrap());
        assert_eq!(s.get("k1"), Some("v1b"));
    }

    #[test]
    fn partition_then_snapshot_exchange_converges() {
        // 模拟分区：A 侧与 B 侧各自演进、互发同键冲突值；恢复后交换快照，
        // 所有节点必须收敛到同一个 LWW 裁决值。
        let mut a = CrdtStore::new("peerA");
        let mut b = CrdtStore::new("peerB");
        let mut c = CrdtStore::new("peerC");

        // 分区期间：A 写 k=x（ts=1000），B 写 k=y（ts=2000），各自本地可见
        a.local_put("k", "x", 1000).unwrap();
        b.local_put("k", "y", 2000).unwrap();

        // 恢复：三方互发快照（A、B、C）。先把 C 与 A/B 都同步。
        let sa = a.snapshot(3000);
        let sb = b.snapshot(3000);
        // A 收 B
        a.merge_snapshot(sb.clone());
        // B 收 A
        b.merge_snapshot(sa);
        // C 收 A 与 B
        c.merge_snapshot(sb.clone());
        c.merge_snapshot(b.snapshot(3001));
        a.merge_snapshot(c.snapshot(3002));

        // 三方逐键一致，且为 ts 更高的 "y"
        assert_eq!(a.get("k"), Some("y"));
        assert_eq!(b.get("k"), Some("y"));
        assert_eq!(c.get("k"), Some("y"));
    }

    #[test]
    fn version_vector_tracks_counters() {
        let mut s = CrdtStore::new("peerA");
        s.local_put("k1", "v", 1000).unwrap();
        s.local_put("k2", "v", 1000).unwrap();
        assert_eq!(s.version_of("peerA"), 2);
        // 网络来源计数
        let op = CrdtOp {
            key: "k3".into(),
            value: "v".into(),
            ts: 1000,
            origin: "peerB".into(),
            counter: 7,
        };
        s.apply_op(op).unwrap();
        assert_eq!(s.version_of("peerB"), 7);
        // 更旧的计数器不回退
        let old = CrdtOp {
            key: "k4".into(),
            value: "v".into(),
            ts: 1000,
            origin: "peerB".into(),
            counter: 3,
        };
        s.apply_op(old).unwrap();
        assert_eq!(s.version_of("peerB"), 7);
    }

    #[test]
    fn message_envelope_roundtrip() {
        let op = CrdtOp {
            key: "k".into(),
            value: "v".into(),
            ts: now_ms(),
            origin: "peerA".into(),
            counter: 1,
        };
        let msg = CrdtMessage::Op(op.clone());
        let j = serde_json::to_string(&msg).unwrap();
        let back: CrdtMessage = serde_json::from_str(&j).unwrap();
        assert_eq!(back, msg);

        let snap = CrdtSnapshot {
            entries: vec![LwwEntry {
                key: "k".into(),
                value: "v".into(),
                ts: 1,
                origin: "peerA".into(),
            }],
            ts: 1,
            origin: "peerA".into(),
            seq: 1,
        };
        let m2 = CrdtMessage::Snapshot(snap);
        let j2 = serde_json::to_string(&m2).unwrap();
        let b2: CrdtMessage = serde_json::from_str(&j2).unwrap();
        assert_eq!(b2, m2);
    }
}
