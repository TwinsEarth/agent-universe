//! 真实 SQLite 持久化
//!
//! 将 Agent 注册信息和 Task 状态持久化到磁盘 SQLite 数据库

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

/// SQLite 持久化存储
pub struct PersistentStore {
    conn: Mutex<Connection>,
}

/// 持久化的 Agent 记录
#[derive(Debug, Clone)]
pub struct StoredAgent {
    pub agent_id: String,
    pub name: String,
    pub skills: String,
    pub stake: f64,
    pub reputation: f64,
    pub created_at: String,
}

/// 持久化的 Task 记录
#[derive(Debug, Clone)]
pub struct StoredTask {
    pub task_id: String,
    pub goal: String,
    pub state: String,
    pub owner: Option<String>,
    pub budget: f64,
    pub created_at: String,
}

impl PersistentStore {
    /// 打开（或创建）数据库文件，自动建表
    pub fn open<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        // 确保父目录存在
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agents (
                agent_id   TEXT PRIMARY KEY,
                name       TEXT NOT NULL,
                skills     TEXT NOT NULL DEFAULT '',
                stake      REAL NOT NULL DEFAULT 0,
                reputation REAL NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tasks (
                task_id    TEXT PRIMARY KEY,
                goal       TEXT NOT NULL,
                state      TEXT NOT NULL,
                owner      TEXT,
                budget     REAL NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS kv_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 插入或更新 Agent
    pub fn upsert_agent(&self, agent: &StoredAgent) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agents (agent_id, name, skills, stake, reputation, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(agent_id) DO UPDATE SET
                name = excluded.name,
                skills = excluded.skills,
                stake = excluded.stake,
                reputation = excluded.reputation",
            params![
                agent.agent_id,
                agent.name,
                agent.skills,
                agent.stake,
                agent.reputation,
                agent.created_at,
            ],
        )?;
        Ok(())
    }

    /// 读取全部 Agent
    pub fn load_agents(&self) -> anyhow::Result<Vec<StoredAgent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT agent_id, name, skills, stake, reputation, created_at FROM agents",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredAgent {
                agent_id: row.get(0)?,
                name: row.get(1)?,
                skills: row.get(2)?,
                stake: row.get(3)?,
                reputation: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut agents = Vec::new();
        for r in rows {
            agents.push(r?);
        }
        Ok(agents)
    }

    /// 插入或更新 Task
    pub fn upsert_task(&self, task: &StoredTask) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tasks (task_id, goal, state, owner, budget, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(task_id) DO UPDATE SET
                goal = excluded.goal,
                state = excluded.state,
                owner = excluded.owner,
                budget = excluded.budget",
            params![
                task.task_id,
                task.goal,
                task.state,
                task.owner,
                task.budget,
                task.created_at,
            ],
        )?;
        Ok(())
    }

    /// 读取全部 Task
    pub fn load_tasks(&self) -> anyhow::Result<Vec<StoredTask>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT task_id, goal, state, owner, budget, created_at FROM tasks",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredTask {
                task_id: row.get(0)?,
                goal: row.get(1)?,
                state: row.get(2)?,
                owner: row.get(3)?,
                budget: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut tasks = Vec::new();
        for r in rows {
            tasks.push(r?);
        }
        Ok(tasks)
    }

    /// 写入键值元数据（如节点 DID）
    pub fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO kv_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// 读取键值元数据
    pub fn get_meta(&self, key: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM kv_meta WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        if let Some(r) = rows.next() {
            return Ok(Some(r?));
        }
        Ok(None)
    }

    /// Agent 总数
    pub fn agent_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM agents", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Task 总数
    pub fn task_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))?;
        Ok(count)
    }
}
