//! 竞争下的确定性 CPU 配额分配仿真（v3.7.8）。
//!
//! v3.7.7 交付了静态的两级优先级/权重模型但**不做容量约束**；本版把它推进到
//! "节点 CPU 已被占用一部分、沙盒之间真实竞争"时的**确定性配额分配**，并用一个
//! **仿真时钟**（多个 tick）可重复地跑分配过程：
//!
//! - **Sensitive 先保障，剩余给 Tolerant**：每个 tick 先把可用容量在时延敏感组内
//!   按权重 + 需求上限分配，敏感组用剩的才给时延容忍组。
//! - **无优先级对照（flat baseline）**：同一 tick 内不分等级、所有人按权重共分可用
//!   容量；对照两种策略下 **Sensitive 组的需求满足率/受影响比例**，用本仓自己的
//!   确定性向量证明"敏感优先"确实压低了敏感任务在竞争下的损失。外部报道所称
//!   "节点 50% CPU 被占时，时延敏感任务受影响 45.2%→17.3%"（**aiwiki.ai /
//!   byteiota.com 对 DSEC 的报道，非本仓复测**）只作为这一机制的动机引用，本版
//!   不声称在本仓复现该数字。
//!
//! 全部纯确定性、整数（u64/u128 + 千分点）、零 syscall、零 unsafe：这是**调度策略
//! 的仿真/决策面**，不做真实 CPU 限速、不推进真实时间，tick 是逻辑步而非墙钟。

use std::collections::HashMap;

use serde::Deserialize;

use crate::ausec::cpu::{arbitrate_priority, CpuError, CpuSandboxRequest, EffectiveCpuEntry};
use crate::ausec::memory::LatencyClass;

/// 配额仿真相关的全具名错误（无 panic 路径）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleError {
    #[error("底层优先级仲裁失败: {0}")]
    Cpu(#[from] CpuError),
    #[error("外部负载 external_load_millis={0} 不能超过节点容量 capacity_millis={1}")]
    ExternalLoadExceedsCapacity(u64, u64),
    #[error("tick {tick} 中沙盒 {sandbox} 不在已仲裁集合里")]
    UnknownTickSandbox { tick: usize, sandbox: String },
    #[error("tick {tick} 沙盒 {sandbox} 重复申报需求")]
    DuplicateTickDemand { tick: usize, sandbox: String },
    #[error(
        "tick {tick} 沙盒 {sandbox} 需求 demand={demand} 非法，须在 1..=申请 requested={requested}"
    )]
    DemandOutOfRange {
        tick: usize,
        sandbox: String,
        demand: u64,
        requested: u64,
    },
    #[error("仿真至少需要 1 个 tick")]
    NoTicks,
}

/// 一个 tick 内某沙盒的实际 CPU 需求（Waiting 的沙盒可以直接不列）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct TickDemand {
    pub sandbox_id: String,
    /// 本 tick 实际想要的 CPU 时间；须 `1..=requested_millis`（申请是上限）。
    pub demand_millis: u64,
}

/// 一个仿真步：被其他任务占掉的外部负载 + 本 tick 活跃沙盒的需求。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct ScheduleTick {
    /// 被"节点上其他任务"占掉、本仿真不可用的 CPU（对应外部参考中 50% 被占）。
    pub external_load_millis: u64,
    /// 本 tick 有 CPU 需求的沙盒；Waiting（等待模型生成）的沙盒不列、granted=0。
    #[serde(default)]
    pub demands: Vec<TickDemand>,
}

/// 一组（或整体）在一个口径下的需求/分配汇总（千分点比率）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AllocSummary {
    pub demand_sum: u64,
    pub granted_sum: u64,
    pub shortfall_sum: u64,
    /// 满足率 granted/demand（千分点）；无需求时为 null（调用方序列化为 null）。
    pub met_permille: Option<u64>,
    /// 受影响比例 shortfall/demand（千分点）；= 1000 - met。
    pub impact_permille: Option<u64>,
}

