//! 内存配额池：共享额度记账 + 超卖准入（v3.7.4）。
//!
//! 背景（见 `docs/ausec/AUSEC-DESIGN.md` §3.1/§5）：Agent 沙盒负载稀疏，约 90%
//! 沙盒平均 CPU 不到申请量 5%，等待模型生成期间要保住内存但 CPU 空闲；大量相同
//! 只读内容若各存一份，内存先成瓶颈。外部报道（**aiwiki.ai / byteiota.com 对 DSEC
//! 的实测，非本仓复测**）称 virtio-pmem+DAX 让峰值内存降 40.2%、DAMON+balloon
//! 累计再降 21.2%、超卖 50×+。
//!
//! 本版**不碰任何 OS 原语**（virtio-pmem/DAX/DAMON/balloon 是 Linux-MicroVM 专有，
//! 按设计在 v3.7.6 声明并在无原语平台具名拒绝）。本版把其中**跨平台可测、与内核
//! 无关**的部分真实落地为一套确定性记账与准入策略：
//!
//! - **共享额度只计一次**：多个沙盒映射同一内容寻址只读块（sha256 id）时，该块
//!   字节在「已提交（committed，宿主必须真实保住）」口径里只计一次（取并集），
//!   而不是每沙盒重复计；「名义（nominal，各自申请）」口径仍逐个计。
//! - **两级准入**：
//!   1. 硬安全闸：投影 committed = 各沙盒独占可写之和 + 共享内容并集，必须
//!      `<= physical_bytes`，超了具名拒绝（宿主物理上保不住）。
//!   2. 超卖上限闸：投影 nominal 允许超过物理内存（稀疏负载实际用不满），但
//!      不得超过 `physical × 超卖比`（默认上限可配，外部 50× 仅为报道口径）。
//!
//! 所有比例用千分点整数 + u128 中间运算，不引浮点、不溢出。

use std::collections::HashMap;

/// 只读共享内容的内容寻址 id（通常是块的 sha256 十六进制）。
pub type ShareId = String;

/// 一块被沙盒映射的只读共享内容。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SharedRef {
    /// 内容寻址 id；相同 id 视为同一份只读内容，全机只计一次。
    pub id: ShareId,
    /// 该内容的字节数（>0）。
    pub bytes: u64,
}

/// 一个沙盒的内存申请：独占可写 + 映射的只读共享内容。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemorySandboxRequest {
    pub sandbox_id: String,
    /// 独占可写内存（每沙盒独立计，不共享）。
    pub private_bytes: u64,
    /// 映射的只读内容（按内容 id 去重，全机并集只计一次）。
    pub shared: Vec<SharedRef>,
}

/// 内存池/准入相关的全具名错误（无 panic 路径）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MemoryError {
    #[error("物理内存额度必须 > 0")]
    BadPhysical,
    #[error("超卖比（千分点）必须 ≥ 1000（1×），得到 {0}")]
    BadOvercommitRatio(u64),
    #[error("沙盒 id 不能为空")]
    EmptySandboxId,
    #[error("沙盒 {0} 已存在，不能重复准入")]
    DuplicateSandbox(String),
    #[error("沙盒 {0} 不存在，无法释放")]
    UnknownSandbox(String),
    #[error("沙盒 {sandbox} 的共享引用 id 为空")]
    EmptyShareId { sandbox: String },
    #[error("沙盒 {sandbox} 的共享内容 {id} 字节数必须 > 0")]
    BadShareBytes { sandbox: String, id: String },
    #[error("沙盒 {sandbox} 的共享内容 {id} 重复声明")]
    DuplicateShareRef { sandbox: String, id: String },
    #[error("准入被拒（物理内存硬闸）：投影已提交 {projected_committed} > 物理 {physical}")]
    RejectCommittedExceedsPhysical {
        projected_committed: u64,
        physical: u64,
    },
    #[error("准入被拒（超卖上限闸）：投影名义 {projected_nominal} > 上限 {ceiling}（物理 {physical} × {ratio_permille}/1000）")]
    RejectOvercommitCeiling {
        projected_nominal: u64,
        ceiling: u64,
        physical: u64,
        ratio_permille: u64,
    },
    #[error("空闲回收观测里沙盒 id 重复：{0}")]
    DuplicateIdleObservation(String),
}

