//! 群体智能实验算法（v3.5.9 状态登记，DOC-05 / 自审 F-4）：**纯算法库/实验性，
//! 未在 `gsn-daemon` 启动运行图中构造。** 这里的 `Swarm` / `LightweightConsensus` /
//! `EmergenceDetector` / `run_experiment` 是早期群体行为模拟与实验代码，仅由 `lib.rs`
//! re-export，生产网络层并不使用——节点联网与事件传播走的是 libp2p（`net::peer` 里的
//! `libp2p::Swarm` 是另一个同名类型，勿混淆）。官方「群体涌现」能力由 swarm-emergence
//! 插件以自己的实现提供，不构造本模块。状态：**库面/实验性，待决（接线 or 删除）**，
//! 见 `docs/CAPABILITY-STATUS.md`。

pub mod collective;
pub mod consensus;
pub mod emergence;
pub mod memory;

pub use collective::{AgentNode, CollectiveDecision, Swarm};
pub use consensus::LightweightConsensus;
pub use emergence::EmergenceDetector;
pub use memory::{run_experiment, Metrics, Rng, SharedMemory, TrialResult};
