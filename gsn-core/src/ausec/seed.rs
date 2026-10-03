//! P2P 种子健康度 + 每块 Ed25519 发布者锚定（v3.7.3）。
//!
//! 承接 v3.7.1（块清单）与 v3.7.2（BlockStore 按需取块）。这一版解决两件
//!「跨网络做种前必须成立」的事，且**不伪造跨网络传输**：
//!
//! 1. **种子健康度的确定性计算**：只根据「副本数」这一可观测事实计算健康度，
//!    不假装已经连到任何对端。约定每增加一个独立副本，健康度 +100%：
//!    - 0 副本 → 0%（拿不到）；
//!    - 1 副本 → 100%（本机/单点可取得）；
//!    - 10 副本 → **1000%**（对应规划里「镜像作为种子保持 1000% 以上健康度，
//!      跨机跨网络体验基本相同」的口径）。
//!
//!    > 外部实测数字（按需加载 1.71×、磁盘写降 57%、访问面 4.2–13.3%、1000%
//!    > 种子健康度等）来自 aiwiki.ai / byteiota.com 对 DSEC 的报道，**非本仓
//!    > 复测**；本仓只做可复算的确定性模型与单测，见 `docs/ausec/AUSEC-DESIGN.md`。
//!
//! 2. **每块 Ed25519 发布者锚定**：发布者对每个块的「镜像名 + 槽位(index) +
//!    偏移 + 长度 + sha256」做规范化签名。校验时这些字段任一被篡改（包括把 A
//!    块的有效证明搬到 B 槽位）都会失败；发布者公钥还必须在本地
//!    [`TrustedPublishers`] 集合里，否则即使签名自洽也 fail-closed。
//!
//! [`AttestedSeedSource`] 把这层校验做成**真实生产调用点**（非测试专用）：它
//! 包在任意 [`BlockSource`] 外，取块时强制读发布者签名、校验锚定与信任集合；
//! 远程 UDOS 仍由内层源具名拒绝（v3.7.2 诚实边界保留）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ed25519_dalek::{Signer, SigningKey};

use crate::identity::{Ed25519Signer, Keypair};

use super::blockstore::{BlockFetchError, BlockSource};
use super::image::{ChunkEntry, ChunkManifest};

/// 每块签名的域分隔与版本前缀（规范化消息的一部分，防止跨协议复用签名）。
pub const ATTESTATION_DOMAIN: &[u8] = b"AUSEC-CHUNK-ATTESTATION-V1\n";

/// 一个块的种子健康度（确定性，只依赖副本数）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSeedHealth {
    /// 该块的独立副本数。
    pub replicas: u32,
    /// 健康度百分比 = 副本数 × 100（0→0%，1→100%，10→1000%）。
    pub health_percent: u64,
    /// 是否至少有一个副本可取得（replicas >= 1）。
    pub available: bool,
    /// 是否达到「1000% 种子」门槛（replicas >= 10）。
    pub meets_seed_target: bool,
}

impl ChunkSeedHealth {
    /// 由副本数计算单块健康度（确定性纯函数）。
    pub fn from_replicas(replicas: u32) -> Self {
        Self {
            replicas,
            health_percent: replicas as u64 * 100,
            available: replicas >= 1,
            meets_seed_target: replicas >= 10,
        }
    }
}

/// 整镜像的种子健康度（按「最弱块」决定整体，避免平均掩盖冷块）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSeedHealth {
    pub chunks_total: usize,
    /// 至少有 1 个副本的块数。
    pub chunks_available: usize,
    /// 有副本块占比的千分点整数（0..=1000，避免浮点）：available/total*1000。
    pub availability_permille: u32,
    /// 最弱块的副本数（决定整体健康度的短板）。
    pub min_replicas: u32,
    /// 平均副本数的千分点整数（sum/total*1000）。
    pub mean_replicas_permille: u64,
    /// 整体健康度百分比 = 最弱块副本数 × 100。
    pub health_percent: u64,
    /// 所有块都至少有 1 副本。
    pub fully_available: bool,
    /// 所有块都达到 10 副本（整镜像 1000% 种子）。
    pub fully_seeded_1000pct: bool,
}

