//! 沙箱生命周期管理与弹性供给（v2.7.8）
//!
//! `SandboxManager` 在多个沙箱之上提供：
//! - 批量创建 / 获取 / 回收；
//! - 预热池（warm pool）：提前创建并 start，请求到达即用，降低冷启动；
//! - 休眠唤醒：空闲沙箱 pause 释放资源，需要时 resume；
//! - Checkpoint / Fork：进程级无法保存内存，用"工作目录快照 + 元数据"实现
//!   状态复用（对应训练评测的 Reset/Fork/Replay）。
//!
//! 边界：进程级 checkpoint 只保存文件系统状态与配置，不保存解释器内存；
//! 真正的内存级 checkpoint 依赖 microVM（v2.8.0+ 部署）。

use super::config::SandboxConfig;
use super::error::SandboxError;
use super::runtime::ProcessSandbox;
use super::state::SandboxState;
use super::Sandbox;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// 受管沙箱的记录
struct Managed {
    sandbox: ProcessSandbox,
    /// 是否在预热池中（空闲待用）
    in_warm_pool: bool,
    /// 最近一次使用时间（毫秒）
    last_used_ms: u64,
}

/// 沙箱管理器
pub struct SandboxManager {
    /// 沙箱临时根目录
    base_dir: PathBuf,
    /// checkpoint 存放目录
    checkpoint_dir: PathBuf,
    /// 受管沙箱
    sandboxes: std::collections::HashMap<String, Managed>,
    /// 预热池：已 start 待用的沙箱 id
    warm_pool: VecDeque<String>,
    /// 预热池目标大小
    warm_pool_size: usize,
    /// 默认配置
    default_cfg: SandboxConfig,
    /// 自增序号（生成唯一 id）
    counter: u64,
}

impl SandboxManager {
    /// 创建管理器
    pub fn new(base_dir: PathBuf, mut default_cfg: SandboxConfig, warm_pool_size: usize) -> Self {
        let checkpoint_dir = base_dir.join("checkpoints");
        let _ = std::fs::create_dir_all(&base_dir);
        let _ = std::fs::create_dir_all(&checkpoint_dir);
        // 所有沙箱都在 manager.base_dir 下
        default_cfg.work_dir_base = Some(base_dir.clone());
        Self {
            base_dir,
            checkpoint_dir,
            sandboxes: std::collections::HashMap::new(),
            warm_pool: VecDeque::new(),
            warm_pool_size,
            default_cfg,
            counter: 0,
        }
    }

    fn next_id(&mut self) -> String {
        self.counter += 1;
        format!("sb-{}", self.counter)
    }

    /// 预热：创建并 start 到 warm_pool_size 个空闲沙箱
    pub fn prewarm(&mut self) -> Result<usize, SandboxError> {
        let mut created = 0;
        while self.warm_pool.len() < self.warm_pool_size {
            let id = self.next_id();
            let mut sb = ProcessSandbox::new(&id);
            let mut cfg = self.default_cfg.clone();
            cfg.sandbox_id = id.clone();
            sb.create(&cfg)?;
            sb.start()?;
            self.sandboxes.insert(
                id.clone(),
                Managed {
                    sandbox: sb,
                    in_warm_pool: true,
                    last_used_ms: now_ms(),
                },
            );
            self.warm_pool.push_back(id);
            created += 1;
        }
        Ok(created)
    }

    /// 获取一个沙箱：优先从预热池取，没有则即时创建
    pub fn acquire(&mut self, cfg: Option<SandboxConfig>) -> Result<String, SandboxError> {
        let id = if let Some(prewarmed) = self.warm_pool.pop_front() {
            if let Some(m) = self.sandboxes.get_mut(&prewarmed) {
                m.in_warm_pool = false;
                m.last_used_ms = now_ms();
            }
            prewarmed
        } else {
            let id = self.next_id();
            let mut sb = ProcessSandbox::new(&id);
            let mut use_cfg = cfg.unwrap_or_else(|| self.default_cfg.clone());
            use_cfg.sandbox_id = id.clone();
            use_cfg.work_dir_base = Some(self.base_dir.clone());
            sb.create(&use_cfg)?;
            sb.start()?;
            self.sandboxes.insert(
                id.clone(),
                Managed {
                    sandbox: sb,
                    in_warm_pool: false,
                    last_used_ms: now_ms(),
                },
            );
            id
        };
        Ok(id)
    }

    /// 归还沙箱：暂停（休眠），需要时再唤醒；不直接放回预热池以避免脏状态
    pub fn release(&mut self, id: &str) -> Result<(), SandboxError> {
        let m = self
            .sandboxes
            .get_mut(id)
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;
        if m.sandbox.state() == SandboxState::Running {
            m.sandbox.pause()?;
        }
        m.last_used_ms = now_ms();
        Ok(())
    }

