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
        self.nodes.insert(did, NodeLoad {
            active_tasks: 0,
            total_tasks: 0,
            capacity,
        });
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
        let available: Vec<_> = self.nodes.iter()
            .filter(|(_, n)| n.active_tasks < n.capacity)
            .collect();

        if available.is_empty() {
            return None;
        }

        match self.strategy {
            BalanceStrategy::RoundRobin => {
                available.first().map(|(did, _)| (*did).clone())
            }
            BalanceStrategy::LeastConnections => {
                available.into_iter()
                    .min_by_key(|(_, n)| n.active_tasks)
                    .map(|(did, _)| did.clone())
            }
            BalanceStrategy::LeastResponseTime => {
                available.into_iter()
                    .min_by(|a, b| {
                        let a_ratio = a.1.active_tasks as f64 / a.1.capacity as f64;
                        let b_ratio = b.1.active_tasks as f64 / b.1.capacity as f64;
                        a_ratio.partial_cmp(&b_ratio).unwrap()
                    })
                    .map(|(did, _)| did.clone())
            }
        }
    }

    pub fn average_utilization(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.nodes.values()
            .map(|n| n.active_tasks as f64 / n.capacity as f64)
            .sum();
        sum / self.nodes.len() as f64
    }
}
