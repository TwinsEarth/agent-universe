//! Memory Sharing（v2.4.2+）
//!
//! 三层记忆：
//! - 个体记忆 `AgentMemory` / `EnhancedMemory`（v2.4.4：检索/评价/压缩遗忘）
//! - 群体记忆 `SwarmMemory`（v2.4.5：TransferBundle 交接、TraceLedger 审计、证据分级）
//! - 跨代记忆 `IntergenMemory`（v2.4.3：分层主体 + 哈希链）
//! - 群体智能飞轮 `Flywheel`（v2.4.6）

pub mod agent_memory;
pub mod enhanced;
pub mod flywheel;
pub mod handoff;
pub mod layered;
pub mod shared_memory;

pub use agent_memory::AgentMemory;
pub use enhanced::{EnhancedMemory, RatedMemory};
pub use flywheel::Flywheel;
pub use handoff::{EvidenceGrade, TraceLedger, TransferBundle};
pub use layered::{IntergenMemory, LayeredMemory};
pub use shared_memory::SwarmMemory;
