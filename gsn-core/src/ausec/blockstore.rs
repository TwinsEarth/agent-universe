//! 块存储 `BlockStore`（v3.7.2）：本地仅存元数据，按需取块、命中复用、只读共享。
//!
//! v3.7.1 的 [`super::image::ChunkManifest`] 只描述「块长什么样、内容地址是什么」；
//! 本模块解决「块在哪、什么时候才真正拿进本地」：
//!
//! - **本地仅存元数据**：[`BlockStore::new`] 只持有清单与源，**不触发任何取块**；
//!   一个几十 GB 的镜像在首次访问前不下载任何块字节。
//! - **缺块即取**：[`BlockStore::get_chunk`] 在本地缓存未命中时，按配置顺序向
//!   [`BlockSource`] 取块；取回的字节**必须通过清单的 `verify_chunk`（sha256 内容
//!   地址校验）才会进入缓存**，坏源给的字节永远不会污染缓存。
//! - **命中复用**：同一块第二次访问直接命中缓存，不再向源请求。
//! - **只读共享**：缓存按块的 **sha256 内容地址**去重，返回 `Arc<[u8]>`；多个
//!   `BlockStore`（哪怕是不同镜像）共享同一个 [`SharedChunkCache`] 时，内容相同的
//!   只读块在本机只占一份（为 v3.7.4 的共享额度/超卖记账铺垫）。
//!
//! 诚实边界：
//! - [`LocalDirBlockSource`] 是**真实**的本地内容寻址块目录（`std::fs` I/O）；
//! - [`UdosRemoteBlockSource`] 是**具名远端**（UDOS 分布式文件系统），v3.7.2 **不实现
//!   跨网络传输**，其 `fetch` 一律 [`BlockFetchError::RemoteNotImplemented`] 具名拒绝，
//!   绝不伪造拉取；真实远端 + 每块 Ed25519 发布者锚定在 v3.7.3 落地。
//!
//! 见 `docs/ausec/AUSEC-DESIGN.md` §3.1（3.7.2）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::image::{ChunkEntry, ChunkManifest, ManifestError};

/// 块源类型（决定是否跨网络、是否可信的记账与展示）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockSourceKind {
    /// 本机内容寻址种子目录（真实 `std::fs`）。
    LocalSeed,
    /// UDOS 远端分布式文件系统（v3.7.2 具名，不实现传输）。
    RemoteUdos { endpoint: String },
}

impl BlockSourceKind {
    pub fn is_remote(&self) -> bool {
        matches!(self, BlockSourceKind::RemoteUdos { .. })
    }
}

/// 单个块源取块失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockFetchError {
    /// 该源没有这一块（正常的「缺块」，调用方可降级到下一个源）。
    NotPresent,
    /// 读取该源时发生本地 I/O 错误（带原因）。
    Io(String),
    /// 远端传输在本版本尚未实现（具名拒绝，不伪造跨网拉取）。
    RemoteNotImplemented(String),
    /// 块缺少/带有无效的发布者证明（签名错、槽位/镜像/摘要不匹配）——v3.7.3 fail-closed。
    Attestation(String),
    /// 块的发布者不在本地信任集合中——v3.7.3 fail-closed。
    UntrustedPublisher,
}

/// 块源契约：给定清单条目，返回该块的原始字节（**不负责校验**，校验由 BlockStore 统一做）。
pub trait BlockSource: Send + Sync {
    /// 源的稳定名字（用于错误归因与统计，禁止伪造来源）。
    fn name(&self) -> &str;
    /// 源类型。
    fn kind(&self) -> BlockSourceKind;
    /// 取块。返回的字节会被 BlockStore 用清单 sha256 复核，校验不过即丢弃。
    fn fetch(&self, image: &str, entry: &ChunkEntry) -> Result<Vec<u8>, BlockFetchError>;
}

/// 本机内容寻址块目录：块以其 sha256 内容地址为文件名存放。
///
/// 这是**真实**源：`fetch` 直接 `std::fs::read(dir/<sha256>)`。文件不存在即
/// [`BlockFetchError::NotPresent`]（该块尚未做种），由 `BlockStore` 降级到下一源。
pub struct LocalDirBlockSource {
    name: String,
    dir: PathBuf,
}

