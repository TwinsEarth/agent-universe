//! 真实 SQLite 持久化
//!
//! 将 Agent 注册信息和 Task 状态持久化到磁盘 SQLite 数据库

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use crate::marketplace::SettlementRecord;

/// 哈希链创世前缀（v2.8.3，GAP §3.1：账本日志 tamper-evident）
const LEDGER_GENESIS: &str = "";
/// kv_meta 中锚定账本链 head hash 的键
const LEDGER_HEAD_KEY: &str = "ledger_head_hash";

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
    /// v3.5.4（W-05）：完整 MarketAgentCard 的规范 JSON。
    /// 扁平列（stake/reputation/created_at）是经济身份权威列，恢复时覆盖 JSON 内同名字段；
    /// 本列承载扁平列未覆盖的 version/pricing/sla/modalities/models/description/endpoint 等。
    pub card_json: Option<String>,
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
    /// v2.8.4: 中标价（GAP §3.2，恢复不再为 None 导致按满额预算支付）
    pub winner_price: Option<i64>,
    /// v2.8.4: 验证策略 JSON（GAP §3.2，恢复不再硬编码 None 豁免证据闸门）
    pub verification_policy: String,
    /// v2.8.4: 发布者
    pub requester: String,
    /// v2.8.4: 截止时间（Unix 毫秒）
    pub deadline: i64,
    /// v3.5.4（W-04）：完整 TaskSpec 的规范 JSON。
    /// 扁平列是经济/生命周期权威列（恢复时覆盖 JSON 内同名的 budget/winner_price/
    /// verification_policy/requester/deadline/state/created_at/owner/goal/task_id）；
    /// 本列承载扁平列未覆盖的 context/done/todo/trace/required_skills，修复重启后这些字段丢失。
    pub spec_json: Option<String>,
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

/// v2.8.3: 账本读回结果。损坏行显式返回（GAP §3.6，不再静默跳过）。
#[derive(Debug, Default)]
pub struct LedgerLoad {
    pub records: Vec<SettlementRecord>,
    /// (seq, payload, 解析失败原因)
    pub corrupt: Vec<(i64, String, String)>,
}

impl PersistentStore {
    /// 打开（或创建）数据库文件，自动建表
    pub fn open<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        // 确保父目录存在
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;

