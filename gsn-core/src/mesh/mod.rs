//! v2.5.3: Mesh 自组网模块
//!
//! 跨网络、跨主机、跨系统的自组网能力：
//! - 心跳（heartbeat）：定期探测 peer 存活
//! - 嗅探（discovery）：自动发现本地网络 peers
//! - 广播（broadcast）：GossipSub 主题消息扩散
//! - 穿透（hole punching）：NAT 类型检测 + ICE 候选
//! - 自组网（auto-mesh）：自动 bootstrap，形成网状拓扑
//! - 临时 SN（session number）：每次会话分配唯一识别码
//! - 永久 DID 绑定：临时 SN ↔ 永久 DID 映射

pub mod heartbeat;
pub mod discovery;
pub mod session;
pub mod mesh;

pub use heartbeat::{HeartbeatConfig, HeartbeatTracker, PeerLiveness};
pub use discovery::{DiscoveryAnnouncement, DiscoveryTable, LocalSniffer};
pub use session::{SessionId, SessionRegistry, PermanentDid};
pub use mesh::{MeshConfig, MeshNode, MeshTopology};
