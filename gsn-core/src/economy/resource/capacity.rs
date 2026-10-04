//! 注册容量账本（v3.8.1）。
//!
//! # 定位
//!
//! 供给方在挂单之前，必须先**声明可提供的资源容量**：某资源形态 + 某计量维度下的
//! 总可售数量，以及一笔准入质押（`stake_micro`，本版本只登记、不冻结、不罚没；
//! 冻结/罚没在 v3.8.4 落地）。撮合在 `Matched → CapacityHeld` 之间从注册容量里
//! **预留（hold）**，取消/结算后 **释放（release）**，注销前必须释放全部预留。
//!
//! # 确定性与诚实边界
//!
//! - 纯内存、纯确定性账本：无浮点、无 syscall、无 unsafe、无 panic 路径；全部金额
//!   /数量为 `u128`，加减用 checked，越界/超卖具名拒绝。
//! - 守恒不变量：对每条注册恒有 `held <= capacity`，`available = capacity - held`。
//!   release 不得超过 held（防重复释放/记账篡改），hold 不得超过 available。
//! - 挂单入场约束：[`CapacityRegistry::check_offer`] 要求挂单供给方已注册同形态/同
//!   维度容量，且挂单量不超过当前可售量；未注册容量不得挂单（fail-closed）。
//! - 本账本**不持久化、不是全局单例**：它是宿主/T1 市场门面持有的内存记账面，
//!   内核只提供可单测的确定性规则。跨节点容量、P2P 供给发现不在本版本。

use serde::{Deserialize, Serialize};

use super::catalog::default_units;
use super::{MeterUnit, ResourceError, ResourceKind, ResourceOffer};

/// 一条容量注册：某供给方在某资源形态 + 计量维度下声明的可售容量。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityRegistration {
    /// 供给方 DID（非空，可追责主体）。
    pub provider_did: String,
    /// 资源形态。
    pub kind: ResourceKind,
    /// 计量单位（必须属于该形态目录）。
    pub unit: MeterUnit,
    /// 声明总容量（按 `unit`，必须为正）。
    pub capacity: u128,
    /// 已为撮合单预留的量（恒满足 `held <= capacity`）。
    pub held: u128,
    /// 准入质押额（micro）。v3.8.1 仅登记，冻结/罚没在 v3.8.4。
    pub stake_micro: u128,
}

impl CapacityRegistration {
    /// 构造一条 `held = 0` 的新注册，并做入场校验。
    pub fn new(
        provider_did: impl Into<String>,
        kind: ResourceKind,
        unit: MeterUnit,
        capacity: u128,
        stake_micro: u128,
    ) -> Result<Self, ResourceError> {
        let provider_did = provider_did.into();
        if provider_did.trim().is_empty() {
            return Err(ResourceError::EmptyProviderDid);
        }
        if capacity == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        if !default_units(kind).contains(&unit) {
            return Err(ResourceError::UnitNotInCatalog { kind, unit });
        }
        Ok(Self {
            provider_did,
            kind,
            unit,
            capacity,
            held: 0,
            stake_micro,
        })
    }

    /// 当前可售容量 = capacity - held（在不变量成立时恒可减）。
    pub fn available(&self) -> Result<u128, ResourceError> {
        self.capacity
            .checked_sub(self.held)
            .ok_or(ResourceError::ArithmeticOverflow)
    }
}

/// 容量注册表：以 `(provider_did, kind, unit)` 为唯一键的内存账本。
///
/// 用有序 `Vec` 而非 `HashMap`，保证遍历/序列化顺序确定，便于单测与回放；
/// 容量注册规模为每节点供给条目数，线性查找足够。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapacityRegistry {
    regs: Vec<CapacityRegistration>,
}

