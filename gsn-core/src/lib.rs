//! Agent Universe gsn-core v2.3.1
//!
//! 群体智能核心库 - 智能体宇宙
//! 
//! 群体智能 = 网络结构的 Scaling Law
//! 从 token 网络结构向智能体网络结构演进

pub mod identity;
pub mod agent;
pub mod net;
pub mod chain;
pub mod storage;
pub mod verifier;
pub mod ffi;
pub mod mode;
pub mod crdt;
pub mod erasure;
pub mod swarm;
pub mod economy;
pub mod scheduler;
pub mod topology;
pub mod proof;

pub use identity::{Did, Keypair, Ed25519Signer, SecureKeyring};
pub use agent::{AgentCard, Task, TaskStatus};
pub use net::{GsnNode, KademliaClient, GossipSub, RootSeedConfig, SeedMode};
pub use chain::{PoCVVerifier, ProofOfComputation};
pub use storage::LocalStorage;
pub use verifier::VerifierClient;
pub use mode::{NodeMode, ModeController};
pub use crdt::VersionVector;
pub use erasure::{ErasureCoder, DecodedShard};
pub use swarm::{Swarm, AgentNode, CollectiveDecision, EmergenceDetector, LightweightConsensus};
pub use economy::{ReputationSystem, ContributionProof, ContributionType, TaskPricing, DifficultyLevel, UrgencyLevel};
pub use scheduler::{TaskRouter, LoadBalancer, BalanceStrategy};
pub use topology::{NeighborManager, TopologyGraph};
pub use proof::ProofOfContribution;