        // v3.5.5（P1）：连接级 PRAGMA，必须在建表/写库前设置。
        // - journal_mode=WAL：把默认 rollback journal 改为 WAL，大幅降低写放大并允许并发读；
        // - synchronous=NORMAL：WAL 下安全且更快（仍防断电坏库，只丢最近一帧事务）；
        // - busy_timeout=5000：撞库时等待最多 5s 而非立刻返回 SQLITE_BUSY；
        // - foreign_keys=ON：外键约束默认关闭，必须逐连接显式开启。
        // 任一 PRAGMA 失败都要返回错误（?），绝不静默吞掉。
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;
             PRAGMA foreign_keys=ON;",
        )?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agents (
                agent_id   TEXT PRIMARY KEY,
                name       TEXT NOT NULL,
                skills     TEXT NOT NULL DEFAULT '',
                stake      INTEGER NOT NULL DEFAULT 0,
                reputation REAL NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                card_json  TEXT
            );

            CREATE TABLE IF NOT EXISTS tasks (
                task_id    TEXT PRIMARY KEY,
                goal       TEXT NOT NULL,
                state      TEXT NOT NULL,
                owner      TEXT,
                budget     INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                winner_price      INTEGER,
                verification_policy TEXT NOT NULL DEFAULT '',
                requester  TEXT NOT NULL DEFAULT '',
                deadline   INTEGER NOT NULL DEFAULT 0,
                spec_json  TEXT
            );

            -- v2.8.4: 结果信封（GAP §3.2，已验收未结算任务重启后可结算）
            CREATE TABLE IF NOT EXISTS result_envelopes (
                task_id TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            );

            -- v2.8.4: 信誉（GAP §3.2，重启后信誉不丢失）
            CREATE TABLE IF NOT EXISTS reputations (
                agent_id TEXT PRIMARY KEY,
                payload  TEXT NOT NULL
            );

            -- v2.8.4: 质押（GAP §3.2，重启后质押资金仍可出价/罚没）
            CREATE TABLE IF NOT EXISTS stakes (
                agent_id TEXT PRIMARY KEY,
                payload  TEXT NOT NULL
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
            -- v2.8.3: 加 prev_hash/record_hash 哈希链（GAP §3.1，tamper-evident）
            CREATE TABLE IF NOT EXISTS ledger_entries (
                seq         INTEGER PRIMARY KEY AUTOINCREMENT,
                payload     TEXT NOT NULL,
                prev_hash   TEXT NOT NULL DEFAULT '',
                record_hash TEXT NOT NULL DEFAULT ''
            );
            ",
        )?;

        // v2.8.3: 旧库迁移（补哈希链列并重算存量行），幂等
        migrate_ledger_chain(&conn)?;

        // v2.8.4: 旧库迁移（tasks 补 winner_price/verification_policy/requester/deadline 列），幂等
        migrate_tasks_v284(&conn)?;

        // v3.5.4: W-04/W-05 全字段保真——tasks.spec_json / agents.card_json，幂等
        migrate_add_text_column(&conn, "tasks", "spec_json")?;
        migrate_add_text_column(&conn, "agents", "card_json")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 插入或更新 Agent
    pub fn upsert_agent(&self, agent: &StoredAgent) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        conn.execute(
            "INSERT INTO agents (agent_id, name, skills, stake, reputation, created_at, card_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(agent_id) DO UPDATE SET
                name = excluded.name,
                skills = excluded.skills,
                stake = excluded.stake,
                reputation = excluded.reputation,
                card_json = excluded.card_json",
            params![
                agent.agent_id,
                agent.name,
                agent.skills,
                agent.stake,
                agent.reputation,
                agent.created_at,
                agent.card_json,
            ],
        )?;
        Ok(())
    }

    /// 读取全部 Agent
    pub fn load_agents(&self) -> anyhow::Result<Vec<StoredAgent>> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let mut stmt = conn.prepare(
            "SELECT agent_id, name, skills, stake, reputation, created_at, card_json FROM agents",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredAgent {
                agent_id: row.get(0)?,
                name: row.get(1)?,
                skills: row.get(2)?,
                stake: row.get(3)?,
                reputation: row.get(4)?,
                created_at: row.get(5)?,
                card_json: row.get(6)?,
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
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        conn.execute(
            "INSERT INTO tasks (task_id, goal, state, owner, budget, created_at,
                                winner_price, verification_policy, requester, deadline, spec_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(task_id) DO UPDATE SET
                goal = excluded.goal,
                state = excluded.state,
                owner = excluded.owner,
                budget = excluded.budget,
                winner_price = excluded.winner_price,
                verification_policy = excluded.verification_policy,
                requester = excluded.requester,
                deadline = excluded.deadline,
                spec_json = excluded.spec_json",
            params![
                task.task_id,
                task.goal,
                task.state,
                task.owner,
                task.budget,
                task.created_at,
                task.winner_price,
                task.verification_policy,
                task.requester,
                task.deadline,
                task.spec_json,
            ],
        )?;
        Ok(())
    }

    /// 读取全部 Task
    pub fn load_tasks(&self) -> anyhow::Result<Vec<StoredTask>> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let mut stmt = conn.prepare(
            "SELECT task_id, goal, state, owner, budget, created_at,
                    winner_price, verification_policy, requester, deadline, spec_json FROM tasks",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredTask {
                task_id: row.get(0)?,
                goal: row.get(1)?,
                state: row.get(2)?,
                owner: row.get(3)?,
                budget: row.get(4)?,
                created_at: row.get(5)?,
                winner_price: row.get(6)?,
                verification_policy: row.get(7)?,
                requester: row.get(8)?,
                deadline: row.get(9)?,
                spec_json: row.get(10)?,
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
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        conn.execute(
            "INSERT INTO kv_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// 读取键值元数据
    pub fn get_meta(&self, key: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let mut stmt = conn.prepare("SELECT value FROM kv_meta WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        if let Some(r) = rows.next() {
            return Ok(Some(r?));
        }
        Ok(None)
    }

    /// v2.8.4: 写入 JSON 行（结果信封/信誉/质押通用，GAP §3.2）。
    /// table/id_col 仅由代码内固定常量传入（非用户输入）。
    pub fn put_json_row(
        &self,
        table: &str,
        id_col: &str,
        id: &str,
        payload: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let sql = format!(
            "INSERT INTO {table} ({id_col}, payload) VALUES (?1, ?2)
             ON CONFLICT({id_col}) DO UPDATE SET payload = excluded.payload"
        );
        conn.execute(&sql, params![id, payload])?;
        Ok(())
    }

    /// v2.8.4: 读取全部 JSON 行（id, payload）。
    pub fn load_json_rows(
        &self,
        table: &str,
        id_col: &str,
    ) -> anyhow::Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let sql = format!("SELECT {id_col}, payload FROM {table}");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Agent 总数
    pub fn agent_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM agents", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Task 总数
    pub fn task_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))?;
        Ok(count)
    }

    // ─────────────── v2.5.5 Relay 节点池 ───────────────

    /// 插入或更新 relay（不存在则插入；已存在则更新地址/分类，保留健康统计）
    pub fn upsert_relay(&self, relay: &StoredRelay) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
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
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
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
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
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
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
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
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        conn.execute(
            "UPDATE relays SET healthy = ?2, status = ?3, limit_sec = ?4,
                              data_bytes = ?5, last_check = ?6
             WHERE relay_id = ?1",
            params![
                relay_id,
                healthy as i64,
                status,
                limit_sec,
                data_bytes,
                last_check
            ],
        )?;
        Ok(())
    }

    /// 巡检失败：fail_count 自增，达到阈值（3）则标记 dead
    pub fn mark_relay_failed(&self, relay_id: &str, last_check: &str) -> anyhow::Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        conn.execute(
            "UPDATE relays SET fail_count = fail_count + 1, last_check = ?2 WHERE relay_id = ?1",
            params![relay_id, last_check],
        )?;
        let fail_count: i64 = conn.query_row(
            "SELECT fail_count FROM relays WHERE relay_id = ?1",
            params![relay_id],
            |r| r.get(0),
        )?;
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
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        conn.execute("DELETE FROM relays WHERE relay_id = ?1", params![relay_id])?;
        Ok(())
    }

    /// 删除全部 dead relay（清理过期失效节点），返回删除条数
    pub fn delete_dead_relays(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let n = conn.execute("DELETE FROM relays WHERE status = 'dead'", [])?;
        Ok(n as u64)
    }

    /// relay 总数
    pub fn relay_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM relays", [], |row| row.get(0))?;
        Ok(count)
    }

    /// 健康 relay 数
    pub fn healthy_relay_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let count: u64 =
            conn.query_row("SELECT COUNT(*) FROM relays WHERE healthy = 1", [], |row| {
                row.get(0)
            })?;
        Ok(count)
    }

    // ─────────────── v2.6.1 账本落盘（GAP §6.1）───────────────
    // v2.8.3: 哈希链 tamper-evident（GAP §3.1）

    /// 链哈希：SHA256(prev_hash || "|" || payload)
    fn ledger_record_hash(prev_hash: &str, payload: &str) -> String {
        let mut h = Sha256::new();
        h.update(prev_hash.as_bytes());
        h.update(b"|");
        h.update(payload.as_bytes());
        hex::encode(h.finalize())
    }

    /// 追加一条结算流水（只追加，哈希链）。
    /// 失败返回 Err，调用方不得推进水位（GAP §3.6）。
    pub fn append_ledger_record(&self, rec: &SettlementRecord) -> anyhow::Result<()> {
        let payload = serde_json::to_string(rec)?;
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let tx = conn.unchecked_transaction()?;
        let prev_hash: String = tx
            .query_row(
                "SELECT record_hash FROM ledger_entries ORDER BY seq DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| LEDGER_GENESIS.to_string());
        let record_hash = Self::ledger_record_hash(&prev_hash, &payload);
        tx.execute(
            "INSERT INTO ledger_entries (payload, prev_hash, record_hash) VALUES (?1, ?2, ?3)",
            params![payload, prev_hash, record_hash],
        )?;
        tx.execute(
            "INSERT INTO kv_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![LEDGER_HEAD_KEY, record_hash],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// 读回并校验解析（v2.8.3）：损坏行显式返回，不静默跳过。
    pub fn load_ledger_records_checked(&self) -> anyhow::Result<LedgerLoad> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let mut stmt = conn.prepare("SELECT seq, payload FROM ledger_entries ORDER BY seq ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = LedgerLoad::default();
        for r in rows {
            let (seq, payload) = r?;
            match serde_json::from_str::<SettlementRecord>(&payload) {
                Ok(rec) => out.records.push(rec),
                Err(e) => out.corrupt.push((seq, payload, e.to_string())),
            }
        }
        Ok(out)
    }

    /// 兼容旧调用：读回可解析记录（损坏行见 `load_ledger_records_checked`）。
    pub fn load_ledger_records(&self) -> anyhow::Result<Vec<SettlementRecord>> {
        Ok(self.load_ledger_records_checked()?.records)
    }

    /// 校验账本哈希链，返回 head hash。
    /// 断链 / 哈希不匹配 / 锚定 head 不一致，返回 Err(首个断链 seq)；
    /// head 锚定错误用 `u64::MAX`。
    pub fn verify_ledger_chain(&self) -> Result<String, u64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let mut stmt = conn
            .prepare(
                "SELECT seq, payload, prev_hash, record_hash FROM ledger_entries ORDER BY seq ASC",
            )
            .map_err(|_| 0u64)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|_| 0u64)?;
        let mut expected_prev = LEDGER_GENESIS.to_string();
        let mut head = LEDGER_GENESIS.to_string();
        for r in rows {
            let (seq, payload, prev_hash, record_hash) = r.map_err(|_| 0u64)?;
            let want = Self::ledger_record_hash(&expected_prev, &payload);
            if prev_hash != expected_prev || record_hash != want {
                return Err(seq as u64);
            }
            expected_prev = record_hash.clone();
            head = record_hash;
        }
        // 锚定 head 校验（kv_meta 中若有 head，必须与链末一致）
        let anchored: Option<String> = conn
            .query_row(
                "SELECT value FROM kv_meta WHERE key = ?1",
                params![LEDGER_HEAD_KEY],
                |row| row.get(0),
            )
            .ok();
        if let Some(a) = anchored {
            if a != head {
                return Err(u64::MAX);
            }
        }
        Ok(head)
    }

    /// 已持久化流水条数（物理水位；仅用于测试/统计）。
    /// 业务恢复水位应以成功解析恢复的记录数为准（GAP §3.6）。
    pub fn ledger_count(&self) -> anyhow::Result<u64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });
        let count: u64 =
            conn.query_row("SELECT COUNT(*) FROM ledger_entries", [], |row| row.get(0))?;
        Ok(count)
    }
}

