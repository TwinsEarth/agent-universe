//! 分布式推理与算力调度
//!
//! 整合分散 GPU 算力，KV Cache 分片，边缘计算节点参与
//! 节点既是资源提供者又是消费者

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 算力资源声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResource {
    pub node_id: String,
    pub gpu_model: String,
    pub vram_gb: u32,
    pub cpus: u32,
    pub ram_gb: u32,
    pub bandwidth_mbps: u32,
    pub available: bool,
    pub kv_cache_shards: Vec<KvCacheShard>,
}

/// KV Cache 分片
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvCacheShard {
    pub shard_id: String,
    pub model_hash: String,
    pub layer_start: u32,
    pub layer_end: u32,
    pub size_mb: u32,
}

/// 推理任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceTask {
    pub task_id: String,
    pub model_hash: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub required_vram_gb: u32,
    pub timeout_secs: u32,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
}

/// 算力调度器
pub struct ComputeScheduler {
    resources: HashMap<String, ComputeResource>,
    tasks: HashMap<String, InferenceTask>,
    assignments: HashMap<String, String>, // task_id -> node_id
}

impl ComputeScheduler {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            tasks: HashMap::new(),
            assignments: HashMap::new(),
        }
    }

    pub fn register_resource(&mut self, resource: ComputeResource) {
        self.resources.insert(resource.node_id.clone(), resource);
    }

    pub fn submit_task(&mut self, task: InferenceTask) -> String {
        let task_id = task.task_id.clone();
        self.tasks.insert(task_id.clone(), task);
        task_id
    }

    /// 简单调度：选择第一个 VRAM 足够的可用节点
    pub fn assign_task(&mut self, task_id: String) -> Option<String> {
        let task = self.tasks.get(&task_id)?;
        let required = task.required_vram_gb;

        for (node_id, resource) in &self.resources {
            if resource.available && resource.vram_gb >= required {
                self.assignments.insert(task_id.clone(), node_id.clone());
                return Some(node_id.clone());
            }
        }
        None
    }

    pub fn get_assignment(&self, task_id: &str) -> Option<&String> {
        self.assignments.get(task_id)
    }

    pub fn available_nodes(&self) -> usize {
        self.resources.values().filter(|r| r.available).count()
    }

    pub fn total_vram_gb(&self) -> u32 {
        self.resources.values().map(|r| r.vram_gb).sum()
    }
}