impl AllocSummary {
    fn from_sums(demand_sum: u128, granted_sum: u128) -> AllocSummary {
        let shortfall = demand_sum.saturating_sub(granted_sum);
        // 用 checked_div：无需求（demand=0）时比率为 None，绝不除零；溢出同样降级 None。
        let met = granted_sum
            .checked_mul(1000)
            .and_then(|n| n.checked_div(demand_sum))
            .map(|m| m as u64);
        let impact = met.map(|m| 1000 - m);
        AllocSummary {
            demand_sum: demand_sum as u64,
            granted_sum: granted_sum as u64,
            shortfall_sum: shortfall as u64,
            met_permille: met,
            impact_permille: impact,
        }
    }
}

/// 一个沙盒在一个 tick 内、两种策略下的分配对照。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TickEntry {
    pub sandbox_id: String,
    pub effective_latency: LatencyClass,
    pub weight: u32,
    pub demand_millis: u64,
    /// 两级优先级策略下分到的 CPU。
    pub granted_priority_millis: u64,
    /// 无优先级（全员按权重共分）策略下分到的 CPU。
    pub granted_flat_millis: u64,
    pub shortfall_priority_millis: u64,
    pub met_priority_permille: u64,
    pub is_sensitive: bool,
}

/// 一个 tick 的完整分配结果。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TickSchedule {
    pub index: usize,
    pub capacity_millis: u64,
    pub external_load_millis: u64,
    pub available_millis: u64,
    /// 按 v3.7.7 确定性顺序（优先级/权重/...）排列的活跃沙盒分配。
    pub entries: Vec<TickEntry>,
    /// 两级优先级策略下 Sensitive 组汇总。
    pub sensitive_priority: AllocSummary,
    /// 两级优先级策略下 Tolerant 组汇总。
    pub tolerant_priority: AllocSummary,
    /// 对照：无优先级策略下 Sensitive 组汇总（用于量化"敏感优先"的保护）。
    pub sensitive_flat: AllocSummary,
}

/// 跨 tick 累计的策略对照。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AggregateSchedule {
    pub ticks: usize,
    pub capacity_millis: u64,
    pub external_load_sum: u64,
    pub available_sum: u64,
    pub sensitive_priority: AllocSummary,
    pub tolerant_priority: AllocSummary,
    pub sensitive_flat: AllocSummary,
}

/// 一次多 tick 仿真的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScheduleSimulation {
    pub capacity_millis: u64,
    pub ticks_run: usize,
    pub ticks: Vec<TickSchedule>,
    pub aggregate: AggregateSchedule,
}

/// 参与一次加权分配的成员（内部用）。
#[derive(Debug, Clone, Copy)]
struct Member {
    /// 在 entries 中的下标。
    pos: usize,
    weight: u32,
    demand: u64,
}

