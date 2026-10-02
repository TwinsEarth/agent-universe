//! 网络拓扑（v3.5.3，AU-02）：纯算法库。当前 run_daemon 未构造本模块的任何组件，
//! 属 library-only 能力，尚未接入 daemon 运行图——不要据此认为拓扑路由已在生产生效。

pub mod graph;
pub mod layer;
pub mod neighbor;
pub mod router;

pub use graph::TopologyGraph;
pub use layer::{LayeredTopology, Level};
pub use neighbor::NeighborManager;
pub use router::TopologyRouter;
