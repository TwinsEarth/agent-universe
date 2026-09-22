pub mod collective;
pub mod emergence;
pub mod consensus;

pub use collective::{Swarm, AgentNode, CollectiveDecision};
pub use emergence::EmergenceDetector;
pub use consensus::LightweightConsensus;
