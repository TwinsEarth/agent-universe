//! v3.7.9 突发涌入准入控制（burst admission）。
//!
//! 创建请求"突然集中涌入"时，在**有限 CPU 容量 + 内存配额池**两个维度上，对一批
//! arrival 做确定性的 `Admitted / Queued / Rejected` 判定：
//!
//! * **CPU 软闸**：已在跑沙盒先占容量，剩余按 [`crate::ausec::cpu::arbitrate_priority`]
//!   裁定好的顺序（Sensitive 先、权重降、id 升）逐个容纳；放不下先排队而非直接拒。
//! * **内存双闸**（复用 v3.7.4 `MemoryPool::project`）：
//!   - 触动物理硬闸（投影 committed > physical）→ **Rejected**（物理放不下，排队也
//!     不会凭空产生物理内存）；
//!   - 仅触碰超卖上限闸（projection nominal > ceiling）→ **Queued**（等空闲回收后可进）。
//! * **突发整形闸**：本波直接准入数达到 `burst_max_admit` 后，其余即使此刻放得下也
//!   一律排队（避免一次性把全部请求灌进调度器），但队列满了仍拒绝（fail-closed，
//!   绝不无界排队）。
//!
//! 全部纯确定性、整数记账、零 syscall、零 unsafe、无 panic；它是准入**决策面**，
//! 不真正创建沙盒、不真正限速、不真正回收内存。
//!
//! 信任仲裁完全复用 v3.7.7：existing 与 arrivals 合并后一次性 `arbitrate_priority`，
//! 黑名单、低信任级自报 Sensitive、空/重复 id、权重/申请越界任一非法 → 整体 fail-closed
//! （不返回部分结果）。

use crate::ausec::cpu::{arbitrate_priority, CpuError, CpuSandboxRequest};
use crate::ausec::memory::{MemoryError, MemoryPool, MemorySandboxRequest};

/// 突发准入全具名错误（无 panic 路径）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BurstError {
    /// 透传 v3.7.7 CPU 信任仲裁/申报错误。
    #[error("CPU 信任仲裁失败: {0}")]
    Cpu(#[from] CpuError),
    /// 透传内存池构造/投影错误（existing 集合自相矛盾等）。
    #[error("内存配额记账失败: {0}")]
    Memory(#[from] MemoryError),
    /// 突发到达列表为空（没有要裁决的涌入请求）。
    #[error("arrivals 不能为空（突发准入至少需要一个到达请求）")]
    NoArrivals,
    /// 一个到达请求的 CPU 身份与内存身份不一致（防止两套 id 绕过单一仲裁）。
    #[error("arrival 的 CPU sandbox_id({cpu}) 必须与内存 sandbox_id({memory}) 一致")]
    IdentityMismatch { cpu: String, memory: String },
    /// 准入投影在已 project 成功后 admit 仍失败（理论不可达，出现即内部不变量被破坏）。
    #[error("准入内部不变量被破坏（project 成功后 admit 失败）: {0}")]
    Invariant(String),
}

/// 排队原因（一个 arrival 可能同时因多个原因排队，全部如实记录）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueReason {
    /// 当前剩余 CPU 容量不足以满足该请求（可在负载下降后进入）。
    CpuShortfall,
    /// 触达名义超卖上限，需等待空闲回收腾出超卖空间。
    OvercommitCeiling,
    /// 本波直接准入名额已用满（突发整形），下一窗再考虑。
    BurstCapped,
}

/// 拒绝原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    /// 投影独占+共享已提交量超过物理内存硬闸。
    PhysicalHardLimit,
    /// 需要排队但等待队列容量已满（不无界排队）。
    QueueFull,
}

/// 单个到达请求的裁决。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BurstDecision {
    /// 本波直接准入（已在投影内存池中落账、占用 CPU 名额）。
    Admitted,
    /// 暂不启动，进入等待队列（携带全部排队原因）。
    Queued,
    /// 明确拒绝（携带拒绝原因）。
    Rejected,
}

