//! v2.5.3: Mesh 自组网模块 —— **v3.5.9 状态登记（DOC-05 / 自审 F-4、承接 GAP §5.7）：
//! 库面/实验性，未在 `gsn-daemon` 启动运行图中构造。** `MeshNode` 等类型仅由 `lib.rs`
//! re-export 供库/测试引用；生产节点的自组网、发现、心跳与 NAT 穿透由 libp2p 承担
//! （Kademlia 发现、GossipSub 广播、AutoNAT、DCUtR、Circuit Relay，见 `net::peer`），
//! 不是这里的模拟实现。下列能力描述为模块设计意图，不代表守护进程实际生效；其中历史上
//! `LocalSniffer` 为空操作、会话 code 由调用方提供等问题见上游 GAP §5.7。状态：
//! **待决（接线启用 or 收敛删除）**，见 `docs/CAPABILITY-STATUS.md`。
//!
//! 跨网络、跨主机、跨系统的自组网能力（设计描述）：
//! - 心跳（heartbeat）：定期探测 peer 存活
//! - 嗅探（discovery）：自动发现本地网络 peers
//! - 广播（broadcast）：GossipSub 主题消息扩散
//! - 穿透（hole punching）：NAT 类型检测 + ICE 候选
//! - 自组网（auto-mesh）：自动 bootstrap，形成网状拓扑
//! - 临时 SN（session number）：每次会话分配唯一识别码
//! - 永久 DID 绑定：临时 SN ↔ 永久 DID 映射

pub mod discovery;
pub mod heartbeat;
pub mod session;
// mesh/mesh.rs 与模块目录同名（常见组织方式），允许 module_inception。
#[allow(clippy::module_inception)]
pub mod mesh;

pub use discovery::{DiscoveryAnnouncement, DiscoveryTable, LocalSniffer};
pub use heartbeat::{HeartbeatConfig, HeartbeatTracker, PeerLiveness};
pub use mesh::{MeshConfig, MeshNode, MeshTopology};
pub use session::{PermanentDid, SessionId, SessionRegistry};
