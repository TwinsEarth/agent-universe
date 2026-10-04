//! 四维信誉账本（v3.8.7）。
//!
//! # 定位
//!
//! v3.8.6 的快照版税回答了「钱按快照怎么分」，但市场撮合/定价还缺一个确定性输入：
//! 某个供给方/Agent 到底**可不可信、快不快、稳不稳**。v3.8.7 新增**四维信誉账本**
//! [`ReputationLedger`]`，把每笔已结算订单的事后反馈按四个相互独立的维度累计：
//!
//! - [`ReputationDimension::Quality`]：结果质量（QA 抽样/买方评分）；
//! - [`ReputationDimension::Speed`]：交付速度（相对时延承诺）；
//! - [`ReputationDimension::Honesty`]：诚实度（是否有作弊/谎报/ equivocation）；
//! - [`ReputationDimension::Availability`]：可用性（接单/在线/履约稳定度）。
//!
//! 纯确定性内存记账面：零浮点、零 syscall、零 unsafe、无 panic 路径。评分整数
//! `0..=100`（[`MAX_RATING`]），均值用整数**千分位**（[`PERMILLE`]，100000=100.0）
//! 表达 `sum×1000/count` 向下取整，复合分用和为 1000‰ 的整数权重加权。信誉**绑定
//! 身份、不可转让**：账本只有「记反馈」与「读聚合」，没有任何转账/过户/买卖接口。
//!
//! # 单一事实源与守恒
//!
//! 账本只保存不可变反馈日志 [`ReputationFeedback`]，所有维度求和/计数/均值/复合分都
//! 在查询时由日志确定性 fold 得出（不另存会漂移的累加器）。恒有：
//! - 全表反馈数 == Σ 每个主体反馈数；
//! - 任一字段任一维度的千分位均值 ∈ [0, 100000]（评分有界，复合权重归一）；
//! - 复合分 == 两独立路径（逐反馈加权 / 逐维度均值加权）重算结果。

use serde::{Deserialize, Serialize};

use super::ResourceError;

/// 评分上限（百分制）：0..=100。
pub const MAX_RATING: u32 = 100;
/// 千分位基数；均值用 permille 表示，100000 permille = 100.0 分。
pub const PERMILLE: u128 = 1000;

/// 信誉的四个相互独立维度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReputationDimension {
    /// 结果质量。
    Quality,
    /// 交付速度。
    Speed,
    /// 诚实度（作弊/谎报/equivocation）。
    Honesty,
    /// 可用性（接单/在线/履约稳定）。
    Availability,
}

impl ReputationDimension {
    /// 固定枚举顺序（与权重/数组下标对齐）。
    pub const ALL: [ReputationDimension; 4] = [
        ReputationDimension::Quality,
        ReputationDimension::Speed,
        ReputationDimension::Honesty,
        ReputationDimension::Availability,
    ];

    /// 稳定标识名（小写）。
    pub fn as_str(self) -> &'static str {
        match self {
            ReputationDimension::Quality => "quality",
            ReputationDimension::Speed => "speed",
            ReputationDimension::Honesty => "honesty",
            ReputationDimension::Availability => "availability",
        }
    }
}

/// 四维整数权重（千分点），四维之和必须恰为 1000（[`DimensionWeights::new`] 校验）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DimensionWeights {
    pub quality: u32,
    pub speed: u32,
    pub honesty: u32,
    pub availability: u32,
}

impl Default for DimensionWeights {
    /// 默认四维等权：各 250‰。
    fn default() -> Self {
        Self {
            quality: 250,
            speed: 250,
            honesty: 250,
            availability: 250,
        }
    }
}

impl DimensionWeights {
    /// 构造并强校验四维权重之和 == 1000‰，否则 fail-closed。
    pub fn new(
        quality: u32,
        speed: u32,
        honesty: u32,
        availability: u32,
    ) -> Result<Self, ResourceError> {
        let w = Self {
            quality,
            speed,
            honesty,
            availability,
        };
        let sum = quality
            .checked_add(speed)
            .and_then(|s| s.checked_add(honesty))
            .and_then(|s| s.checked_add(availability))
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if sum != 1000 {
            return Err(ResourceError::ReputationWeightsNotNormalized { sum });
        }
        Ok(w)
    }

    /// 取某维权重。
    pub fn weight_of(self, dim: ReputationDimension) -> u32 {
        match dim {
            ReputationDimension::Quality => self.quality,
            ReputationDimension::Speed => self.speed,
            ReputationDimension::Honesty => self.honesty,
            ReputationDimension::Availability => self.availability,
        }
    }

    fn sum(self) -> Result<u128, ResourceError> {
        u128::from(self.quality)
            .checked_add(u128::from(self.speed))
            .and_then(|s| s.checked_add(u128::from(self.honesty)))
            .and_then(|s| s.checked_add(u128::from(self.availability)))
            .ok_or(ResourceError::ArithmeticOverflow)
    }
}