/// 突发整形/队列参数（全部可选；缺省不整形、队列有界默认值见入口）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BurstLimits {
    /// 本波最多直接准入数；None = 不限（仅受 CPU/内存约束）。
    pub burst_max_admit: Option<u64>,
    /// 等待队列容量；None = 不限制排队（仅物理硬闸会拒绝）。
    pub queue_capacity: Option<u64>,
}

/// 一个突发到达请求：CPU 画像与内存申请成对出现，且必须属于同一沙盒。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BurstArrival {
    pub cpu: CpuSandboxRequest,
    pub memory: MemorySandboxRequest,
}

/// 单个 arrival 的裁决明细。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ArrivalDecision {
    pub sandbox_id: String,
    pub decision: BurstDecision,
    /// 该 arrival 在涌入批中的原始位置。
    pub arrival_index: usize,
    /// 仲裁后实际生效的优先级（Sensitive=2、Tolerant=1），便于审计排序依据。
    pub effective_priority: u8,
    /// 排队原因（decision=queued 时非空）。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub queue_reasons: Vec<QueueReason>,
    /// 拒绝原因（decision=rejected 时存在）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reject_reason: Option<RejectReason>,
}

/// 突发裁决汇总。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BurstSummary {
    pub arrivals: u64,
    pub admitted: u64,
    pub queued: u64,
    pub rejected: u64,
    /// 已在跑 + 本波准入沙盒的 CPU 申请合计（millis）。
    pub cpu_projected_used_millis: u64,
    pub cpu_capacity_millis: u64,
    /// 裁决后 CPU 剩余容量（容量不足时为 0，不用 saturating 掩盖）。
    pub cpu_remaining_millis: u64,
    /// 裁决后内存池投影：已提交字节。
    pub committed_bytes: u64,
    /// 裁决后内存池投影：名义申请字节。
    pub nominal_bytes: u64,
    /// 物理内存硬闸字节。
    pub physical_bytes: u64,
    /// 名义超卖上限字节。
    pub nominal_ceiling: u64,
}

/// 一次完整突发涌入裁决。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BurstAdmission {
    /// 裁决明细，按**裁决顺序**（Sensitive 先、权重降、id 升）排列。
    pub decisions: Vec<ArrivalDecision>,
    pub summary: BurstSummary,
}