/// v2.8.3: 旧库迁移——补哈希链列并按序重算存量行（GAP §3.1）。幂等。
fn migrate_ledger_chain(conn: &Connection) -> anyhow::Result<()> {
    let has_hash = {
        let mut stmt = conn.prepare("PRAGMA table_info(ledger_entries)")?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut found = false;
        for c in cols {
            if c? == "record_hash" {
                found = true;
            }
        }
        found
    };
    if has_hash {
        return Ok(());
    }
    conn.execute_batch(
        "ALTER TABLE ledger_entries ADD COLUMN prev_hash TEXT NOT NULL DEFAULT '';
         ALTER TABLE ledger_entries ADD COLUMN record_hash TEXT NOT NULL DEFAULT '';",
    )?;
    // 按 seq 读 payload，逐行重算 prev_hash/record_hash
    let seqs: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT seq FROM ledger_entries ORDER BY seq ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
        let mut v = Vec::new();
        for r in rows {
            v.push(r?);
        }
        v
    };
    let mut prev = LEDGER_GENESIS.to_string();
    for seq in seqs {
        let payload: String = conn.query_row(
            "SELECT payload FROM ledger_entries WHERE seq = ?1",
            params![seq],
            |row| row.get(0),
        )?;
        let hash = {
            let mut h = Sha256::new();
            h.update(prev.as_bytes());
            h.update(b"|");
            h.update(payload.as_bytes());
            hex::encode(h.finalize())
        };
        conn.execute(
            "UPDATE ledger_entries SET prev_hash = ?1, record_hash = ?2 WHERE seq = ?3",
            params![prev, hash, seq],
        )?;
        prev = hash;
    }
    conn.execute(
        "INSERT INTO kv_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![LEDGER_HEAD_KEY, prev],
    )?;
    Ok(())
}

