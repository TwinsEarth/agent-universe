//! ACA（Agent Communication Architecture）兼容层
//!
//! 四个协议对象：Agent Manifest → Task Envelope → Receipt → Reputation
//! 五级验证分层：L0 抽样 → L1 冗余 → L2 TEE → L3 zkML → L4 委员会仲裁

pub mod crypto;
pub mod clock;
pub mod manifest;

/// 协议规范版本（GAP §4.5）。
///
/// **签名契约**：签名覆盖结构的**规范形式**（canonical bytes，紧凑 JSON、按 Unicode
/// 码点序键、任意深度剥离 signature），而非接收端反序列化后再序列化的字节。
/// 接收端默认值填充、类型重规范化都不得改变验签载荷。
///
/// **版本契约**：任何会改变规范字节的新字段 / 新规则 / 新键序约定，
/// 必须同时 bump 本版本号，否则新旧节点会对同一逻辑对象验签不一致。
/// 重放保护由 [`crate::marketplace`] 的 nonce + 时间戳 + `verify_fresh` 承担。
pub const PROTOCOL_VERSION: &str = "nau/1";
pub mod envelope;
pub mod receipt;
pub mod reputation;
pub mod verification;
pub mod message;
pub mod runtime;

pub use crypto::{canonical_payload, sign_hex, verify_hex};
pub use manifest::{AgentManifest, HardwareProfile, VerificationMode};
pub use envelope::{TaskEnvelope, PrivacyRequirement, TaskPriority};
pub use receipt::{Receipt, ReceiptStatus, ResourceMetering};
pub use reputation::{MultiReputation, ReputationDimension};
pub use verification::{QaCommitteeSpec, VerificationLevel, VerificationPolicy, VerificationResult};
pub use message::{AcaMessage, MessageType};
pub use runtime::{AcaProcessor, AcaDecision, ProcessOutcome};
