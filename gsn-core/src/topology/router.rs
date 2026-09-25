//! 拓扑路由器（v2.4.1）
//!
//! 默认走新分层拓扑 `LayeredTopology`；分层路由失败（节点不在分层表）时，
//! 自动回退到旧扁平 `TopologyGraph`，并记录 fallback 次数。

use super::{LayeredTopology, TopologyGraph};

#[derive(Debug, Clone)]
pub struct TopologyRouter {
    layered: LayeredTopology,
    flat: TopologyGraph,
    /// 回退到扁平拓扑的次数。
    pub fallback_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteBackend {
    Layered,
    Flat,
    SameNode,
}

impl TopologyRouter {
    pub fn new() -> Self {
        Self {
            layered: LayeredTopology::new(9),
            flat: TopologyGraph::new(),
            fallback_count: 0,
        }
    }

    pub fn join(&mut self, did: String) {
        self.layered.join(did.clone());
        self.flat.add_node(did);
    }

    /// 路由：先分层，失败回退扁平。
    /// 返回 (跳数, 实际使用的后端)。
    pub fn route(&mut self, from: &str, to: &str) -> (Option<usize>, RouteBackend) {
        if from == to {
            return (Some(0), RouteBackend::SameNode);
        }
        match self.layered.route_hops(from, to) {
            Some(hops) => (Some(hops), RouteBackend::Layered),
            None => {
                self.fallback_count += 1;
                (self.flat.shortest_path(from, to), RouteBackend::Flat)
            }
        }
    }

    pub fn node_count(&self) -> usize {
        self.layered.node_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_via_layered_by_default() {
        let mut r = TopologyRouter::new();
        for i in 0..200usize {
            r.join(format!("did:aip:{:04x}", i));
        }
        let (hops, backend) = r.route("did:aip:0000", &format!("did:aip:{:04x}", 199));
        assert_eq!(backend, RouteBackend::Layered);
        assert_eq!(hops, Some(7));
        assert_eq!(r.fallback_count, 0);
    }

    #[test]
    fn falls_back_to_flat_when_node_unknown() {
        let mut r = TopologyRouter::new();
        r.join("did:aip:0001".into());
        r.join("did:aip:0002".into());
        // 第三个节点只进扁平图、不进分层（模拟分层表缺失）
        r.flat.add_node("did:aip:dead".into());
        r.flat.add_edge("did:aip:0001".into(), "did:aip:dead".into());
        // 目标不在分层表 -> 回退扁平
        let (hops, backend) = r.route("did:aip:0001", "did:aip:dead");
        assert_eq!(backend, RouteBackend::Flat);
        assert_eq!(hops, Some(1));
        assert_eq!(r.fallback_count, 1);
    }
}