impl LocalDirBlockSource {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let name = format!("local-dir:{}", dir.display());
        Self { name, dir }
    }

    /// 块在目录中的内容寻址路径（测试/做种时可用）。
    pub fn chunk_path(&self, digest: &str) -> PathBuf {
        self.dir.join(digest)
    }
}

impl BlockSource for LocalDirBlockSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> BlockSourceKind {
        BlockSourceKind::LocalSeed
    }
    fn fetch(&self, _image: &str, entry: &ChunkEntry) -> Result<Vec<u8>, BlockFetchError> {
        let path = self.chunk_path(&entry.sha256);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(BlockFetchError::NotPresent),
            Err(e) => Err(BlockFetchError::Io(format!("{}: {e}", path.display()))),
        }
    }
}

/// UDOS 远端块源（具名）。
///
/// v3.7.2 **只登记 endpoint，不做任何网络 I/O**：所有 `fetch` 一律具名返回
/// [`BlockFetchError::RemoteNotImplemented`]。这是刻意的诚实边界——本版的目标是把
/// 「按需取块、校验、缓存、共享」的本地控制面做成可验证逻辑，而不是假装已经能跨
/// P2P 拉块。真实远端 + 发布者签名锚定在 v3.7.3。
pub struct UdosRemoteBlockSource {
    name: String,
    endpoint: String,
}

impl UdosRemoteBlockSource {
    pub fn new(endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        let name = format!("udos-remote:{endpoint}");
        Self { name, endpoint }
    }
}

impl BlockSource for UdosRemoteBlockSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> BlockSourceKind {
        BlockSourceKind::RemoteUdos {
            endpoint: self.endpoint.clone(),
        }
    }
    fn fetch(&self, _image: &str, _entry: &ChunkEntry) -> Result<Vec<u8>, BlockFetchError> {
        Err(BlockFetchError::RemoteNotImplemented(self.endpoint.clone()))
    }
}

/// `BlockStore` 对外的错误（全具名、无 panic）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockStoreError {
    #[error("块清单未通过完整性校验：{0}")]
    BadManifest(String),
    #[error("块序号 {index} 越界（清单共 {count} 块）")]
    ChunkOutOfRange { index: usize, count: usize },
    #[error("块 {index} 无法从任何已配置源取得（尝试过：{tried}）")]
    Unavailable { index: usize, tried: String },
    #[error("本地块源 I/O 失败：{0}")]
    SourceIo(String),
    #[error("UDOS 远端块传输 v3.7.2 尚未实现（具名拒绝，不伪造跨网传输）：endpoint={0}")]
    RemoteNotImplemented(String),
    #[error("块 {index} 的发布者证明无效（fail-closed）：{reason}")]
    BadAttestation { index: usize, reason: String },
    #[error("块 {index} 的发布者不在信任集合中（fail-closed）")]
    UntrustedPublisher { index: usize },
    #[error(transparent)]
    Manifest(#[from] ManifestError),
}

/// 只读共享块缓存：按 sha256 内容地址去重，跨 `BlockStore` 共享。
///
/// 内部 `Mutex` 仅在极短的查表/插入区间持有；锁中毒时取回内部数据继续工作
/// （与仓库其它持久化锁一致，不 panic）。
#[derive(Default)]
pub struct SharedChunkCache {
    inner: Mutex<HashMap<String, Arc<Vec<u8>>>>,
}

impl SharedChunkCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn lookup(&self, digest: &str) -> Option<Arc<Vec<u8>>> {
        let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        guard.get(digest).cloned()
    }

    /// 插入一块已通过校验的字节；若相同内容已存在则**复用既有 Arc**（去重，不复制）。
    fn intern(&self, digest: String, bytes: Vec<u8>) -> Arc<Vec<u8>> {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = guard.get(&digest) {
            return existing.clone();
        }
        let arc = Arc::new(bytes);
        guard.insert(digest, arc.clone());
        arc
    }

    /// 当前去重后的不同块数。
    pub fn distinct_chunks(&self) -> usize {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    /// 当前去重后的共享缓存字节占用（同内容只计一次）。
    pub fn shared_bytes(&self) -> u64 {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .map(|b| b.len() as u64)
            .sum()
    }
}