impl ImageSeedHealth {
    /// 由「块序号 → 副本数」表计算整镜像健康度。
    ///
    /// 副本表必须覆盖清单中每一个块序号；缺记按 0 副本处理（不伪造存在性）。
    pub fn for_manifest(manifest: &ChunkManifest, replica_counts: &HashMap<usize, u32>) -> Self {
        let total = manifest.chunks.len();
        let mut available = 0usize;
        let mut min = u32::MAX;
        let mut sum: u64 = 0;
        for (i, _entry) in manifest.chunks.iter().enumerate() {
            let r = replica_counts.get(&i).copied().unwrap_or(0);
            if r >= 1 {
                available += 1;
            }
            if r < min {
                min = r;
            }
            sum += r as u64;
        }
        if total == 0 {
            min = 0;
        }
        let avail_permille = if total == 0 {
            1000
        } else {
            (available as u64 * 1000 / total as u64) as u32
        };
        let mean_permille = if total == 0 {
            0
        } else {
            sum * 1000 / total as u64
        };
        Self {
            chunks_total: total,
            chunks_available: available,
            availability_permille: avail_permille,
            min_replicas: min,
            mean_replicas_permille: mean_permille,
            health_percent: min as u64 * 100,
            fully_available: total > 0 && available == total,
            fully_seeded_1000pct: total > 0 && min >= 10,
        }
    }
}

/// 证明/信任相关错误（全具名、无 panic）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttestationError {
    #[error("发布者公钥长度非法（应为 32 字节，实际 {0}）")]
    BadPublicKeyLen(usize),
    #[error("签名长度非法（应为 64 字节，实际 {0}）")]
    BadSignatureLen(usize),
    #[error("块 {index} 的发布者证明验证失败（签名与锚定字段不符，可能被篡改或槽位迁移）")]
    VerifyFailed { index: usize },
    #[error("发布者不在本地信任集合中（公钥 {0}）")]
    UntrustedPublisher(String),
}

/// 发布者身份 = Ed25519 验证公钥的 32 字节。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublisherId(pub [u8; 32]);

