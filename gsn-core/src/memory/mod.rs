//! Memory Sharing（v2.4.2+）
//!
//! 三层记忆：
//! - 个体记忆 `AgentMemory` / `EnhancedMemory`（v2.4.4：检索/评价/压缩遗忘）
//! - 群体记忆 `SwarmMemory`（v2.4.5：TransferBundle 交接、TraceLedger 审计、证据分级）
//! - 跨代记忆 `IntergenMemory`（v2.4.3：分层主体 + 哈希链）
//! - 群体智能飞轮 `Flywheel`（v2.4.6）

pub mod agent_memory;
pub mod shared_memory;
pub mod layered;
pub mod enhanced;
pub mod handoff;
pub mod flywheel;

pub use agent_memory::AgentMemory;
pub use shared_memory::SwarmMemory;
pub use layered::{LayeredMemory, IntergenMemory};
pub use enhanced::{EnhancedMemory, RatedMemory};
pub use handoff::{TransferBundle, EvidenceGrade, TraceLedger};
pub use flywheel::Flywheel;
