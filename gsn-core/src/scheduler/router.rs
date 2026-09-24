//! 任务路由器
//! 
//! 智能任务分配：根据能力、信誉、负载、距离选择最佳执行者

use std::collections::HashMap;
use crate::agent::Task;

#[derive(Debug, Clone)]
pub struct RouteScore {
    pub agent_did: String,
    pub score: f64,
    pub estimated_latency_ms: u64,
    pub estimated_cost: u64,
}

pub struct TaskRouter {
    /// 节点负载表
    loads: HashMap<String, u64>, // did -> active_tasks
    /// 节点延迟表
    latencies: HashMap<String, u64>, // did -> avg_latency_ms
    /// 最大并发任务
    max_concurrent: u64,
}

impl TaskRouter {
    pub fn new(max_concurrent: u64) -> Self {
        Self {
            loads: HashMap::new(),
            latencies: HashMap::new(),
            max_concurrent,
        }
    }

    pub fn register_node(&mut self, did: String, latency_ms: u64) {
        self.loads.insert(did.clone(), 0);
        self.latencies.insert(did, latency_ms);
    }

    pub fn update_load(&mut self, did: &str, active_tasks: u64) {
        self.loads.insert(did.to_string(), active_tasks);
    }

    pub fn assign_task(&mut self, task: &Task, candidates: &[String]) -> Option<RouteScore> {
        let mut best: Option<RouteScore> = None;
        let mut best_score = f64::NEG_INFINITY;

        for candidate in candidates {
            let load = self.loads.get(candidate).copied().unwrap_or(0);
            
            // 负载检查
            if load >= self.max_concurrent {
                continue;
            }

            let latency = self.latencies.get(candidate).copied().unwrap_or(100);
            
            // 评分公式：信誉(外部) / 负载惩罚 / 延迟惩罚
            let load_penalty = (load as f64 / self.max_concurrent as f64) * 0.3;
            let latency_penalty = (latency as f64 / 1000.0).min(1.0) * 0.2;
            let score = 1.0 - load_penalty - latency_penalty;

            if score > best_score {
                best_score = score;
                best = Some(RouteScore {
                    agent_did: candidate.clone(),
                    score,
                    estimated_latency_ms: latency,
                    estimated_cost: task.budget / candidates.len() as u64,
                });
            }
        }

        if let Some(ref score) = best {
            self.loads.entry(score.agent_did.clone()).and_modify(|l| *l += 1);
        }

        best
    }

    pub fn release_task(&mut self, did: &str) {
        if let Some(load) = self.loads.get_mut(did) {
            if *load > 0 {
                *load -= 1;
            }
        }
    }

    pub fn node_load(&self, did: &str) -> u64 {
        self.loads.get(did).copied().unwrap_or(0)
    }
}
