//! 能力声明：后端**实际能强制什么**，以及把无法强制的边界变成拒绝而不是注释。
//!
//! ## 规则
//!
//! 一个被写进文档、但没有执行点的边界，比没有边界更糟——它会"洗白"信任。
//! 所以策略字段只有两种命运：
//!
//! 1. 后端声明匹配的 [`Capability`] 并真正强制它，或
//! 2. 校验失败，返回 [`SandboxError::PolicyNotEnforceable`]，**具名**点出是哪条边界。
//!
//! 没有第三种命运。特别地，这里没有一个调用方可以"不小心忽略"的 bool：
//! [`CapabilityDeclaration::check`] 返回 `Result<(), SandboxError>`。
//!
//! 唯一能在缺少边界时继续运行的方式是显式 [`Waiver`]——它必须写明操作者的
//! 理由，且该理由非空，管理器会把边界与理由一起记入审计日志。

use serde::{Deserialize, Serialize};

use super::error::SandboxError;

/// 沙箱能力校验结果
type Result<T> = std::result::Result<T, SandboxError>;

/// 后端能强制的边界（机制无关命名，不按 syscall 命名）。
///
/// 两个后端可以同意"同一条边界"，但用不同机制实现——这正是命名按边界而非
/// 系统调用的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Capability {
    /// 子进程环境由显式白名单构建，绝不继承 daemon 自身环境。
    EnvAllowlist,
    /// stdout/stderr 在父进程有上限，截断被报告。
    OutputCap,
    /// 超时后杀掉沙箱，含整棵进程树而非仅直接子进程。
    Timeout,
    /// 子进程的 current_dir 是仅此沙箱专用、在进程启动时固定的根下的目录。
    WorkDirIsolation,
    /// 出站网络可被真正拒绝。
    NetworkDenyAll,
    /// 出站网络可被限制到指定目标集合。
    NetworkAllowList,
    /// 地址空间/提交内存可被封顶，超限杀进程。
    MemoryLimit,
    /// CPU 时间可被封顶。
    CpuLimit,
    /// 磁盘写入字节可被封顶。
    DiskQuota,
    /// 沙箱内进程数可被封顶。
    ProcessCountLimit,
    /// 打开的文件描述符/句柄数可被封顶。
    OpenFileLimit,
    /// 子进程可被限制在文件系统的某子树内。
    FilesystemConfinement,
    /// 后端须能交付**至少调用方请求的隔离强度**（Process < Container < MicroVM）。
    ///
    /// v3.5.2（AU-28）：此前 `cfg.isolation` 只是自报标签，进程后端从不与实际可达
    /// 级别比对——调用方写 MicroVM 也会被静默按 Process 跑。此边界把"请求级别 >
    /// 后端实际可达级别"变成具名拒绝，除非带理由豁免。
    IsolationLevel,
}

impl Capability {
    /// 稳定名称（用于错误、JSON、审计）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnvAllowlist => "env_allowlist",
            Self::OutputCap => "output_cap",
            Self::Timeout => "timeout",
            Self::WorkDirIsolation => "work_dir_isolation",
            Self::NetworkDenyAll => "network_deny_all",
            Self::NetworkAllowList => "network_allow_list",
            Self::MemoryLimit => "memory_limit",
            Self::CpuLimit => "cpu_limit",
            Self::DiskQuota => "disk_quota",
            Self::ProcessCountLimit => "process_count_limit",
            Self::OpenFileLimit => "open_file_limit",
            Self::FilesystemConfinement => "filesystem_confinement",
            Self::IsolationLevel => "isolation_level",
        }
    }

    /// 全部能力（穷举测试与报告用）
    pub const ALL: [Capability; 13] = [
        Self::EnvAllowlist,
        Self::OutputCap,
        Self::Timeout,
        Self::WorkDirIsolation,
        Self::NetworkDenyAll,
        Self::NetworkAllowList,
        Self::MemoryLimit,
        Self::CpuLimit,
        Self::DiskQuota,
        Self::ProcessCountLimit,
        Self::OpenFileLimit,
        Self::FilesystemConfinement,
        Self::IsolationLevel,
    ];
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 显式豁免：在缺少某条边界时继续运行，必须写明理由。
///
/// 这是唯一能"无边界运行"的方式，刻意做得不轻松：理由非空，且被记入审计。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Waiver {
    /// 被豁免的边界
    pub boundary: Capability,
    /// 为什么能在缺少该边界时运行（非空，如"运行可信代码，ticket SEC-42"）
    pub justification: String,
}

