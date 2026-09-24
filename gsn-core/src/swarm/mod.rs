pub mod collective;
pub mod emergence;
pub mod consensus;
pub mod memory;

pub use collective::{Swarm, AgentNode, CollectiveDecision};
pub use emergence::EmergenceDetector;
pub use consensus::LightweightConsensus;
pub use memory::{run_experiment, SharedMemory, Metrics, TrialResult, Rng};