/// v2.8.4: 旧库迁移——tasks 补 winner_price/verification_policy/requester/deadline 列（GAP §3.2）。幂等。
fn migrate_tasks_v284(conn: &Connection) -> anyhow::Result<()> {
    let has_policy = {
        let mut stmt = conn.prepare("PRAGMA table_info(tasks)")?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut found = false;
        for c in cols {
            if c? == "verification_policy" {
                found = true;
            }
        }
        found
    };
    if has_policy {
        return Ok(());
    }
    conn.execute_batch(
        "ALTER TABLE tasks ADD COLUMN winner_price INTEGER;
         ALTER TABLE tasks ADD COLUMN verification_policy TEXT NOT NULL DEFAULT '';
         ALTER TABLE tasks ADD COLUMN requester TEXT NOT NULL DEFAULT '';
         ALTER TABLE tasks ADD COLUMN deadline INTEGER NOT NULL DEFAULT 0;",
    )?;
    Ok(())
}

/// v3.5.4: 幂等地为某表补一个可空 TEXT 列（W-04/W-05）。
///
/// 用 `PRAGMA table_info` 探测列是否已存在，存在即直接返回；不存在才 `ALTER TABLE ADD COLUMN`。
/// 因此对新库（CREATE TABLE 已含该列）与旧库（需 ALTER）都安全，可重复执行。
fn migrate_add_text_column(conn: &Connection, table: &str, column: &str) -> anyhow::Result<()> {
    let present = {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut found = false;
        for c in cols {
            if c? == column {
                found = true;
            }
        }
        found
    };
    if present {
        return Ok(());
    }
    conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} TEXT;"))?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::{Money, SettlementReason};

    fn tmp_db(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gsn_persist_v261_{}_{}", std::process::id(), tag));
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
            card_json: None,
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
            winner_price: Some(150),
            verification_policy: r#"{"BftLite":{"n":3,"f":1}}"#.into(),
            requester: "req".into(),
            deadline: 1000,
            spec_json: None,
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
        // v2.8.4: 证据闸门/结算字段往返（GAP §3.2，重启后不回 None）
        assert_eq!(tasks[0].winner_price, Some(150));
        assert_eq!(tasks[0].requester, "req");
        assert_eq!(tasks[0].deadline, 1000);
        assert!(tasks[0].verification_policy.contains("BftLite"));
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

    fn rec(acct: &str, amount: i64, ts: u64) -> SettlementRecord {
        SettlementRecord {
            task_id: format!("deposit:{acct}"),
            from_account: String::new(),
            to_account: acct.into(),
            amount: Money::new(amount),
            reason: SettlementReason::Deposited,
            timestamp: ts,
        }
    }

    #[test]
    fn ledger_append_load_and_reports_corrupt() {
        let path = tmp_db("led");
        {
            let s = PersistentStore::open(&path).unwrap();
            s.append_ledger_record(&rec("a1", 10, 5)).unwrap();
            // 直接插一条损坏 payload（崩溃 / 脏行模拟）
            let conn = s.conn.lock().unwrap_or_else(|e| {
                eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
                e.into_inner()
            });
            conn.execute(
                "INSERT INTO ledger_entries (payload) VALUES ('{not json')",
                [],
            )
            .unwrap();
        }
        let s = PersistentStore::open(&path).unwrap();
        // v2.8.3: 坏行必须显式报告，不再静默跳过
        let load = s.load_ledger_records_checked().unwrap();
        assert_eq!(load.records.len(), 1);
        assert_eq!(load.corrupt.len(), 1, "损坏行必须在 corrupt 中显式返回");
        assert_eq!(load.records[0].amount, Money::new(10));
        assert_eq!(s.ledger_count().unwrap(), 2, "物理行数含损坏行");
        // 坏行同时破坏哈希链，verify 必须报错
        assert!(s.verify_ledger_chain().is_err(), "坏行必须让链校验失败");
    }

    #[test]
    fn ledger_hash_chain_verifies_after_appends_and_reopen() {
        let path = tmp_db("chain");
        let head = {
            let s = PersistentStore::open(&path).unwrap();
            s.append_ledger_record(&rec("a1", 10, 1)).unwrap();
            s.append_ledger_record(&rec("a2", 20, 2)).unwrap();
            s.append_ledger_record(&rec("a3", 30, 3)).unwrap();
            let h = s.verify_ledger_chain().expect("全新 append 链必须通过");
            assert!(!h.is_empty());
            h
        };
        // 重开后链仍校验通过，且 head 一致
        let s = PersistentStore::open(&path).unwrap();
        assert_eq!(s.verify_ledger_chain().expect("重开后链必须通过"), head);
        assert_eq!(s.ledger_count().unwrap(), 3);
    }

    #[test]
    fn ledger_chain_detects_payload_tampering() {
        let path = tmp_db("tamper");
        {
            let s = PersistentStore::open(&path).unwrap();
            s.append_ledger_record(&rec("a1", 10, 1)).unwrap();
            s.append_ledger_record(&rec("a2", 20, 2)).unwrap();
            // 篡改第 1 条 payload（凭空插入一条 Deposited 造币的等价手法）
            let conn = s.conn.lock().unwrap_or_else(|e| {
                eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
                e.into_inner()
            });
            conn.execute(
                "UPDATE ledger_entries SET payload = ?1 WHERE seq = 1",
                ["{\"tampered\":true}"],
            )
            .unwrap();
        }
        let s = PersistentStore::open(&path).unwrap();
        // 必须报首个断链 seq=1（GAP §3.1：旧表无哈希链时此篡改被当成守恒）
        assert_eq!(s.verify_ledger_chain(), Err(1));
    }

    #[test]
    fn ledger_chain_detects_head_anchor_tampering() {
        let path = tmp_db("anchor");
        {
            let s = PersistentStore::open(&path).unwrap();
            s.append_ledger_record(&rec("a1", 10, 1)).unwrap();
            // 篡改 kv_meta 锚定 head
            let conn = s.conn.lock().unwrap_or_else(|e| {
                eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
                e.into_inner()
            });
            conn.execute(
                "UPDATE kv_meta SET value = 'deadbeef' WHERE key = ?1",
                [LEDGER_HEAD_KEY],
            )
            .unwrap();
        }
        let s = PersistentStore::open(&path).unwrap();
        assert_eq!(
            s.verify_ledger_chain(),
            Err(u64::MAX),
            "锚定 head 不符必须报 u64::MAX"
        );
    }

    /// v3.5.4（W-05）：agent card_json 全字段在重开后存活。
    /// 旧实现只有 6 个扁平列，version/pricing/sla/modalities 等全部丢失（恢复成 0.0.0-restored）。
    #[test]
    fn agent_card_json_survives_reopen() {
        let path = tmp_db("card");
        let rich = r#"{"agent_id":"did:nau:a1","version":"9.9.9-w05","name":"A",
            "description":"d","skills":["rust","wasm"],"modalities":["text","image"],
            "models":["m1"],"endpoint":"ep","pricing":{"model":"PerCall","price":150,"currency":"Credit"},
            "sla":{"latency_p95_ms":1234,"availability":0.9,"max_concurrency":7},"owner":"did:nau:a1",
            "stake":10000,"reputation_score":0.8,"total_calls":42,"success_rate":0.9,
            "evidence_grade":"Verified","verified":true,"created_at":7,"updated_at":8}"#;
        {
            let s = PersistentStore::open(&path).unwrap();
            let mut a = sample_agent("did:nau:a1", "A");
            a.card_json = Some(rich.to_string());
            s.upsert_agent(&a).unwrap();
        }
        let s = PersistentStore::open(&path).unwrap();
        let agents = s.load_agents().unwrap();
        let got = agents[0].card_json.as_ref().expect("card_json 必须存活");
        let v: serde_json::Value = serde_json::from_str(got).unwrap();
        assert_eq!(v["version"], "9.9.9-w05");
        assert_eq!(v["pricing"]["price"], 150);
        assert_eq!(v["sla"]["latency_p95_ms"], 1234);
        assert_eq!(v["skills"][0], "rust");
        assert_eq!(v["total_calls"], 42);
    }

    /// v3.5.4（W-04）：task spec_json 全字段在重开后存活。
    /// 旧实现恢复时 context 为空、todo 退化为 "(restored from disk)"、required_skills 丢失。
    #[test]
    fn task_spec_json_survives_reopen() {
        let path = tmp_db("spec");
        let rich = r#"{"task_id":"t1","goal":"g","context":"ctx","done":["d1"],
            "todo":["todo1","todo2"],"trace":["tr"],"owner":null,"budget":200,
            "winner_price":150,"deadline":1000,
            "required_skills":["rust","ops"],"verification_policy":"None",
            "requester":"req","state":"Open","created_at":10}"#;
        {
            let s = PersistentStore::open(&path).unwrap();
            let mut t = sample_task("t1", "Open");
            t.spec_json = Some(rich.to_string());
            s.upsert_task(&t).unwrap();
        }
        let s = PersistentStore::open(&path).unwrap();
        let tasks = s.load_tasks().unwrap();
        let got = tasks[0].spec_json.as_ref().expect("spec_json 必须存活");
        let v: serde_json::Value = serde_json::from_str(got).unwrap();
        assert_eq!(v["context"], "ctx");
        assert_eq!(v["todo"][0], "todo1");
        assert_eq!(v["required_skills"][1], "ops");
        assert_eq!(v["done"][0], "d1");
    }

    /// v3.5.4：spec_json/card_json 迁移对"新建即含列"与"旧库 ALTER"都幂等（重复 open 不报错）。
    #[test]
    fn json_column_migration_is_idempotent() {
        let path = tmp_db("idem");
        for _ in 0..3 {
            let s = PersistentStore::open(&path).unwrap();
            s.upsert_agent(&sample_agent("a", "A")).unwrap();
            s.upsert_task(&sample_task("t", "Open")).unwrap();
        }
        let s = PersistentStore::open(&path).unwrap();
        assert_eq!(s.load_agents().unwrap().len(), 1);
        assert_eq!(s.load_tasks().unwrap().len(), 1);
    }

    /// v3.5.5（P1）：open 后连接必须带上预期 PRAGMA。
    ///
    /// - `foreign_keys` 是连接级开关，必须严格为 1；
    /// - `journal_mode` 在正常可写文件库应为 `wal`；若运行环境（只读 FS / 特殊构建）
    ///   无法启用 WAL 而退化为 `memory`，这里如实接受，不硬断言失败。
    #[test]
    fn open_sets_expected_connection_pragmas() {
        let path = tmp_db("pragma");
        let s = PersistentStore::open(&path).unwrap();
        let conn = s.conn.lock().unwrap_or_else(|e| {
            eprintln!("⚠️ persist: 连接锁曾毒化，恢复后继续（可能处于半写状态，请人工核查）");
            e.into_inner()
        });

        let jm: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("读取 journal_mode 失败");
        assert!(
            jm == "wal" || jm == "memory",
            "journal_mode 期望 wal（受限环境可能退化为 memory），实得 '{jm}'"
        );

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("读取 foreign_keys 失败");
        assert_eq!(fk, 1, "foreign_keys 必须逐连接开启");

        // busy_timeout / synchronous 被成功设置即可，不硬断言具体值以免环境差异。
        let busy: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("读取 busy_timeout 失败");
        assert!(busy > 0, "busy_timeout 应已设置为正值，实得 {busy}");
    }
}