/// 超卖比（nominal / physical 上限），千分点；1000=1×，50000=50×。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvercommitRatio {
    pub permille: u64,
}

impl OvercommitRatio {
    pub fn from_permille(permille: u64) -> Result<Self, MemoryError> {
        if permille < 1000 {
            return Err(MemoryError::BadOvercommitRatio(permille));
        }
        Ok(Self { permille })
    }
    /// 以「倍」构造：`times(50)` = 50× = 50000‰。
    pub fn times(times: u32) -> Result<Self, MemoryError> {
        if times < 1 {
            return Err(MemoryError::BadOvercommitRatio(0));
        }
        Ok(Self {
            permille: times as u64 * 1000,
        })
    }
}

/// 一次准入评估结果（纯投影，不落库）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Admission {
    pub admitted: bool,
    /// 准入后宿主必须真实保住的字节（独占之和 + 共享并集）。
    pub committed_bytes: u64,
    /// 准入后各沙盒名义申请之和（共享按沙盒重复计）。
    pub nominal_bytes: u64,
    /// 共享去重省下的字节（nominal 中共享部分 − 并集）。
    pub shared_dedup_saving: u64,
    /// committed/physical 的千分点（0..=1000 为物理内）。
    pub committed_utilization_permille: u64,
    /// nominal/committed 的千分点（空池为 1000）。
    pub observed_overcommit_permille: u64,
    /// nominal 超卖上限（字节）= physical*ratio/1000。
    pub nominal_ceiling: u64,
}

/// 单机内存配额池（确定性记账，可被真实调度器复用）。
#[derive(Debug, Clone)]
pub struct MemoryPool {
    physical_bytes: u64,
    max_overcommit_permille: u64,
    /// 每沙盒独占可写字节。
    private: HashMap<String, u64>,
    /// 每沙盒名义共享字节（各自映射，重复计），用于 nominal 与释放。
    private_shared_sum: HashMap<String, u64>,
    /// 每沙盒持有的共享引用（释放时据此重算并集）。
    refs: HashMap<String, Vec<(ShareId, u64)>>,
}

impl MemoryPool {
    pub fn new(physical_bytes: u64, ratio: OvercommitRatio) -> Result<Self, MemoryError> {
        if physical_bytes == 0 {
            return Err(MemoryError::BadPhysical);
        }
        Ok(Self {
            physical_bytes,
            max_overcommit_permille: ratio.permille,
            private: HashMap::new(),
            private_shared_sum: HashMap::new(),
            refs: HashMap::new(),
        })
    }

    pub fn physical_bytes(&self) -> u64 {
        self.physical_bytes
    }
    pub fn sandbox_count(&self) -> usize {
        self.private.len()
    }

    /// 全机共享内容并集：每个内容 id 取最大声明字节（内容寻址大小确定，取 max 稳健）。
    fn shared_union_map(&self) -> HashMap<&str, u64> {
        let mut union: HashMap<&str, u64> = HashMap::new();
        for list in self.refs.values() {
            for (id, b) in list {
                union
                    .entry(id.as_str())
                    .and_modify(|cur| {
                        if *b > *cur {
                            *cur = *b;
                        }
                    })
                    .or_insert(*b);
            }
        }
        union
    }

    /// 已提交字节：各沙盒独占之和 + 共享内容并集（同一只读内容全机只计一次）。
    pub fn committed_bytes(&self) -> u64 {
        let priv_sum: u128 = self.private.values().map(|b| *b as u128).sum();
        let union_sum: u128 = self.shared_union_map().values().map(|b| *b as u128).sum();
        saturating_u64(priv_sum + union_sum)
    }

    /// 名义字节：每沙盒（独占 + 自己映射的共享）之和；共享跨沙盒重复计。
    pub fn nominal_bytes(&self) -> u64 {
        let total: u128 = self.private.values().map(|b| *b as u128).sum::<u128>()
            + self
                .private_shared_sum
                .values()
                .map(|b| *b as u128)
                .sum::<u128>();
        saturating_u64(total)
    }

    /// 共享去重省下的字节：各沙盒共享之和 − 并集。
    pub fn shared_dedup_saving(&self) -> u64 {
        let per_sandbox: u128 = self.private_shared_sum.values().map(|b| *b as u128).sum();
        let union: u128 = self.shared_union_map().values().map(|b| *b as u128).sum();
        saturating_u64(per_sandbox.saturating_sub(union))
    }