/// 整数加权配水（weighted water-filling，含需求上限），守恒、确定、无浮点。
///
/// - 每个成员最终 `0 <= granted <= demand`；
/// - 总分配 `= min(capacity, Σdemand)`（需求不足则剩余容量留空，不硬塞）；
/// - 同段内严格按 weight 比例；被需求 cap 住的成员固定后，其权重从后续比例基中剔除
///   （标准 water-filling）；比例 floor 的余数按"权重降→pos 升"逐个 +1 补完，确定。
///
/// 返回与 `members` 同序的 granted。
fn allocate_weighted_capped(capacity: u64, members: &[Member]) -> Vec<u64> {
    let n = members.len();
    let mut granted = vec![0u64; n];
    if n == 0 {
        return granted;
    }
    let mut remaining: u128 = u128::from(capacity);
    // active[pos] = 仍未被需求 cap、还参与比例分配的成员。
    let mut active = vec![true; n];

    loop {
        let idx: Vec<usize> = (0..n).filter(|&i| active[i]).collect();
        if idx.is_empty() || remaining == 0 {
            break;
        }
        let weight_sum: u128 = idx.iter().map(|&i| u128::from(members[i].weight)).sum();
        // 判定"假设当前 active 全员共分 remaining"下，哪些成员这一段会触顶：
        // residual_i * W <= remaining * weight_i  ⇔  其按权应得 >= 还想要的。
        let mut capped = Vec::new();
        for &i in &idx {
            let residual = u128::from(members[i].demand - granted[i]);
            let lhs = residual * weight_sum;
            let rhs = remaining * u128::from(members[i].weight);
            if lhs <= rhs {
                capped.push(i);
            }
        }
        if capped.is_empty() {
            // 无人触顶：按权重 floor 配给，余数确定性补齐（每成员至多 +1，不超 demand）。
            let mut base_sum: u128 = 0;
            let mut base = vec![0u128; n];
            for &i in &idx {
                let b = remaining * u128::from(members[i].weight) / weight_sum;
                base[i] = b;
                base_sum += b;
            }
            let mut leftover = (remaining - base_sum) as u64;
            for &i in &idx {
                granted[i] = (u128::from(granted[i]) + base[i]) as u64;
            }
            // 余数按 weight 降、pos 升 逐个 +1（leftover < 成员数）。
            let mut order = idx.clone();
            order.sort_by(|&a, &b| {
                members[b]
                    .weight
                    .cmp(&members[a].weight)
                    .then_with(|| a.cmp(&b))
            });
            for &i in &order {
                if leftover == 0 {
                    break;
                }
                granted[i] += 1;
                leftover -= 1;
            }
            break;
        }
        // 触顶成员固定拿满 residual 并退出（Σresidual_capped <= remaining，不会负）。
        for &i in &capped {
            let residual = members[i].demand - granted[i];
            granted[i] += residual;
            remaining -= u128::from(residual);
            active[i] = false;
        }
    }
    granted
}

/// 从已仲裁条目 + 一个 tick 的需求构造活跃成员（按 entry 顺序），并做 tick 校验。
fn tick_members<'a>(
    tick_index: usize,
    entries: &'a [EffectiveCpuEntry],
    by_id: &HashMap<String, usize>,
    tick: &ScheduleTick,
) -> Result<(Vec<&'a EffectiveCpuEntry>, HashMap<usize, u64>), ScheduleError> {
    let mut demand_by_pos: HashMap<usize, u64> = HashMap::new();
    for d in &tick.demands {
        let &pos = by_id
            .get(&d.sandbox_id)
            .ok_or_else(|| ScheduleError::UnknownTickSandbox {
                tick: tick_index,
                sandbox: d.sandbox_id.clone(),
            })?;
        let requested = entries[pos].requested_millis;
        if d.demand_millis == 0 || d.demand_millis > requested {
            return Err(ScheduleError::DemandOutOfRange {
                tick: tick_index,
                sandbox: d.sandbox_id.clone(),
                demand: d.demand_millis,
                requested,
            });
        }
        if demand_by_pos.insert(pos, d.demand_millis).is_some() {
            return Err(ScheduleError::DuplicateTickDemand {
                tick: tick_index,
                sandbox: d.sandbox_id.clone(),
            });
        }
    }
    // 活跃条目按 v3.7.7 的确定性 entry 顺序（entries 已排好）。
    let active: Vec<&EffectiveCpuEntry> = entries
        .iter()
        .enumerate()
        .filter(|(pos, _)| demand_by_pos.contains_key(pos))
        .map(|(_, e)| e)
        .collect();
    Ok((active, demand_by_pos))
}

