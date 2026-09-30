//! 负载均衡器

use std::collections::HashMap;

pub struct LoadBalancer {
    nodes: HashMap<String, NodeLoad>,
    strategy: BalanceStrategy,
}

#[derive(Debug, Clone)]
struct NodeLoad {
    active_tasks: u64,
    total_tasks: u64,
    capacity: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum BalanceStrategy {
    /// 轮询
    RoundRobin,
    /// 最少连接
    LeastConnections,
    /// 最少响应时间
    LeastResponseTime,
}

impl LoadBalancer {
    pub fn new(strategy: BalanceStrategy) -> Self {
        Self {
            nodes: HashMap::new(),
            strategy,
        }
    }

    pub fn add_node(&mut self, did: String, capacity: u64) {
        self.nodes.insert(
            did,
            NodeLoad {
                active_tasks: 0,
                total_tasks: 0,
                capacity,
            },
        );
    }

    pub fn remove_node(&mut self, did: &str) {
        self.nodes.remove(did);
    }

    pub fn record_task_start(&mut self, did: &str) {
        if let Some(node) = self.nodes.get_mut(did) {
            node.active_tasks += 1;
            node.total_tasks += 1;
        }
    }

    pub fn record_task_complete(&mut self, did: &str) {
        if let Some(node) = self.nodes.get_mut(did) {
            if node.active_tasks > 0 {
                node.active_tasks -= 1;
            }
        }
    }

    pub fn select_node(&self) -> Option<String> {
        let available: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.active_tasks < n.capacity)
            .collect();

        if available.is_empty() {
            return None;
        }

        match self.strategy {
            BalanceStrategy::RoundRobin => available.first().map(|(did, _)| (*did).clone()),
            BalanceStrategy::LeastConnections => available
                .into_iter()
                .min_by_key(|(_, n)| n.active_tasks)
                .map(|(did, _)| did.clone()),
            BalanceStrategy::LeastResponseTime => available
                .into_iter()
                .min_by(|a, b| {
                    // 比较利用率 active/capacity：用交叉相乘避免浮点与 NaN
                    // （available 已保证 capacity > 0）
                    let lhs = a.1.active_tasks.saturating_mul(b.1.capacity);
                    let rhs = b.1.active_tasks.saturating_mul(a.1.capacity);
                    lhs.cmp(&rhs)
                })
                .map(|(did, _)| did.clone()),
        }
    }

    pub fn average_utilization(&self) -> f64 {
        // 仅纳入 capacity > 0 的节点，避免 0/0 = NaN
        let usable: Vec<&NodeLoad> = self.nodes.values().filter(|n| n.capacity > 0).collect();
        if usable.is_empty() {
            return 0.0;
        }
        let sum: f64 = usable
            .iter()
            .map(|n| n.active_tasks as f64 / n.capacity as f64)
            .sum();
        sum / usable.len() as f64
    }
}
