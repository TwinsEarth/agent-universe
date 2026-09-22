//! Agent Universe gsn-core v2.2.3
//!
//! 对等网络核心库 - 最终版

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

pub use identity::{Did, Keypair, Ed25519Signer, SecureKeyring};
pub use agent::{AgentCard, Task, TaskStatus};
pub use net::{GsnNode, KademliaClient, GossipSub, RootSeedConfig, SeedMode};
pub use chain::PoCVVerifier;
pub use storage::LocalStorage;
pub use verifier::VerifierClient;
pub use mode::{NodeMode, ModeController};
pub use crdt::VersionVector;
pub use erasure::ErasureCoder;
