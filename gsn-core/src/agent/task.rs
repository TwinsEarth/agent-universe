use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Verified,
    Settled,
    Failed,
}

/// 任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub requester: String,
    pub capability: String,
    pub payload: serde_json::Value,
    pub executor: Option<String>,
    pub result: Option<serde_json::Value>,
    pub status: TaskStatus,
    pub budget: u64,
    pub created_at: u64,
}

impl Task {
    pub fn new(requester: String, capability: String, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            requester,
            capability,
            payload,
            executor: None,
            result: None,
            status: TaskStatus::Pending,
            budget: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn assign(&mut self, executor: String) {
        self.executor = Some(executor);
        self.status = TaskStatus::Assigned;
    }

    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
    }

    pub fn complete(&mut self, result: serde_json::Value) {
        self.result = Some(result);
        self.status = TaskStatus::Completed;
    }

    pub fn verify(&mut self) {
        self.status = TaskStatus::Verified;
    }

    pub fn settle(&mut self) {
        self.status = TaskStatus::Settled;
    }

    pub fn fail(&mut self) {
        self.status = TaskStatus::Failed;
    }
}