/// 一条不可变事后反馈（一笔订单对一个主体评一次四维分）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReputationFeedback {
    /// 反馈/结算订单 id，全表唯一（一笔订单只能记一次反馈）。
    pub feedback_id: String,
    /// 被评主体 DID（供给方/Agent），非空。
    pub subject_did: String,
    /// 质量分 0..=100。
    pub quality: u32,
    /// 速度分 0..=100。
    pub speed: u32,
    /// 诚实分 0..=100。
    pub honesty: u32,
    /// 可用分 0..=100。
    pub availability: u32,
}

impl ReputationFeedback {
    fn rating_of(&self, dim: ReputationDimension) -> u32 {
        match dim {
            ReputationDimension::Quality => self.quality,
            ReputationDimension::Speed => self.speed,
            ReputationDimension::Honesty => self.honesty,
            ReputationDimension::Availability => self.availability,
        }
    }
}

/// 四维信誉账本：主体不可转让，只接受有界反馈并提供确定性聚合。
#[derive(Debug, Clone, Default)]
pub struct ReputationLedger {
    weights: DimensionWeights,
    feedbacks: Vec<ReputationFeedback>,
}

impl ReputationLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// 用自定义归一双重建账（权重四维和必须 1000‰）。
    pub fn with_weights(weights: DimensionWeights) -> Self {
        Self {
            weights,
            feedbacks: Vec::new(),
        }
    }

    pub fn weights(&self) -> DimensionWeights {
        self.weights
    }

    /// 全表反馈条数。
    pub fn len(&self) -> usize {
        self.feedbacks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.feedbacks.is_empty()
    }

    pub fn feedbacks(&self) -> &[ReputationFeedback] {
        &self.feedbacks
    }

    /// 出现过的不同主体（首次出现顺序）。
    pub fn subjects(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.feedbacks {
            if !out.contains(&f.subject_did) {
                out.push(f.subject_did.clone());
            }
        }
        out
    }

    /// 记一条反馈：空 id/空主体、任一维分越界、重复 feedback_id 均 fail-closed。
    pub fn record(&mut self, fb: ReputationFeedback) -> Result<(), ResourceError> {
        if fb.feedback_id.trim().is_empty() {
            return Err(ResourceError::EmptyFeedbackId);
        }
        if fb.subject_did.trim().is_empty() {
            return Err(ResourceError::EmptyReputationSubject);
        }
        for dim in ReputationDimension::ALL {
            let v = fb.rating_of(dim);
            if v > MAX_RATING {
                return Err(ResourceError::ReputationRatingOutOfRange {
                    dimension: dim.as_str().to_string(),
                    value: v,
                });
            }
        }
        if self
            .feedbacks
            .iter()
            .any(|x| x.feedback_id == fb.feedback_id)
        {
            return Err(ResourceError::DuplicateReputationFeedback {
                feedback_id: fb.feedback_id,
            });
        }
        self.feedbacks.push(fb);
        Ok(())
    }

    fn for_subject<'a>(
        &'a self,
        subject_did: &'a str,
    ) -> impl Iterator<Item = &'a ReputationFeedback> + 'a {
        self.feedbacks
            .iter()
            .filter(move |f| f.subject_did == subject_did)
    }

    /// 某主体某维度的反馈数。
    pub fn feedback_count_for(&self, subject_did: &str) -> usize {
        self.for_subject(subject_did).count()
    }

    /// 某主体某维度评分之和（checked，u128）。
    pub fn dimension_sum(
        &self,
        subject_did: &str,
        dim: ReputationDimension,
    ) -> Result<u128, ResourceError> {
        self.for_subject(subject_did)
            .map(|f| u128::from(f.rating_of(dim)))
            .try_fold(0u128, |acc, v| {
                acc.checked_add(v).ok_or(ResourceError::ArithmeticOverflow)
            })
    }

    /// 某主体某维度的千分位均值 `sum×1000/count`（向下取整）；无反馈返回 0。
    pub fn dimension_average_permille(
        &self,
        subject_did: &str,
        dim: ReputationDimension,
    ) -> Result<u128, ResourceError> {
        let count = self.feedback_count_for(subject_did) as u128;
        if count == 0 {
            return Ok(0);
        }
        self.dimension_sum(subject_did, dim)?
            .checked_mul(PERMILLE)
            .and_then(|v| v.checked_div(count))
            .ok_or(ResourceError::ArithmeticOverflow)
    }

    /// 某主体四维复合信誉分（千分位）：按账本归一权重对四维均值加权求和后 /1000。
    /// 无任何反馈返回 0。
    pub fn composite_permille(&self, subject_did: &str) -> Result<u128, ResourceError> {
        if self.feedback_count_for(subject_did) == 0 {
            return Ok(0);
        }
        // 路径 A：逐维度均值 × 权重。
        let mut weighted: u128 = 0;
        for dim in ReputationDimension::ALL {
            let avg = self.dimension_average_permille(subject_did, dim)?;
            weighted = avg
                .checked_mul(u128::from(self.weights.weight_of(dim)))
                .and_then(|v| weighted.checked_add(v))
                .ok_or(ResourceError::ArithmeticOverflow)?;
        }
        let by_dim = weighted
            .checked_div(self.weights.sum()?)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        Ok(by_dim)
    }

    /// 守恒不变量：全表计数==Σ主体计数；各维均值有界；复合分与逐反馈重算路径一致。
    pub fn invariant_holds(&self) -> bool {
        // 权重必须归一（构造时保证，这里再独立校验一次）。
        if !matches!(self.weights.sum(), Ok(s) if s == 1000) {
            return false;
        }
        let total = self.feedbacks.len();
        let sum_by_subject: usize = self
            .subjects()
            .iter()
            .map(|s| self.feedback_count_for(s))
            .sum();
        if sum_by_subject != total {
            return false;
        }
        for s in self.subjects() {
            for dim in ReputationDimension::ALL {
                match self.dimension_average_permille(&s, dim) {
                    Ok(avg) if avg <= 100_000 => {}
                    _ => return false,
                }
            }
            // 路径 B：直接对每条反馈的四维加权平均再对反馈取均值（与路径 A 独立）。
            let mut acc_sum = 0u128;
            let mut n = 0u128;
            for f in self.for_subject(&s) {
                let mut per_fb = 0u128;
                for dim in ReputationDimension::ALL {
                    let term = match u128::from(f.rating_of(dim))
                        .checked_mul(PERMILLE)
                        .and_then(|v| v.checked_mul(u128::from(self.weights.weight_of(dim))))
                    {
                        Some(t) => t,
                        None => return false,
                    };
                    per_fb = match per_fb.checked_add(term) {
                        Some(v) => v,
                        None => return false,
                    };
                }
                // 每条反馈的复合（百分→permille，再除以权重和 1000）。
                let per_fb_comp = per_fb / 1000;
                acc_sum = match acc_sum.checked_add(per_fb_comp) {
                    Some(v) => v,
                    None => return false,
                };
                n += 1;
            }
            let by_feedback = acc_sum.checked_div(n).unwrap_or(0);
            match self.composite_permille(&s) {
                Ok(c) if c == by_feedback => {}
                _ => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fb(id: &str, who: &str, q: u32, s: u32, h: u32, a: u32) -> ReputationFeedback {
        ReputationFeedback {
            feedback_id: id.to_string(),
            subject_did: who.to_string(),
            quality: q,
            speed: s,
            honesty: h,
            availability: a,
        }
    }

    #[test]
    fn record_feedback_updates_four_dimensions_and_mean() {
        let mut led = ReputationLedger::new();
        led.record(fb("ord-1", "did:nau:p", 90, 80, 70, 100))
            .unwrap();
        assert_eq!(led.len(), 1);
        assert_eq!(led.feedback_count_for("did:nau:p"), 1);
        // 单条反馈：permille 均值 = 评分×10。
        assert_eq!(
            led.dimension_average_permille("did:nau:p", ReputationDimension::Quality)
                .unwrap(),
            90_000
        );
        assert_eq!(
            led.dimension_average_permille("did:nau:p", ReputationDimension::Speed)
                .unwrap(),
            80_000
        );
        assert_eq!(
            led.dimension_average_permille("did:nau:p", ReputationDimension::Honesty)
                .unwrap(),
            70_000
        );
        assert_eq!(
            led.dimension_average_permille("did:nau:p", ReputationDimension::Availability)
                .unwrap(),
            100_000
        );
        // 等权复合 = (90+80+70+100)/4 = 85 分 = 85000 permille。
        assert_eq!(led.composite_permille("did:nau:p").unwrap(), 85_000);
        assert!(led.invariant_holds());
    }

    #[test]
    fn multiple_feedback_integer_mean_deterministic_floor() {
        let mut led = ReputationLedger::new();
        led.record(fb("o1", "p", 90, 100, 100, 100)).unwrap();
        led.record(fb("o2", "p", 71, 100, 100, 100)).unwrap();
        // quality 均值 (90+71)/2 = 80.5 分 -> permille 161*1000/2 = 80500。
        assert_eq!(
            led.dimension_average_permille("p", ReputationDimension::Quality)
                .unwrap(),
            80_500
        );
        // 极端取整：90+90+91 -> 271*1000/3 = 90333（向下取整，确定性）。
        let mut l2 = ReputationLedger::new();
        l2.record(fb("a", "x", 90, 0, 0, 0)).unwrap();
        l2.record(fb("b", "x", 90, 0, 0, 0)).unwrap();
        l2.record(fb("c", "x", 91, 0, 0, 0)).unwrap();
        assert_eq!(
            l2.dimension_average_permille("x", ReputationDimension::Quality)
                .unwrap(),
            90_333
        );
        assert!(led.invariant_holds() && l2.invariant_holds());
    }

    #[test]
    fn rating_out_of_range_rejected_and_not_recorded() {
        let mut led = ReputationLedger::new();
        let cases = [
            (101, 0, 0, 0),
            (0, 200, 0, 0),
            (0, 0, 101, 0),
            (5, 5, 5, 102),
        ];
        for (i, (q, s, h, a)) in cases.iter().enumerate() {
            let err = led.record(fb(&format!("bad{i}"), "p", *q, *s, *h, *a));
            assert!(matches!(
                err,
                Err(ResourceError::ReputationRatingOutOfRange { .. })
            ));
        }
        assert!(led.is_empty());
        // 边界 0 与 100 合法。
        led.record(fb("ok", "p", 0, 100, 0, 100)).unwrap();
        assert_eq!(led.len(), 1);
    }

    #[test]
    fn duplicate_feedback_id_once_and_empty_fields_rejected() {
        let mut led = ReputationLedger::new();
        led.record(fb("dup", "p", 80, 80, 80, 80)).unwrap();
        assert!(matches!(
            led.record(fb("dup", "p2", 90, 90, 90, 90)),
            Err(ResourceError::DuplicateReputationFeedback { .. })
        ));
        assert!(matches!(
            led.record(fb("  ", "p", 1, 1, 1, 1)),
            Err(ResourceError::EmptyFeedbackId)
        ));
        assert!(matches!(
            led.record(fb("x", "  ", 1, 1, 1, 1)),
            Err(ResourceError::EmptyReputationSubject)
        ));
        // 重复 id 即便主体不同也拒绝（feedback_id 全表唯一），且不新增记录。
        assert_eq!(led.len(), 1);
    }

    #[test]
    fn unknown_subject_returns_zero_not_error() {
        let led = ReputationLedger::new();
        assert_eq!(led.feedback_count_for("ghost"), 0);
        for dim in ReputationDimension::ALL {
            assert_eq!(led.dimension_average_permille("ghost", dim).unwrap(), 0);
        }
        assert_eq!(led.composite_permille("ghost").unwrap(), 0);
        assert!(led.subjects().is_empty());
        assert!(led.invariant_holds());
    }

    #[test]
    fn custom_normalized_weights_composite_and_weight_validation() {
        // 非归一权重构造即 fail-closed。
        assert!(matches!(
            DimensionWeights::new(400, 200, 300, 50),
            Err(ResourceError::ReputationWeightsNotNormalized { sum: 950 })
        ));
        // 400/200/300/100 = 1000 合法；单条 (q90,s80,h70,a60)。
        let w = DimensionWeights::new(400, 200, 300, 100).unwrap();
        let mut led = ReputationLedger::with_weights(w);
        led.record(fb("o", "p", 90, 80, 70, 60)).unwrap();
        // 复合(百分) = (90*400+80*200+70*300+60*100)/1000
        //           = (36000+16000+21000+6000)/1000 = 79 分 = 79000 permille。
        assert_eq!(led.composite_permille("p").unwrap(), 79_000);
        assert!(led.invariant_holds());
    }

    #[test]
    fn multiple_subjects_independent_and_non_transferable() {
        let mut led = ReputationLedger::new();
        led.record(fb("o1", "alice", 100, 90, 80, 70)).unwrap();
        led.record(fb("o2", "alice", 80, 70, 60, 50)).unwrap();
        led.record(fb("o3", "bob", 40, 40, 40, 40)).unwrap();
        // 两主体计数独立：全表 3 == alice2 + bob1。
        assert_eq!(led.len(), 3);
        assert_eq!(led.feedback_count_for("alice"), 2);
        assert_eq!(led.feedback_count_for("bob"), 1);
        let subjects = led.subjects();
        assert_eq!(subjects, vec!["alice".to_string(), "bob".to_string()]);
        // bob 复合 = 40 分；alice 复合 = 各维 (100+80)/2=90,(90+70)/2=80,(80+60)/2=70,(70+50)/2=60
        // 等权 = (90+80+70+60)/4 = 75 分。
        assert_eq!(led.composite_permille("bob").unwrap(), 40_000);
        assert_eq!(led.composite_permille("alice").unwrap(), 75_000);
        // 信誉不可转让：账本 API 不存在任何 transfer/move 方法（编译期保证），
        // 主体只能靠自身反馈累积，alice 的分不会影响 bob。
        assert!(led.invariant_holds());
    }
}
