//! Memory Sharing（v2.4.2+）。
//!
//! 接线状态（v5.0.1 起）：
//! - [`persistent`] 持久化个体记忆（append-only `memory.jsonl`）**已接线进 `gsn-daemon`
//!   启动运行图**：sandbox exec 成功后自动构造并追加一条记忆（见 node.rs 的
//!   `record_sandbox_exec_memory`）。
//! - 下列三层记忆与飞轮（AgentMemory / SwarmMemory / IntergenMemory / Flywheel）仍为纯
//!   算法库，仅由 `lib.rs` re-export 供库使用者/测试引用，生产数据面暂不读写，属待决项
//!   （后续版本接线或收敛删除），见 `docs/CAPABILITY-STATUS.md`。
//!
//! 三层记忆：
//! - 个体记忆 `AgentMemory` / `EnhancedMemory`（v2.4.4：检索/评价/压缩遗忘）
//! - 群体记忆 `SwarmMemory`（v2.4.5：TransferBundle 交接、TraceLedger 审计、证据分级）
//! - 跨代记忆 `IntergenMemory`（v2.4.3：分层主体 + 哈希链）
//! - 群体智能飞轮 `Flywheel`（v2.4.6）

pub mod agent_memory;
pub mod cat7;
pub mod chain;
pub mod consolidation;
pub mod digest;
pub mod enhanced;
pub mod envtrace;
pub mod fade;
pub mod fleet;
pub mod flywheel;
pub mod handoff;
pub mod layered;
pub mod lineage;
pub mod persistent;
pub mod privacy;
pub mod remix;
pub mod shared_memory;
pub mod svaf;

pub use agent_memory::AgentMemory;
pub use cat7::Cat7Block;
pub use chain::{verify_chain_file, ChainFileReport};
pub use consolidation::{
    consolidate_mid_to_long, consolidate_short_to_mid, merkle_sha256, ConsolidationEntry,
    ConsolidationError, ConsolidationStore, Stage,
};
pub use digest::{digest_sessions, SessionDigest, DIGEST_CONCLUSION_MAX};
pub use enhanced::{EnhancedMemory, RatedMemory};
pub use envtrace::{EnvTrace, EnvTraceError, EnvTraceStore, TraceDelta, TraceKind};
pub use fade::{
    FadeEntry, FadeError, FadeOutcome, FadePolicy, FadeReport, FadeStage, FadeStats, FadeStore,
};
pub use fleet::{
    fleet_claim_hash, FleetAccess, FleetEntry, FleetError, FleetKind, FleetMemory, FleetStatus,
    FleetStatusReport,
};
pub use flywheel::Flywheel;
pub use handoff::{EvidenceGrade, TraceLedger, TransferBundle};
pub use layered::{IntergenMemory, LayeredMemory};
pub use lineage::{LineageError, LineageGraph, LineageRecord};
pub use persistent::{
    MemoryEntry, MemoryError, MemoryQuery, MemoryStorageStatus, PersistentMemoryStore,
    RotatedFileStatus, DEFAULT_KEEP_ROTATED, DEFAULT_MAX_BYTES, DEFAULT_TAIL_MAX_CHARS,
};
pub use privacy::{redact_entry, redact_secrets, scan_entry_for_secret, SecretHit};
pub use remix::{RemixError, RemixRecord, RemixStore};
pub use shared_memory::SwarmMemory;
pub use svaf::{Cat7Field, FieldVerdict, SvafAssessment, SvafDecision, SvafError};