impl Waiver {
    /// 创建豁免
    pub fn new(boundary: Capability, justification: impl Into<String>) -> Self {
        Self {
            boundary,
            justification: justification.into(),
        }
    }
}

/// 后端能力声明：强制了什么、未强制什么及原因。
///
/// 这是数据而非文档：[`CapabilityDeclaration::check`] 是唯一消费者，在每次
/// create / exec 时被调用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct CapabilityDeclaration {
    backend: String,
    enforced: Vec<Capability>,
    unenforced: Vec<UnenforcedCapability>,
}

/// 后端不强制的能力及原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct UnenforcedCapability {
    /// 边界
    pub boundary: Capability,
    /// 后端在本平台为何无法交付
    pub reason: String,
}

impl CapabilityDeclaration {
    /// 声明后端能力。
    ///
    /// `unenforced` 每条都必须带理由；理由为空会被拒绝——"未强制且无解释"
    /// 正是本模块要消除的漂移。
    pub fn new(
        backend: impl Into<String>,
        enforced: impl IntoIterator<Item = Capability>,
        unenforced: impl IntoIterator<Item = (Capability, String)>,
    ) -> std::result::Result<Self, String> {
        let backend = backend.into();
        if backend.trim().is_empty() {
            return Err("后端必须命名".to_string());
        }
        let enforced: Vec<Capability> = enforced.into_iter().collect();
        let mut unenforced: Vec<UnenforcedCapability> = unenforced
            .into_iter()
            .map(|(boundary, reason)| UnenforcedCapability { boundary, reason })
            .collect();
        for entry in &unenforced {
            if entry.reason.trim().is_empty() {
                return Err(format!(
                    "后端 `{backend}` 声明 `{}` 未强制却无理由",
                    entry.boundary
                ));
            }
            if enforced.contains(&entry.boundary) {
                return Err(format!(
                    "后端 `{backend}` 对 `{}` 同时声明强制与未强制",
                    entry.boundary
                ));
            }
        }
        // 确定性排序，相同声明的两个后端比较相等
        unenforced.sort_by_key(|entry| entry.boundary as u8);
        Ok(Self {
            backend,
            enforced,
            unenforced,
        })
    }

    /// 从已校验的列表构建声明（用于本 crate 内的常量声明，无需 Result/expect）
    pub(crate) fn from_parts(
        backend: &str,
        enforced: Vec<Capability>,
        mut unenforced: Vec<UnenforcedCapability>,
    ) -> Self {
        unenforced.sort_by_key(|entry| entry.boundary as u8);
        Self {
            backend: backend.to_string(),
            enforced,
            unenforced,
        }
    }

    /// 后端名称
    pub fn backend(&self) -> &str {
        &self.backend
    }

    /// 是否强制某能力
    pub fn enforces(&self, cap: Capability) -> bool {
        self.enforced.contains(&cap)
    }

    /// 未强制某能力的原因
    pub fn reason_unenforced(&self, cap: Capability) -> Option<&str> {
        self.unenforced
            .iter()
            .find(|entry| entry.boundary == cap)
            .map(|entry| entry.reason.as_str())
    }

    /// 未强制部分（报告/审计用）
    pub fn unenforced(&self) -> &[UnenforcedCapability] {
        &self.unenforced
    }

    /// 要求某能力：强制了 → Ok；否则返回具名拒绝。
    pub fn require(&self, cap: Capability) -> Result<()> {
        if self.enforces(cap) {
            return Ok(());
        }
        Err(SandboxError::PolicyNotEnforceable {
            boundary: cap,
            backend: self.backend.clone(),
            detail: self
                .reason_unenforced(cap)
                .unwrap_or("该后端没有发布对它的强制能力")
                .to_string(),
        })
    }

