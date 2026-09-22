//! 网络拓扑图
//! 
//! 表示智能体网络的连接关系和演化

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct TopologyGraph {
    /// 节点集合
    nodes: HashSet<String>,
    /// 边集合（双向）
    edges: HashMap<String, HashSet<String>>,
}

impl TopologyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            edges: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, did: String) {
        self.nodes.insert(did);
    }

    pub fn remove_node(&mut self, did: &str) {
        self.nodes.remove(did);
        self.edges.remove(did);
        for neighbors in self.edges.values_mut() {
            neighbors.remove(did);
        }
    }

    pub fn add_edge(&mut self, from: String, to: String) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        
        self.edges.entry(from.clone()).or_insert_with(HashSet::new).insert(to.clone());
        self.edges.entry(to).or_insert_with(HashSet::new).insert(from);
    }

    pub fn remove_edge(&mut self, from: &str, to: &str) {
        if let Some(neighbors) = self.edges.get_mut(from) {
            neighbors.remove(to);
        }
        if let Some(neighbors) = self.edges.get_mut(to) {
            neighbors.remove(from);
        }
    }

    pub fn neighbors_of(&self, did: &str) -> Vec<&String> {
        self.edges.get(did)
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|s| s.len()).sum::<usize>() / 2
    }

    /// 平均度数
    pub fn avg_degree(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        (self.edge_count() * 2) as f64 / self.nodes.len() as f64
    }

    /// 网络密度
    pub fn density(&self) -> f64 {
        let n = self.nodes.len();
        if n < 2 {
            return 0.0;
        }
        let max_edges = n * (n - 1) / 2;
        self.edge_count() as f64 / max_edges as f64
    }

    /// BFS 最短路径长度
    pub fn shortest_path(&self, from: &str, to: &str) -> Option<usize> {
        if from == to {
            return Some(0);
        }

        let mut visited = HashSet::new();
        let mut queue = vec![(from, 0)];

        while let Some((node, depth)) = queue.first().cloned() {
            queue.remove(0);
            
            if node == to {
                return Some(depth);
            }

            if visited.contains(node) {
                continue;
            }
            visited.insert(node);

            if let Some(neighbors) = self.edges.get(node) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor.as_str()) {
                        queue.push((neighbor, depth + 1));
                    }
                }
            }
        }

        None
    }
}
