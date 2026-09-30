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
use super::security::{AuditLog, EvidenceGrade, NetworkGuard, PermissionChecker};
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
    /// 所有者（认证主体；预热/空闲时为 None，acquire 后绑定）
    owner: Option<String>,
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
    /// 审计日志（create/exec/pause/destroy 全记录）
    audit: AuditLog,
}

impl SandboxManager {
    /// 创建管理器
    pub fn new(base_dir: PathBuf, mut default_cfg: SandboxConfig, warm_pool_size: usize) -> Self {
        let checkpoint_dir = base_dir.join("checkpoints");
        let _ = std::fs::create_dir_all(&base_dir);
        let _ = std::fs::create_dir_all(&checkpoint_dir);
        // v2.8.7：启动时清扫上次运行残留的孤儿沙箱目录（进程级沙箱不跨重启）。
        // 此时内存为空、预热池未建，base 下所有 au-sandbox-* 均为孤儿。
        sweep_orphan_sandboxes(&base_dir);
        // 所有沙箱都在 manager.base_dir 下
        default_cfg.work_dir_base = Some(base_dir.clone());
        let audit = AuditLog::new(Some(base_dir.join("sandbox-audit.log")));
        Self {
            base_dir,
            checkpoint_dir,
            sandboxes: std::collections::HashMap::new(),
            warm_pool: VecDeque::new(),
            warm_pool_size,
            default_cfg,
            audit,
        }
    }

    /// 生成密码学随机、不可预测的沙箱 id（v2.8.7：替代可猜的 sb-N 计数器）
    fn next_id(&mut self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        // 8 字节随机 → 16 hex；碰撞概率极低，且经单一校验函数
        let raw: String = (0..8).map(|_| format!("{:02x}", rng.gen::<u8>())).collect();
        let id = format!("sb-{raw}");
        // 自检：生成的 id 必须通过校验
        debug_assert!(Self::validate_sandbox_id(&id).is_ok());
        id
    }

