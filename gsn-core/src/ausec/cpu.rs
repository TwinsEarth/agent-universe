//! CPU 两级优先级模型：时延敏感 / 时延容忍 + 优先级与权重（v3.7.7）。
//!
//! 背景（见 `docs/ausec/AUSEC-DESIGN.md` §3.1）：Agent 沙盒负载稀疏，约 90% 沙盒
//! 平均 CPU 使用不到申请资源的 5%——多数沙盒在等待模型生成下一步。调度上必须区分
//! 两类负载：**时延敏感**（Agent 交互主循环、通信总线等关键路径）与**时延容忍**
//! （后台批处理、可晚一点完成的任务）。外部报道（**aiwiki.ai / byteiota.com 对
//! DSEC 的实测，非本仓复测**）称节点 50% CPU 被占时，调度优化使时延敏感任务受到的
//! 延迟影响从 45.2% 降到 17.3%。
//!
//! 本版**只交付跨平台、纯确定性的「优先级模型 + 权重」**，不调用任何 OS 调度原语
//! （不写 cgroup cpu.shares/quota、不调 sched_setaffinity、不做真实限速）：
//!
//! - **两级优先级**：[`LatencyClass::Sensitive`] 优先级（`PRIORITY_SENSITIVE=2`）
//!   整体高于 [`LatencyClass::Tolerant`]（`PRIORITY_TOLERANT=1`）；调度顺序上
//!   Sensitive 组先被保障，剩余算力才给 Tolerant 组（真正的竞争配额分配仿真在
//!   v3.7.8）。
//! - **权重**：同一时延等级内，各沙盒按 `weight` 决定相对份额（千分点，确定性
//!   归一化）。默认权重 [`DEFAULT_WEIGHT`]；权重合法区间 `1..=MAX_WEIGHT`。
//! - **信任仲裁（防自我提级）**：沙盒自报的 `latency` 是**可伪造输入**。Sensitive
//!   只允许宿主系统（T0）/官方（T1）负载；认证（T2）、第三方（T3）自报 Sensitive
//!   一律 fail-closed 拒绝；黑名单不参与调度。低信任级缺省时安全降为 Tolerant。
//!
//! 所有权重/份额用 u128 中间运算 + 千分点整数，不引浮点、不溢出；权重份额是组内
//! 相对值，整数取整误差以 `*_share_total_permille` 如实呈现，不伪称守恒。

use serde::Deserialize;

use crate::ausec::memory::LatencyClass;
use crate::plugin::tier::Tier;

/// 时延敏感负载的优先级数值（越大越优先）。
pub const PRIORITY_SENSITIVE: u8 = 2;
/// 时延容忍负载的优先级数值。
pub const PRIORITY_TOLERANT: u8 = 1;
/// 默认权重（同类内一份标准份额）。
pub const DEFAULT_WEIGHT: u32 = 100;
/// 权重上限（防止恶意申报把组内份额表撑爆 / 制造 u128 压力）。
pub const MAX_WEIGHT: u32 = 10_000;

/// CPU 优先级建模相关的全具名错误（无 panic 路径）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CpuError {
    #[error("节点 CPU 容量 capacity_millis 必须 > 0")]
    ZeroCapacity,
    #[error("沙盒 id 不能为空")]
    EmptySandboxId,
    #[error("沙盒 {0} 重复登记")]
    DuplicateSandbox(String),
    #[error("沙盒 {0} 的权重必须在 1..={1}，得到 {2}")]
    WeightOutOfRange(String, u32, u32),
    #[error("沙盒 {0} 的 requested_millis 必须 > 0")]
    BadRequestedMillis(String),
    #[error("沙盒 {0} 的权重/申请求和溢出 u64/u128")]
    ArithmeticOverflow(String),
    #[error("未知信任级别 {0:?}（仅 system/official/certified/third_party）")]
    InvalidTier(String),
    #[error("未知时延等级 {0:?}（仅 sensitive/tolerant）")]
    InvalidLatency(String),
    #[error("黑名单沙盒 {0} 不参与 CPU 调度")]
    BlacklistedSandbox(String),
    #[error("沙盒 {0}（信任级别 {1}）不得自我提升为时延敏感，Sensitive 仅限 system/official")]
    IllegalLatencySelfPromotion(String, &'static str),
}