impl CapacityRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self { regs: Vec::new() }
    }

    /// 已注册条目数。
    pub fn len(&self) -> usize {
        self.regs.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.regs.is_empty()
    }

    fn index_of(&self, provider_did: &str, kind: ResourceKind, unit: MeterUnit) -> Option<usize> {
        self.regs
            .iter()
            .position(|r| r.provider_did == provider_did && r.kind == kind && r.unit == unit)
    }

    /// 只读获取某条注册。
    pub fn get(
        &self,
        provider_did: &str,
        kind: ResourceKind,
        unit: MeterUnit,
    ) -> Option<&CapacityRegistration> {
        self.index_of(provider_did, kind, unit)
            .map(|i| &self.regs[i])
    }

    /// 注册一条新容量：校验 + 唯一性，初始 `held = 0`。
    pub fn register(
        &mut self,
        provider_did: impl Into<String>,
        kind: ResourceKind,
        unit: MeterUnit,
        capacity: u128,
        stake_micro: u128,
    ) -> Result<(), ResourceError> {
        let reg = CapacityRegistration::new(provider_did, kind, unit, capacity, stake_micro)?;
        if self
            .index_of(&reg.provider_did, reg.kind, reg.unit)
            .is_some()
        {
            return Err(ResourceError::DuplicateRegistration {
                provider_did: reg.provider_did,
                kind: reg.kind,
                unit: reg.unit,
            });
        }
        self.regs.push(reg);
        Ok(())
    }

    /// 某 (供给方,形态,维度) 当前可售容量；未注册具名拒绝（区别于容量为 0）。
    pub fn available(
        &self,
        provider_did: &str,
        kind: ResourceKind,
        unit: MeterUnit,
    ) -> Result<u128, ResourceError> {
        self.get(provider_did, kind, unit)
            .ok_or(ResourceError::RegistrationNotFound)?
            .available()
    }

    /// 撮合预留：把 `amount` 容量从可售转为已持有；返回预留后的可售量。
    ///
    /// fail-closed：未注册 / 预留量为 0 / 超出当前可售，一律具名拒绝，不允许超卖。
    pub fn hold(
        &mut self,
        provider_did: &str,
        kind: ResourceKind,
        unit: MeterUnit,
        amount: u128,
    ) -> Result<u128, ResourceError> {
        if amount == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        let i = self
            .index_of(provider_did, kind, unit)
            .ok_or(ResourceError::RegistrationNotFound)?;
        let reg = &mut self.regs[i];
        let available = reg.available()?;
        let new_held = reg
            .held
            .checked_add(amount)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        if new_held > reg.capacity {
            return Err(ResourceError::CapacityExceeded {
                hold_requested: amount,
                available,
            });
        }
        reg.held = new_held;
        reg.available()
    }

    /// 取消/结算后释放预留：从 held 减回 `amount`；返回释放后的已持有量。
    ///
    /// 守恒：释放量必须为正且不得超过已持有，否则疑似重复释放/记账篡改，具名拒绝。
    pub fn release(
        &mut self,
        provider_did: &str,
        kind: ResourceKind,
        unit: MeterUnit,
        amount: u128,
    ) -> Result<u128, ResourceError> {
        if amount == 0 {
            return Err(ResourceError::NonPositiveQuantity);
        }
        let i = self
            .index_of(provider_did, kind, unit)
            .ok_or(ResourceError::RegistrationNotFound)?;
        let reg = &mut self.regs[i];
        if amount > reg.held {
            return Err(ResourceError::OverRelease {
                attempted: amount,
                held: reg.held,
            });
        }
        reg.held = reg
            .held
            .checked_sub(amount)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        Ok(reg.held)
    }

    /// 注销容量注册：仅当无任何预留（`held == 0`）时允许，防止带着未结订单撤供给。
    pub fn deregister(
        &mut self,
        provider_did: &str,
        kind: ResourceKind,
        unit: MeterUnit,
    ) -> Result<(), ResourceError> {
        let i = self
            .index_of(provider_did, kind, unit)
            .ok_or(ResourceError::RegistrationNotFound)?;
        let held = self.regs[i].held;
        if held > 0 {
            return Err(ResourceError::CapacityStillHeld { held });
        }
        self.regs.remove(i);
        Ok(())
    }

    /// 挂单入场校验（fail-closed）：供给方必须已注册同形态/同维度容量，且挂单量不
    /// 超过当前可售量。这是 `Drafted → Published` 挂单受理的容量闸门（3.8.3 撮合复用）。
    pub fn check_offer(&self, offer: &ResourceOffer) -> Result<(), ResourceError> {
        let reg = self
            .get(&offer.provider_did, offer.kind, offer.unit)
            .ok_or(ResourceError::RegistrationNotFound)?;
        let available = reg.available()?;
        if offer.quantity > available {
            return Err(ResourceError::OfferExceedsRegisteredCapacity {
                offer_quantity: offer.quantity,
                available,
            });
        }
        Ok(())
    }

    /// 遍历所有注册（快照顺序）。
    pub fn registrations(&self) -> &[CapacityRegistration] {
        &self.regs
    }

    /// 全表不变量：每条注册恒有 `held <= capacity`。供宿主在关键边界做 fail-closed
    /// 自检；正常路径下由 hold/release 保证，永不被违反。
    pub fn invariant_holds(&self) -> bool {
        self.regs.iter().all(|r| r.held <= r.capacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "did:nau:provider";

    fn offer(provider: &str, qty: u128) -> ResourceOffer {
        ResourceOffer::new(
            provider,
            ResourceKind::Compute,
            MeterUnit::CpuMillis,
            qty,
            2,
            None,
        )
        .unwrap()
    }

    #[test]
    fn register_valid_starts_full_available() {
        let mut reg = CapacityRegistry::new();
        assert!(reg.is_empty());
        reg.register(P, ResourceKind::Compute, MeterUnit::CpuMillis, 1000, 0)
            .unwrap();
        assert_eq!(reg.len(), 1);
        let entry = reg
            .get(P, ResourceKind::Compute, MeterUnit::CpuMillis)
            .unwrap();
        assert_eq!(entry.held, 0);
        assert_eq!(entry.capacity, 1000);
        assert_eq!(
            reg.available(P, ResourceKind::Compute, MeterUnit::CpuMillis)
                .unwrap(),
            1000
        );
        assert!(reg.invariant_holds());
    }

    #[test]
    fn register_rejects_empty_did_zero_capacity_and_foreign_unit() {
        let mut reg = CapacityRegistry::new();
        // 空/空白 DID 拒绝。
        assert!(matches!(
            reg.register("   ", ResourceKind::Compute, MeterUnit::CpuMillis, 10, 0),
            Err(ResourceError::EmptyProviderDid)
        ));
        // 零容量拒绝。
        assert!(matches!(
            reg.register(P, ResourceKind::Compute, MeterUnit::CpuMillis, 0, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        // 计量维度不属于该形态目录：Compute 不接受 StorageGbSec。
        assert!(matches!(
            reg.register(P, ResourceKind::Compute, MeterUnit::StorageGbSec, 10, 0),
            Err(ResourceError::UnitNotInCatalog { .. })
        ));
    }

    #[test]
    fn duplicate_registration_rejected_but_distinct_units_coexist() {
        let mut reg = CapacityRegistry::new();
        reg.register(P, ResourceKind::Compute, MeterUnit::CpuMillis, 1000, 0)
            .unwrap();
        // 同键重复注册拒绝。
        assert!(matches!(
            reg.register(P, ResourceKind::Compute, MeterUnit::CpuMillis, 2000, 0),
            Err(ResourceError::DuplicateRegistration { .. })
        ));
        // 同一形态不同维度是不同键，可共存。
        reg.register(P, ResourceKind::Compute, MeterUnit::GpuMillis, 500, 0)
            .unwrap();
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn hold_respects_capacity_and_reports_available() {
        let mut reg = CapacityRegistry::new();
        reg.register(P, ResourceKind::Compute, MeterUnit::CpuMillis, 1000, 0)
            .unwrap();
        // 预留 600 → 可售 400。
        assert_eq!(
            reg.hold(P, ResourceKind::Compute, MeterUnit::CpuMillis, 600)
                .unwrap(),
            400
        );
        // 再要 500 超出可售 400，拒绝（不超卖）。
        assert!(matches!(
            reg.hold(P, ResourceKind::Compute, MeterUnit::CpuMillis, 500),
            Err(ResourceError::CapacityExceeded {
                hold_requested: 500,
                available: 400
            })
        ));
        // 预留 0 非法；未注册具名拒绝。
        assert!(matches!(
            reg.hold(P, ResourceKind::Compute, MeterUnit::CpuMillis, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        assert!(matches!(
            reg.hold(
                "did:nau:other",
                ResourceKind::Compute,
                MeterUnit::CpuMillis,
                1
            ),
            Err(ResourceError::RegistrationNotFound)
        ));
        assert!(reg.invariant_holds());
    }

    #[test]
    fn release_conserves_held_and_blocks_over_release() {
        let mut reg = CapacityRegistry::new();
        reg.register(P, ResourceKind::Compute, MeterUnit::CpuMillis, 1000, 0)
            .unwrap();
        reg.hold(P, ResourceKind::Compute, MeterUnit::CpuMillis, 600)
            .unwrap();
        // 释放 200 → held 400，可售恢复到 600。
        assert_eq!(
            reg.release(P, ResourceKind::Compute, MeterUnit::CpuMillis, 200)
                .unwrap(),
            400
        );
        assert_eq!(
            reg.available(P, ResourceKind::Compute, MeterUnit::CpuMillis)
                .unwrap(),
            600
        );
        // 释放 500 > held 400，守恒拒绝。
        assert!(matches!(
            reg.release(P, ResourceKind::Compute, MeterUnit::CpuMillis, 500),
            Err(ResourceError::OverRelease {
                attempted: 500,
                held: 400
            })
        ));
        // 释放 0 非法。
        assert!(matches!(
            reg.release(P, ResourceKind::Compute, MeterUnit::CpuMillis, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        assert!(reg.invariant_holds());
    }

    #[test]
    fn deregister_blocked_while_held_then_allowed() {
        let mut reg = CapacityRegistry::new();
        reg.register(P, ResourceKind::Storage, MeterUnit::StorageGbSec, 800, 0)
            .unwrap();
        reg.hold(P, ResourceKind::Storage, MeterUnit::StorageGbSec, 300)
            .unwrap();
        // 有预留时禁止注销。
        assert!(matches!(
            reg.deregister(P, ResourceKind::Storage, MeterUnit::StorageGbSec),
            Err(ResourceError::CapacityStillHeld { held: 300 })
        ));
        // 全部释放后允许注销。
        reg.release(P, ResourceKind::Storage, MeterUnit::StorageGbSec, 300)
            .unwrap();
        reg.deregister(P, ResourceKind::Storage, MeterUnit::StorageGbSec)
            .unwrap();
        assert!(matches!(
            reg.deregister(P, ResourceKind::Storage, MeterUnit::StorageGbSec),
            Err(ResourceError::RegistrationNotFound)
        ));
        assert!(reg.is_empty());
    }

    #[test]
    fn offer_cannot_list_beyond_registered_available() {
        let mut reg = CapacityRegistry::new();
        // 未注册容量直接挂单：fail-closed 拒绝。
        assert!(matches!(
            reg.check_offer(&offer(P, 1000)),
            Err(ResourceError::RegistrationNotFound)
        ));
        reg.register(P, ResourceKind::Compute, MeterUnit::CpuMillis, 1000, 0)
            .unwrap();
        // 全部可售时挂 1000 合法。
        reg.check_offer(&offer(P, 1000)).unwrap();
        // 预留 600 后只剩 400，再挂 1000 超量拒绝。
        reg.hold(P, ResourceKind::Compute, MeterUnit::CpuMillis, 600)
            .unwrap();
        assert!(matches!(
            reg.check_offer(&offer(P, 1000)),
            Err(ResourceError::OfferExceedsRegisteredCapacity {
                offer_quantity: 1000,
                available: 400
            })
        ));
        // 挂 400 仍合法。
        reg.check_offer(&offer(P, 400)).unwrap();
        // 别的供给方未注册同样拒绝。
        assert!(matches!(
            reg.check_offer(&offer("did:nau:other", 1)),
            Err(ResourceError::RegistrationNotFound)
        ));
        assert!(reg.invariant_holds());
    }
}
