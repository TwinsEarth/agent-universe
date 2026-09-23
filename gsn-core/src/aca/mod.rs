//! ACA（Agent Communication Architecture）兼容层
//!
//! 四个协议对象：Agent Manifest → Task Envelope → Receipt → Reputation
//! 五级验证分层：L0 抽样 → L1 冗余 → L2 TEE → L3 zkML → L4 委员会仲裁

pub mod manifest;
pub mod envelope;
pub mod receipt;
pub mod reputation;
pub mod verification;
pub mod message;

pub use manifest::{AgentManifest, HardwareProfile, VerificationMode};
pub use envelope::{TaskEnvelope, PrivacyRequirement, TaskPriority};
pub use receipt::{Receipt, ReceiptStatus, ResourceMetering};
pub use reputation::{MultiReputation, ReputationDimension};
pub use verification::{VerificationLevel, VerificationPolicy, VerificationResult};
pub use message::{AcaMessage, MessageType};
