//! 真实 SQLite 持久化
//!
//! 将 Agent 注册信息和 Task 状态持久化到磁盘 SQLite 数据库

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

use crate::marketplace::SettlementRecord;

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
    pub stake: i64,
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
    pub budget: i64,
    pub created_at: String,
}

/// v2.5.5: 持久化的 Relay 节点记录
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoredRelay {
    pub relay_id: String,
    pub multiaddr: String,
    /// 分类：dedicated(专用) / self_hosted(自有) / third_party(第三方) / general(通用)
    pub class: String,
    /// 状态：healthy / dead / unknown / active
    pub status: String,
    pub healthy: bool,
    pub fail_count: i64,
    pub limit_sec: i64,
    pub data_bytes: i64,
    pub last_check: String,
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
                stake      INTEGER NOT NULL DEFAULT 0,
                reputation REAL NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tasks (
                task_id    TEXT PRIMARY KEY,
                goal       TEXT NOT NULL,
                state      TEXT NOT NULL,
                owner      TEXT,
                budget     INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS kv_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- v2.5.5: Circuit Relay 节点池
            CREATE TABLE IF NOT EXISTS relays (
                relay_id   TEXT PRIMARY KEY,
                multiaddr  TEXT NOT NULL,
                class      TEXT NOT NULL DEFAULT 'general',
                status     TEXT NOT NULL DEFAULT 'unknown',
                healthy    INTEGER NOT NULL DEFAULT 0,
                fail_count INTEGER NOT NULL DEFAULT 0,
                limit_sec  INTEGER NOT NULL DEFAULT 0,
                data_bytes INTEGER NOT NULL DEFAULT 0,
                last_check TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL
            );

            -- v2.6.1: 只追加结算流水（账本落盘，重启可重放恢复，GAP §6.1）
            CREATE TABLE IF NOT EXISTS ledger_entries (
                seq     INTEGER PRIMARY KEY AUTOINCREMENT,
                payload TEXT NOT NULL
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

    // ─────────────── v2.5.5 Relay 节点池 ───────────────

    /// 插入或更新 relay（不存在则插入；已存在则更新地址/分类，保留健康统计）
    pub fn upsert_relay(&self, relay: &StoredRelay) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO relays (relay_id, multiaddr, class, status, healthy, fail_count,
                                 limit_sec, data_bytes, last_check, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(relay_id) DO UPDATE SET
                multiaddr  = excluded.multiaddr,
                class      = excluded.class,
                status     = excluded.status,
                healthy    = excluded.healthy,
                fail_count = excluded.fail_count,
                limit_sec  = excluded.limit_sec,
                data_bytes = excluded.data_bytes,
                last_check = excluded.last_check",
            params![
                relay.relay_id,
                relay.multiaddr,
                relay.class,
                relay.status,
                relay.healthy as i64,
                relay.fail_count,
                relay.limit_sec,
                relay.data_bytes,
                relay.last_check,
                relay.created_at,
            ],
        )?;
        Ok(())
    }

    /// 读取全部 relay
    pub fn load_relays(&self) -> anyhow::Result<Vec<StoredRelay>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT relay_id, multiaddr, class, status, healthy, fail_count, limit_sec,
                    data_bytes, last_check, created_at FROM relays",
        )?;
        let rows = stmt.query_map([], row_to_relay)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// 按分类读取 relay
    pub fn load_relays_by_class(&self, class: &str) -> anyhow::Result<Vec<StoredRelay>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT relay_id, multiaddr, class, status, healthy, fail_count, limit_sec,
                    data_bytes, last_check, created_at FROM relays WHERE class = ?1",
        )?;
        let rows = stmt.query_map(params![class], row_to_relay)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// 读取全部健康 relay（可用于建立 reservation）
    pub fn healthy_relays(&self) -> anyhow::Result<Vec<StoredRelay>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT relay_id, multiaddr, class, status, healthy, fail_count, limit_sec,
                    data_bytes, last_check, created_at FROM relays WHERE healthy = 1",
        )?;
        let rows = stmt.query_map([], row_to_relay)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// 更新 relay 健康状态（巡检结果）
    pub fn set_relay_status(
        &self,
        relay_id: &str,
        healthy: bool,
        status: &str,
        limit_sec: i64,
        data_bytes: i64,
        last_check: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE relays SET healthy = ?2, status = ?3, limit_sec = ?4,
                              data_bytes = ?5, last_check = ?6
             WHERE relay_id = ?1",
            params![relay_id, healthy as i64, status, limit_sec, data_bytes, last_check],
        )?;
        Ok(())
    }

    /// 巡检失败：fail_count 自增，达到阈值（3）则标记 dead
    pub fn mark_relay_failed(&self, relay_id: &str, last_check: &str) -> anyhow::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE relays SET fail_count = fail_count + 1, last_check = ?2 WHERE relay_id = ?1",
            params![relay_id, last_check],
        )?;
        let fail_count: i64 =
            conn.query_row("SELECT fail_count FROM relays WHERE relay_id = ?1", params![relay_id], |r| r.get(0))?;
        if fail_count >= 3 {
            conn.execute(
                "UPDATE relays SET healthy = 0, status = 'dead' WHERE relay_id = ?1",
                params![relay_id],
            )?;
        }
        Ok(fail_count)
    }

    /// 删除 relay
    pub fn delete_relay(&self, relay_id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM relays WHERE relay_id = ?1", params![relay_id])?;
        Ok(())
    }

    /// 删除全部 dead relay（清理过期失效节点），返回删除条数
    pub fn delete_dead_relays(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM relays WHERE status = 'dead'", [])?;
        Ok(n as u64)
    }

    /// relay 总数
    pub fn relay_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM relays", [], |row| row.get(0))?;
        Ok(count)
    }

    /// 健康 relay 数
    pub fn healthy_relay_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 =
            conn.query_row("SELECT COUNT(*) FROM relays WHERE healthy = 1", [], |row| row.get(0))?;
        Ok(count)
    }

    // ─────────────── v2.6.1 账本落盘（GAP §6.1）───────────────

    /// 追加一条结算流水（只追加，不可变；SQLite 事务保证不会出现半行）
    pub fn append_ledger_record(&self, rec: &SettlementRecord) -> anyhow::Result<()> {
        let payload = serde_json::to_string(rec)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO ledger_entries (payload) VALUES (?1)",
            params![payload],
        )?;
        Ok(())
    }

    /// 按序读回全部结算流水（用于重放恢复）。
    /// 损坏 / 无法解析的行被容错跳过，不使整个账本不可读。
    pub fn load_ledger_records(&self) -> anyhow::Result<Vec<SettlementRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT payload FROM ledger_entries ORDER BY seq ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            let payload = r?;
            if let Ok(rec) = serde_json::from_str::<SettlementRecord>(&payload) {
                out.push(rec);
            }
        }
        Ok(out)
    }

    /// 已持久化流水条数（持久化水位）
    pub fn ledger_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 =
            conn.query_row("SELECT COUNT(*) FROM ledger_entries", [], |row| row.get(0))?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::{Money, SettlementReason};

    fn tmp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("gsn_persist_v261_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("gsn.db")
    }

    fn sample_agent(id: &str, name: &str) -> StoredAgent {
        StoredAgent {
            agent_id: id.into(),
            name: name.into(),
            skills: "a,b".into(),
            stake: 100,
            reputation: 0.5,
            created_at: "10".into(),
        }
    }

    fn sample_task(id: &str, state: &str) -> StoredTask {
        StoredTask {
            task_id: id.into(),
            goal: "goal".into(),
            state: state.into(),
            owner: Some("owner".into()),
            budget: 200,
            created_at: "10".into(),
        }
    }

    #[test]
    fn agent_task_roundtrip_after_reopen() {
        let path = tmp_db("rt");
        {
            let s = PersistentStore::open(&path).unwrap();
            s.upsert_agent(&sample_agent("a1", "A")).unwrap();
            s.upsert_task(&sample_task("t1", "Open")).unwrap();
        } // drop
        let s = PersistentStore::open(&path).unwrap();
        let agents = s.load_agents().unwrap();
        let tasks = s.load_tasks().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].agent_id, "a1");
        assert_eq!(agents[0].name, "A");
        assert_eq!(agents[0].skills, "a,b");
        assert_eq!(agents[0].stake, 100);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, "t1");
        assert_eq!(tasks[0].state, "Open");
        assert_eq!(tasks[0].owner.as_deref(), Some("owner"));
        assert_eq!(tasks[0].budget, 200);
    }

    #[test]
    fn same_id_last_write_wins() {
        let path = tmp_db("ow");
        let s = PersistentStore::open(&path).unwrap();
        s.upsert_agent(&sample_agent("a1", "A")).unwrap();
        let mut later = sample_agent("a1", "B");
        later.stake = 250;
        s.upsert_agent(&later).unwrap();
        let agents = s.load_agents().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "B");
        assert_eq!(agents[0].stake, 250);
    }

    #[test]
    fn ledger_append_load_and_skip_corrupt() {
        let path = tmp_db("led");
        {
            let s = PersistentStore::open(&path).unwrap();
            let rec = SettlementRecord {
                task_id: "deposit:a1".into(),
                from_account: String::new(),
                to_account: "a1".into(),
                amount: Money::new(10),
                reason: SettlementReason::Deposited,
                timestamp: 5,
            };
            s.append_ledger_record(&rec).unwrap();
            assert_eq!(s.ledger_count().unwrap(), 1);
            // 直接插一条损坏 payload（崩溃 / 脏行模拟）
            let conn = s.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO ledger_entries (payload) VALUES ('{not json')",
                [],
            )
            .unwrap();
        }
        let s = PersistentStore::open(&path).unwrap();
        let recs = s.load_ledger_records().unwrap();
        assert_eq!(recs.len(), 1, "损坏行应被容错跳过");
        assert_eq!(recs[0].amount, Money::new(10));
        assert_eq!(s.ledger_count().unwrap(), 2, "物理行数含损坏行");
    }
}

/// 行映射：relay
fn row_to_relay(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRelay> {
    Ok(StoredRelay {
        relay_id: row.get(0)?,
        multiaddr: row.get(1)?,
        class: row.get(2)?,
        status: row.get(3)?,
        healthy: row.get::<_, i64>(4)? != 0,
        fail_count: row.get(5)?,
        limit_sec: row.get(6)?,
        data_bytes: row.get(7)?,
        last_check: row.get(8)?,
        created_at: row.get(9)?,
    })
}
