//! 分布式纠删码分片存储：把本地 Reed-Solomon 编码升级为「可接线到生产网络」的分布式分片层。
//!
//! # 模块组成
//! - [`ShardEnvelope`]：节点间消息信封（StoreShard / NeedShards / ShardReply / BlobSealed）。
//! - [`ShardTransport`]：抽象传输 trait，把网络 IO 与节点逻辑解耦；测试里用
//!   [`InMemoryShardNet`] 证明端到端正确性，后续由编排者接线到真实 libp2p
//!   request-response / KAD 。
//! - [`DistributedShardNode`]：在 [`ErasureCoder`] 之上做确定性分片放置、元数据携带、
//!   缺失分片拉取与重建。
//!
//! # 信任与安全（第一轮 B-6 修复）
//! - **`original_size` / `shard_size` / `data_shards` / `total` 随消息携带**：解码一律使用
//!   消息里携带的 `original_size`，绝不根据收到的分片长度盲目推测；收到 `StoreShard` 时
//!   校验 `payload.len() == shard_size`，不一致直接丢弃（防截断/伪造长度攻击）。
//!
//! # 需外部审计（自研部分，非 vetted RS 库）
//! - 纠删码数学委托给 `reed-solomon-erasure`（vetted 库）；但以下均为本模块**自研**，
//!   需外部安全审计后方可上生产：
//!   - 确定性分片放置（FNV-1a 哈希选节点）；
//!   - 元数据记录 / 校验 / 拼接；
//!   - `NeedShards` 拉取、重试与重建编排。
//!
//! # 边界（诚实标注）
//! - 分片 GC / TTL / 去重 / 持久化落盘 / 副本再平衡均为后续工作；本地分片存储只增不减，
//!   长期运行需上层接入 GC（本文件只给出 per-blob / per-blob-meta 数量上限护栏）。
//! - **真实 libp2p 网络上的分发与重建未验证**：本模块仅在内存传输 [`InMemoryShardNet`]
//!   上端到端验证；真实多节点网络由编排者接线后另行测试。
//! - 不处理拜占庭分片：收到错误分片导致 RS reconstruct 失败时只返回 `Err`，不会静默用
//!   错误数据重建；但本模块不做分片签名 / MAC，真实性校验需上层注入。

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use super::{DecodedShard, ErasureCoder};

/// 默认数据片数（4 数据 + 2 校验）。
pub const DEFAULT_DATA_SHARDS: u8 = 4;
/// 默认校验片数（即最多容忍丢失的分片数）。
pub const DEFAULT_PARITY_SHARDS: u8 = 2;
/// 单 blob 本地分片数硬上限（GF(2^8) 最多 256 片）。
pub const MAX_SHARDS_PER_BLOB: usize = 256;
/// 已记录元数据的 blob 数上限（防膨胀护栏）。
pub const MAX_BLOBS: usize = 1_000_000;
/// 单个 blob 重建时最多发起几轮 NeedShards 拉取，超过即硬失败（防无限重试）。
pub const MAX_RECON_ATTEMPTS: u32 = 5;

// ---------------------------------------------------------------------------
// 1. 分片网络消息
// ---------------------------------------------------------------------------

/// 分片网络消息信封。
///
/// 所有长度类元数据（`total` / `data_shards` / `shard_size` / `original_size`）都随消息
/// 携带，接收方据此校验，**不**盲信外部输入（B-6）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShardEnvelope {
    /// 分发 / 存储单个分片。
    StoreShard {
        blob_id: String,
        index: u8,
        total: u8,
        data_shards: u8,
        shard_size: u32,
        original_size: u64,
        payload: Vec<u8>,
    },
    /// 拉取缺失分片。
    NeedShards {
        blob_id: String,
        requester: String,
        indices: Vec<u8>,
    },
    /// 回应单个分片。
    ShardReply {
        blob_id: String,
        index: u8,
        payload: Vec<u8>,
    },
    /// 广播 blob 元数据（让不持有分片的节点也能发起重建）。
    BlobSealed {
        blob_id: String,
        total: u8,
        data_shards: u8,
        shard_size: u32,
        original_size: u64,
    },
}

/// 一个 blob 的元数据（随 StoreShard / BlobSealed 到达）。
#[derive(Debug, Clone)]
pub struct BlobMeta {
    pub total: u8,
    pub data_shards: u8,
    pub shard_size: u32,
    pub original_size: u64,
}