    /// 名义超卖上限（字节）：physical*ratio/1000，u128 中间运算。
    pub fn nominal_ceiling(&self) -> u64 {
        saturating_u64(
            (self.physical_bytes as u128) * (self.max_overcommit_permille as u128) / 1000,
        )
    }

    /// 校验单个申请的内部一致性（不依赖池当前状态）。
    fn validate(req: &MemorySandboxRequest) -> Result<(), MemoryError> {
        if req.sandbox_id.is_empty() {
            return Err(MemoryError::EmptySandboxId);
        }
        let mut seen = HashMap::new();
        for r in &req.shared {
            if r.id.is_empty() {
                return Err(MemoryError::EmptyShareId {
                    sandbox: req.sandbox_id.clone(),
                });
            }
            if r.bytes == 0 {
                return Err(MemoryError::BadShareBytes {
                    sandbox: req.sandbox_id.clone(),
                    id: r.id.clone(),
                });
            }
            if seen.insert(r.id.clone(), ()).is_some() {
                return Err(MemoryError::DuplicateShareRef {
                    sandbox: req.sandbox_id.clone(),
                    id: r.id.clone(),
                });
            }
        }
        Ok(())
    }

    /// 纯投影：若加入 `req`，计算准入结论（不修改池）。
    pub fn project(&self, req: &MemorySandboxRequest) -> Result<Admission, MemoryError> {
        Self::validate(req)?;
        if self.private.contains_key(&req.sandbox_id) {
            return Err(MemoryError::DuplicateSandbox(req.sandbox_id.clone()));
        }

        let req_shared_sum: u128 = req.shared.iter().map(|r| r.bytes as u128).sum();

        // 投影 committed：在现有并集上并入 req 的共享内容。
        let mut union = self.shared_union_map();
        for r in &req.shared {
            union
                .entry(r.id.as_str())
                .and_modify(|cur| {
                    if r.bytes > *cur {
                        *cur = r.bytes;
                    }
                })
                .or_insert(r.bytes);
        }
        let priv_total: u128 =
            self.private.values().map(|b| *b as u128).sum::<u128>() + req.private_bytes as u128;
        let union_total: u128 = union.values().map(|b| *b as u128).sum();
        let projected_committed = saturating_u64(priv_total + union_total);

        // 投影 nominal：现有 nominal + 本沙盒（独占 + 自己的共享）。
        let projected_nominal = saturating_u64(
            self.nominal_bytes() as u128 + req.private_bytes as u128 + req_shared_sum,
        );

        // 闸 1：物理硬安全。
        if projected_committed > self.physical_bytes {
            return Err(MemoryError::RejectCommittedExceedsPhysical {
                projected_committed,
                physical: self.physical_bytes,
            });
        }
        // 闸 2：名义超卖上限。
        let ceiling = self.nominal_ceiling();
        if projected_nominal > ceiling {
            return Err(MemoryError::RejectOvercommitCeiling {
                projected_nominal,
                ceiling,
                physical: self.physical_bytes,
                ratio_permille: self.max_overcommit_permille,
            });
        }

        let util = projected_committed as u128 * 1000 / self.physical_bytes as u128;
        let over = if projected_committed == 0 {
            1000
        } else {
            projected_nominal as u128 * 1000 / projected_committed as u128
        };
        // 去重节省（投影后）：名义共享之和 − 并集。
        let per_sandbox_shared: u128 = self
            .private_shared_sum
            .values()
            .map(|b| *b as u128)
            .sum::<u128>()
            + req_shared_sum;
        let saving = saturating_u64(per_sandbox_shared.saturating_sub(union_total));

        Ok(Admission {
            admitted: true,
            committed_bytes: projected_committed,
            nominal_bytes: projected_nominal,
            shared_dedup_saving: saving,
            committed_utilization_permille: saturating_u64(util),
            observed_overcommit_permille: saturating_u64(over),
            nominal_ceiling: ceiling,
        })
    }

    /// 评估并在通过时落库（原子：被拒则池不变）。
    pub fn admit(&mut self, req: MemorySandboxRequest) -> Result<Admission, MemoryError> {
        let decision = self.project(&req)?;
        let sum: u64 = req.shared.iter().map(|r| r.bytes).sum();
        self.private
            .insert(req.sandbox_id.clone(), req.private_bytes);
        self.private_shared_sum.insert(req.sandbox_id.clone(), sum);
        self.refs.insert(
            req.sandbox_id,
            req.shared.into_iter().map(|r| (r.id, r.bytes)).collect(),
        );
        Ok(decision)
    }