/// 单个沙盒在 CPU 优先级模型中的原始申报（`latency` 是**待仲裁的自报值**）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct CpuSandboxRequest {
    pub sandbox_id: String,
    /// 信任级别（宿主按插件清单前缀裁定后传入；不由沙盒自己说了算）。
    pub tier: Tier,
    /// 自报时延等级；最终是否采纳由 [`arbitrate_priority`] 依 tier 决定。
    #[serde(default)]
    pub latency: Option<LatencyClass>,
    /// 权重；缺省 [`DEFAULT_WEIGHT`]。
    #[serde(default)]
    pub weight: Option<u32>,
    /// 申请的 CPU 时间（millicore·毫秒口径，仅记账，可整体超卖；本版不与容量比较）。
    pub requested_millis: u64,
}

/// 仲裁后一个沙盒的有效 CPU 调度画像（全部由宿主裁定，不可被申报伪造）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EffectiveCpuEntry {
    pub sandbox_id: String,
    pub tier: Tier,
    /// 裁定后的有效时延等级（低信任级自报 Sensitive 会被拒绝，缺省按级默认）。
    pub effective_latency: LatencyClass,
    /// 数值优先级（Sensitive=2、Tolerant=1）。
    pub effective_priority: u8,
    /// 生效权重（缺省填默认值后）。
    pub weight: u32,
    /// 同等级组内的权重份额（千分点，weight / 组权重和 ×1000，整数取整）。
    pub weight_share_in_class_permille: u64,
    pub requested_millis: u64,
}

/// 一个时延等级组的汇总。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CpuClassSummary {
    pub count: usize,
    pub weight_sum: u128,
    pub requested_millis_sum: u128,
    /// 该组各成员份额千分点之和（整数取整后可能略 ≠1000；单成员=1000，空组=0）。
    pub weight_share_total_permille: u64,
}

/// 一次 CPU 优先级仲裁的完整确定性结果。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CpuPriorityModel {
    pub capacity_millis: u64,
    /// 按优先级（Sensitive 先）、再按权重（大先）、再按 id 字典序排好的条目。
    pub entries: Vec<EffectiveCpuEntry>,
    pub sensitive: CpuClassSummary,
    pub tolerant: CpuClassSummary,
}

/// 解析信任级别字符串（与 `Tier::as_str` 的 snake_case 口径一致）。
pub fn parse_tier(s: &str) -> Result<Tier, CpuError> {
    match s {
        "system" => Ok(Tier::System),
        "official" => Ok(Tier::Official),
        "certified" => Ok(Tier::Certified),
        "third_party" => Ok(Tier::ThirdParty),
        "blacklist" => Ok(Tier::Blacklist),
        other => Err(CpuError::InvalidTier(other.to_string())),
    }
}

/// 解析时延等级字符串（小写）；serde 派生的变体名是 `Sensitive/Tolerant`，这里
/// 额外接受小写与连写，便于 PMB JSON 使用。
pub fn parse_latency(s: &str) -> Result<LatencyClass, CpuError> {
    match s {
        "sensitive" => Ok(LatencyClass::Sensitive),
        "tolerant" => Ok(LatencyClass::Tolerant),
        other => Err(CpuError::InvalidLatency(other.to_string())),
    }
}

/// 该信任级别是否允许进入 Sensitive 等级（仅宿主系统/官方核心路径）。
#[inline]
fn tier_may_be_sensitive(tier: Tier) -> bool {
    matches!(tier, Tier::System | Tier::Official)
}

/// 信任级别的默认时延等级：System/Official 默认 Sensitive；其余默认 Tolerant。
#[inline]
fn default_latency_for_tier(tier: Tier) -> LatencyClass {
    if tier_may_be_sensitive(tier) {
        LatencyClass::Sensitive
    } else {
        LatencyClass::Tolerant
    }
}

#[inline]
fn priority_of(latency: LatencyClass) -> u8 {
    match latency {
        LatencyClass::Sensitive => PRIORITY_SENSITIVE,
        LatencyClass::Tolerant => PRIORITY_TOLERANT,
    }
}