/// 运行突发涌入准入裁决（纯函数）。
///
/// * `capacity`：CPU 容量（millis，必须 >0，由仲裁器校验）；
/// * `existing_cpu`：已在运行沙盒的 CPU 画像（先占容量，参与信任仲裁）；
/// * `pool`：已按 existing 内存集合建好账的内存配额池（函数内对准入 arrival 落账投影）；
/// * `arrivals`：本波突发到达（非空），CPU/内存身份必须一致；
/// * `limits`：突发整形名额与队列容量。
pub fn run_burst_admission(
    capacity: u64,
    existing_cpu: &[CpuSandboxRequest],
    mut pool: MemoryPool,
    arrivals: &[BurstArrival],
    limits: BurstLimits,
) -> Result<BurstAdmission, BurstError> {
    if arrivals.is_empty() {
        return Err(BurstError::NoArrivals);
    }
    // 身份一致性：CPU 与内存必须同一沙盒（否则可借两套 id 规避单一仲裁/记账）。
    for a in arrivals.iter() {
        if a.cpu.sandbox_id != a.memory.sandbox_id {
            return Err(BurstError::IdentityMismatch {
                cpu: a.cpu.sandbox_id.clone(),
                memory: a.memory.sandbox_id.clone(),
            });
        }
    }

    // existing + arrivals 合并一次性信任仲裁（黑名单/自提级/空重复 id/越界整体 fail-closed）。
    let mut all: Vec<CpuSandboxRequest> = existing_cpu.to_vec();
    all.extend(arrivals.iter().map(|a| a.cpu.clone()));
    let model = arbitrate_priority(capacity, &all)?;

    // 已在跑沙盒的 CPU 占用（先占容量）。
    let existing_used: u64 = existing_cpu.iter().map(|r| r.requested_millis).sum();

    // 按仲裁顺序（entries 已 Sensitive 先/权重降/id 升）过滤出本波 arrival，并携带其
    // 在 arrivals 中的原始位置；不使用任何会 panic 的索引/unwrap。
    let ordered: Vec<(&crate::ausec::cpu::EffectiveCpuEntry, usize)> = model
        .entries
        .iter()
        .filter_map(|e| {
            arrivals
                .iter()
                .position(|a| a.cpu.sandbox_id == e.sandbox_id)
                .map(|idx| (e, idx))
        })
        .collect();

    let mut decisions: Vec<ArrivalDecision> = Vec::with_capacity(arrivals.len());
    let mut cpu_used: u64 = existing_used;
    let mut admitted_count: u64 = 0;
    let mut queued_count: u64 = 0;

    for (entry, idx) in ordered {
        let id = entry.sandbox_id.clone();
        let prio = entry.effective_priority;
        let requested = entry.requested_millis;
        let memory_req = arrivals[idx].memory.clone();

        // CPU 软闸：已用 + 本申请是否仍在容量内。
        let cpu_fits = cpu_used.saturating_add(requested) <= capacity;
        // 突发整形名额。
        let burst_capped = matches!(
            limits.burst_max_admit,
            Some(max) if admitted_count >= max
        );

        // 内存双闸：投影（不落账）。
        let (memory_hard, memory_ceiling) = match pool.project(&memory_req) {
            Ok(_) => (false, false),
            Err(MemoryError::RejectCommittedExceedsPhysical { .. }) => (true, false),
            Err(MemoryError::RejectOvercommitCeiling { .. }) => (false, true),
            // existing 已建账、arrival 结构在解析层校验；其余错误视为记账非法整体失败。
            Err(other) => return Err(BurstError::Memory(other)),
        };

        let decision;
        if memory_hard {
            // 物理硬闸：直接拒，排队也无法获得物理内存。
            decision = ArrivalDecision {
                sandbox_id: id,
                decision: BurstDecision::Rejected,
                arrival_index: idx,
                effective_priority: prio,
                queue_reasons: Vec::new(),
                reject_reason: Some(RejectReason::PhysicalHardLimit),
            };
        } else {
            // 收集所有"此刻不能直接进"的原因。
            let mut reasons = Vec::new();
            if !cpu_fits {
                reasons.push(QueueReason::CpuShortfall);
            }
            if memory_ceiling {
                reasons.push(QueueReason::OvercommitCeiling);
            }
            if burst_capped {
                reasons.push(QueueReason::BurstCapped);
            }

            if reasons.is_empty() {
                // 所有闸门通过：真正向投影池落账 + 占 CPU 名额。
                pool.admit(memory_req)
                    .map_err(|e| BurstError::Invariant(e.to_string()))?;
                cpu_used = cpu_used.saturating_add(requested);
                admitted_count += 1;
                decision = ArrivalDecision {
                    sandbox_id: id,
                    decision: BurstDecision::Admitted,
                    arrival_index: idx,
                    effective_priority: prio,
                    queue_reasons: Vec::new(),
                    reject_reason: None,
                };
            } else if matches!(limits.queue_capacity, Some(cap) if queued_count >= cap) {
                // 需要排队但等待队列已满：拒绝（不无界排队）。
                decision = ArrivalDecision {
                    sandbox_id: id,
                    decision: BurstDecision::Rejected,
                    arrival_index: idx,
                    effective_priority: prio,
                    queue_reasons: Vec::new(),
                    reject_reason: Some(RejectReason::QueueFull),
                };
            } else {
                queued_count += 1;
                decision = ArrivalDecision {
                    sandbox_id: id,
                    decision: BurstDecision::Queued,
                    arrival_index: idx,
                    effective_priority: prio,
                    queue_reasons: reasons,
                    reject_reason: None,
                };
            }
        }
        decisions.push(decision);
    }

    let rejected_count = (arrivals.len() as u64) - admitted_count - queued_count;
    let ps = pool.status();
    let remaining = capacity.saturating_sub(cpu_used);

    Ok(BurstAdmission {
        decisions,
        summary: BurstSummary {
            arrivals: arrivals.len() as u64,
            admitted: admitted_count,
            queued: queued_count,
            rejected: rejected_count,
            cpu_projected_used_millis: cpu_used,
            cpu_capacity_millis: capacity,
            cpu_remaining_millis: remaining,
            committed_bytes: ps.committed_bytes,
            nominal_bytes: ps.nominal_bytes,
            physical_bytes: ps.physical_bytes,
            nominal_ceiling: ps.nominal_ceiling,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ausec::memory::OvercommitRatio;

    fn cpu(id: &str, tier: &str, req: u64) -> CpuSandboxRequest {
        CpuSandboxRequest {
            sandbox_id: id.to_string(),
            tier: crate::ausec::cpu::parse_tier(tier).unwrap(),
            latency: None,
            weight: None,
            requested_millis: req,
        }
    }

    fn mem(id: &str, private: u64) -> MemorySandboxRequest {
        MemorySandboxRequest {
            sandbox_id: id.to_string(),
            private_bytes: private,
            shared: Vec::new(),
        }
    }

    fn arr(id: &str, tier: &str, cpu_req: u64, private: u64) -> BurstArrival {
        BurstArrival {
            cpu: cpu(id, tier, cpu_req),
            memory: mem(id, private),
        }
    }

    fn pool(physical: u64, times: u32) -> MemoryPool {
        MemoryPool::new(physical, OvercommitRatio::times(times).unwrap()).unwrap()
    }

    #[test]
    fn sensitive_admitted_before_tolerant_under_cpu_contention() {
        // 容量 100：system 敏感需 60，third_party 容忍需 60 → 敏感先入，容忍 CPU 排队。
        let arrivals = vec![
            arr("bg", "third_party", 60, 10),
            arr("core", "system", 60, 10),
        ];
        let out = run_burst_admission(100, &[], pool(1000, 1), &arrivals, BurstLimits::default())
            .unwrap();
        // 裁决顺序 core 在前。
        assert_eq!(out.decisions[0].sandbox_id, "core");
        assert_eq!(out.decisions[0].decision, BurstDecision::Admitted);
        assert_eq!(out.decisions[1].sandbox_id, "bg");
        assert_eq!(out.decisions[1].decision, BurstDecision::Queued);
        assert!(out.decisions[1]
            .queue_reasons
            .contains(&QueueReason::CpuShortfall));
        let s = &out.summary;
        assert_eq!(s.admitted, 1);
        assert_eq!(s.queued, 1);
        assert_eq!(s.rejected, 0);
        assert_eq!(s.cpu_projected_used_millis, 60);
        assert_eq!(s.cpu_remaining_millis, 40);
    }

    #[test]
    fn physical_hard_limit_rejects_but_overcommit_only_queues() {
        // 物理 50、1×：private 100 触物理硬闸 → reject（排队也无法获得物理内存）。
        let arrivals = vec![arr("big", "system", 10, 100)];
        let out =
            run_burst_admission(1000, &[], pool(50, 1), &arrivals, BurstLimits::default()).unwrap();
        assert_eq!(out.decisions[0].decision, BurstDecision::Rejected);
        assert_eq!(
            out.decisions[0].reject_reason,
            Some(RejectReason::PhysicalHardLimit)
        );
        assert_eq!(out.summary.rejected, 1);

        // 超卖 ceiling 排队（区别于物理硬闸）：physical=200、2× → ceiling=400。
        // 三个 official 各 private=10、共享同一 img=150（并集只计一次）：
        //   committed = 10n + 150；nominal = 160n（共享按沙盒重复计）。
        //   n=1: committed160/nominal160 admit；n=2: committed170/nominal320 admit；
        //   n=3: committed180<=200（物理内）但 nominal480>400 → 仅超卖闸 → Queued。
        let shared = |id: &str| BurstArrival {
            cpu: cpu(id, "official", 10),
            memory: MemorySandboxRequest {
                sandbox_id: id.into(),
                private_bytes: 10,
                shared: vec![crate::ausec::memory::SharedRef {
                    id: "img".into(),
                    bytes: 150,
                }],
            },
        };
        let ar = vec![shared("a"), shared("b"), shared("c")];
        let p = MemoryPool::new(200, OvercommitRatio::times(2).unwrap()).unwrap();
        let out2 = run_burst_admission(1000, &[], p, &ar, BurstLimits::default()).unwrap();
        assert_eq!(out2.summary.admitted, 2);
        assert_eq!(out2.summary.queued, 1);
        assert_eq!(out2.summary.rejected, 0);
        let c = out2.decisions.iter().find(|d| d.sandbox_id == "c").unwrap();
        assert_eq!(c.decision, BurstDecision::Queued);
        assert!(c.queue_reasons.contains(&QueueReason::OvercommitCeiling));
        // 物理硬闸未触发：committed 仍是两沙盒投影 170（c 未落账）。
        assert_eq!(out2.summary.committed_bytes, 170);
    }

    #[test]
    fn burst_max_admit_caps_direct_admits_and_rest_queue() {
        // 容量/内存都放得下，但本波最多准入 2：第 3 个排队（BurstCapped）。
        let arrivals = vec![
            arr("a", "system", 10, 10),
            arr("b", "system", 10, 10),
            arr("c", "official", 10, 10),
        ];
        let limits = BurstLimits {
            burst_max_admit: Some(2),
            queue_capacity: None,
        };
        let out = run_burst_admission(1000, &[], pool(100000, 50), &arrivals, limits).unwrap();
        assert_eq!(out.summary.admitted, 2);
        assert_eq!(out.summary.queued, 1);
        let capped = out.decisions.iter().find(|d| d.sandbox_id == "c").unwrap();
        assert_eq!(capped.decision, BurstDecision::Queued);
        assert!(capped.queue_reasons.contains(&QueueReason::BurstCapped));
    }

    #[test]
    fn bounded_queue_rejects_when_full_not_unbounded() {
        // CPU 只能放 1 个，队列容量 1，共 3 个 → 1 admit、1 queue、1 queue_full reject。
        let arrivals = vec![
            arr("a", "third_party", 60, 10),
            arr("b", "third_party", 60, 10),
            arr("c", "third_party", 60, 10),
        ];
        let limits = BurstLimits {
            burst_max_admit: None,
            queue_capacity: Some(1),
        };
        let out = run_burst_admission(100, &[], pool(100000, 50), &arrivals, limits).unwrap();
        assert_eq!(out.summary.admitted, 1);
        assert_eq!(out.summary.queued, 1);
        assert_eq!(out.summary.rejected, 1);
        assert!(out
            .decisions
            .iter()
            .any(|d| d.reject_reason == Some(RejectReason::QueueFull)));
    }

    #[test]
    fn fail_closed_empty_identity_blacklist_self_promotion_duplicate() {
        // 空 arrivals。
        assert!(matches!(
            run_burst_admission(100, &[], pool(100, 1), &[], BurstLimits::default()),
            Err(BurstError::NoArrivals)
        ));
        // CPU/内存身份不一致。
        let mut bad = arr("x", "system", 10, 10);
        bad.memory.sandbox_id = "y".into();
        assert!(matches!(
            run_burst_admission(100, &[], pool(100, 1), &[bad], BurstLimits::default()),
            Err(BurstError::IdentityMismatch { .. })
        ));
        // 黑名单。
        assert!(run_burst_admission(
            100,
            &[],
            pool(100, 1),
            &[arr("z", "blacklist", 10, 10)],
            BurstLimits::default()
        )
        .is_err());
        // 第三方自报 sensitive：构造 latency=Some(Sensitive)。
        let mut self_promo = arr("w", "certified", 10, 10);
        self_promo.cpu.latency = Some(crate::ausec::memory::LatencyClass::Sensitive);
        assert!(run_burst_admission(
            100,
            &[],
            pool(100, 1),
            &[self_promo],
            BurstLimits::default()
        )
        .is_err());
        // 与 existing 重复 id。
        let existing = vec![cpu("dup", "system", 10)];
        assert!(run_burst_admission(
            100,
            &existing,
            pool(100, 1),
            &[arr("dup", "system", 10, 10)],
            BurstLimits::default()
        )
        .is_err());
    }
}
