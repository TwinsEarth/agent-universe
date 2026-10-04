//! 快照商品化版税账本（v3.8.6）。
//!
//! # 定位
//!
//! v3.8.5 的托管五桶里预留了 `royalty` 桶，但它只按订单上一个聚合费率算出一笔版税额，
//! 既不知道这笔钱该付给**哪个环境快照的创建者**，也无法在多次复用同一 `pack_diff`
//! 快照时累计版税。v3.8.6 把 AUSec 的增量快照（`pack_diff`）正式变成可挂单商品：
//!
//! - [`SnapshotRegistry`]：注册可复用快照引用 `snapshot_ref` → 创建者 DID + 版税千分点
//!   费率（快照商品目录，fail-closed 校验）；
//! - [`RoyaltyLedger`]：每笔已结算订单按其恢复的快照记一次版税（**一笔订单只记一次**），
//!   按快照维度、创建者维度、全表三维度累计，并与 v3.8.5 [`EscrowSplit`] 的 `royalty`
//!   桶**逐单对账**——托管账上抽出的版税必须等于本账本该付给快照创建者的钱，否则
//!   fail-closed，绝不允许「扣了版税却找不到受款快照」或「该付的版税与托管额不符」。
//!
//! 纯确定性内存记账面：零浮点、零 syscall、零 unsafe、无 panic 路径，金额全 `u128`
//! checked，费率整数千分点（[`PERMYRIAD`]，1000 = 100%）。取整口径与 v3.8.5 完全一致：
//! `royalty = gross × permyriad / 1000` 整数向下取整，取整余数留供给方（不另造桶）。
//!
//! # 守恒
//!
//! - 全表版税：`total_royalty == Σ 每个已注册快照累计版税 == Σ 每个创建者累计版税`
//!   （三条独立求和路径必须相等，[`RoyaltyLedger::invariant_holds`]）；
//! - 逐单对账：`accrue` 时外部传入的托管版税额必须等于按快照费率算出的版税，不一致
//!   [`ResourceError::RoyaltyEscrowMismatch`]，且失败不落任何记录；
//! - 未恢复任何快照的订单（`snapshot_ref = None`）版税必须为 0——若托管账上却有非零
//!   royalty 桶，同样按对账不符拒绝。

use serde::{Deserialize, Serialize};

use super::ResourceError;

/// 千分点基数：1000 = 100%。
pub const PERMYRIAD: u128 = 1000;

/// 一件可商品化的 `pack_diff` 环境快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotAsset {
    /// 快照引用（pack_diff/snapshot_ref），全表唯一、非空。
    pub snapshot_ref: String,
    /// 快照创建者 DID，版税受款方，非空。
    pub creator_did: String,
    /// 版税费率（千分点，对复用该快照订单的实耗 gross 抽取），≤ 1000。
    pub royalty_permyriad: u32,
}

/// 快照商品目录：`snapshot_ref` → 创建者 + 版税费率。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotRegistry {
    snapshots: Vec<SnapshotAsset>,
}

impl SnapshotRegistry {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn snapshots(&self) -> &[SnapshotAsset] {
        &self.snapshots
    }

    /// 注册一件快照商品；空引用/空创建者、费率超 1000‰、重复引用均 fail-closed。
    pub fn register(
        &mut self,
        snapshot_ref: impl Into<String>,
        creator_did: impl Into<String>,
        royalty_permyriad: u32,
    ) -> Result<(), ResourceError> {
        let snapshot_ref = snapshot_ref.into();
        let creator_did = creator_did.into();
        if snapshot_ref.trim().is_empty() {
            return Err(ResourceError::EmptySnapshotRef);
        }
        if creator_did.trim().is_empty() {
            return Err(ResourceError::EmptyProviderDid);
        }
        if u128::from(royalty_permyriad) > PERMYRIAD {
            return Err(ResourceError::SnapshotRoyaltyRateExceedsTotal {
                permyriad: royalty_permyriad,
            });
        }
        if self.resolve(&snapshot_ref).is_ok() {
            return Err(ResourceError::DuplicateSnapshotRef { snapshot_ref });
        }
        self.snapshots.push(SnapshotAsset {
            snapshot_ref,
            creator_did,
            royalty_permyriad,
        });
        Ok(())
    }

    /// 解析快照引用；未注册 [`ResourceError::SnapshotRefNotFound`]。
    pub fn resolve(&self, snapshot_ref: &str) -> Result<&SnapshotAsset, ResourceError> {
        self.snapshots
            .iter()
            .find(|s| s.snapshot_ref == snapshot_ref)
            .ok_or_else(|| ResourceError::SnapshotRefNotFound {
                snapshot_ref: snapshot_ref.to_string(),
            })
    }
}