    /// 释放沙盒；共享并集随后自动重算（最后一个持有者走后该内容才消失）。
    pub fn release(&mut self, sandbox_id: &str) -> Result<(), MemoryError> {
        if self.private.remove(sandbox_id).is_none() {
            return Err(MemoryError::UnknownSandbox(sandbox_id.to_string()));
        }
        self.private_shared_sum.remove(sandbox_id);
        self.refs.remove(sandbox_id);
        Ok(())
    }

    /// 当前池状态快照（供 PMB `memory_status` 只读查询）。
    pub fn status(&self) -> PoolStatus {
        let committed = self.committed_bytes();
        let nominal = self.nominal_bytes();
        PoolStatus {
            physical_bytes: self.physical_bytes,
            max_overcommit_permille: self.max_overcommit_permille,
            sandbox_count: self.sandbox_count() as u64,
            committed_bytes: committed,
            nominal_bytes: nominal,
            shared_dedup_saving: self.shared_dedup_saving(),
            nominal_ceiling: self.nominal_ceiling(),
            committed_utilization_permille: saturating_u64(
                committed as u128 * 1000 / self.physical_bytes as u128,
            ),
            observed_overcommit_permille: if committed == 0 {
                1000
            } else {
                saturating_u64(nominal as u128 * 1000 / committed as u128)
            },
            distinct_shared_contents: self.shared_union_map().len() as u64,
        }
    }
}

/// 池状态快照。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PoolStatus {
    pub physical_bytes: u64,
    pub max_overcommit_permille: u64,
    pub sandbox_count: u64,
    pub committed_bytes: u64,
    pub nominal_bytes: u64,
    pub shared_dedup_saving: u64,
    pub nominal_ceiling: u64,
    pub committed_utilization_permille: u64,
    pub observed_overcommit_permille: u64,
    pub distinct_shared_contents: u64,
}

// ───────────────────────── v3.7.5：等待期保内存 + 空闲优先回收 ─────────────────────────

/// 沙盒活动状态：等待模型生成期间为 [`ActivityState::Waiting`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActivityState {
    /// 正在执行动作（CPU 活跃路径上）。
    Running,
    /// 动作已完成、正在等待模型生成下一步；文件/进程状态必须保住，但 CPU 空闲。
    Waiting,
}

/// 时延等级（回收排序的次级键；CPU 两级优先级在 v3.7.7 正式建模）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LatencyClass {
    /// 时延敏感：同等条件下最后回收它的空闲页。
    Sensitive,
    /// 时延容忍：同等条件下优先回收它的空闲页。
    Tolerant,
}

/// 一个沙盒在某时刻的空闲内存观测（纯输入，由调用方给出，本模块不读 OS 计数）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SandboxIdleObservation {
    pub sandbox_id: String,
    pub state: ActivityState,
    /// 距上次活动的毫秒数（注入时钟口径，越大越空闲）。
    pub idle_ms: u64,
    pub latency: LatencyClass,
    /// **等待期必须保住**的状态内存（匿名/进程/文件状态）；回收规划器永不触碰。
    pub reserved_bytes: u64,
    /// 当前可被安全丢弃的空闲/干净页字节（可经缺页重新取回），是唯一可回收来源。
    pub reclaimable_idle_bytes: u64,
}

/// 一条回收步骤：从某个沙盒回收多少空闲页，以及选中理由（确定性、可审计）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReclaimStep {
    pub sandbox_id: String,
    pub bytes: u64,
    /// 例如 `waiting/tolerant/idle=12000ms`，说明排序依据。
    pub reason: String,
}

/// 一次空闲回收的确定性计划（纯建议，不释放沙盒、不改配额账）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReclaimPlan {
    pub target_bytes: u64,
    /// 计划回收总量 = min(target, 全部可回收之和)。
    pub reclaimed_bytes: u64,
    /// 仍缺口（target − reclaimed）；>0 表示空闲页不够，调用方须另做准入/balloon 决策。
    pub shortfall_bytes: u64,
    /// 是否足额。
    pub sufficient: bool,
    /// 按回收先后排序的步骤。
    pub steps: Vec<ReclaimStep>,
}