/// 取块统计（用于证明「懒加载 / 命中复用」与后续 3.7.6 回收统计）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockStats {
    /// 缓存命中次数（未访问源）。
    pub hits: u64,
    /// 缓存未命中次数。
    pub misses: u64,
    /// 真正从源成功取回并通过校验的块数（每个不同内容块最多贡献一次）。
    pub fetches: u64,
    /// 被源返回但未通过 sha256 校验、因而被丢弃的坏块次数。
    pub rejected_corrupt: u64,
}

/// 按需块存储：清单 + 有序源 + 共享只读缓存。
pub struct BlockStore {
    manifest: ChunkManifest,
    cache: Arc<SharedChunkCache>,
    sources: Vec<Arc<dyn BlockSource>>,
    stats: Mutex<BlockStats>,
}

impl BlockStore {
    /// 仅用清单与源构造：**不触发任何取块**（本地只存元数据）。
    pub fn new(
        manifest: ChunkManifest,
        sources: Vec<Arc<dyn BlockSource>>,
    ) -> Result<Self, BlockStoreError> {
        Self::with_shared_cache(manifest, sources, SharedChunkCache::new())
    }

    /// 用一个外部共享缓存构造（多个 store / 多个镜像共享只读块）。
    pub fn with_shared_cache(
        manifest: ChunkManifest,
        sources: Vec<Arc<dyn BlockSource>>,
        cache: Arc<SharedChunkCache>,
    ) -> Result<Self, BlockStoreError> {
        manifest
            .validate()
            .map_err(|e| BlockStoreError::BadManifest(e.to_string()))?;
        Ok(Self {
            manifest,
            cache,
            sources,
            stats: Mutex::new(BlockStats::default()),
        })
    }