impl PublisherId {
    pub fn from_bytes(b: &[u8]) -> Result<Self, AttestationError> {
        let arr: [u8; 32] = b
            .try_into()
            .map_err(|_| AttestationError::BadPublicKeyLen(b.len()))?;
        Ok(Self(arr))
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// 小写十六进制（用于错误信息与配置，不用于安全比较）。
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in &self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

/// 受信发布者集合：空集合意味着「谁都不信」（fail-closed）。
#[derive(Debug, Clone, Default)]
pub struct TrustedPublishers {
    inner: std::collections::HashSet<PublisherId>,
}

impl TrustedPublishers {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, id: PublisherId) -> &mut Self {
        self.inner.insert(id);
        self
    }
    pub fn contains(&self, id: &PublisherId) -> bool {
        self.inner.contains(id)
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// 构造某块的规范化锚定消息（域前缀 + 长度自描述字段，防歧义/防槽位迁移）。
pub fn chunk_attestation_message(image: &str, entry: &ChunkEntry) -> Vec<u8> {
    let img = image.as_bytes();
    let digest = entry.sha256.as_bytes();
    let mut m =
        Vec::with_capacity(ATTESTATION_DOMAIN.len() + 8 + img.len() + 4 + 8 + 8 + 8 + digest.len());
    m.extend_from_slice(ATTESTATION_DOMAIN);
    m.extend_from_slice(&(img.len() as u64).to_le_bytes());
    m.extend_from_slice(img);
    m.extend_from_slice(&entry.index.to_le_bytes());
    m.extend_from_slice(&entry.offset.to_le_bytes());
    m.extend_from_slice(&entry.length.to_le_bytes());
    m.extend_from_slice(&(digest.len() as u64).to_le_bytes());
    m.extend_from_slice(digest);
    m
}

/// 发布者对一个块签名（64 字节 Ed25519）。生产路径优先用
/// [`Keypair`] + [`sign_chunk_with_keypair`]（复用仓库既有 identity 封装）。
pub fn sign_chunk(signing_key: &SigningKey, image: &str, entry: &ChunkEntry) -> [u8; 64] {
    signing_key
        .sign(&chunk_attestation_message(image, entry))
        .to_bytes()
}

/// 用仓库既有 [`Keypair`] / [`Ed25519Signer`] 对一个块签名（生产工具路径）。
pub fn sign_chunk_with_keypair(kp: &Keypair, image: &str, entry: &ChunkEntry) -> Vec<u8> {
    Ed25519Signer::new(kp).sign(&chunk_attestation_message(image, entry))
}

/// 用给定发布者公钥校验某块的锚定签名（不含信任集合判断）。
///
/// 复用仓库既有 [`Ed25519Signer::verify_with_pubkey`]，不重复 dalek 样板。
pub fn verify_chunk_attestation(
    image: &str,
    entry: &ChunkEntry,
    publisher: &PublisherId,
    signature: &[u8],
) -> Result<(), AttestationError> {
    if signature.len() != 64 {
        return Err(AttestationError::BadSignatureLen(signature.len()));
    }
    let msg = chunk_attestation_message(image, entry);
    if Ed25519Signer::verify_with_pubkey(publisher.as_bytes(), &msg, signature) {
        Ok(())
    } else {
        Err(AttestationError::VerifyFailed {
            index: entry.index as usize,
        })
    }
}

/// 受信发布者 + 锚定签名的组合校验（生产闸门）：先验签名锚定，再查信任集合。
pub fn verify_trusted_chunk(
    image: &str,
    entry: &ChunkEntry,
    publisher: &PublisherId,
    signature: &[u8],
    trusted: &TrustedPublishers,
) -> Result<(), AttestationError> {
    verify_chunk_attestation(image, entry, publisher, signature)?;
    if !trusted.contains(publisher) {
        return Err(AttestationError::UntrustedPublisher(publisher.to_hex()));
    }
    Ok(())
}

/// 把 [`BlockSource`] 包成「必须带受信发布者锚定签名」的源。
///
/// 这是 v3.7.3 的**真实生产接线点**：签名约定为旁路文件
/// `<dir>/<sha256>.sig`（64 字节原始 Ed25519 签名）。它：
/// - 读不到签名 → [`BlockFetchError::Attestation`]（fail-closed，不当成 NotPresent）；
/// - 签名锚定失败 / 槽位不符 → `Attestation`；
/// - 发布者不在信任集合 → [`BlockFetchError::UntrustedPublisher`]；
/// - 内层是 UDOS 远端时，其「远端未实现」原样透传（不伪造跨网）。
///
/// 字节本身的 sha256 复核仍由外层 `BlockStore` 用清单统一做（双保险：
/// 签名绑定 sha256，BlockStore 再证明字节 == 该 sha256）。
pub struct AttestedSeedSource {
    name: String,
    inner: Arc<dyn BlockSource>,
    dir: PathBuf,
    publisher: PublisherId,
    trusted: Arc<TrustedPublishers>,
}

impl AttestedSeedSource {
    pub fn new(
        inner: Arc<dyn BlockSource>,
        dir: impl Into<PathBuf>,
        publisher: PublisherId,
        trusted: Arc<TrustedPublishers>,
    ) -> Self {
        let dir = dir.into();
        let name = format!("attested:{}", inner.name());
        Self {
            name,
            inner,
            dir,
            publisher,
            trusted,
        }
    }

    fn sig_path(&self, digest: &str) -> PathBuf {
        self.dir.join(format!("{digest}.sig"))
    }
}

impl BlockSource for AttestedSeedSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> super::blockstore::BlockSourceKind {
        self.inner.kind()
    }
    fn fetch(&self, image: &str, entry: &ChunkEntry) -> Result<Vec<u8>, BlockFetchError> {
        // 先取字节（内层源负责 NotPresent / IO / 远端未实现的具名语义）。
        let bytes = self.inner.fetch(image, entry)?;

        // 信任集合闸门（即使后面读不到签名，也先确认发布者受信）。
        if !self.trusted.contains(&self.publisher) {
            return Err(BlockFetchError::UntrustedPublisher);
        }

        // 签名必须存在：读不到一律按证明缺失 fail-closed（不是缺块）。
        let sig = match std::fs::read(self.sig_path(&entry.sha256)) {
            Ok(s) => s,
            Err(_) => {
                return Err(BlockFetchError::Attestation(format!(
                    "块 {} 缺少发布者签名旁路文件 {}.sig",
                    entry.index, entry.sha256
                )))
            }
        };

        verify_trusted_chunk(image, entry, &self.publisher, &sig, &self.trusted).map_err(|e| {
            match e {
                AttestationError::UntrustedPublisher(_) => BlockFetchError::UntrustedPublisher,
                other => BlockFetchError::Attestation(other.to_string()),
            }
        })?;
        Ok(bytes)
    }
}

/// 做种辅助：用发布者 [`Keypair`] 为已落盘的块写 `<sha256>.sig` 旁路（生产/工具复用）。
pub fn write_chunk_signature(
    dir: &std::path::Path,
    kp: &Keypair,
    image: &str,
    entry: &ChunkEntry,
) -> std::io::Result<()> {
    let sig = sign_chunk_with_keypair(kp, image, entry);
    std::fs::write(dir.join(format!("{}.sig", entry.sha256)), sig)
}

/// 记账表：按块内容地址（sha256）登记观测到的副本数（供健康度确定性计算）。
#[derive(Default)]
pub struct ReplicaLedger {
    counts: Mutex<HashMap<String, u32>>,
}

impl ReplicaLedger {
    pub fn new() -> Self {
        Self::default()
    }
    /// 某内容地址观测到一个副本（按对端去重后调用）。
    pub fn observe(&self, digest: &str) {
        *self
            .counts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(digest.to_string())
            .or_insert(0) += 1;
    }
    pub fn replicas_of(&self, digest: &str) -> u32 {
        self.counts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(digest)
            .copied()
            .unwrap_or(0)
    }
    /// 结合清单生成「块序号 → 副本数」表。
    pub fn counts_for_manifest(&self, manifest: &ChunkManifest) -> HashMap<usize, u32> {
        let g = self.counts.lock().unwrap_or_else(|p| p.into_inner());
        manifest
            .chunks
            .iter()
            .enumerate()
            .map(|(i, e)| (i, g.get(&e.sha256).copied().unwrap_or(0)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ausec::blockstore::{BlockStore, LocalDirBlockSource};
    use crate::ausec::image::build_manifest;

    fn data(n: u64) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    static CNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "au-seed373-{tag}-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                CNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ));
            std::fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// 固定种子发布者（可复现），32 字节来自简单递增。
    fn publisher() -> Keypair {
        let mut seed = [0u8; 32];
        for (i, b) in seed.iter_mut().enumerate() {
            *b = i as u8;
        }
        Keypair::from_seed(&seed)
    }

    /// 在目录里为某镜像所有块做种：写字节文件 + 受信签名旁路。
    fn seed_attested(
        dir: &std::path::Path,
        kp: &Keypair,
        image: &str,
        m: &ChunkManifest,
        bytes: &[u8],
    ) {
        for (i, e) in m.chunks.iter().enumerate() {
            let s = e.offset as usize;
            let end = s + e.length as usize;
            std::fs::write(dir.join(&e.sha256), &bytes[s..end]).unwrap();
            write_chunk_signature(dir, kp, image, m.chunks.get(i).unwrap()).unwrap();
        }
    }

    fn attested_store(
        dir: &std::path::Path,
        kp: &Keypair,
        trusted: TrustedPublishers,
        m: ChunkManifest,
    ) -> BlockStore {
        let pk = PublisherId::from_bytes(kp.public_key()).unwrap();
        let raw: Arc<dyn BlockSource> = Arc::new(LocalDirBlockSource::new(dir));
        let src: Arc<dyn BlockSource> =
            Arc::new(AttestedSeedSource::new(raw, dir, pk, Arc::new(trusted)));
        BlockStore::new(m, vec![src]).unwrap()
    }

    #[test]
    fn health_is_deterministic_from_replicas() {
        assert_eq!(ChunkSeedHealth::from_replicas(0).health_percent, 0);
        assert!(!ChunkSeedHealth::from_replicas(0).available);
        assert_eq!(ChunkSeedHealth::from_replicas(1).health_percent, 100);
        assert!(ChunkSeedHealth::from_replicas(1).available);
        let ten = ChunkSeedHealth::from_replicas(10);
        assert_eq!(ten.health_percent, 1000);
        assert!(ten.meets_seed_target);
        assert!(!ChunkSeedHealth::from_replicas(9).meets_seed_target);
    }

    #[test]
    fn image_health_uses_weakest_chunk_and_availability() {
        let bytes = data(40);
        let m = build_manifest("img/h", &bytes, 16).unwrap(); // 3 块
        let mut rc = HashMap::new();
        rc.insert(0usize, 12u32);
        rc.insert(1usize, 10u32);
        // 块 2 缺记 → 0 副本。
        let h = ImageSeedHealth::for_manifest(&m, &rc);
        assert_eq!(h.chunks_total, 3);
        assert_eq!(h.chunks_available, 2);
        assert_eq!(h.availability_permille, 666); // 2/3*1000 截断
        assert_eq!(h.min_replicas, 0); // 短板决定
        assert_eq!(h.health_percent, 0);
        assert!(!h.fully_available);
        assert!(!h.fully_seeded_1000pct);

        // 全部块 >=10 副本 → 整镜像 1000% 种子。
        let rc2: HashMap<usize, u32> = (0..3).map(|i| (i, 10u32)).collect();
        let h2 = ImageSeedHealth::for_manifest(&m, &rc2);
        assert!(h2.fully_available);
        assert!(h2.fully_seeded_1000pct);
        assert_eq!(h2.health_percent, 1000);
        assert_eq!(h2.availability_permille, 1000);
    }

    #[test]
    fn empty_image_health_is_well_defined() {
        let m = build_manifest("img/empty", &[], 16).unwrap();
        let h = ImageSeedHealth::for_manifest(&m, &HashMap::new());
        assert_eq!(h.chunks_total, 0);
        assert!(!h.fully_available); // 空镜像不声称可服务
        assert_eq!(h.availability_permille, 1000); // 无缺失块
    }

    #[test]
    fn attestation_binds_all_fields_and_rejects_slot_move() {
        let bytes = data(40);
        let m = build_manifest("img/bind", &bytes, 16).unwrap();
        let sk = SigningKey::from_bytes(&publisher().seed());
        let pk = PublisherId(sk.verifying_key().to_bytes());

        let e0 = &m.chunks[0];
        let sig = sign_chunk(&sk, &m.image, e0);
        // 正确锚定通过。
        assert!(verify_chunk_attestation(&m.image, e0, &pk, &sig).is_ok());

        // 把块0的签名搬到块1槽位 → 必须失败（防槽位迁移）。
        assert!(matches!(
            verify_chunk_attestation(&m.image, &m.chunks[1], &pk, &sig),
            Err(AttestationError::VerifyFailed { index: 1 })
        ));
        // 改镜像名 → 失败。
        assert!(verify_chunk_attestation("img/other", e0, &pk, &sig).is_err());
        // 改长度 → 失败。
        let mut tampered = e0.clone();
        tampered.length += 1;
        assert!(verify_chunk_attestation(&m.image, &tampered, &pk, &sig).is_err());
        // 改 sha256 → 失败。
        let mut t2 = e0.clone();
        t2.sha256 = "0".repeat(64);
        assert!(verify_chunk_attestation(&m.image, &t2, &pk, &sig).is_err());
        // 签名长度非法。
        assert!(matches!(
            verify_chunk_attestation(&m.image, e0, &pk, &sig[..10]),
            Err(AttestationError::BadSignatureLen(10))
        ));
    }

    #[test]
    fn untrusted_publisher_fails_even_with_self_consistent_signature() {
        let bytes = data(20);
        let m = build_manifest("img/trust", &bytes, 16).unwrap();
        let kp = publisher();
        let pk = PublisherId::from_bytes(kp.public_key()).unwrap();
        let sig = sign_chunk_with_keypair(&kp, &m.image, &m.chunks[0]);

        let empty = TrustedPublishers::new();
        assert!(matches!(
            verify_trusted_chunk(&m.image, &m.chunks[0], &pk, &sig, &empty),
            Err(AttestationError::UntrustedPublisher(_))
        ));
        let mut one = TrustedPublishers::new();
        one.add(pk.clone());
        assert!(verify_trusted_chunk(&m.image, &m.chunks[0], &pk, &sig, &one).is_ok());
    }

    #[test]
    fn attested_source_end_to_end_through_blockstore() {
        let bytes = data(36);
        let image = "img/e2e";
        let m = build_manifest(image, &bytes, 16).unwrap();
        let dir = Tmp::new("e2e");
        let kp = publisher();
        seed_attested(&dir.0, &kp, image, &m, &bytes);

        let mut trusted = TrustedPublishers::new();
        trusted.add(PublisherId::from_bytes(kp.public_key()).unwrap());
        // BlockStore 取得清单所有权，先克隆一份用于回读断言。
        let manifest_for_check = m.clone();
        let store = attested_store(&dir.0, &kp, trusted, m);

        // 三块都经「签名锚定 + 信任集合 + sha256」三重闸取回。
        for (i, e) in manifest_for_check.chunks.iter().enumerate() {
            let got = store.get_chunk(i).unwrap();
            let s = e.offset as usize;
            assert_eq!(&*got, &bytes[s..s + e.length as usize]);
        }
    }

    #[test]
    fn missing_signature_is_fail_closed_not_not_present() {
        let bytes = data(20);
        let image = "img/nosig";
        let m = build_manifest(image, &bytes, 16).unwrap();
        let dir = Tmp::new("nosig");
        let kp = publisher();
        // 只写字节，不写签名。
        let e0 = &m.chunks[0];
        std::fs::write(dir.0.join(&e0.sha256), &bytes[..16]).unwrap();

        let mut trusted = TrustedPublishers::new();
        trusted.add(PublisherId::from_bytes(kp.public_key()).unwrap());
        let pk = PublisherId::from_bytes(kp.public_key()).unwrap();
        let raw: Arc<dyn BlockSource> = Arc::new(LocalDirBlockSource::new(&dir.0));
        let src = AttestedSeedSource::new(raw, &dir.0, pk, Arc::new(trusted));
        // 缺签名 → Attestation（不是 NotPresent，防止被当成普通降级跳过）。
        assert!(matches!(
            src.fetch(image, e0),
            Err(BlockFetchError::Attestation(_))
        ));
    }

    #[test]
    fn blockstore_untrusted_publisher_is_rejected() {
        let bytes = data(36);
        let image = "img/untrusted";
        let m = build_manifest(image, &bytes, 16).unwrap();
        let dir = Tmp::new("untrusted");
        let kp = publisher();
        seed_attested(&dir.0, &kp, image, &m, &bytes);

        // 空信任集合 → BlockStore 直接 UntrustedPublisher（fail-closed）。
        let store = attested_store(&dir.0, &kp, TrustedPublishers::new(), m);
        assert!(matches!(
            store.get_chunk(0),
            Err(crate::ausec::BlockStoreError::UntrustedPublisher { index: 0 })
        ));
    }

    #[test]
    fn corrupted_signature_is_rejected() {
        let bytes = data(36);
        let image = "img/badsig";
        let m = build_manifest(image, &bytes, 16).unwrap();
        let dir = Tmp::new("badsig");
        let kp = publisher();
        seed_attested(&dir.0, &kp, image, &m, &bytes);

        // 翻转块0签名首字节。
        let sigp = dir.0.join(format!("{}.sig", m.chunks[0].sha256));
        let mut bad = std::fs::read(&sigp).unwrap();
        bad[0] ^= 0xFF;
        std::fs::write(&sigp, bad).unwrap();

        let mut trusted = TrustedPublishers::new();
        trusted.add(PublisherId::from_bytes(kp.public_key()).unwrap());
        let store = attested_store(&dir.0, &kp, trusted, m);
        assert!(matches!(
            store.get_chunk(0),
            Err(crate::ausec::BlockStoreError::BadAttestation { index: 0, .. })
        ));
    }

    #[test]
    fn replica_ledger_drives_image_health() {
        let bytes = data(32);
        let m = build_manifest("img/ledger", &bytes, 16).unwrap();
        let ledger = ReplicaLedger::new();
        // 块0见到3个副本，块1见到10个。
        for _ in 0..3 {
            ledger.observe(&m.chunks[0].sha256);
        }
        for _ in 0..10 {
            ledger.observe(&m.chunks[1].sha256);
        }
        let rc = ledger.counts_for_manifest(&m);
        let h = ImageSeedHealth::for_manifest(&m, &rc);
        assert_eq!(h.min_replicas, 3); // 短板
        assert_eq!(h.health_percent, 300);
        assert!(h.fully_available);
        assert!(!h.fully_seeded_1000pct);
        assert_eq!(ledger.replicas_of(&m.chunks[1].sha256), 10);
    }
}
