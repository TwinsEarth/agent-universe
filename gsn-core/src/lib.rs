// payment::status_payload 的 serde_json::json! 字面量较大（v3.9.8 增 ZK/OWS/合规三块），
// 默认 128 的宏递归深度不足；提升到 256（仅编译期展开上限，不影响运行期行为）。
#![recursion_limit = "256"]
//! Agent Universe gsn-core v3.9.13
//!
//! 群体智能核心库 - 智能体宇宙
//!
//! 群体智能 = 网络结构的 Scaling Law
//! 从 token 网络结构向智能体网络结构演进
//! v2.3.4: 智能体市场 Agent Market
//! v2.3.6: 跨平台客户端 + CLI/REST/MCP 三大连接层重构

pub mod aca;
pub mod agent;
pub mod api;
pub mod ausec;
pub mod chain;
pub mod collaboration;
pub mod crdt;
pub mod crowdsource;
pub mod deepseek;
pub mod economy;
pub mod erasure;
pub mod ffi;
pub mod identity;
pub mod inference;
pub mod llm;
pub mod marketplace;
pub mod mcp;
pub mod memory;
pub mod mesh;
pub mod mode;
pub mod nat;
pub mod net;
pub mod node;
pub mod plugin;
pub mod proof;
pub mod relay_pool;
pub mod sandbox;
pub mod scheduler;
pub mod security;
pub mod storage;
pub mod swarm;
pub mod topology;
pub mod update;
pub mod verifier;

pub use agent::{AgentCard, Task, TaskStatus};
pub use chain::{PoCVVerifier, ProofOfComputation};
pub use crdt::VersionVector;
pub use crdt::{CrdtMessage, CrdtOp, CrdtSnapshot, CrdtStats, CrdtStore, LwwEntry, CRDT_TOPIC};
pub use economy::{
    ContributionProof, ContributionType, DifficultyLevel, ReputationSystem, TaskPricing,
    UrgencyLevel,
};
pub use erasure::{DecodedShard, ErasureCoder};
pub use identity::{Did, Ed25519Signer, Keypair, SecureKeyring};
pub use mode::{ModeController, NodeMode};
pub use net::{InMemoryGossip, InMemoryKademlia, InMemoryNode, RootSeedConfig, SeedMode};
pub use proof::ProofOfContribution;
pub use scheduler::{BalanceStrategy, LoadBalancer, TaskRouter};
pub use storage::LocalStorage;
pub use swarm::{AgentNode, CollectiveDecision, EmergenceDetector, LightweightConsensus, Swarm};
pub use topology::{NeighborManager, TopologyGraph};
pub use verifier::VerifierClient;

// MCP + ACA (v2.3.2)
pub use mcp::McpServer;
pub use mcp::{McpError, McpMessage, McpMethod, McpRequest, McpResponse, RequestId};
pub use mcp::{PromptArgument, PromptDefinition, PromptMessage, PromptRole};
pub use mcp::{ResourceContents, ResourceDefinition, ResourceUri};
pub use mcp::{ToolDefinition, ToolParameter, ToolResult, ToolSchema};

// ACA (v2.3.2)
pub use aca::{AcaMessage, MessageType};
pub use aca::{AgentManifest, HardwareProfile, VerificationMode};
pub use aca::{MultiReputation, ReputationDimension};
pub use aca::{PrivacyRequirement, TaskEnvelope, TaskPriority};
pub use aca::{Receipt, ReceiptStatus, ResourceMetering};
pub use aca::{VerificationLevel, VerificationPolicy, VerificationResult};

// v2.3.3: P2P 分布式网络
pub use collaboration::{
    AgentTier, CollaborationGroup, CollaborationManager, CollaborationMessage,
    MessageType as CollabMessageType, SceneType,
};
pub use crowdsource::{
    CrowdTask, CrowdTaskStatus, CrowdsourcingMarket, Solver, TaskType as CrowdTaskType,
};
pub use inference::{
    ComputeResource, ComputeScheduler, InferenceTask, KvCacheShard,
    TaskStatus as InferenceTaskStatus,
};
pub use nat::{CandidateType, ConnectionState, IceCandidate, NatTraversalManager, NatType};
pub use security::{SecurityEngine, SecurityFlag, SecurityScore};

// v2.3.4: 智能体市场 Agent Market
pub use marketplace::{
    AccountMismatch, AgentCategory, AgentMarket, AuditReport, Bid, ConservationReport, Currency,
    DisputeCase, ErrorType, EvidenceGrade, MarketAgentCard, MarketReputation, Pricing,
    PricingModel, QaCommittee, QaDecision, QaMember, QaVote, ReputationManager, ResultEnvelope,
    SchemaField, SettlementEngine, SettlementReason, SettlementRecord, SkillManifest, Sla,
    StakeRecord, StakeStatus, TaskSpec, TaskState, VerificationPolicy as MarketVerificationPolicy,
};