    /// 校验沙箱 id 是合法、安全的单个路径组件（v2.8.7 唯一校验入口）
    pub fn validate_sandbox_id(id: &str) -> Result<(), SandboxError> {
        let body = id.strip_prefix("sb-").ok_or_else(|| {
            SandboxError::InvalidConfig(format!("沙箱 id 必须以 'sb-' 开头: {id}"))
        })?;
        if body.is_empty() || body.len() > 40 {
            return Err(SandboxError::InvalidConfig(format!(
                "沙箱 id 主体长度须在 1..=40: {id}"
            )));
        }
        if !body
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(SandboxError::InvalidConfig(format!(
                "沙箱 id 只允许小写字母/数字/'-': {id}"
            )));
        }
        // 作为单一路径组件，禁止分隔符与逃逸
        if id.contains('/') || id.contains('\\') || id.contains("..") {
            return Err(SandboxError::InvalidConfig(format!(
                "沙箱 id 不得含路径分隔符或 '..': {id}"
            )));
        }
        Ok(())
    }

    /// 绑定沙箱所有者（acquire 后由 API 层调用）
    pub fn bind_owner(&mut self, id: &str, owner: &str) -> Result<(), SandboxError> {
        let owner = owner.trim();
        if owner.is_empty() {
            return Err(SandboxError::IsolationViolation("不能绑定空所有者".into()));
        }
        let m = self
            .sandboxes
            .get_mut(id)
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;
        m.owner = Some(owner.to_string());
        Ok(())
    }

    /// 查询沙箱所有者
    pub fn owner_of(&self, id: &str) -> Option<&str> {
        self.sandboxes.get(id).and_then(|m| m.owner.as_deref())
    }

    /// 校验调用者是沙箱所有者（v2.8.7 所有权闸门，内部走 PermissionChecker）
    pub fn check_owner(&self, id: &str, caller: Option<&str>) -> Result<(), SandboxError> {
        let caller = caller
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .ok_or_else(|| SandboxError::IsolationViolation("未认证调用（缺少调用主体）".into()))?;
        let m = self
            .sandboxes
            .get(id)
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;
        let owner = m.owner.as_deref().unwrap_or_default();
        // 用 PermissionChecker 把"拥有该沙箱"作为 scope 校验
        let held = vec![format!("sandbox:{owner}")];
        let required = format!("sandbox:{caller}");
        PermissionChecker::check(&held, &required).map_err(|_| {
            SandboxError::IsolationViolation(format!(
                "调用者 {caller} 不是沙箱 {id} 的所有者（owner={owner}）"
            ))
        })
    }

    /// 出站网络检查（v2.8.7：NetworkGuard 接入，供宿主侧受控出站/代理调用）。
    ///
    /// 诚实边界：此检查只约束**经宿主代理发起**的请求；进程级沙箱**无法**
    /// 拦截子进程内部直接创建的 socket（无网络命名空间/seccomp）。需要强制
    /// 网络隔离必须使用容器/microVM 后端。
    pub fn check_egress(&self, host: &str, port: u16) -> Result<(), SandboxError> {
        NetworkGuard::new(&self.default_cfg.network).check_egress(host, port)
    }

    /// 记录一条审计（v2.8.7：AuditLog 接入生产路径）
    pub fn record_audit(
        &mut self,
        sandbox_id: &str,
        agent_did: &str,
        action: &str,
        target: &str,
        outcome: &str,
        grade: EvidenceGrade,
    ) -> Result<(), SandboxError> {
        self.audit.append(super::security::audit_entry(
            sandbox_id, agent_did, action, target, outcome, grade,
        ))
    }

    /// 审计条目数（测试/运维用）
    pub fn audit_len(&self) -> usize {
        self.audit.len()
    }

    /// 默认配置（API 层据此叠加请求体覆盖项）
    pub fn default_config(&self) -> &SandboxConfig {
        &self.default_cfg
    }

    /// 列出所有受管沙箱 id
    pub fn list_sandboxes(&self) -> Vec<String> {
        self.sandboxes.keys().cloned().collect()
    }

    /// 是否存在指定沙箱
    pub fn exists(&self, id: &str) -> bool {
        self.sandboxes.contains_key(id)
    }

    /// 暂停沙箱（语义化入口；内部走 release，Running 时执行 pause）
    pub fn pause(&mut self, id: &str) -> Result<(), SandboxError> {
        self.release(id)
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
                    owner: None,
                },
            );
            self.warm_pool.push_back(id);
            created += 1;
        }
        Ok(created)
    }

    /// 获取一个沙箱：
    /// - 传 `Some(cfg)`（自定义配置）→ 强制即时创建，绕过预热池，保证配置生效；
    /// - 传 `None`（默认）→ 优先预热池，空则用默认配置即时创建。
    pub fn acquire(&mut self, cfg: Option<SandboxConfig>) -> Result<String, SandboxError> {
        if let Some(mut use_cfg) = cfg {
            let id = self.next_id();
            use_cfg.sandbox_id = id.clone();
            use_cfg.work_dir_base = Some(self.base_dir.clone());
            let mut sb = ProcessSandbox::new(&id);
            sb.create(&use_cfg)?;
            sb.start()?;
            self.sandboxes.insert(
                id.clone(),
                Managed {
                    sandbox: sb,
                    in_warm_pool: false,
                    last_used_ms: now_ms(),
                    owner: None,
                },
            );
            return Ok(id);
        }
        let id = if let Some(prewarmed) = self.warm_pool.pop_front() {
            if let Some(m) = self.sandboxes.get_mut(&prewarmed) {
                m.in_warm_pool = false;
                m.last_used_ms = now_ms();
            }
            prewarmed
        } else {
            let id = self.next_id();
            let mut sb = ProcessSandbox::new(&id);
            let mut use_cfg = self.default_cfg.clone();
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
                    owner: None,
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
    pub fn fork_from(
        &mut self,
        source: &str,
        new_id: Option<String>,
    ) -> Result<String, SandboxError> {
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
                owner: None,
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

/// 清扫 base 下残留的孤儿沙箱工作目录（v2.8.7）
fn sweep_orphan_sandboxes(base: &Path) {
    let Ok(rd) = std::fs::read_dir(base) else {
        return;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // 只认沙箱工作目录前缀；checkpoints 目录保留
        if name.starts_with("au-sandbox-") {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

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
