pub mod collective;
pub mod consensus;
pub mod emergence;
pub mod memory;

pub use collective::{AgentNode, CollectiveDecision, Swarm};
pub use consensus::LightweightConsensus;
pub use emergence::EmergenceDetector;
pub use memory::{run_experiment, Metrics, Rng, SharedMemory, TrialResult};