    /// 唤醒一个休眠沙箱
    pub fn wake(&mut self, id: &str) -> Result<(), SandboxError> {
        let m = self
            .sandboxes
            .get_mut(id)
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;
        if m.sandbox.state() == SandboxState::Paused {
            m.sandbox.resume()?;
        }
        m.last_used_ms = now_ms();
        Ok(())
    }

    /// 可变借用沙箱（执行代码用）
    pub fn sandbox_mut(&mut self, id: &str) -> Result<&mut ProcessSandbox, SandboxError> {
        let m = self
            .sandboxes
            .get_mut(id)
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;
        Ok(&mut m.sandbox)
    }

    /// 沙箱状态
    pub fn state_of(&self, id: &str) -> Option<SandboxState> {
        self.sandboxes.get(id).map(|m| m.sandbox.state())
    }

    /// Checkpoint：把沙箱工作目录快照到 checkpoint 区，返回 checkpoint id
    pub fn checkpoint(&mut self, id: &str) -> Result<String, SandboxError> {
        let cp_id = format!("{}-{}", id, now_ms());
        let work = self
            .sandboxes
            .get(id)
            .and_then(|m| m.sandbox.work_dir().map(|p| p.to_path_buf()))
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;
        let dest = self.checkpoint_dir.join(&cp_id);
        copy_dir_recursive(&work, &dest)?;
        Ok(cp_id)
    }

    /// Fork：从源（活沙箱或 checkpoint）复制出一个新沙箱，从同一起点继续
    pub fn fork_from(&mut self, source: &str, new_id: Option<String>) -> Result<String, SandboxError> {
        let new_id = new_id.unwrap_or_else(|| self.next_id());
        let dest = self.base_dir.join(format!("au-sandbox-{}", new_id));

        // 解析源目录（活沙箱工作目录或 checkpoint）
        let src_dir: PathBuf = if let Some(m) = self.sandboxes.get(source) {
            m.sandbox
                .work_dir()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| SandboxError::NotFound(source.to_string()))?
        } else {
            let cp = self.checkpoint_dir.join(source);
            if cp.exists() {
                cp
            } else {
                return Err(SandboxError::NotFound(source.to_string()));
            }
        };

        // 先 create 新沙箱（建立目录，不删内容），再从源复制合并，最后 start
        let mut sb = ProcessSandbox::new(&new_id);
        let mut cfg = self.default_cfg.clone();
        cfg.sandbox_id = new_id.clone();
        sb.create(&cfg)?;
        copy_dir_recursive(&src_dir, &dest)?;
        sb.start()?;
        self.sandboxes.insert(
            new_id.clone(),
            Managed {
                sandbox: sb,
                in_warm_pool: false,
                last_used_ms: now_ms(),
            },
        );
        Ok(new_id)
    }

    /// 回收并销毁一个沙箱
    pub fn destroy(&mut self, id: &str) -> Result<(), SandboxError> {
        self.warm_pool.retain(|x| x != id);
        match self.sandboxes.remove(id) {
            Some(mut m) => m.sandbox.destroy(),
            None => Err(SandboxError::NotFound(id.to_string())),
        }
    }

    /// 清理所有沙箱（含预热池），返回销毁数量
    pub fn shutdown(&mut self) -> Result<usize, SandboxError> {
        let ids: Vec<String> = self.sandboxes.keys().cloned().collect();
        let n = ids.len();
        for id in ids {
            self.destroy(&id)?;
        }
        self.warm_pool.clear();
        Ok(n)
    }

    /// 当前受管沙箱数
    pub fn count(&self) -> usize {
        self.sandboxes.len()
    }

    /// 预热池当前大小
    pub fn warm_pool_len(&self) -> usize {
        self.warm_pool.len()
    }

    /// 回收过期/休眠过久的沙箱（返回回收数）。idle_ms 阈值
    pub fn evict_idle(&mut self, idle_ms: u64) -> Result<usize, SandboxError> {
        let now = now_ms();
        let to_destroy: Vec<String> = self
            .sandboxes
            .iter()
            .filter(|(id, m)| {
                !m.in_warm_pool
                    && !matches!(m.sandbox.state(), SandboxState::Running)
                    && now.saturating_sub(m.last_used_ms) >= idle_ms
                    && self.warm_pool.iter().all(|w| w != *id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &to_destroy {
            self.destroy(id)?;
        }
        // 销毁后补预热
        self.prewarm()?;
        Ok(to_destroy.len())
    }
}

// ---------- helpers ----------

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), SandboxError> {
    std::fs::create_dir_all(dest).map_err(|e| SandboxError::Internal(e.to_string()))?;
    let entries = std::fs::read_dir(src).map_err(|e| SandboxError::Internal(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| SandboxError::Internal(e.to_string()))?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).map_err(|e| SandboxError::Internal(e.to_string()))?;
        }
    }
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