/// 校验单个申报并产出**裁定后**的有效时延等级与权重（不构造 entry，便于纯函数测试）。
fn resolve_one(req: &CpuSandboxRequest) -> Result<(LatencyClass, u32), CpuError> {
    if req.sandbox_id.is_empty() {
        return Err(CpuError::EmptySandboxId);
    }
    if req.tier == Tier::Blacklist {
        return Err(CpuError::BlacklistedSandbox(req.sandbox_id.clone()));
    }
    let weight = req.weight.unwrap_or(DEFAULT_WEIGHT);
    if !(1..=MAX_WEIGHT).contains(&weight) {
        return Err(CpuError::WeightOutOfRange(
            req.sandbox_id.clone(),
            MAX_WEIGHT,
            weight,
        ));
    }
    if req.requested_millis == 0 {
        return Err(CpuError::BadRequestedMillis(req.sandbox_id.clone()));
    }
    let effective = match req.latency {
        None => default_latency_for_tier(req.tier),
        Some(LatencyClass::Sensitive) => {
            if !tier_may_be_sensitive(req.tier) {
                return Err(CpuError::IllegalLatencySelfPromotion(
                    req.sandbox_id.clone(),
                    req.tier.as_str(),
                ));
            }
            LatencyClass::Sensitive
        }
        Some(t @ LatencyClass::Tolerant) => t,
    };
    Ok((effective, weight))
}