/// 空闲优先回收规划器（无状态纯函数）。
pub struct IdleReclaimer;

impl IdleReclaimer {
    /// 依据「等待期保内存 + 空闲优先回收」产出确定性计划。
    ///
    /// 规则：
    /// 1. **保内存**：只从各沙盒申报的 `reclaimable_idle_bytes` 取，永不触碰
    ///    `reserved_bytes`，也绝不释放任何沙盒（Waiting 沙盒的状态槽位始终保留）。
    ///    即使 target 超过全部可回收量，缺口记为 `shortfall_bytes`，绝不向保留内存伸手。
    /// 2. **空闲优先排序**（键从先到后）：Waiting 早于 Running；同为 Tolerant 早于
    ///    Sensitive；`idle_ms` 大者优先；仍并列则按 sandbox_id 字典序，保证确定性。
    /// 3. 贪心取 `min(剩余缺口, 该沙盒可回收)`，达到 target 即停（不多收）。
    pub fn plan(
        observations: &[SandboxIdleObservation],
        target_bytes: u64,
    ) -> Result<ReclaimPlan, MemoryError> {
        let mut seen = std::collections::HashSet::new();
        for o in observations {
            if o.sandbox_id.is_empty() {
                return Err(MemoryError::EmptySandboxId);
            }
            if !seen.insert(o.sandbox_id.clone()) {
                return Err(MemoryError::DuplicateIdleObservation(o.sandbox_id.clone()));
            }
        }

        // target=0 是合法空操作；无任何回收动作。
        if target_bytes == 0 {
            return Ok(ReclaimPlan {
                target_bytes: 0,
                reclaimed_bytes: 0,
                shortfall_bytes: 0,
                sufficient: true,
                steps: Vec::new(),
            });
        }

        // 仅保留确实有空闲页可回收的候选，并排序。
        let mut order: Vec<&SandboxIdleObservation> = observations
            .iter()
            .filter(|o| o.reclaimable_idle_bytes > 0)
            .collect();
        order.sort_by(|a, b| {
            let state_rank = |s: ActivityState| match s {
                ActivityState::Waiting => 0u8,
                ActivityState::Running => 1,
            };
            let lat_rank = |l: LatencyClass| match l {
                LatencyClass::Tolerant => 0u8,
                LatencyClass::Sensitive => 1,
            };
            state_rank(a.state)
                .cmp(&state_rank(b.state))
                .then(lat_rank(a.latency).cmp(&lat_rank(b.latency)))
                .then(b.idle_ms.cmp(&a.idle_ms)) // 空闲更久者排前（降序）
                .then(a.sandbox_id.cmp(&b.sandbox_id))
        });

        let mut remaining = target_bytes;
        let mut steps: Vec<ReclaimStep> = Vec::new();
        for o in order {
            if remaining == 0 {
                break;
            }
            let take = o.reclaimable_idle_bytes.min(remaining);
            if take == 0 {
                continue;
            }
            let state = if o.state == ActivityState::Waiting {
                "waiting"
            } else {
                "running"
            };
            let lat = if o.latency == LatencyClass::Tolerant {
                "tolerant"
            } else {
                "sensitive"
            };
            steps.push(ReclaimStep {
                sandbox_id: o.sandbox_id.clone(),
                bytes: take,
                reason: format!("{state}/{lat}/idle={}ms", o.idle_ms),
            });
            remaining -= take;
        }

        let reclaimed = target_bytes - remaining;
        let available: u128 = observations
            .iter()
            .map(|o| o.reclaimable_idle_bytes as u128)
            .sum();
        let total_available = saturating_u64(available);
        let reclaimed = reclaimed.min(total_available);
        let shortfall = target_bytes.saturating_sub(reclaimed);
        Ok(ReclaimPlan {
            target_bytes,
            reclaimed_bytes: reclaimed,
            shortfall_bytes: shortfall,
            sufficient: shortfall == 0,
            steps,
        })
    }
}