/// 跑单个 tick 的两级优先级分配 + 无优先级对照。
fn simulate_one_tick(
    index: usize,
    capacity: u64,
    entries: &[EffectiveCpuEntry],
    by_id: &HashMap<String, usize>,
    tick: &ScheduleTick,
) -> Result<TickSchedule, ScheduleError> {
    if tick.external_load_millis > capacity {
        return Err(ScheduleError::ExternalLoadExceedsCapacity(
            tick.external_load_millis,
            capacity,
        ));
    }
    let available = capacity - tick.external_load_millis;
    let (active, demand_by_pos) = tick_members(index, entries, by_id, tick)?;

    // pos -> 在 active 列表中的序号（Member.pos 用 entry 原下标，方便回查）。
    let to_members = |list: &[&EffectiveCpuEntry]| -> Vec<Member> {
        list.iter()
            .map(|e| {
                let pos = by_id[&e.sandbox_id];
                Member {
                    pos,
                    weight: e.weight,
                    demand: demand_by_pos[&pos],
                }
            })
            .collect()
    };

    // 两级优先级：Sensitive 先分，剩余给 Tolerant。
    let sens_active: Vec<&EffectiveCpuEntry> = active
        .iter()
        .copied()
        .filter(|e| e.effective_latency == LatencyClass::Sensitive)
        .collect();
    let tol_active: Vec<&EffectiveCpuEntry> = active
        .iter()
        .copied()
        .filter(|e| e.effective_latency == LatencyClass::Tolerant)
        .collect();

    let sens_members = to_members(&sens_active);
    let sens_grant = allocate_weighted_capped(available, &sens_members);
    let sens_used: u128 = sens_grant.iter().map(|g| u128::from(*g)).sum();
    let leftover = u128::from(available) - sens_used;
    let tol_members = to_members(&tol_active);
    let tol_grant = allocate_weighted_capped(leftover as u64, &tol_members);

    // 无优先级对照：全员按权重共分 available。
    let flat_members = to_members(&active);
    let flat_grant = allocate_weighted_capped(available, &flat_members);

    // pos -> granted 映射。
    let mut pri_by_pos: HashMap<usize, u64> = HashMap::new();
    for (m, g) in sens_members.iter().zip(sens_grant.iter()) {
        pri_by_pos.insert(m.pos, *g);
    }
    for (m, g) in tol_members.iter().zip(tol_grant.iter()) {
        pri_by_pos.insert(m.pos, *g);
    }
    let mut flat_by_pos: HashMap<usize, u64> = HashMap::new();
    for (m, g) in flat_members.iter().zip(flat_grant.iter()) {
        flat_by_pos.insert(m.pos, *g);
    }

    let mut tick_entries = Vec::with_capacity(active.len());
    for e in &active {
        let pos = by_id[&e.sandbox_id];
        let demand = demand_by_pos[&pos];
        let gp = pri_by_pos[&pos];
        let gf = flat_by_pos[&pos];
        tick_entries.push(TickEntry {
            sandbox_id: e.sandbox_id.clone(),
            effective_latency: e.effective_latency,
            weight: e.weight,
            demand_millis: demand,
            granted_priority_millis: gp,
            granted_flat_millis: gf,
            shortfall_priority_millis: demand - gp,
            met_priority_permille: (u128::from(gp) * 1000 / u128::from(demand)) as u64,
            is_sensitive: e.effective_latency == LatencyClass::Sensitive,
        });
    }

    Ok(build_tick_schedule(
        index,
        capacity,
        tick.external_load_millis,
        available,
        tick_entries,
    ))
}

/// 按等级/策略口径汇总一个 tick 的已活跃条目（返回 owned，供后续移动 entries）。
fn summarize_entries(entries: &[TickEntry], lat: Option<LatencyClass>, flat: bool) -> AllocSummary {
    let mut d: u128 = 0;
    let mut g: u128 = 0;
    for e in entries {
        if let Some(want) = lat {
            if e.effective_latency != want {
                continue;
            }
        }
        d += u128::from(e.demand_millis);
        g += u128::from(if flat {
            e.granted_flat_millis
        } else {
            e.granted_priority_millis
        });
    }
    AllocSummary::from_sums(d, g)
}

/// 用已计算好的条目组装单个 tick 结果（汇总在移动前完成）。
fn build_tick_schedule(
    index: usize,
    capacity: u64,
    external_load: u64,
    available: u64,
    entries: Vec<TickEntry>,
) -> TickSchedule {
    let sensitive_priority = summarize_entries(&entries, Some(LatencyClass::Sensitive), false);
    let tolerant_priority = summarize_entries(&entries, Some(LatencyClass::Tolerant), false);
    let sensitive_flat = summarize_entries(&entries, Some(LatencyClass::Sensitive), true);
    TickSchedule {
        index,
        capacity_millis: capacity,
        external_load_millis: external_load,
        available_millis: available,
        entries,
        sensitive_priority,
        tolerant_priority,
        sensitive_flat,
    }
}