    pub fn image(&self) -> &str {
        &self.manifest.image
    }
    pub fn chunk_count(&self) -> usize {
        self.manifest.chunks.len()
    }
    pub fn total_length(&self) -> u64 {
        self.manifest.total_length
    }
    pub fn stats_snapshot(&self) -> BlockStats {
        self.stats.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// 取一个块：先查共享缓存（命中复用），未命中才按源顺序按需拉取，
    /// 取回字节通过清单 sha256 校验后才入缓存。
    pub fn get_chunk(&self, index: usize) -> Result<Arc<Vec<u8>>, BlockStoreError> {
        let entry = self
            .manifest
            .chunks
            .get(index)
            .ok_or(BlockStoreError::ChunkOutOfRange {
                index,
                count: self.manifest.chunks.len(),
            })?
            .clone();

        // 1) 共享缓存命中（按内容地址去重）。
        if let Some(cached) = self.cache.lookup(&entry.sha256) {
            self.bump(|s| s.hits += 1);
            return Ok(cached);
        }
        self.bump(|s| s.misses += 1);

        // 2) 缺块即取：按源顺序尝试；缺块(NotPresent)降级，I/O/远端具名报错但仍尝试下一个。
        let mut attempts: Vec<String> = Vec::new();
        let mut hard_error: Option<BlockStoreError> = None;
        for source in &self.sources {
            match source.fetch(&self.manifest.image, &entry) {
                Ok(bytes) => {
                    // 3) 唯一信任闸：用清单内容地址复核，坏字节绝不入缓存。
                    match self.manifest.verify_chunk(index, &bytes) {
                        Ok(()) => {
                            let arc = self.cache.intern(entry.sha256.clone(), bytes);
                            self.bump(|s| s.fetches += 1);
                            return Ok(arc);
                        }
                        Err(_) => {
                            self.bump(|s| s.rejected_corrupt += 1);
                            attempts.push(format!("{}=坏块被拒", source.name()));
                        }
                    }
                }
                Err(BlockFetchError::NotPresent) => {
                    attempts.push(format!("{}=无此块", source.name()));
                }
                Err(BlockFetchError::Io(reason)) => {
                    attempts.push(format!("{}=IO错误", source.name()));
                    hard_error = Some(BlockStoreError::SourceIo(reason));
                }
                Err(BlockFetchError::RemoteNotImplemented(ep)) => {
                    attempts.push(format!("{}=远端未实现", source.name()));
                    hard_error = Some(BlockStoreError::RemoteNotImplemented(ep));
                }
                // v3.7.3：证明无效 / 发布者不受信是**安全**硬错误，立即 fail-closed，
                // 不再降级到其它源（避免用另一个源的“成功”掩盖一次被篡改的投递）。
                Err(BlockFetchError::Attestation(reason)) => {
                    attempts.push(format!("{}=证明无效", source.name()));
                    return Err(BlockStoreError::BadAttestation { index, reason });
                }
                Err(BlockFetchError::UntrustedPublisher) => {
                    attempts.push(format!("{}=发布者不受信", source.name()));
                    return Err(BlockStoreError::UntrustedPublisher { index });
                }
            }
        }

        // 全部源都没给出有效块：若遇到过硬错误则优先透出，否则报「所有源都缺块」。
        if let Some(e) = hard_error {
            // 仅当没有任何源可供降级时才透出硬错误；这里尝试已穷尽。
            return Err(e);
        }
        Err(BlockStoreError::Unavailable {
            index,
            tried: attempts.join("; "),
        })
    }

    /// 按需逐块取回并拼出完整镜像（只在调用时才真正取全所有块）。
    pub fn assemble_image(&self) -> Result<Vec<u8>, BlockStoreError> {
        let mut out = Vec::with_capacity(self.manifest.total_length as usize);
        for i in 0..self.manifest.chunks.len() {
            out.extend_from_slice(&self.get_chunk(i)?);
        }
        Ok(out)
    }

    fn bump(&self, f: impl FnOnce(&mut BlockStats)) {
        let mut g = self.stats.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ausec::image::build_manifest;
    use std::path::Path;

    fn data(full: usize, cs: u64, tail: usize) -> Vec<u8> {
        let n = full as u64 * cs + tail as u64;
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    /// 唯一真实临时目录（沿用仓库 std::env::temp_dir 惯例，测试结束清理）。
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "au-bl-372-{tag}-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                // 计数器无法获得时由时间戳保证唯一；额外加一个静态原子更稳：
                COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ));
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    /// 把若干块「做种」到本地目录（文件名=内容地址）。
    fn seed(dir: &Path, manifest: &ChunkManifest, bytes: &[u8], indices: &[usize]) {
        for &i in indices {
            let e = &manifest.chunks[i];
            let start = e.offset as usize;
            let end = start + e.length as usize;
            std::fs::write(dir.join(&e.sha256), &bytes[start..end]).unwrap();
        }
    }

    #[test]
    fn new_store_does_not_fetch_anything() {
        let data = data(4, 16, 9);
        let m = build_manifest("img/lazy", &data, 16).unwrap();
        // 给一个空目录做源：构造期不得去读它。
        let dir = TempDir::new("nofetch");
        let src: Arc<dyn BlockSource> = Arc::new(LocalDirBlockSource::new(&dir.0));
        let store = BlockStore::new(m, vec![src]).unwrap();
        assert_eq!(store.chunk_count(), 5);
        // 关键：一次块都没取。
        assert_eq!(store.stats_snapshot(), BlockStats::default());
    }

    #[test]
    fn miss_fetches_once_then_cache_hit_reuses() {
        let data = data(3, 16, 5);
        let m = build_manifest("img/cache", &data, 16).unwrap();
        let dir = TempDir::new("hit");
        seed(&dir.0, &m, &data, &[0]); // 只有块 0 做种。
        let src: Arc<dyn BlockSource> = Arc::new(LocalDirBlockSource::new(&dir.0));
        let store = BlockStore::new(m, vec![src]).unwrap();

        let a = store.get_chunk(0).unwrap();
        let b = store.get_chunk(0).unwrap();
        let c = store.get_chunk(0).unwrap();
        // 内容正确。
        assert_eq!(&*a, &data[0..16]);
        // 三次访问只从源取一次，后两次命中同一 Arc（命中复用 + 共享内存）。
        assert!(Arc::ptr_eq(&a, &b));
        assert!(Arc::ptr_eq(&b, &c));
        let s = store.stats_snapshot();
        assert_eq!(s.fetches, 1);
        assert_eq!(s.hits, 2);
        assert_eq!(s.misses, 1);
    }

    #[test]
    fn on_demand_only_needs_seeded_accessed_chunks() {
        // 镜像 10 块，但本地只做种 2 块（按需加载：访问 4.2–13.3% 即可工作）。
        let data = data(9, 16, 13);
        let m = build_manifest("img/sparse", &data, 16).unwrap();
        let dir = TempDir::new("sparse");
        seed(&dir.0, &m, &data, &[2, 7]);
        let src: Arc<dyn BlockSource> = Arc::new(LocalDirBlockSource::new(&dir.0));
        let store = BlockStore::new(m.clone(), vec![src]).unwrap();

        assert!(store.get_chunk(2).is_ok());
        assert!(store.get_chunk(7).is_ok());
        // 未做种的块明确不可用（不伪造、不返回空）。
        let err = store.get_chunk(0).unwrap_err();
        assert!(matches!(err, BlockStoreError::Unavailable { index: 0, .. }));
        assert_eq!(store.stats_snapshot().fetches, 2);
    }

    #[test]
    fn corrupt_source_bytes_are_rejected_and_never_cached() {
        // 源总返回坏字节：必须被 verify_chunk 拒绝、不入缓存。
        struct BadSource;
        impl BlockSource for BadSource {
            fn name(&self) -> &str {
                "bad"
            }
            fn kind(&self) -> BlockSourceKind {
                BlockSourceKind::LocalSeed
            }
            fn fetch(&self, _i: &str, e: &ChunkEntry) -> Result<Vec<u8>, BlockFetchError> {
                let mut v = vec![0u8; e.length as usize];
                v[0] = 0xFF; // 与真实内容不符
                Ok(v)
            }
        }
        let data = data(2, 16, 4);
        let m = build_manifest("img/bad", &data, 16).unwrap();
        let store = BlockStore::new(m, vec![Arc::new(BadSource)]).unwrap();
        assert!(matches!(
            store.get_chunk(0),
            Err(BlockStoreError::Unavailable { index: 0, .. })
        ));
        let s = store.stats_snapshot();
        assert_eq!(s.rejected_corrupt, 1);
        assert_eq!(s.fetches, 0); // 坏块不计成功 fetch
    }

    #[test]
    fn falls_back_to_next_good_source_after_corrupt_one() {
        struct BadThenGone;
        impl BlockSource for BadThenGone {
            fn name(&self) -> &str {
                "bad-first"
            }
            fn kind(&self) -> BlockSourceKind {
                BlockSourceKind::LocalSeed
            }
            fn fetch(&self, _i: &str, e: &ChunkEntry) -> Result<Vec<u8>, BlockFetchError> {
                Ok(vec![0xAB; e.length as usize]) // 必然过不了 sha256
            }
        }
        let data = data(1, 16, 3);
        let m = build_manifest("img/fallback", &data, 16).unwrap();
        let dir = TempDir::new("fallback");
        seed(&dir.0, &m, &data, &[0]);
        let bad: Arc<dyn BlockSource> = Arc::new(BadThenGone);
        let good: Arc<dyn BlockSource> = Arc::new(LocalDirBlockSource::new(&dir.0));
        let store = BlockStore::new(m, vec![bad, good]).unwrap();

        let got = store.get_chunk(0).unwrap();
        assert_eq!(&*got, &data[0..16]);
        let s = store.stats_snapshot();
        assert_eq!(s.rejected_corrupt, 1); // 第一个源的坏块被记一次
        assert_eq!(s.fetches, 1); // 好源成功
    }

    #[test]
    fn shared_cache_dedupes_across_stores_and_images() {
        // 两个不同镜像共享一个内容块（如相同基础层只读页）。
        let cs = 16u64;
        let shared_tail: Vec<u8> = (0..7u8).collect(); // 相同内容
        let data_a = {
            let mut v: Vec<u8> = (0..32u8).collect();
            v.extend_from_slice(&shared_tail);
            v
        };
        let data_b = {
            let mut v: Vec<u8> = (100..132u8).collect();
            v.extend_from_slice(&shared_tail);
            v
        };
        let ma = build_manifest("img/a", &data_a, cs).unwrap();
        let mb = build_manifest("img/b", &data_b, cs).unwrap();
        // 两个镜像的末块内容相同 ⇒ sha256 相同。
        assert_eq!(ma.chunks[2].sha256, mb.chunks[2].sha256);

        // 每个镜像各有自己的种子目录，但只放共同块；用共享计数源统计调用次数。
        struct CountingSource {
            inner: LocalDirBlockSource,
            calls: Mutex<u64>,
        }
        impl BlockSource for CountingSource {
            fn name(&self) -> &str {
                self.inner.name()
            }
            fn kind(&self) -> BlockSourceKind {
                self.inner.kind()
            }
            fn fetch(&self, i: &str, e: &ChunkEntry) -> Result<Vec<u8>, BlockFetchError> {
                *self.calls.lock().unwrap() += 1;
                self.inner.fetch(i, e)
            }
        }
        let dir_a = TempDir::new("shared-a");
        seed(&dir_a.0, &ma, &data_a, &[2]);
        let dir_b = TempDir::new("shared-b");
        seed(&dir_b.0, &mb, &data_b, &[2]);
        let sa = Arc::new(CountingSource {
            inner: LocalDirBlockSource::new(&dir_a.0),
            calls: Mutex::new(0),
        });
        let sb = Arc::new(CountingSource {
            inner: LocalDirBlockSource::new(&dir_b.0),
            calls: Mutex::new(0),
        });

        let cache = SharedChunkCache::new();
        let store_a = BlockStore::with_shared_cache(ma, vec![sa.clone()], cache.clone()).unwrap();
        let store_b = BlockStore::with_shared_cache(mb, vec![sb.clone()], cache.clone()).unwrap();

        let x = store_a.get_chunk(2).unwrap();
        let y = store_b.get_chunk(2).unwrap();
        // 内容相同 ⇒ 跨镜像命中同一份只读内存（Arc 相同），只从源实际取了一次。
        assert!(Arc::ptr_eq(&x, &y));
        assert_eq!(*sa.calls.lock().unwrap(), 1);
        assert_eq!(*sb.calls.lock().unwrap(), 0); // b 直接命中共享缓存，未问源
        assert_eq!(cache.distinct_chunks(), 1);
        assert_eq!(cache.shared_bytes(), 7);
    }

    #[test]
    fn udos_remote_is_named_unavailable_not_fake_transport() {
        let data = data(1, 16, 1);
        let m = build_manifest("img/remote", &data, 16).unwrap();
        let remote: Arc<dyn BlockSource> =
            Arc::new(UdosRemoteBlockSource::new("udos://example.invalid"));
        assert!(remote.kind().is_remote());
        let store = BlockStore::new(m, vec![remote]).unwrap();
        match store.get_chunk(0) {
            Err(BlockStoreError::RemoteNotImplemented(ep)) => assert!(ep.contains("example")),
            other => panic!("应具名拒绝远端，实际: {other:?}"),
        }
    }

    #[test]
    fn assemble_image_roundtrip_and_each_chunk_fetched_once() {
        let data = data(3, 16, 8);
        let m = build_manifest("img/full", &data, 16).unwrap();
        let dir = TempDir::new("assemble");
        seed(&dir.0, &m, &data, &(0..4).collect::<Vec<_>>());
        let src: Arc<dyn BlockSource> = Arc::new(LocalDirBlockSource::new(&dir.0));
        let store = BlockStore::new(m, vec![src]).unwrap();

        let back = store.assemble_image().unwrap();
        assert_eq!(back, data);
        // 再拼一次：全部命中，零额外 fetch。
        let back2 = store.assemble_image().unwrap();
        assert_eq!(back2, data);
        let s = store.stats_snapshot();
        assert_eq!(s.fetches, 4);
        assert_eq!(s.hits, 4);
    }

    #[test]
    fn out_of_range_and_bad_manifest() {
        let data = data(1, 16, 1);
        let m = build_manifest("img/range", &data, 16).unwrap();
        let store = BlockStore::new(m, vec![]).unwrap();
        assert!(matches!(
            store.get_chunk(9),
            Err(BlockStoreError::ChunkOutOfRange { index: 9, count: 2 })
        ));

        let mut bad = build_manifest("img/bad2", &data, 16).unwrap();
        bad.chunks[0].offset = 5; // 空洞
        assert!(matches!(
            BlockStore::new(bad, vec![]),
            Err(BlockStoreError::BadManifest(_))
        ));
    }
}