fn saturating_u64(v: u128) -> u64 {
    if v > u64::MAX as u128 {
        u64::MAX
    } else {
        v as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: &str, private: u64, shared: &[(&str, u64)]) -> MemorySandboxRequest {
        MemorySandboxRequest {
            sandbox_id: id.to_string(),
            private_bytes: private,
            shared: shared
                .iter()
                .map(|(id, b)| SharedRef {
                    id: (*id).to_string(),
                    bytes: *b,
                })
                .collect(),
        }
    }

    fn obs(
        id: &str,
        state: ActivityState,
        idle_ms: u64,
        lat: LatencyClass,
        reserved: u64,
        reclaimable: u64,
    ) -> SandboxIdleObservation {
        SandboxIdleObservation {
            sandbox_id: id.to_string(),
            state,
            idle_ms,
            latency: lat,
            reserved_bytes: reserved,
            reclaimable_idle_bytes: reclaimable,
        }
    }

    fn pool(phys: u64, times: u32) -> MemoryPool {
        MemoryPool::new(phys, OvercommitRatio::times(times).unwrap()).unwrap()
    }

    #[test]
    fn shared_readonly_content_counted_once() {
        // 两沙盒：各独占 100，都映射同一只读块 60。
        let mut p = pool(1000, 1);
        let a = p.admit(req("a", 100, &[("x", 60)])).unwrap();
        assert_eq!(a.committed_bytes, 160); // 100 + 60
        let b = p.admit(req("b", 100, &[("x", 60)])).unwrap();
        // committed：独占 200 + 共享并集 60 = 260（不是 320）。
        assert_eq!(b.committed_bytes, 260);
        // nominal：各自 (100+60) = 320（共享重复计）。
        assert_eq!(b.nominal_bytes, 320);
        // 去重省下正好一份共享 60。
        assert_eq!(b.shared_dedup_saving, 60);
        assert_eq!(p.status().distinct_shared_contents, 1);
    }

    #[test]
    fn distinct_shared_contents_all_count() {
        let mut p = pool(1000, 1);
        p.admit(req("a", 100, &[("x", 60)])).unwrap();
        let b = p.admit(req("b", 100, &[("y", 40)])).unwrap();
        assert_eq!(b.committed_bytes, 200 + 60 + 40); // 300
        assert_eq!(b.nominal_bytes, (100 + 60) + (100 + 40)); // 300（无重复）
        assert_eq!(b.shared_dedup_saving, 0); // 无共享
    }

    #[test]
    fn physical_hard_gate_rejects() {
        // 物理 250：committed 260 > 250，即使 1× 超卖也必须物理闸拒绝。
        let mut p = pool(250, 50);
        p.admit(req("a", 100, &[("x", 60)])).unwrap();
        let e = p.admit(req("b", 100, &[("x", 60)])).unwrap_err();
        assert!(matches!(
            e,
            MemoryError::RejectCommittedExceedsPhysical {
                projected_committed: 260,
                physical: 250
            }
        ));
        // 被拒后池不变（原子）。
        assert_eq!(p.sandbox_count(), 1);
        assert_eq!(p.committed_bytes(), 160);
    }

    #[test]
    fn overcommit_ceiling_gate_rejects() {
        // 物理 1000、超卖 2×：名义上限 2000。
        // 多个沙盒映射同一大块：committed 只计一次（物理闸不触发），
        // nominal 逐沙盒累加，用来单独验证「超卖上限闸」。
        let mut p = pool(1000, 2);
        // 共享块 900：committed 900，a nominal 900。
        p.admit(req("a", 0, &[("big", 900)])).unwrap();
        // b 再映射同一块：committed 仍 900，nominal 1800（≤2000，准入）。
        let b = p.admit(req("b", 0, &[("big", 900)])).unwrap();
        assert_eq!(b.committed_bytes, 900);
        assert_eq!(b.nominal_bytes, 1800);
        // c 第三次映射：nominal 2700 > 2000 → 超卖闸拒（committed 仍 900，未触物理闸）。
        let e = p.admit(req("c", 0, &[("big", 900)])).unwrap_err();
        assert!(matches!(
            e,
            MemoryError::RejectOvercommitCeiling {
                projected_nominal: 2700,
                ceiling: 2000,
                ..
            }
        ));
        assert_eq!(p.sandbox_count(), 2); // 被拒不落库
    }

    #[test]
    fn release_recomputes_shared_union() {
        let mut p = pool(1000, 10);
        p.admit(req("a", 100, &[("x", 60)])).unwrap();
        p.admit(req("b", 100, &[("x", 60), ("y", 40)])).unwrap();
        assert_eq!(p.committed_bytes(), 200 + 60 + 40); // 300
                                                        // 释放 b：x 仍被 a 持有保留，y 消失。
        p.release("b").unwrap();
        assert_eq!(p.committed_bytes(), 100 + 60); // 160
        assert_eq!(p.shared_dedup_saving(), 0);
        // 释放 a：x 也消失。
        p.release("a").unwrap();
        assert_eq!(p.committed_bytes(), 0);
        assert_eq!(p.nominal_bytes(), 0);
        assert!(p.release("a").is_err()); // 未知沙盒
    }

    #[test]
    fn inconsistent_shared_size_union_uses_max() {
        let mut p = pool(1000, 10);
        p.admit(req("a", 0, &[("x", 60)])).unwrap();
        let b = p.admit(req("b", 0, &[("x", 80)])).unwrap();
        // 同一内容 id 声明不一致大小：并集取 max=80（确定性，不崩）。
        assert_eq!(b.committed_bytes, 80);
    }

    #[test]
    fn constructor_and_request_validation() {
        assert!(matches!(
            MemoryPool::new(0, OvercommitRatio::times(1).unwrap()),
            Err(MemoryError::BadPhysical)
        ));
        assert!(matches!(
            OvercommitRatio::times(0),
            Err(MemoryError::BadOvercommitRatio(0))
        ));
        assert!(OvercommitRatio::times(50).unwrap().permille == 50000);

        let mut p = pool(1000, 1);
        assert!(matches!(
            p.admit(req("", 10, &[])),
            Err(MemoryError::EmptySandboxId)
        ));
        assert!(matches!(
            p.admit(req("a", 10, &[("", 10)])),
            Err(MemoryError::EmptyShareId { .. })
        ));
        assert!(matches!(
            p.admit(req("a", 10, &[("x", 0)])),
            Err(MemoryError::BadShareBytes { .. })
        ));
        assert!(matches!(
            p.admit(req("a", 10, &[("x", 5), ("x", 6)])),
            Err(MemoryError::DuplicateShareRef { .. })
        ));
        p.admit(req("a", 10, &[])).unwrap();
        assert!(matches!(
            p.admit(req("a", 10, &[])),
            Err(MemoryError::DuplicateSandbox(_))
        ));
    }

    #[test]
    fn ratio_maths_no_overflow_large_values() {
        // 物理 ~TiB，50× 上限；u128 中间运算不溢出。
        let tib = 1u64 << 40;
        let mut p = pool(tib, 50);
        let s = p.admit(req("a", tib / 2, &[])).unwrap();
        assert_eq!(s.nominal_ceiling, tib * 50);
        assert_eq!(s.committed_utilization_permille, 500);
        assert_eq!(s.observed_overcommit_permille, 1000);
    }

    #[test]
    fn empty_pool_status_baseline() {
        let p = pool(1000, 50);
        let s = p.status();
        assert_eq!(s.sandbox_count, 0);
        assert_eq!(s.committed_bytes, 0);
        assert_eq!(s.observed_overcommit_permille, 1000);
        assert_eq!(s.nominal_ceiling, 50000);
    }

    // ── v3.7.5：等待期保内存 + 空闲优先回收 ──

    #[test]
    fn reclaim_waiting_idle_before_running_busy() {
        // 一个 Running/Sensitive/刚活动 的沙盒有空闲页；
        // 一个 Waiting/Tolerant/空闲很久 的沙盒也有。空闲优先：先取后者。
        let os = vec![
            obs(
                "run",
                ActivityState::Running,
                10,
                LatencyClass::Sensitive,
                1000,
                100,
            ),
            obs(
                "wait",
                ActivityState::Waiting,
                12_000,
                LatencyClass::Tolerant,
                1000,
                100,
            ),
        ];
        let plan = IdleReclaimer::plan(&os, 100).unwrap();
        assert!(plan.sufficient);
        assert_eq!(plan.reclaimed_bytes, 100);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].sandbox_id, "wait");
        assert!(plan.steps[0].reason.contains("waiting"));
    }

    #[test]
    fn reclaim_does_not_touch_reserved_and_reports_shortfall() {
        // 等待沙盒保留 5000 状态内存，但空闲页只有 30；target=100。
        // 必须：只回收 30，绝不向 5000 保留内存伸手，缺口=70，不足额。
        let os = vec![obs(
            "w",
            ActivityState::Waiting,
            9000,
            LatencyClass::Tolerant,
            5000,
            30,
        )];
        let plan = IdleReclaimer::plan(&os, 100).unwrap();
        assert!(!plan.sufficient);
        assert_eq!(plan.reclaimed_bytes, 30);
        assert_eq!(plan.shortfall_bytes, 70);
        assert_eq!(plan.steps[0].bytes, 30);
        // 回收永不超过申报可回收量。
        assert!(plan.reclaimed_bytes <= 30);
    }

    #[test]
    fn reclaim_never_over_reclaims_and_stops_at_target() {
        // 两个各可回收 100；target=150 → 取 100 + 50，不多收。
        let os = vec![
            obs(
                "a",
                ActivityState::Waiting,
                100,
                LatencyClass::Tolerant,
                10,
                100,
            ),
            obs(
                "b",
                ActivityState::Waiting,
                50,
                LatencyClass::Tolerant,
                10,
                100,
            ),
        ];
        let plan = IdleReclaimer::plan(&os, 150).unwrap();
        assert_eq!(plan.reclaimed_bytes, 150);
        assert_eq!(plan.shortfall_bytes, 0);
        assert_eq!(plan.steps[0].sandbox_id, "a"); // idle 100 > 50
        assert_eq!(plan.steps[0].bytes, 100);
        assert_eq!(plan.steps[1].sandbox_id, "b");
        assert_eq!(plan.steps[1].bytes, 50);
    }

    #[test]
    fn reclaim_deterministic_tie_break_by_id() {
        // 同状态/同类/同 idle → 字典序；同样输入两次计划逐字节一致。
        let os = vec![
            obs(
                "zeta",
                ActivityState::Waiting,
                100,
                LatencyClass::Tolerant,
                0,
                10,
            ),
            obs(
                "alpha",
                ActivityState::Waiting,
                100,
                LatencyClass::Tolerant,
                0,
                10,
            ),
        ];
        let p1 = IdleReclaimer::plan(&os, 20).unwrap();
        let p2 = IdleReclaimer::plan(&os, 20).unwrap();
        assert_eq!(p1, p2);
        assert_eq!(p1.steps[0].sandbox_id, "alpha");
        assert_eq!(p1.steps[1].sandbox_id, "zeta");
    }

    #[test]
    fn reclaim_skips_zero_idle_and_target_zero_is_noop() {
        let os = vec![
            obs(
                "busy",
                ActivityState::Running,
                0,
                LatencyClass::Sensitive,
                500,
                0,
            ),
            obs(
                "w",
                ActivityState::Waiting,
                5,
                LatencyClass::Tolerant,
                500,
                0,
            ),
        ];
        // 无可回收：target=50 → 0 回收、缺口 50。
        let plan = IdleReclaimer::plan(&os, 50).unwrap();
        assert!(!plan.sufficient);
        assert_eq!(plan.reclaimed_bytes, 0);
        assert!(plan.steps.is_empty());
        // target=0 合法空操作。
        let nop = IdleReclaimer::plan(&os, 0).unwrap();
        assert!(nop.sufficient && nop.steps.is_empty());
    }

    #[test]
    fn reclaim_tolerant_before_sensitive_same_state() {
        let os = vec![
            obs(
                "sens",
                ActivityState::Waiting,
                1000,
                LatencyClass::Sensitive,
                0,
                40,
            ),
            obs(
                "tol",
                ActivityState::Waiting,
                1000,
                LatencyClass::Tolerant,
                0,
                40,
            ),
        ];
        let plan = IdleReclaimer::plan(&os, 40).unwrap();
        assert_eq!(plan.steps[0].sandbox_id, "tol");
    }

    #[test]
    fn reclaim_validates_observations() {
        assert!(matches!(
            IdleReclaimer::plan(
                &[obs(
                    "",
                    ActivityState::Waiting,
                    1,
                    LatencyClass::Tolerant,
                    0,
                    1
                )],
                10
            ),
            Err(MemoryError::EmptySandboxId)
        ));
        let dup = vec![
            obs("x", ActivityState::Waiting, 1, LatencyClass::Tolerant, 0, 1),
            obs("x", ActivityState::Waiting, 2, LatencyClass::Tolerant, 0, 1),
        ];
        assert!(matches!(
            IdleReclaimer::plan(&dup, 10),
            Err(MemoryError::DuplicateIdleObservation(_))
        ));
    }
}