/// 跑完整多 tick 仿真：先做 v3.7.7 优先级仲裁（含低信任自提级 fail-closed），再逐
/// tick 做两级配额分配 + 无优先级对照，最后跨 tick 累计汇总。
pub fn run_schedule_simulation(
    capacity_millis: u64,
    sandboxes: &[CpuSandboxRequest],
    ticks: &[ScheduleTick],
) -> Result<ScheduleSimulation, ScheduleError> {
    if ticks.is_empty() {
        return Err(ScheduleError::NoTicks);
    }
    // 仲裁同时校验 capacity>0、权重/申请/重复 id/低信任自报 Sensitive。
    let model = arbitrate_priority(capacity_millis, sandboxes)?;
    let entries = model.entries;
    let by_id: HashMap<String, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.sandbox_id.clone(), i))
        .collect();

    let mut out_ticks = Vec::with_capacity(ticks.len());
    let mut ext_sum: u128 = 0;
    let mut avail_sum: u128 = 0;
    // 跨 tick 累计（求和后再算比率，而非比率平均）。
    let (mut s_d, mut s_gp, mut s_gf, mut t_d, mut t_gp) = (0u128, 0u128, 0u128, 0u128, 0u128);

    for (i, tick) in ticks.iter().enumerate() {
        let ts = simulate_one_tick(i, capacity_millis, &entries, &by_id, tick)?;
        ext_sum += u128::from(ts.external_load_millis);
        avail_sum += u128::from(ts.available_millis);
        s_d += u128::from(ts.sensitive_priority.demand_sum);
        s_gp += u128::from(ts.sensitive_priority.granted_sum);
        s_gf += u128::from(ts.sensitive_flat.granted_sum);
        t_d += u128::from(ts.tolerant_priority.demand_sum);
        t_gp += u128::from(ts.tolerant_priority.granted_sum);
        out_ticks.push(ts);
    }

    Ok(ScheduleSimulation {
        capacity_millis,
        ticks_run: ticks.len(),
        ticks: out_ticks,
        aggregate: AggregateSchedule {
            ticks: ticks.len(),
            capacity_millis,
            external_load_sum: ext_sum as u64,
            available_sum: avail_sum as u64,
            sensitive_priority: AllocSummary::from_sums(s_d, s_gp),
            tolerant_priority: AllocSummary::from_sums(t_d, t_gp),
            sensitive_flat: AllocSummary::from_sums(s_d, s_gf),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ausec::cpu::CpuSandboxRequest;
    use crate::plugin::tier::Tier;

    fn sb(id: &str, tier: Tier, weight: u32, requested: u64) -> CpuSandboxRequest {
        CpuSandboxRequest {
            sandbox_id: id.to_string(),
            tier,
            latency: None,
            weight: Some(weight),
            requested_millis: requested,
        }
    }

    #[test]
    fn weighted_alloc_is_conserving_and_capped() {
        // 容量 100；a 需求 20（会被 cap），b 需求 200。
        // a 固定 20 后，剩 80 全给 b（无竞争）；总和=min(100,220)=100。
        let ms = vec![
            Member {
                pos: 0,
                weight: 1,
                demand: 20,
            },
            Member {
                pos: 1,
                weight: 1,
                demand: 200,
            },
        ];
        let g = allocate_weighted_capped(100, &ms);
        assert_eq!(g, vec![20, 80]);

        // 总需求 < 容量：不硬塞，剩 30 留空。
        let ms2 = vec![
            Member {
                pos: 0,
                weight: 1,
                demand: 30,
            },
            Member {
                pos: 1,
                weight: 1,
                demand: 40,
            },
        ];
        let g2 = allocate_weighted_capped(100, &ms2);
        assert_eq!(g2, vec![30, 40]);
    }

    #[test]
    fn weighted_alloc_shares_by_weight_with_integer_remainder() {
        // 等权三人争 10：floor(10/3)=3 各得 9，余 1 按 (weight 同→pos 升) 给 pos0。
        let ms = vec![
            Member {
                pos: 0,
                weight: 100,
                demand: 100,
            },
            Member {
                pos: 1,
                weight: 100,
                demand: 100,
            },
            Member {
                pos: 2,
                weight: 100,
                demand: 100,
            },
        ];
        let g = allocate_weighted_capped(10, &ms);
        assert_eq!(g.iter().sum::<u64>(), 10);
        assert_eq!(g, vec![4, 3, 3]);

        // 权重 3:1 争 8：6 与 2。
        let ms2 = vec![
            Member {
                pos: 0,
                weight: 300,
                demand: 100,
            },
            Member {
                pos: 1,
                weight: 100,
                demand: 100,
            },
        ];
        assert_eq!(allocate_weighted_capped(8, &ms2), vec![6, 2]);
    }

    #[test]
    fn sensitive_protected_first_then_tolerant_gets_leftover() {
        // 容量 100，外部占 50 → 可用 50。一个 system(sensitive) 需求 50，
        // 一个 third_party(tolerant) 需求 50；priority 策略先满足 sensitive 50，
        // tolerant 只剩 0。
        let sbs = vec![
            sb("core", Tier::System, 100, 50),
            sb("bg", Tier::ThirdParty, 100, 50),
        ];
        let ticks = vec![ScheduleTick {
            external_load_millis: 50,
            demands: vec![
                TickDemand {
                    sandbox_id: "core".into(),
                    demand_millis: 50,
                },
                TickDemand {
                    sandbox_id: "bg".into(),
                    demand_millis: 50,
                },
            ],
        }];
        let sim = run_schedule_simulation(100, &sbs, &ticks).unwrap();
        let t = &sim.ticks[0];
        assert_eq!(t.available_millis, 50);
        assert_eq!(t.sensitive_priority.granted_sum, 50);
        assert_eq!(t.sensitive_priority.met_permille, Some(1000));
        assert_eq!(t.tolerant_priority.granted_sum, 0);
        // flat 对照：两人等权共分 50 → sensitive 仅 25（受影响 500‰）。
        assert_eq!(t.sensitive_flat.granted_sum, 25);
        assert_eq!(t.sensitive_flat.impact_permille, Some(500));
        // priority 完全保护了 sensitive（impact 0），严格优于 flat。
        assert_eq!(
            t.sensitive_priority.impact_permille,
            Some(0),
            "sensitive must be fully protected"
        );
    }

    #[test]
    fn priority_reduces_sensitive_impact_vs_flat_in_contention() {
        // 不把 tolerant 饿死的一般竞争：容量 100、外部 40 → 可用 60。
        // 两个 system(sensitive, 权重各 100) 各需求 40（合计 80），
        // 一个 tolerant(权重 100) 需求 40。Sensitive 先按等权分 60 → 各 30（合计 60），
        // leftover=0，tolerant 0；sensitive 满足率 600/1000。
        let sbs = vec![
            sb("s1", Tier::System, 100, 40),
            sb("s2", Tier::Official, 100, 40),
            sb("w1", Tier::ThirdParty, 100, 40),
        ];
        let ticks = vec![ScheduleTick {
            external_load_millis: 40,
            demands: vec![
                TickDemand {
                    sandbox_id: "s1".into(),
                    demand_millis: 40,
                },
                TickDemand {
                    sandbox_id: "s2".into(),
                    demand_millis: 40,
                },
                TickDemand {
                    sandbox_id: "w1".into(),
                    demand_millis: 40,
                },
            ],
        }];
        let sim = run_schedule_simulation(100, &sbs, &ticks).unwrap();
        let agg = &sim.aggregate;
        // priority 下 sensitive 拿到 60/80，flat 下三人等权共分 60 → sensitive(两人)40/80。
        assert_eq!(agg.sensitive_priority.granted_sum, 60);
        assert_eq!(agg.sensitive_flat.granted_sum, 40);
        assert!(
            agg.sensitive_priority.met_permille.unwrap() > agg.sensitive_flat.met_permille.unwrap()
        );
        // 守恒：priority 总分配不超过可用 60。
        let total_pri = agg.sensitive_priority.granted_sum + agg.tolerant_priority.granted_sum;
        assert_eq!(total_pri, 60);
    }

    #[test]
    fn waiting_sandboxes_omitted_get_zero_and_multi_tick_aggregates() {
        let sbs = vec![sb("core", Tier::System, 100, 100)];
        // tick0：core Waiting（不列需求）→ 全 0；tick1：外部 0、需求 60 → 60。
        let ticks = vec![
            ScheduleTick {
                external_load_millis: 0,
                demands: vec![],
            },
            ScheduleTick {
                external_load_millis: 0,
                demands: vec![TickDemand {
                    sandbox_id: "core".into(),
                    demand_millis: 60,
                }],
            },
        ];
        let sim = run_schedule_simulation(100, &sbs, &ticks).unwrap();
        assert_eq!(sim.ticks_run, 2);
        assert_eq!(sim.ticks[0].entries.len(), 0);
        assert_eq!(sim.ticks[0].sensitive_priority.demand_sum, 0);
        assert_eq!(sim.ticks[0].sensitive_priority.met_permille, None);
        assert_eq!(sim.aggregate.sensitive_priority.granted_sum, 60);
        assert_eq!(sim.aggregate.sensitive_priority.demand_sum, 60);
        assert_eq!(sim.aggregate.available_sum, 200);
    }

    #[test]
    fn validation_fails_closed() {
        let ok_sbs = vec![sb("a", Tier::System, 100, 100)];
        // 无 tick。
        assert!(matches!(
            run_schedule_simulation(100, &ok_sbs, &[]).unwrap_err(),
            ScheduleError::NoTicks
        ));
        // 外部负载超容量。
        assert!(matches!(
            run_schedule_simulation(
                100,
                &ok_sbs,
                &[ScheduleTick {
                    external_load_millis: 101,
                    demands: vec![]
                }],
            )
            .unwrap_err(),
            ScheduleError::ExternalLoadExceedsCapacity(101, 100)
        ));
        // 未知沙盒 / 需求越界 / 重复需求。
        let bad = |demands: Vec<TickDemand>, load: u64| ScheduleTick {
            external_load_millis: load,
            demands,
        };
        assert!(matches!(
            run_schedule_simulation(
                100,
                &ok_sbs,
                &[bad(
                    vec![TickDemand {
                        sandbox_id: "ghost".into(),
                        demand_millis: 1
                    }],
                    0
                )],
            )
            .unwrap_err(),
            ScheduleError::UnknownTickSandbox { .. }
        ));
        assert!(matches!(
            run_schedule_simulation(
                100,
                &ok_sbs,
                &[bad(
                    vec![TickDemand {
                        sandbox_id: "a".into(),
                        demand_millis: 101
                    }],
                    0
                )],
            )
            .unwrap_err(),
            ScheduleError::DemandOutOfRange { .. }
        ));
        assert!(matches!(
            run_schedule_simulation(
                100,
                &ok_sbs,
                &[bad(
                    vec![
                        TickDemand {
                            sandbox_id: "a".into(),
                            demand_millis: 10
                        },
                        TickDemand {
                            sandbox_id: "a".into(),
                            demand_millis: 20
                        },
                    ],
                    0,
                )],
            )
            .unwrap_err(),
            ScheduleError::DuplicateTickDemand { .. }
        ));
        // 低信任自报 Sensitive 仍由底层仲裁拒绝（透传 Cpu）。
        let cheat = vec![CpuSandboxRequest {
            sandbox_id: "x".into(),
            tier: Tier::ThirdParty,
            latency: Some(crate::ausec::memory::LatencyClass::Sensitive),
            weight: None,
            requested_millis: 10,
        }];
        assert!(matches!(
            run_schedule_simulation(
                100,
                &cheat,
                &[bad(
                    vec![TickDemand {
                        sandbox_id: "x".into(),
                        demand_millis: 10
                    }],
                    0
                )],
            )
            .unwrap_err(),
            ScheduleError::Cpu(_)
        ));
    }
}
