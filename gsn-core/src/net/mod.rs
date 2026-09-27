//! 网络层
//!
//! - [`peer`] / [`P2pPeer`]：**真实** libp2p 网络（传输、加密、DHT、GossipSub、中继/DCUtR）。
//! - [`memory_node`] / [`memory_dht`] / [`memory_gossip`]：**进程内替身**（in-memory），
//!   仅用于单元测试与离线演示，不产生任何网络流量；勿用于断言真实联网。

pub mod peer;
pub mod root_seed;
pub mod memory_node;
pub mod memory_dht;
pub mod memory_gossip;

pub use peer::P2pPeer;
pub use memory_node::InMemoryNode;
pub use memory_dht::InMemoryKademlia;
pub use memory_gossip::InMemoryGossip;
pub use root_seed::{RootSeedConfig, SeedMode};
