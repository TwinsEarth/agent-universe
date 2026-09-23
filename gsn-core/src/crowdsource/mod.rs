//! 去中心化任务众包
//!
//! 类似 Clawrma：Agent 发布任务，Solver 执行，赚取积分
//! "以工换工"自我维持模式

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 众包任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrowdTask {
    pub task_id: String,
    pub publisher: String,
    pub title: String,
    pub description: String,
    pub task_type: TaskType,
    pub reward_points: u32,
    pub deadline: u64,
    pub status: CrowdTaskStatus,
    pub solver: Option<String>,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    WebScraping,
    Screenshot,
    Inference,
    DataLabeling,
    Translation,
    Research,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrowdTaskStatus {
    Open,
    Assigned,
    InProgress,
    Completed,
    Cancelled,
}

/// Solver 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solver {
    pub solver_id: String,
    pub name: String,
    pub reputation: f32,
    pub completed_tasks: u32,
    pub success_rate: f32,
    pub available: bool,
    pub points: i64,
}

/// 众包市场
pub struct CrowdsourcingMarket {
    tasks: HashMap<String, CrowdTask>,
    solvers: HashMap<String, Solver>,
    task_queue: Vec<String>,
}

impl CrowdsourcingMarket {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            solvers: HashMap::new(),
            task_queue: Vec::new(),
        }
    }

    pub fn publish_task(&mut self, task: CrowdTask) {
        self.task_queue.push(task.task_id.clone());
        self.tasks.insert(task.task_id.clone(), task);
    }

    pub fn register_solver(&mut self, solver: Solver) {
        self.solvers.insert(solver.solver_id.clone(), solver);
    }

    pub fn accept_task(&mut self, task_id: String, solver_id: String) -> bool {
        if let Some(task) = self.tasks.get_mut(&task_id) {
            if task.status == CrowdTaskStatus::Open {
                task.status = CrowdTaskStatus::Assigned;
                task.solver = Some(solver_id);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn complete_task(&mut self, task_id: String, result: String) -> bool {
        if let Some(task) = self.tasks.get_mut(&task_id) {
            task.status = CrowdTaskStatus::Completed;
            task.result = Some(result);

            // 奖励积分
            if let Some(solver_id) = &task.solver {
                if let Some(solver) = self.solvers.get_mut(solver_id) {
                    solver.points += task.reward_points as i64;
                    solver.completed_tasks += 1;
                }
            }
            true
        } else {
            false
        }
    }

    pub fn open_tasks(&self) -> Vec<&CrowdTask> {
        self.tasks.values()
            .filter(|t| t.status == CrowdTaskStatus::Open)
            .collect()
    }

    pub fn total_solvers(&self) -> usize {
        self.solvers.len()
    }

    pub fn get_solver(&self, solver_id: &str) -> Option<&Solver> {
        self.solvers.get(solver_id)
    }
}
