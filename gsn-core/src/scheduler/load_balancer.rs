//! 负载均衡器

use std::cell::Cell;
use std::collections::HashMap;

pub struct LoadBalancer {
    nodes: HashMap<String, NodeLoad>,
    strategy: BalanceStrategy,
    /// RoundRobin 轮转游标（仅 RoundRobin 策略使用）。
    ///
    /// 用 `Cell` 做内部可变性，使 `select_node(&self)` 的对外签名保持不变；
    /// 游标每次取节点后 +1，对"当前可用节点数"取模得到本次下标。
    rr_cursor: Cell<usize>,
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
            rr_cursor: Cell::new(0),
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
        let mut available: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.active_tasks < n.capacity)
            .collect();

        if available.is_empty() {
            return None;
        }

        match self.strategy {
            BalanceStrategy::RoundRobin => {
                // 确定性排序：HashMap 迭代序在每次构建间随机化，
                // 不排序则"轮转"没有稳定序，游标指向会随随机序乱跳。
                available.sort_by_key(|(did, _)| *did);
                let n = available.len();
                let idx = self.rr_cursor.get() % n;
                // 取到即推进游标（下次调用再对那时的可用数取模，自动跳过已满载节点）。
                self.rr_cursor.set(self.rr_cursor.get() + 1);
                Some(available[idx].0.clone())
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_robin_cycles_instead_of_picking_first() {
        let mut lb = LoadBalancer::new(BalanceStrategy::RoundRobin);
        lb.add_node("a".into(), 10);
        lb.add_node("b".into(), 10);
        lb.add_node("c".into(), 10);
        // 连续 6 次必须轮转 a,b,c,a,b,c，而不是固定命中首项 a。
        let seq: Vec<String> = (0..6).map(|_| lb.select_node().unwrap()).collect();
        assert_eq!(seq, vec!["a", "b", "c", "a", "b", "c"]);
    }

    #[test]
    fn round_robin_skips_unavailable_node() {
        let mut lb = LoadBalancer::new(BalanceStrategy::RoundRobin);
        lb.add_node("a".into(), 1);
        lb.add_node("b".into(), 1);
        lb.add_node("c".into(), 1);
        // 占满 a 的容量 → a 不再满足 active < capacity，必须被跳过。
        lb.record_task_start("a");
        let seq: Vec<String> = (0..4).map(|_| lb.select_node().unwrap()).collect();
        // 排序后可用集合为 [b, c]，游标在二者间轮转。
        assert_eq!(seq, vec!["b", "c", "b", "c"]);
    }

    #[test]
    fn round_robin_none_when_all_full() {
        let mut lb = LoadBalancer::new(BalanceStrategy::RoundRobin);
        lb.add_node("a".into(), 1);
        lb.record_task_start("a");
        assert!(lb.select_node().is_none());
    }

    #[test]
    fn round_robin_single_node_stable() {
        let mut lb = LoadBalancer::new(BalanceStrategy::RoundRobin);
        lb.add_node("only".into(), 5);
        for _ in 0..5 {
            assert_eq!(lb.select_node().as_deref(), Some("only"));
        }
    }
}