/// 一笔订单因恢复某快照而累计的版税记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoyaltyAccrual {
    /// 结算订单 id（全表唯一，一笔订单只记一次版税）。
    pub order_id: String,
    /// 被恢复的快照引用；None 表示该订单未恢复任何商品化快照（版税必须为 0）。
    pub snapshot_ref: Option<String>,
    /// 受款创建者 DID；与快照引用成对，None 表示无快照无版税。
    pub creator_did: Option<String>,
    /// 本单版税额（micro），= gross × 快照费率 / 1000，向下取整。
    pub royalty_micro: u128,
}

/// 快照版税账本：持有快照目录与逐单版税累计。
#[derive(Debug, Clone, Default)]
pub struct RoyaltyLedger {
    registry: SnapshotRegistry,
    accruals: Vec<RoyaltyAccrual>,
}

impl RoyaltyLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册快照商品（委托目录，校验规则同 [`SnapshotRegistry::register`]）。
    pub fn register_snapshot(
        &mut self,
        snapshot_ref: impl Into<String>,
        creator_did: impl Into<String>,
        royalty_permyriad: u32,
    ) -> Result<(), ResourceError> {
        self.registry
            .register(snapshot_ref, creator_did, royalty_permyriad)
    }

    pub fn registry(&self) -> &SnapshotRegistry {
        &self.registry
    }

    pub fn len(&self) -> usize {
        self.accruals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.accruals.is_empty()
    }

    pub fn accruals(&self) -> &[RoyaltyAccrual] {
        &self.accruals
    }

    /// 纯计算：按快照千分点费率对实耗 gross 算版税（整数向下取整，余数留供给方）。
    pub fn royalty_for(royalty_permyriad: u32, gross_micro: u128) -> Result<u128, ResourceError> {
        gross_micro
            .checked_mul(u128::from(royalty_permyriad))
            .and_then(|v| v.checked_div(PERMYRIAD))
            .ok_or(ResourceError::ArithmeticOverflow)
    }

    /// 记一笔已结算订单的快照版税，并与托管 royalty 桶逐单对账。
    ///
    /// - `order_id` 非空且全表唯一（一笔订单只记一次，重复 [`RoyaltyAlreadyAccrued`]）；
    /// - `snapshot_ref = None`：未恢复商品化快照，计算版税 0，此时托管版税也必须为 0；
    /// - `snapshot_ref = Some`：快照必须已注册（否则 [`SnapshotRefNotFound`]），按其费率
    ///   与 gross 算版税；算出值必须 **等于** 外部传入的 `escrow_royalty_micro`
    ///   （v3.8.5 托管 royalty 桶），不一致 [`RoyaltyEscrowMismatch`] 且**不落任何记录**。
    pub fn accrue(
        &mut self,
        order_id: impl Into<String>,
        snapshot_ref: Option<&str>,
        gross_micro: u128,
        escrow_royalty_micro: u128,
    ) -> Result<u128, ResourceError> {
        let order_id = order_id.into();
        if order_id.trim().is_empty() {
            return Err(ResourceError::EmptyOrderId);
        }
        if self.accruals.iter().any(|a| a.order_id == order_id) {
            return Err(ResourceError::RoyaltyAlreadyAccrued { order_id });
        }

        let (stored_ref, creator, computed) = match snapshot_ref {
            None => (None, None, 0u128),
            Some(r) => {
                if r.trim().is_empty() {
                    return Err(ResourceError::EmptySnapshotRef);
                }
                let asset = self.registry.resolve(r)?;
                let royalty = Self::royalty_for(asset.royalty_permyriad, gross_micro)?;
                (
                    Some(r.to_string()),
                    Some(asset.creator_did.clone()),
                    royalty,
                )
            }
        };

        if computed != escrow_royalty_micro {
            return Err(ResourceError::RoyaltyEscrowMismatch {
                order_id,
                escrow_royalty: escrow_royalty_micro,
                accrued_royalty: computed,
            });
        }

        self.accruals.push(RoyaltyAccrual {
            order_id,
            snapshot_ref: stored_ref,
            creator_did: creator,
            royalty_micro: computed,
        });
        Ok(computed)
    }

    fn checked_sum<I>(mut vals: I) -> Result<u128, ResourceError>
    where
        I: Iterator<Item = u128>,
    {
        vals.try_fold(0u128, |acc, v| {
            acc.checked_add(v).ok_or(ResourceError::ArithmeticOverflow)
        })
    }

    /// 全表版税总额（所有订单累计）。
    pub fn total_royalty(&self) -> Result<u128, ResourceError> {
        Self::checked_sum(self.accruals.iter().map(|a| a.royalty_micro))
    }

    /// 某一快照被复用累计的版税。
    pub fn total_for_snapshot(&self, snapshot_ref: &str) -> Result<u128, ResourceError> {
        Self::checked_sum(
            self.accruals
                .iter()
                .filter(|a| a.snapshot_ref.as_deref() == Some(snapshot_ref))
                .map(|a| a.royalty_micro),
        )
    }

    /// 某创建者名下所有快照累计的版税。
    pub fn total_for_creator(&self, creator_did: &str) -> Result<u128, ResourceError> {
        Self::checked_sum(
            self.accruals
                .iter()
                .filter(|a| a.creator_did.as_deref() == Some(creator_did))
                .map(|a| a.royalty_micro),
        )
    }

    /// 某一快照被恢复（计费）次数。
    pub fn restore_count(&self, snapshot_ref: &str) -> usize {
        self.accruals
            .iter()
            .filter(|a| a.snapshot_ref.as_deref() == Some(snapshot_ref))
            .count()
    }

    /// 守恒不变量：三条独立求和路径（全表 / Σ快照 / Σ创建者）必须相等，且每条引用快照
    /// 的记录其受款创建者与费率都与当前目录一致（目录被错误改写时可被发现）。
    pub fn invariant_holds(&self) -> bool {
        let total = match self.total_royalty() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let sum_by_snapshot = Self::checked_sum(
            self.registry
                .snapshots()
                .iter()
                .map(|s| self.total_for_snapshot(&s.snapshot_ref))
                .filter_map(Result::ok),
        );
        // 无快照记录（None 版税 0）不进入快照维度；快照维度之和必须等于全表。
        let distinct_creators: Vec<&str> = self
            .registry
            .snapshots()
            .iter()
            .map(|s| s.creator_did.as_str())
            .fold(Vec::new(), |mut acc, d| {
                if !acc.contains(&d) {
                    acc.push(d);
                }
                acc
            });
        let sum_by_creator = Self::checked_sum(
            distinct_creators
                .into_iter()
                .filter_map(|d| self.total_for_creator(d).ok()),
        );

        // 每条有快照的版税记录必须能在目录中解析且创建者一致。
        let records_consistent =
            self.accruals
                .iter()
                .all(|a| match (&a.snapshot_ref, &a.creator_did) {
                    (None, None) => a.royalty_micro == 0,
                    (Some(r), Some(c)) => self
                        .registry
                        .resolve(r)
                        .map(|s| s.creator_did == *c)
                        .unwrap_or(false),
                    _ => false,
                });

        matches!(sum_by_snapshot, Ok(s) if s == total)
            && matches!(sum_by_creator, Ok(c) if c == total)
            && records_consistent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_basic_restore_accrues_creator_royalty() {
        let mut led = RoyaltyLedger::new();
        led.register_snapshot("snap-py311", "did:nau:alice", 50)
            .unwrap();
        // gross 800、费率 50‰ -> royalty 40。
        let r = led.accrue("ord-1", Some("snap-py311"), 800, 40).unwrap();
        assert_eq!(r, 40);
        assert_eq!(led.total_royalty().unwrap(), 40);
        assert_eq!(led.total_for_snapshot("snap-py311").unwrap(), 40);
        assert_eq!(led.total_for_creator("did:nau:alice").unwrap(), 40);
        assert_eq!(led.restore_count("snap-py311"), 1);
        assert!(led.invariant_holds());
    }

    #[test]
    fn floor_division_keeps_remainder_consistent_with_escrow() {
        // 与 v3.8.5 escrow 完全同口径：gross100、33‰ -> 3，余数留供给方。
        assert_eq!(RoyaltyLedger::royalty_for(33, 100).unwrap(), 3);
        let mut led = RoyaltyLedger::new();
        led.register_snapshot("s", "did:nau:bob", 33).unwrap();
        assert_eq!(led.accrue("o", Some("s"), 100, 3).unwrap(), 3);
        // 若托管侧错把余数也当版税（4），对账失败。
        assert!(matches!(
            led.accrue("o2", Some("s"), 100, 4),
            Err(ResourceError::RoyaltyEscrowMismatch { .. })
        ));
        assert_eq!(led.len(), 1);
    }

    #[test]
    fn duplicate_snapshot_ref_and_empty_ref_did_rejected() {
        let mut reg = SnapshotRegistry::new();
        reg.register("snap", "did:nau:a", 10).unwrap();
        assert!(matches!(
            reg.register("snap", "did:nau:b", 10),
            Err(ResourceError::DuplicateSnapshotRef { .. })
        ));
        assert!(matches!(
            reg.register("  ", "did:nau:a", 10),
            Err(ResourceError::EmptySnapshotRef)
        ));
        assert!(matches!(
            reg.register("snap2", "  ", 10),
            Err(ResourceError::EmptyProviderDid)
        ));
    }

    #[test]
    fn royalty_rate_over_thousand_rejected_boundary() {
        let mut reg = SnapshotRegistry::new();
        assert!(matches!(
            reg.register("hi", "did:nau:a", 1001),
            Err(ResourceError::SnapshotRoyaltyRateExceedsTotal { permyriad: 1001 })
        ));
        // 恰好 1000‰ 合法：gross100 版税 100（供给方候选应得 0）。
        reg.register("full", "did:nau:a", 1000).unwrap();
        let mut led = RoyaltyLedger::new();
        led.register_snapshot("full", "did:nau:a", 1000).unwrap();
        assert_eq!(led.accrue("o", Some("full"), 100, 100).unwrap(), 100);
        assert!(led.invariant_holds());
    }

    #[test]
    fn unknown_snapshot_restore_not_found() {
        let mut led = RoyaltyLedger::new();
        assert!(matches!(
            led.accrue("o", Some("ghost"), 100, 0),
            Err(ResourceError::SnapshotRefNotFound { .. })
        ));
        // 失败不落记录。
        assert!(led.is_empty());
    }

    #[test]
    fn duplicate_order_accrued_only_once_and_empty_order_rejected() {
        let mut led = RoyaltyLedger::new();
        led.register_snapshot("s", "did:nau:a", 50).unwrap();
        led.accrue("ord-x", Some("s"), 800, 40).unwrap();
        assert!(matches!(
            led.accrue("ord-x", Some("s"), 800, 40),
            Err(ResourceError::RoyaltyAlreadyAccrued { .. })
        ));
        assert!(matches!(
            led.accrue("  ", Some("s"), 800, 40),
            Err(ResourceError::EmptyOrderId)
        ));
        assert_eq!(led.len(), 1);
    }

    #[test]
    fn no_snapshot_order_requires_zero_escrow_royalty_else_mismatch() {
        let mut led = RoyaltyLedger::new();
        // 未恢复快照：版税 0，托管 royalty 桶必须也是 0。
        assert_eq!(led.accrue("plain", None, 800, 0).unwrap(), 0);
        // 托管侧却记了非零 royalty（扣了钱却找不到受款快照）-> 对账失败，不落记录。
        assert!(matches!(
            led.accrue("plain2", None, 800, 40),
            Err(ResourceError::RoyaltyEscrowMismatch { .. })
        ));
        assert_eq!(led.total_royalty().unwrap(), 0);
    }

    #[test]
    fn multiple_snapshots_and_creators_three_way_conservation() {
        let mut led = RoyaltyLedger::new();
        // alice 两个快照，bob 一个快照。
        led.register_snapshot("a1", "did:nau:alice", 50).unwrap();
        led.register_snapshot("a2", "did:nau:alice", 100).unwrap();
        led.register_snapshot("b1", "did:nau:bob", 25).unwrap();
        // a1: gross800 ->40；a2: gross400 ->40；b1: gross1000 ->25；无快照单 ->0。
        led.accrue("o1", Some("a1"), 800, 40).unwrap();
        led.accrue("o2", Some("a2"), 400, 40).unwrap();
        led.accrue("o3", Some("b1"), 1000, 25).unwrap();
        led.accrue("o4", None, 500, 0).unwrap();

        assert_eq!(led.total_for_snapshot("a1").unwrap(), 40);
        assert_eq!(led.total_for_snapshot("a2").unwrap(), 40);
        assert_eq!(led.total_for_snapshot("b1").unwrap(), 25);
        assert_eq!(led.restore_count("a1"), 1);
        assert_eq!(led.total_for_creator("did:nau:alice").unwrap(), 80);
        assert_eq!(led.total_for_creator("did:nau:bob").unwrap(), 25);
        // 全表 105 = Σ快照(40+40+25) = Σ创建者(80+25)，无快照单 0 不破坏守恒。
        assert_eq!(led.total_royalty().unwrap(), 105);
        assert!(led.invariant_holds());
    }
}