// ---------------------------------------------------------------------------
// 2. 分片传输 trait + 内存测试实现
// ---------------------------------------------------------------------------

/// 抽象分片传输：把网络 IO 抽象掉，便于测试与真实接线。
pub trait ShardTransport {
    /// 向 `to` 节点发送一个信封。
    fn send(&mut self, to: &str, env: ShardEnvelope);
    /// 取出本节点收到的下一条消息（`from` 为发送者）；无消息返回 `None`。
    fn next(&mut self) -> Option<(String, ShardEnvelope)>;
    /// 已知 peer 列表（包含本节点，用于确定性放置含本地副本）。
    fn peers(&self) -> Vec<String>;
}

struct SharedBus {
    inboxes: HashMap<String, VecDeque<(String, ShardEnvelope)>>,
    peers: Vec<String>,
    /// 确定性丢包率 0.0..=1.0（用于韧性测试；不开启时为 0）。
    loss_rate: f64,
    delivered: u64,
    dropped: u64,
}

/// N 个节点共享的内存消息总线（测试用）。
///
/// 基于 `HashMap<收件人, VecDeque<(发送者, 信封)>>`；每个节点通过 [`InMemoryShardNet::handle`]
/// 拿到一个绑定自身 id 的 [`NetHandle`]。支持确定性丢包注入（`set_loss_rate`）。
#[derive(Clone)]
pub struct InMemoryShardNet {
    shared: Rc<RefCell<SharedBus>>,
}

/// 绑定到单个节点的传输句柄（实现 [`ShardTransport`]）。
pub struct NetHandle {
    shared: Rc<RefCell<SharedBus>>,
    me: String,
    seq: u64,
}

impl InMemoryShardNet {
    /// 创建一条总线；`peers` 为全部参与节点 id（含本节点）。
    pub fn new(peers: Vec<String>) -> Self {
        let mut inboxes = HashMap::new();
        for p in &peers {
            inboxes.entry(p.clone()).or_default();
        }
        Self {
            shared: Rc::new(RefCell::new(SharedBus {
                inboxes,
                peers,
                loss_rate: 0.0,
                delivered: 0,
                dropped: 0,
            })),
        }
    }

    /// 为节点 `me` 取一个传输句柄。
    pub fn handle(&self, me: &str) -> NetHandle {
        NetHandle {
            shared: Rc::clone(&self.shared),
            me: me.to_string(),
            seq: 0,
        }
    }

    /// 设置确定性丢包率（0.0 = 不丢包；1.0 = 全丢）。
    pub fn set_loss_rate(&self, rate: f64) {
        self.shared.borrow_mut().loss_rate = rate.clamp(0.0, 1.0);
    }

    /// 所有收件箱是否均已排空（用于测试判断 quiescence）。
    pub fn is_idle(&self) -> bool {
        self.shared
            .borrow()
            .inboxes
            .values()
            .all(|q| q.is_empty())
    }

    /// (delivered, dropped) 统计。
    pub fn stats(&self) -> (u64, u64) {
        let b = self.shared.borrow();
        (b.delivered, b.dropped)
    }
}

impl ShardTransport for NetHandle {
    fn send(&mut self, to: &str, env: ShardEnvelope) {
        self.seq = self.seq.wrapping_add(1);
        let mut bus = self.shared.borrow_mut();
        if bus.loss_rate > 0.0 {
            // 确定性丢包：对 seq 做 FNV 风格混合，取高位做概率判定（同一进程内可复现）。
            let h = self.seq.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let frac = ((h >> 33) as u32) as f64 / (u32::MAX as f64 + 1.0);
            if frac < bus.loss_rate {
                bus.dropped = bus.dropped.saturating_add(1);
                return;
            }
        }
        bus.inboxes
            .entry(to.to_string())
            .or_default()
            .push_back((self.me.clone(), env));
        bus.delivered = bus.delivered.saturating_add(1);
    }

    fn next(&mut self) -> Option<(String, ShardEnvelope)> {
        let mut bus = self.shared.borrow_mut();
        bus.inboxes
            .get_mut(&self.me)
            .and_then(VecDeque::pop_front)
    }

    fn peers(&self) -> Vec<String> {
        self.shared.borrow().peers.clone()
    }
}

// ---------------------------------------------------------------------------
// 3. 确定性分片放置（自研，需外部审计）
// ---------------------------------------------------------------------------