    /// 校验一条边界：后端强制它，或被带非空理由的豁免覆盖。
    ///
    /// 这是"文档化但未强制"缺陷的强制点：无法强制且无豁免 → 具名拒绝。
    pub fn check(&self, cap: Capability, waivers: &[Waiver]) -> Result<()> {
        if self.enforces(cap) {
            return Ok(());
        }
        if let Some(w) = waivers.iter().find(|w| w.boundary == cap) {
            if w.justification.trim().is_empty() {
                return Err(SandboxError::PolicyNotEnforceable {
                    boundary: cap,
                    backend: self.backend.clone(),
                    detail: "豁免必须带非空理由以便审计".to_string(),
                });
            }
            return Ok(());
        }
        self.require(cap)
    }
}

/// 进程后端（process）在**当前平台**的能力声明。
///
/// 作为数据写一次，紧挨实现它的代码，使声明与实现无法漂移。
pub fn process_declaration() -> CapabilityDeclaration {
    use Capability::*;
    // 所有平台都强制的：环境白名单、有界输出、超时、独立工作目录
    let mut enforced = vec![EnvAllowlist, OutputCap, Timeout, WorkDirIsolation];

    // 所有平台都无法强制的：出站网络（拒绝/白名单）、磁盘配额、文件系统子树限制
    let mut unenforced: Vec<UnenforcedCapability> = vec![
        UnenforcedCapability {
            boundary: NetworkDenyAll,
            reason: "普通子进程在本平台没有可用的出站过滤器；请改用带网络命名空间的后端，\
                     或用带理由的豁免（记入审计）"
                .to_string(),
        },
        UnenforcedCapability {
            boundary: NetworkAllowList,
            reason: "同 network_deny_all：本后端没有任何东西能限制子进程的目标地址".to_string(),
        },
        UnenforcedCapability {
            boundary: DiskQuota,
            reason: "本后端不施加文件系统配额：Windows 上配额是卷级策略，Unix 上需要支持
                     project-quota 的文件系统"
                .to_string(),
        },
        UnenforcedCapability {
            boundary: FilesystemConfinement,
            reason: "本后端不调用 chroot/pivot_root；mount 命名空间或 AppContainer 属于
                     另一个后端"
                .to_string(),
        },
    ];

    if cfg!(windows) {
        // Windows Job Object：内存上限、进程数上限
        enforced.push(MemoryLimit);
        enforced.push(ProcessCountLimit);
        // Job Object 对普通进程不强制 CPU 时间/句柄数
        unenforced.push(UnenforcedCapability {
            boundary: CpuLimit,
            reason: "Job Object 对普通进程没有强制的 CPU 时间上限；墙钟超时是强制边界".to_string(),
        });
        unenforced.push(UnenforcedCapability {
            boundary: OpenFileLimit,
            reason: "Job Object 没有句柄数上限：JOB_OBJECT_LIMIT_* 覆盖内存/进程数/用户时间，
                     不覆盖句柄"
                .to_string(),
        });
    } else if cfg!(target_os = "linux") {
        // Linux：bash ulimit 强制 CPU 时间(-t)、进程数(-u)、句柄(-n)。
        // 内存：Python 用 ulimit -v（虚拟地址，精确）；Node/V8 因启动预留
        // 大块虚拟 CodeRange，ulimit -v 无法可靠限制，改用 V8
        // --max-old-space-size 限制 JS 堆，ulimit -v 给预留余量并兜底总
        // 虚拟地址（Buffer 外部内存不被 heap flag 限制，靠 vmem 兜底）。
        enforced.push(MemoryLimit);
        enforced.push(CpuLimit);
        enforced.push(ProcessCountLimit);
        enforced.push(OpenFileLimit);
    } else {
        // macOS：ulimit -t/-u/-n 有效，但内核不支持 RLIMIT_AS/VMEM，
        // 故 Python 内存无强制原语（Node 的 --max-old-space-size 仍限制
        // JS 堆，但无法对所有语言保证内存边界）→ MemoryLimit 记为 unenforced。
        enforced.push(CpuLimit);
        enforced.push(ProcessCountLimit);
        enforced.push(OpenFileLimit);
        unenforced.push(UnenforcedCapability {
            boundary: MemoryLimit,
            reason: "macOS 内核不支持 RLIMIT_AS/VMEM（ulimit -v 失败或静默无效）；Node 的 \
                     --max-old-space-size 仅限制 JS 堆，无法对所有语言保证内存边界"
                .to_string(),
        });
    }

    CapabilityDeclaration::from_parts("process", enforced, unenforced)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(enforced: &[Capability], unenforced: &[(Capability, &str)]) -> CapabilityDeclaration {
        CapabilityDeclaration::new(
            "test-backend",
            enforced.iter().copied(),
            unenforced
                .iter()
                .map(|(c, r)| (*c, (*r).to_string()))
                .collect::<Vec<_>>(),
        )
        .expect("声明")
    }

    /// 每个能力在两个方向都被验证：既不会假"强制"，也不会假"拒绝"。
    #[test]
    fn every_capability_is_exercised_in_both_directions() {
        for cap in Capability::ALL {
            let enforcing = caps(&[cap], &[]);
            assert!(enforcing.enforces(cap), "{cap} 应强制");
            assert!(enforcing.require(cap).is_ok(), "{cap} 应可用");
            assert!(enforcing.check(cap, &[]).is_ok());

            let others = Capability::ALL
                .iter()
                .copied()
                .filter(|c| *c != cap)
                .collect::<Vec<_>>();
            let refusing = caps(&others, &[(cap, "本平台无原语")]);
            assert!(!refusing.enforces(cap), "{cap} 应拒绝");
            let err = refusing.require(cap).expect_err("必须拒绝");
            assert!(matches!(
                err,
                SandboxError::PolicyNotEnforceable { boundary, .. } if boundary == cap
            ));
        }
    }

    /// 豁免是无边界运行的唯一方式，需带理由；且不会让 Required 变成全局放开。
    #[test]
    fn a_waiver_needs_a_justification() {
        let refusing = caps(&[], &[(Capability::NetworkDenyAll, "无包过滤")]);
        // 无豁免 → 拒绝
        assert!(refusing.check(Capability::NetworkDenyAll, &[]).is_err());
        // 空理由的豁免 → 仍拒绝
        let empty = vec![Waiver::new(Capability::NetworkDenyAll, "   ")];
        assert!(refusing.check(Capability::NetworkDenyAll, &empty).is_err());
        // 带理由的豁免 → 通过
        let ok = vec![Waiver::new(
            Capability::NetworkDenyAll,
            "air-gapped CI, ticket SEC-42",
        )];
        assert!(refusing.check(Capability::NetworkDenyAll, &ok).is_ok());
    }

    /// 声明不能自相矛盾或隐藏理由
    #[test]
    fn a_declaration_cannot_contradict_itself() {
        assert!(CapabilityDeclaration::new("b", [Capability::Timeout], []).is_ok());
        assert!(CapabilityDeclaration::new(
            "b",
            [Capability::Timeout],
            [(Capability::Timeout, "x".into())]
        )
        .is_err());
        assert!(CapabilityDeclaration::new("b", [], [(Capability::Timeout, "  ".into())]).is_err());
        assert!(CapabilityDeclaration::new("  ", [Capability::Timeout], []).is_err());
    }

    /// 进程后端声明：网络/磁盘/FS 一律未强制；其余按平台强制。
    #[test]
    fn process_backend_is_honest_about_network_and_fs_gaps() {
        let d = process_declaration();
        assert!(!d.enforces(Capability::NetworkDenyAll));
        assert!(!d.enforces(Capability::NetworkAllowList));
        assert!(!d.enforces(Capability::DiskQuota));
        assert!(!d.enforces(Capability::FilesystemConfinement));
        // 所有平台都强制的基础能力
        assert!(d.enforces(Capability::Timeout));
        assert!(d.enforces(Capability::OutputCap));
        assert!(d.enforces(Capability::WorkDirIsolation));
        assert!(d.enforces(Capability::EnvAllowlist));
        // 每条未强制都有理由
        for entry in d.unenforced() {
            assert!(!entry.reason.trim().is_empty());
        }
        // 进程数：Windows 用 Job、Linux/macOS 用 ulimit，均强制
        assert!(d.enforces(Capability::ProcessCountLimit));
        // 内存：Windows 用 Job、Linux 用 ulimit -v，强制；macOS 无 RLIMIT_AS，
        // 诚实标为 unenforced（必有理由）。
        if cfg!(target_os = "macos") {
            assert!(!d.enforces(Capability::MemoryLimit));
            assert!(d
                .unenforced()
                .iter()
                .any(|e| e.boundary == Capability::MemoryLimit && !e.reason.trim().is_empty()));
        } else {
            assert!(d.enforces(Capability::MemoryLimit));
        }
    }
}
