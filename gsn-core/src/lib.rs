//! Agent Universe gsn-core v2.3.4
//!
//! 群体智能核心库 - 智能体宇宙
//! 
//! 群体智能 = 网络结构的 Scaling Law
//! 从 token 网络结构向智能体网络结构演进
//! v2.3.4: 智能体市场 Agent Market

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
pub mod mcp;
pub mod aca;
pub mod collaboration;
pub mod inference;
pub mod crowdsource;
pub mod security;
pub mod nat;
pub mod marketplace;

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

// MCP + ACA (v2.3.2)
pub use mcp::{McpMessage, McpRequest, McpResponse, McpError, RequestId, McpMethod};
pub use mcp::{ToolDefinition, ToolParameter, ToolSchema, ToolResult};
pub use mcp::{ResourceDefinition, ResourceContents, ResourceUri};
pub use mcp::{PromptDefinition, PromptMessage, PromptArgument, PromptRole};
pub use mcp::McpServer;

// ACA (v2.3.2)
pub use aca::{AgentManifest, HardwareProfile, VerificationMode};
pub use aca::{TaskEnvelope, PrivacyRequirement, TaskPriority};
pub use aca::{Receipt, ReceiptStatus, ResourceMetering};
pub use aca::{MultiReputation, ReputationDimension};
pub use aca::{VerificationLevel, VerificationPolicy, VerificationResult};
pub use aca::{AcaMessage, MessageType};

// v2.3.3: P2P 分布式网络
pub use collaboration::{CollaborationGroup, CollaborationMessage, CollaborationManager, AgentTier, SceneType, MessageType as CollabMessageType};
pub use inference::{ComputeResource, ComputeScheduler, InferenceTask, KvCacheShard, TaskStatus as InferenceTaskStatus};
pub use crowdsource::{CrowdTask, CrowdTaskStatus, CrowdsourcingMarket, Solver, TaskType as CrowdTaskType};
pub use security::{SecurityEngine, SecurityScore, SecurityFlag};
pub use nat::{NatTraversalManager, NatType, IceCandidate, CandidateType, ConnectionState};

// v2.3.4: 智能体市场 Agent Market
pub use marketplace::{
    AgentMarket, Bid, DisputeCase,
    MarketAgentCard, SkillManifest, Pricing, PricingModel, Currency, Sla, SchemaField, AgentCategory,
    TaskSpec, TaskState, ResultEnvelope, ErrorType, VerificationPolicy as MarketVerificationPolicy,
    QaCommittee, QaMember, QaVote, QaDecision,
    SettlementEngine, SettlementRecord, SettlementReason, ConservationReport,
    ReputationManager, MarketReputation, StakeRecord, StakeStatus,
    EvidenceGrade,
};
