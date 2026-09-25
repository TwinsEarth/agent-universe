//! Memory Sharing（v2.4.2）
//!
//! - 内部记忆库 `AgentMemory`：单个智能体自己的执行历史、成功/失败案例、学到的模式。
//! - 外部记忆库 `SwarmMemory`：群体共享，任何智能体可发布经验、按信誉加权检索，
//!   让每个 Agent 站在所有 Agent 的肩膀上（后训练提升命中率/成功率）。

pub mod agent_memory;
pub mod shared_memory;
pub mod layered;

pub use agent_memory::AgentMemory;
pub use shared_memory::SwarmMemory;
pub use layered::{LayeredMemory, IntergenMemory};