/// 对 `blob_id + index` 做 FNV-1a 64 位哈希，从 `peers` 中确定性选出负责该分片的节点。
///
/// 所有节点用同一函数 + 同一 peer 列表，得到一致的放置决策；`peers` 必须非空。
pub fn responsible_peer<'a>(peers: &'a [String], blob_id: &str, index: u8) -> &'a str {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for b in blob_id.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3); // FNV prime
    }
    h ^= u64::from(index);
    h = h.wrapping_mul(0x0100_0000_01b3);
    &peers[(h % peers.len() as u64) as usize]
}

// ---------------------------------------------------------------------------
// 4. 分布式分片节点
// ---------------------------------------------------------------------------

/// 一个参与分布式纠删码分片存储的节点。
pub struct DistributedShardNode {
    node_id: String,
    coder: ErasureCoder,
    /// 本地托管的分片：`(blob_id, index) -> payload`。
    local: HashMap<(String, u8), Vec<u8>>,
    /// 已知 blob 元数据（来自 StoreShard / BlobSealed）。
    meta: HashMap<String, BlobMeta>,
    /// 重建过程中收集到的分片（来自 ShardReply），与本地托管分片区分。
    rebuild: HashMap<String, HashMap<u8, Vec<u8>>>,
    /// 已重建完成的 blob 结果缓存。
    rebuilt: HashMap<String, Vec<u8>>,
    /// 每个 blob 已发起的 NeedShards 轮数（用于有界重试）。
    req_attempts: HashMap<String, u32>,
    max_shards_per_blob: usize,
    max_blobs: usize,
}