/// 对一组沙盒申报做 CPU 两级优先级仲裁，产出确定性的优先级/权重模型。
///
/// - `capacity_millis` 为节点 CPU 容量（>0）；本版仅登记，**不做容量约束**
///   （竞争下的确定性配额分配在 v3.7.8，突发准入在 v3.7.9）。
/// - 任一沙盒非法（空/重复 id、权重越界、申请为 0、低信任自报 Sensitive、黑名单）
///   → 整体 fail-closed，不返回部分结果。
pub fn arbitrate_priority(
    capacity_millis: u64,
    requests: &[CpuSandboxRequest],
) -> Result<CpuPriorityModel, CpuError> {
    if capacity_millis == 0 {
        return Err(CpuError::ZeroCapacity);
    }

    // 先逐个仲裁 + 去重（重复 id 与非法申报一并 fail-closed）。
    let mut seen = std::collections::HashSet::new();
    let mut resolved: Vec<(CpuSandboxRequest, LatencyClass, u32)> =
        Vec::with_capacity(requests.len());
    for req in requests {
        if !seen.insert(req.sandbox_id.clone()) {
            return Err(CpuError::DuplicateSandbox(req.sandbox_id.clone()));
        }
        let (latency, weight) = resolve_one(req)?;
        resolved.push((req.clone(), latency, weight));
    }

    // 组权重/申请求和（u128，先算和，再统一算份额，避免重复溢出检查散落）。
    let mut sens_weight: u128 = 0;
    let mut sens_request: u128 = 0;
    let mut tol_weight: u128 = 0;
    let mut tol_request: u128 = 0;
    for (_, latency, weight) in &resolved {
        let w = u128::from(*weight);
        match latency {
            LatencyClass::Sensitive => {
                sens_weight = sens_weight.checked_add(w).ok_or_else(|| {
                    CpuError::ArithmeticOverflow("sensitive.weight_sum".to_string())
                })?;
            }
            LatencyClass::Tolerant => {
                tol_weight = tol_weight.checked_add(w).ok_or_else(|| {
                    CpuError::ArithmeticOverflow("tolerant.weight_sum".to_string())
                })?;
            }
        }
    }
    for (req, latency, _) in &resolved {
        let r = u128::from(req.requested_millis);
        match latency {
            LatencyClass::Sensitive => {
                sens_request = sens_request.checked_add(r).ok_or_else(|| {
                    CpuError::ArithmeticOverflow("sensitive.requested_sum".to_string())
                })?;
            }
            LatencyClass::Tolerant => {
                tol_request = tol_request.checked_add(r).ok_or_else(|| {
                    CpuError::ArithmeticOverflow("tolerant.requested_sum".to_string())
                })?;
            }
        }
    }

    // 构造条目并按组权重和归一化份额（千分点）。
    let mut entries: Vec<EffectiveCpuEntry> = Vec::with_capacity(resolved.len());
    let mut sens_share_total: u128 = 0;
    let mut tol_share_total: u128 = 0;
    for (req, latency, weight) in resolved {
        let group_weight = match latency {
            LatencyClass::Sensitive => sens_weight,
            LatencyClass::Tolerant => tol_weight,
        };
        // group_weight 至少有该成员自身 weight≥1，故恒 >0。
        let share = u128::from(weight)
            .checked_mul(1000)
            .and_then(|n| n.checked_div(group_weight))
            .ok_or_else(|| CpuError::ArithmeticOverflow(req.sandbox_id.clone()))?
            as u64;
        match latency {
            LatencyClass::Sensitive => {
                sens_share_total = sens_share_total.saturating_add(u128::from(share))
            }
            LatencyClass::Tolerant => {
                tol_share_total = tol_share_total.saturating_add(u128::from(share))
            }
        }
        entries.push(EffectiveCpuEntry {
            sandbox_id: req.sandbox_id,
            tier: req.tier,
            effective_latency: latency,
            effective_priority: priority_of(latency),
            weight,
            weight_share_in_class_permille: share,
            requested_millis: req.requested_millis,
        });
    }

    // 确定性排序：优先级降序 → 权重降序 → 申请降序 → id 字典序。
    entries.sort_by(|a, b| {
        b.effective_priority
            .cmp(&a.effective_priority)
            .then_with(|| b.weight.cmp(&a.weight))
            .then_with(|| b.requested_millis.cmp(&a.requested_millis))
            .then_with(|| a.sandbox_id.cmp(&b.sandbox_id))
    });

    let sens_count = entries
        .iter()
        .filter(|e| e.effective_latency == LatencyClass::Sensitive)
        .count();
    let tol_count = entries.len() - sens_count;

    Ok(CpuPriorityModel {
        capacity_millis,
        entries,
        sensitive: CpuClassSummary {
            count: sens_count,
            weight_sum: sens_weight,
            requested_millis_sum: sens_request,
            weight_share_total_permille: sens_share_total as u64,
        },
        tolerant: CpuClassSummary {
            count: tol_count,
            weight_sum: tol_weight,
            requested_millis_sum: tol_request,
            weight_share_total_permille: tol_share_total as u64,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(
        id: &str,
        tier: Tier,
        latency: Option<LatencyClass>,
        weight: Option<u32>,
        millis: u64,
    ) -> CpuSandboxRequest {
        CpuSandboxRequest {
            sandbox_id: id.to_string(),
            tier,
            latency,
            weight,
            requested_millis: millis,
        }
    }

    #[test]
    fn sensitive_group_ranks_ahead_of_tolerant() {
        let m = arbitrate_priority(
            1000,
            &[
                req(
                    "bg",
                    Tier::ThirdParty,
                    Some(LatencyClass::Tolerant),
                    None,
                    500,
                ),
                req("core", Tier::System, None, None, 300),
            ],
        )
        .unwrap();
        // core 为 system、缺省 Sensitive，必须排在第三方 tolerant 之前。
        assert_eq!(m.entries[0].sandbox_id, "core");
        assert_eq!(m.entries[0].effective_priority, PRIORITY_SENSITIVE);
        assert_eq!(m.entries[1].sandbox_id, "bg");
        assert_eq!(m.entries[1].effective_priority, PRIORITY_TOLERANT);
        assert_eq!(m.sensitive.count, 1);
        assert_eq!(m.tolerant.count, 1);
    }

    #[test]
    fn default_latency_follows_tier_and_low_trust_cannot_self_promote() {
        // Certified/ThirdParty 缺省 → Tolerant。
        let m = arbitrate_priority(
            1000,
            &[
                req("c", Tier::Certified, None, None, 100),
                req("t", Tier::ThirdParty, None, None, 100),
                req("o", Tier::Official, None, None, 100),
            ],
        )
        .unwrap();
        assert_eq!(m.sensitive.count, 1); // 只有 official
        assert_eq!(m.tolerant.count, 2);

        // 自报 Sensitive 的低信任级必须被拒绝。
        let err = arbitrate_priority(
            1000,
            &[req(
                "x",
                Tier::Certified,
                Some(LatencyClass::Sensitive),
                None,
                100,
            )],
        )
        .unwrap_err();
        assert!(
            matches!(err, CpuError::IllegalLatencySelfPromotion(_, "certified")),
            "{err:?}"
        );
        let err2 = arbitrate_priority(
            1000,
            &[req(
                "y",
                Tier::ThirdParty,
                Some(LatencyClass::Sensitive),
                None,
                100,
            )],
        )
        .unwrap_err();
        assert!(matches!(
            err2,
            CpuError::IllegalLatencySelfPromotion(_, "third_party")
        ));

        // 黑名单拒绝。
        assert!(matches!(
            arbitrate_priority(1000, &[req("z", Tier::Blacklist, None, None, 100)]).unwrap_err(),
            CpuError::BlacklistedSandbox(_)
        ));
    }

    #[test]
    fn weight_shares_are_in_class_permille_and_ordered() {
        // 同一 Sensitive 组：权重 100/300 → 250‰/750‰。
        let m = arbitrate_priority(
            1000,
            &[
                req(
                    "a",
                    Tier::Official,
                    Some(LatencyClass::Sensitive),
                    Some(100),
                    100,
                ),
                req(
                    "b",
                    Tier::System,
                    Some(LatencyClass::Sensitive),
                    Some(300),
                    100,
                ),
            ],
        )
        .unwrap();
        let a = m.entries.iter().position(|e| e.sandbox_id == "a").unwrap();
        let b = m.entries.iter().position(|e| e.sandbox_id == "b").unwrap();
        assert_eq!(m.entries[b].weight_share_in_class_permille, 750);
        assert_eq!(m.entries[a].weight_share_in_class_permille, 250);
        assert_eq!(m.sensitive.weight_share_total_permille, 1000);
        // 权重大者在同组内排前。
        assert_eq!(m.entries[0].sandbox_id, "b");
        // Tolerant 空组份额和为 0。
        assert_eq!(m.tolerant.weight_share_total_permille, 0);
    }

    #[test]
    fn rounding_total_is_reported_not_pretended_exact() {
        // 三个等权成员：1000/3=333 整除，和=999，如实呈现而非伪称 1000。
        let m = arbitrate_priority(
            1000,
            &[
                req("a", Tier::ThirdParty, None, Some(100), 10),
                req("b", Tier::ThirdParty, None, Some(100), 10),
                req("c", Tier::ThirdParty, None, Some(100), 10),
            ],
        )
        .unwrap();
        for e in &m.entries {
            assert_eq!(e.weight_share_in_class_permille, 333);
        }
        assert_eq!(m.tolerant.weight_share_total_permille, 999);
    }

    #[test]
    fn deterministic_order_tie_breaks_by_id() {
        let mk = |id: &str| req(id, Tier::ThirdParty, None, Some(100), 10);
        let m1 = arbitrate_priority(1000, &[mk("z"), mk("a"), mk("m")]).unwrap();
        let m2 = arbitrate_priority(1000, &[mk("m"), mk("z"), mk("a")]).unwrap();
        let ids1: Vec<_> = m1.entries.iter().map(|e| e.sandbox_id.clone()).collect();
        let ids2: Vec<_> = m2.entries.iter().map(|e| e.sandbox_id.clone()).collect();
        assert_eq!(ids1, vec!["a", "m", "z"]);
        assert_eq!(ids1, ids2);
    }

    #[test]
    fn validation_fails_closed() {
        // 容量 0。
        assert!(matches!(
            arbitrate_priority(0, &[req("a", Tier::System, None, None, 1)]).unwrap_err(),
            CpuError::ZeroCapacity
        ));
        // 空 id。
        assert!(matches!(
            arbitrate_priority(1, &[req("", Tier::System, None, None, 1)]).unwrap_err(),
            CpuError::EmptySandboxId
        ));
        // 重复 id。
        assert!(matches!(
            arbitrate_priority(
                1,
                &[
                    req("a", Tier::System, None, None, 1),
                    req("a", Tier::Official, None, None, 1)
                ]
            )
            .unwrap_err(),
            CpuError::DuplicateSandbox(_)
        ));
        // 权重 0 与越上界。
        assert!(matches!(
            arbitrate_priority(1, &[req("a", Tier::System, None, Some(0), 1)]).unwrap_err(),
            CpuError::WeightOutOfRange(_, MAX_WEIGHT, 0)
        ));
        assert!(matches!(
            arbitrate_priority(1, &[req("a", Tier::System, None, Some(MAX_WEIGHT + 1), 1)])
                .unwrap_err(),
            CpuError::WeightOutOfRange(_, MAX_WEIGHT, _)
        ));
        // 申请为 0。
        assert!(matches!(
            arbitrate_priority(1, &[req("a", Tier::System, None, None, 0)]).unwrap_err(),
            CpuError::BadRequestedMillis(_)
        ));
        // 默认权重填充。
        let m = arbitrate_priority(1, &[req("a", Tier::System, None, None, 1)]).unwrap();
        assert_eq!(m.entries[0].weight, DEFAULT_WEIGHT);
    }

    #[test]
    fn parsers_accept_and_reject() {
        assert_eq!(parse_tier("official").unwrap(), Tier::Official);
        assert!(matches!(
            parse_tier("sys").unwrap_err(),
            CpuError::InvalidTier(_)
        ));
        assert_eq!(parse_latency("sensitive").unwrap(), LatencyClass::Sensitive);
        assert!(matches!(
            parse_latency("urgent").unwrap_err(),
            CpuError::InvalidLatency(_)
        ));
        assert_eq!(priority_of(LatencyClass::Sensitive), PRIORITY_SENSITIVE);
        assert_eq!(priority_of(LatencyClass::Tolerant), PRIORITY_TOLERANT);
    }
}