impl DistributedShardNode {
    /// 以节点 id + 纠删码参数创建（冗余度由 `coder` 决定，默认 4+2）。
    pub fn new(node_id: impl Into<String>, coder: ErasureCoder) -> Self {
        Self {
            node_id: node_id.into(),
            coder,
            local: HashMap::new(),
            meta: HashMap::new(),
            rebuild: HashMap::new(),
            rebuilt: HashMap::new(),
            req_attempts: HashMap::new(),
            max_shards_per_blob: MAX_SHARDS_PER_BLOB,
            max_blobs: MAX_BLOBS,
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// 本地托管分片总数（用于放置均衡性观察）。
    pub fn local_shard_count(&self) -> usize {
        self.local.len()
    }

    /// 是否已知该 blob 元数据。
    pub fn has_meta(&self, blob_id: &str) -> bool {
        self.meta.contains_key(blob_id)
    }

    /// 删除本地托管的某个分片（测试用：模拟分片丢失）。返回是否真的删除了。
    pub fn drop_local_shard(&mut self, blob_id: &str, index: u8) -> bool {
        self.local.remove(&(blob_id.to_string(), index)).is_some()
    }

    /// 编码 `data` 并按确定性放置把每个分片分发给负责节点（含本地），
    /// 随后广播 `BlobSealed` 元数据。
    pub fn ingest_blob(
        &mut self,
        blob_id: &str,
        data: &[u8],
        transport: &mut dyn ShardTransport,
    ) -> Result<(), String> {
        let peers = transport.peers();
        if peers.is_empty() {
            return Err("ingest: 未知 peer 列表为空".to_string());
        }
        let shards = self.coder.encode(data)?;
        if shards.is_empty() {
            return Err("ingest: 编码未产生任何分片".to_string());
        }
        let shard_size_u32: u32 = u32::try_from(shards[0].data.len())
            .map_err(|_| "ingest: shard_size 超过 u32".to_string())?;
        let total = self.coder.total_shards();
        let data_shards = self.coder.data_shards();
        let original_size = u64::try_from(data.len()).map_err(|_| "data 过长".to_string())?;

        for s in &shards {
            let to = responsible_peer(&peers, blob_id, s.index);
            transport.send(
                to,
                ShardEnvelope::StoreShard {
                    blob_id: blob_id.to_string(),
                    index: s.index,
                    total,
                    data_shards,
                    shard_size: shard_size_u32,
                    original_size,
                    payload: s.data.clone(),
                },
            );
        }
        // 广播元数据：让不持有分片的节点也能发起重建。
        for p in &peers {
            transport.send(
                p,
                ShardEnvelope::BlobSealed {
                    blob_id: blob_id.to_string(),
                    total,
                    data_shards,
                    shard_size: shard_size_u32,
                    original_size,
                },
            );
        }
        Ok(())
    }

    /// 处理一条入站信封。
    pub fn handle(&mut self, env: ShardEnvelope, transport: &mut dyn ShardTransport) {
        match env {
            ShardEnvelope::StoreShard {
                blob_id,
                index,
                total,
                data_shards,
                shard_size,
                original_size,
                payload,
            } => {
                // B-6: 不信任外部长度——用携带的 shard_size 校验 payload。
                if index >= total {
                    return;
                }
                if payload.len() != shard_size as usize {
                    return;
                }
                // per-blob 分片数护栏：新分片且已达上限则丢弃（不 panic）。
                let cnt = self
                    .local
                    .keys()
                    .filter(|(b, _)| b == &blob_id)
                    .count();
                let is_new = !self.local.contains_key(&(blob_id.clone(), index));
                if is_new && cnt >= self.max_shards_per_blob {
                    return;
                }
                self.meta.entry(blob_id.clone()).or_insert(BlobMeta {
                    total,
                    data_shards,
                    shard_size,
                    original_size,
                });
                self.local.insert((blob_id, index), payload);
            }

            ShardEnvelope::NeedShards {
                blob_id,
                requester,
                indices,
            } => {
                for idx in indices {
                    if let Some(payload) = self.local.get(&(blob_id.clone(), idx)).cloned() {
                        transport.send(
                            &requester,
                            ShardEnvelope::ShardReply {
                                blob_id: blob_id.clone(),
                                index: idx,
                                payload,
                            },
                        );
                    }
                }
            }

            ShardEnvelope::ShardReply {
                blob_id,
                index,
                payload,
            } => {
                // 若已知元数据，校验长度一致；未知则先收下，后续解码时再过滤。
                if let Some(m) = self.meta.get(&blob_id) {
                    if payload.len() != m.shard_size as usize {
                        return;
                    }
                }
                self.rebuild
                    .entry(blob_id.clone())
                    .or_default()
                    .insert(index, payload);
                let _ = self.maybe_decode(&blob_id);
            }

            ShardEnvelope::BlobSealed {
                blob_id,
                total,
                data_shards,
                shard_size,
                original_size,
            } => {
                if !self.meta.contains_key(&blob_id) && self.meta.len() >= self.max_blobs {
                    return;
                }
                self.meta.insert(
                    blob_id,
                    BlobMeta {
                        total,
                        data_shards,
                        shard_size,
                        original_size,
                    },
                );
            }
        }
    }

    /// 重建指定 blob：先尝试本地 + 已收集分片；不足则向负责节点发 `NeedShards`。
    ///
    /// - 若本地已缓存重建结果，直接返回。
    /// - 若本地 + 已收集分片数 ≥ `data_shards`，立即 RS 解码并缓存。
    /// - 否则发送 `NeedShards` 拉取缺失 index，返回 `Err(pending)`；调用方驱动网络后重试。
    ///   超过 [`MAX_RECON_ATTEMPTS`] 轮仍不足则返回硬 `Err`（不会 panic / 不会返回错误数据）。
    pub fn reconstruct_blob(
        &mut self,
        blob_id: &str,
        transport: &mut dyn ShardTransport,
    ) -> Result<Vec<u8>, String> {
        if let Some(d) = self.rebuilt.get(blob_id) {
            return Ok(d.clone());
        }
        let m = self
            .meta
            .get(blob_id)
            .cloned()
            .ok_or_else(|| format!("reconstruct: 无 blob {blob_id} 的元数据，拒绝盲解码"))?;

        let shards = self.collect_shards(blob_id, &m);
        if (shards.len() as u8) >= m.data_shards {
            let data = self.coder.decode(&shards, m.original_size as usize)?;
            if data.len() as u64 != m.original_size {
                return Err(format!(
                    "reconstruct: {blob_id} 重建长度 {} != 携带 original_size {}",
                    data.len(),
                    m.original_size
                ));
            }
            self.rebuilt.insert(blob_id.to_string(), data.clone());
            return Ok(data);
        }

        // 还不够：计算缺失 index 并发起拉取。
        let present: HashSet<u8> = shards.iter().map(|s| s.index).collect();
        let missing: Vec<u8> = (0..m.total).filter(|i| !present.contains(i)).collect();
        if missing.is_empty() {
            return Err(format!("reconstruct: blob {blob_id} 无任何可用分片"));
        }
        let attempts = self.req_attempts.entry(blob_id.to_string()).or_insert(0);
        *attempts = attempts.saturating_add(1);
        if *attempts > MAX_RECON_ATTEMPTS {
            return Err(format!(
                "reconstruct: blob {blob_id} 在 {MAX_RECON_ATTEMPTS} 轮拉取后仍只有 {}/{} 个分片，硬失败",
                shards.len(),
                m.data_shards
            ));
        }

        let peers = transport.peers();
        for idx in &missing {
            let to = responsible_peer(&peers, blob_id, *idx);
            transport.send(
                to,
                ShardEnvelope::NeedShards {
                    blob_id: blob_id.to_string(),
                    requester: self.node_id.clone(),
                    indices: vec![*idx],
                },
            );
        }
        Err(format!(
            "reconstruct: blob {blob_id} pending（已有 {}/{}，请求 {} 个缺失分片，第 {} 轮）",
            shards.len(),
            m.data_shards,
            missing.len(),
            attempts
        ))
    }

    /// 汇总本地托管 + 重建缓冲里的分片，按 index 去重并校验长度。
    fn collect_shards(&self, blob_id: &str, m: &BlobMeta) -> Vec<DecodedShard> {
        let mut by_index: HashMap<u8, Vec<u8>> = HashMap::new();
        for ((b, idx), payload) in &self.local {
            if b != blob_id || *idx >= m.total {
                continue;
            }
            if payload.len() != m.shard_size as usize {
                continue;
            }
            by_index.insert(*idx, payload.clone());
        }
        if let Some(rb) = self.rebuild.get(blob_id) {
            for (idx, payload) in rb {
                if *idx >= m.total || payload.len() != m.shard_size as usize {
                    continue;
                }
                by_index.insert(*idx, payload.clone());
            }
        }
        by_index
            .into_iter()
            .map(|(index, data)| DecodedShard {
                index,
                data,
                is_parity: index >= m.data_shards,
            })
            .collect()
    }

    /// 收到新分片后尝试自动解码；成功则写入 `rebuilt`。
    fn maybe_decode(&mut self, blob_id: &str) -> Option<Vec<u8>> {
        let m = self.meta.get(blob_id)?.clone();
        let shards = self.collect_shards(blob_id, &m);
        if (shards.len() as u8) < m.data_shards {
            return None;
        }
        let data = self.coder.decode(&shards, m.original_size as usize).ok()?;
        if data.len() as u64 != m.original_size {
            return None;
        }
        self.rebuilt.insert(blob_id.to_string(), data.clone());
        Some(data)
    }
}

// ---------------------------------------------------------------------------
// 端到端测试（全部基于 InMemoryShardNet 多节点）
// 注意：真实 libp2p 网络上的分发与重建未验证（in-memory 传输已验证）。
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 5;

    fn make_cluster() -> (InMemoryShardNet, Vec<NetHandle>, Vec<DistributedShardNode>) {
        let peers: Vec<String> = (0..N).map(|i| format!("n{i}")).collect();
        let net = InMemoryShardNet::new(peers);
        let handles: Vec<NetHandle> = (0..N).map(|i| net.handle(&format!("n{i}"))).collect();
        let nodes: Vec<DistributedShardNode> = (0..N)
            .map(|i| {
                DistributedShardNode::new(format!("n{i}"), ErasureCoder::new(4, 2).unwrap())
            })
            .collect();
        (net, handles, nodes)
    }

    /// 驱动所有节点直到消息总线排空。
    fn pump(nodes: &mut [DistributedShardNode], handles: &mut [NetHandle], max_rounds: usize) {
        for _ in 0..max_rounds {
            let mut handled = 0usize;
            for i in 0..nodes.len() {
                while let Some((_from, env)) = handles[i].next() {
                    nodes[i].handle(env, &mut handles[i]);
                    handled += 1;
                }
            }
            if handled == 0 {
                break;
            }
        }
    }

    /// 从 `start` 节点循环重建，最多尝试 `max_try` 轮；返回重建结果。
    fn run_reconstruct(
        net: &InMemoryShardNet,
        nodes: &mut [DistributedShardNode],
        handles: &mut [NetHandle],
        blob_id: &str,
        start: usize,
        max_try: usize,
    ) -> Result<Vec<u8>, String> {
        for _ in 0..max_try {
            pump(nodes, handles, 100);
            match nodes[start].reconstruct_blob(blob_id, &mut handles[start]) {
                Ok(d) => return Ok(d),
                Err(e) => {
                    // 总线已空仍拿不到数据：要么硬失败，要么 pending 且再无消息可推进，都直接返回。
                    if net.is_idle() {
                        return Err(e);
                    }
                }
            }
        }
        Err("run_reconstruct: 超过最大轮次".to_string())
    }

    #[test]
    fn encode_distribute_then_reconstruct_roundtrip() {
        let (net, mut handles, mut nodes) = make_cluster();
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();

        nodes[0].ingest_blob("blob1", &data, &mut handles[0]).unwrap();
        pump(&mut nodes, &mut handles, 100);

        // 从任一其它节点（n3）重建，逐字节相等。
        let got = run_reconstruct(&net, &mut nodes, &mut handles, "blob1", 3, 20).unwrap();
        assert_eq!(got, data);
    }

    #[test]
    fn rebuild_after_losing_parity_count_shards() {
        let (net, mut handles, mut nodes) = make_cluster();
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();

        nodes[0].ingest_blob("blob1", &data, &mut handles[0]).unwrap();
        pump(&mut nodes, &mut handles, 100);

        // 随机/确定性丢掉恰好 parity_shards(=2) 个分片（含数据片 index 0、1）。
        for idx in [0u8, 1u8] {
            let owner = responsible_peer(
                &(0..N).map(|i| format!("n{i}")).collect::<Vec<_>>(),
                "blob1",
                idx,
            )
            .to_string();
            let ni = (0..N).find(|i| format!("n{i}") == owner).unwrap();
            assert!(nodes[ni].drop_local_shard("blob1", idx));
        }

        // 仍应能重建（剩余 4 片 ≥ data_shards）。
        let got = run_reconstruct(&net, &mut nodes, &mut handles, "blob1", 4, 20).unwrap();
        assert_eq!(got, data);
    }

    #[test]
    fn rebuild_fails_when_below_threshold() {
        let (net, mut handles, mut nodes) = make_cluster();
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();

        nodes[0].ingest_blob("blob1", &data, &mut handles[0]).unwrap();
        pump(&mut nodes, &mut handles, 100);

        // 丢掉超过 parity_shards(=2)：3 个分片（index 0、1、2）。
        for idx in [0u8, 1u8, 2u8] {
            let owner = responsible_peer(
                &(0..N).map(|i| format!("n{i}")).collect::<Vec<_>>(),
                "blob1",
                idx,
            )
            .to_string();
            let ni = (0..N).find(|i| format!("n{i}") == owner).unwrap();
            nodes[ni].drop_local_shard("blob1", idx);
        }

        // 只剩 3 片 < data_shards(=4)：必须返回 Err，不得 panic / 返回错误数据。
        let res = run_reconstruct(&net, &mut nodes, &mut handles, "blob1", 2, 20);
        assert!(res.is_err(), "应当重建失败，实际成功: {res:?}");
    }

    #[test]
    fn deterministic_placement_is_balanced() {
        let (_net, mut handles, mut nodes) = make_cluster();
        let n_blobs = 40usize;
        for b in 0..n_blobs {
            let id = format!("blob-{b}");
            nodes[0]
                .ingest_blob(&id, &[b as u8; 512], &mut handles[0])
                .unwrap();
        }
        pump(&mut nodes, &mut handles, 200);

        let counts: Vec<usize> = nodes.iter().map(|n| n.local_shard_count()).collect();
        let sum: usize = counts.iter().sum();
        // 每个 blob = 6 片（4 数据 + 2 校验）。
        assert_eq!(sum, n_blobs * 6);
        let avg = sum as f64 / N as f64;
        let max = *counts.iter().max().unwrap() as f64;
        // 单节点承载不应超过均值的 1.8 倍（防止全落一个节点）。
        assert!(
            max <= avg * 1.8,
            "分片分布不均: counts={counts:?} avg={avg} max={max}"
        );
    }

    #[test]
    fn original_size_metadata_prevents_truncation() {
        let (net, mut handles, mut nodes) = make_cluster();
        // 非 data_shards 整数倍长度：考验 original_size 元数据是否被正确携带/截断。
        let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();

        nodes[0].ingest_blob("blob1", &data, &mut handles[0]).unwrap();
        pump(&mut nodes, &mut handles, 100);

        let got = run_reconstruct(&net, &mut nodes, &mut handles, "blob1", 1, 20).unwrap();
        assert_eq!(got.len(), data.len());
        assert_eq!(got, data);
    }
}
