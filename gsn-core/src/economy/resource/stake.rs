//! 准入质押账本（v3.8.4）。
//!
//! # 定位
//!
//! 资源贡献者在注册可售容量的同时需质押一笔准入保证金。v3.8.1 的
//! [`super::CapacityRegistration::stake_micro`] 只**登记**申报额，不冻结、不罚没；
//! 本版本新增**独立**质押账本 [`StakeLedger`]，把质押的资金状态真正表达出来，但**不
//! 改动**容量账本（[`super::CapacityRegistry`]）的任何 hold/release 语义——容量是
//! 「资源量」，质押是「资金量」，两账分离、各自守恒。
//!
//! # 四个资金分桶与守恒
//!
//! 每个供给方账户（按 DID）的质押资金在任一时刻必落在四个互斥分桶之一：
//!
//! - `available`：已质押、可自由解冻后提取的保证金；
//! - `frozen`：因在挂单/在持订单而被**冻结**、承担担保责任的保证金；
//! - `slashed`：经仲裁**罚没**、累计离开该主体的资金（终局，不可回流）；
//! - `withdrawn`：正常**提取**离场、累计离开该主体的资金。
//!
//! 守恒恒等式（对每个账户恒成立）：
//! ```text
//! deposited == available + frozen + slashed + withdrawn
//! ```
//!
//! 其中 `deposited` 为累计净存入。冻结/解冻只在 `available ↔ frozen` 间搬运，
//! 不改变总额；罚没把 `frozen → slashed`，提取把 `available → withdrawn`。
//!
//! # 决策与资金分离（关键安全口径）
//!
//! - **只能罚没已冻结的保证金**：[`StakeLedger::slash`] 只能从 `frozen` 扣，绝不
//!   能直接划走 `available`。要惩罚一个主体，治理裁决必须先令其保证金处于冻结态，
//!   这把「谁违规」（决策，来自 v3.8.8 QA / Agent 审判）与「动多少钱」（本账记账）
//!   解耦，账本本身不做任何违规判定。
//! - fail-closed：任何越界（冻结超过可用、解冻/罚没超过冻结、提取超过可用、零额、
//!   未知账户、u128 溢出）一律具名拒绝，且**先在局部完成全部可能失败的记账、成功后
//!   才落账**，绝不在失败路径上留下半笔资金移动。
//!
//! # 确定性与诚实边界
//!
//! 纯内存、纯确定性：零浮点、零 syscall、零 unsafe、无 panic 路径；金额全 `u128`
//! micro，checked 运算。**不持久化、不是全局单例、不连链、不受理 PMB 外部写、不
//! 新增能力令牌**；它只是宿主市场门面持有的可单测记账面。本版本也**不把质押与撮合
//! 订单/容量联动**（何时按订单冻结、罚没多少由 3.8.5 托管结算与后续 QA/审判接线）。

use serde::{Deserialize, Serialize};

use super::ResourceError;

/// 单个供给方的准入质押账户：四个互斥资金分桶，金额单位 micro。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StakeAccount {
    /// 供给方 DID（非空、可追责主体）。
    pub provider_did: String,
    /// 累计净存入（= 四桶之和，守恒锚）。
    pub deposited_micro: u128,
    /// 可用保证金（已质押、未冻结）。
    pub available_micro: u128,
    /// 冻结保证金（承担担保责任，罚没只能从此桶出）。
    pub frozen_micro: u128,
    /// 累计罚没（终局，不可回流）。
    pub slashed_micro: u128,
    /// 累计正常提取离场。
    pub withdrawn_micro: u128,
}

impl StakeAccount {
    fn new(provider_did: impl Into<String>) -> Self {
        Self {
            provider_did: provider_did.into(),
            deposited_micro: 0,
            available_micro: 0,
            frozen_micro: 0,
            slashed_micro: 0,
            withdrawn_micro: 0,
        }
    }

    /// 可用保证金。
    pub fn available(&self) -> u128 {
        self.available_micro
    }

    /// 冻结保证金。
    pub fn frozen(&self) -> u128 {
        self.frozen_micro
    }

    /// 累计罚没。
    pub fn slashed(&self) -> u128 {
        self.slashed_micro
    }

    /// 累计提取。
    pub fn withdrawn(&self) -> u128 {
        self.withdrawn_micro
    }

    /// 累计净存入。
    pub fn deposited(&self) -> u128 {
        self.deposited_micro
    }

    /// 单账户守恒：deposited == available + frozen + slashed + withdrawn。
    pub fn conserves(&self) -> bool {
        match (
            self.available_micro.checked_add(self.frozen_micro),
            self.slashed_micro.checked_add(self.withdrawn_micro),
        ) {
            (Some(a), Some(b)) => match a.checked_add(b) {
                Some(total) => total == self.deposited_micro,
                None => false,
            },
            (None, _) | (_, None) => false,
        }
    }
}

/// 质押账本：以供给方 DID 为键的有序内存账户集（顺序确定，便于单测与回放）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StakeLedger {
    accounts: Vec<StakeAccount>,
}

impl StakeLedger {
    /// 空账本。
    pub fn new() -> Self {
        Self {
            accounts: Vec::new(),
        }
    }

    /// 账户数。
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    fn index_of(&self, provider_did: &str) -> Option<usize> {
        self.accounts
            .iter()
            .position(|a| a.provider_did == provider_did)
    }

    /// 只读获取某账户。
    pub fn get(&self, provider_did: &str) -> Option<&StakeAccount> {
        self.index_of(provider_did).map(|i| &self.accounts[i])
    }

    /// 遍历所有账户（插入顺序）。
    pub fn accounts(&self) -> &[StakeAccount] {
        &self.accounts
    }

    fn ensure_positive(amount: u128) -> Result<(), ResourceError> {
        if amount == 0 {
            Err(ResourceError::NonPositiveQuantity)
        } else {
            Ok(())
        }
    }

    fn ensure_did(did: &str) -> Result<(), ResourceError> {
        if did.trim().is_empty() {
            Err(ResourceError::EmptyProviderDid)
        } else {
            Ok(())
        }
    }

    /// 存入保证金：`deposited/available` 同增；账户不存在则开户。可多次累计。
    pub fn deposit(
        &mut self,
        provider_did: impl Into<String>,
        amount_micro: u128,
    ) -> Result<(), ResourceError> {
        let did = provider_did.into();
        Self::ensure_did(&did)?;
        Self::ensure_positive(amount_micro)?;
        let i = match self.index_of(&did) {
            Some(i) => i,
            None => {
                self.accounts.push(StakeAccount::new(did.clone()));
                self.accounts.len() - 1
            }
        };
        let acc = &mut self.accounts[i];
        // 先完成可能溢出的记账，成功后才落账。
        let new_deposited = acc
            .deposited_micro
            .checked_add(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let new_available = acc
            .available_micro
            .checked_add(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        acc.deposited_micro = new_deposited;
        acc.available_micro = new_available;
        Ok(())
    }

    /// 冻结保证金：`available → frozen`；可用不足 fail-closed，不动任何分桶。
    pub fn freeze(&mut self, provider_did: &str, amount_micro: u128) -> Result<(), ResourceError> {
        Self::ensure_did(provider_did)?;
        Self::ensure_positive(amount_micro)?;
        let i = self
            .index_of(provider_did)
            .ok_or(ResourceError::StakeAccountNotFound)?;
        let acc = &mut self.accounts[i];
        if amount_micro > acc.available_micro {
            return Err(ResourceError::InsufficientFreeStake {
                requested: amount_micro,
                available: acc.available_micro,
            });
        }
        let new_available = acc
            .available_micro
            .checked_sub(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let new_frozen = acc
            .frozen_micro
            .checked_add(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        acc.available_micro = new_available;
        acc.frozen_micro = new_frozen;
        Ok(())
    }

    /// 解冻保证金：`frozen → available`；解冻超过冻结额 fail-closed。
    pub fn unfreeze(
        &mut self,
        provider_did: &str,
        amount_micro: u128,
    ) -> Result<(), ResourceError> {
        Self::ensure_did(provider_did)?;
        Self::ensure_positive(amount_micro)?;
        let i = self
            .index_of(provider_did)
            .ok_or(ResourceError::StakeAccountNotFound)?;
        let acc = &mut self.accounts[i];
        if amount_micro > acc.frozen_micro {
            return Err(ResourceError::UnfreezeExceedsFrozen {
                attempted: amount_micro,
                frozen: acc.frozen_micro,
            });
        }
        let new_frozen = acc
            .frozen_micro
            .checked_sub(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let new_available = acc
            .available_micro
            .checked_add(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        acc.frozen_micro = new_frozen;
        acc.available_micro = new_available;
        Ok(())
    }

    /// 罚没保证金：`frozen → slashed`（终局不可回流）。
    ///
    /// **只能罚没已冻结部分**；冻结不足具名拒绝，绝不直接划走可用余额（决策/资金
    /// 分离）。失败不改写。
    pub fn slash(&mut self, provider_did: &str, amount_micro: u128) -> Result<(), ResourceError> {
        Self::ensure_did(provider_did)?;
        Self::ensure_positive(amount_micro)?;
        let i = self
            .index_of(provider_did)
            .ok_or(ResourceError::StakeAccountNotFound)?;
        let acc = &mut self.accounts[i];
        if amount_micro > acc.frozen_micro {
            return Err(ResourceError::SlashExceedsFrozen {
                attempted: amount_micro,
                frozen: acc.frozen_micro,
            });
        }
        let new_frozen = acc
            .frozen_micro
            .checked_sub(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let new_slashed = acc
            .slashed_micro
            .checked_add(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        acc.frozen_micro = new_frozen;
        acc.slashed_micro = new_slashed;
        Ok(())
    }

    /// 正常提取离场：`available → withdrawn`；只能提可用余额，冻结中不可提。
    pub fn withdraw(
        &mut self,
        provider_did: &str,
        amount_micro: u128,
    ) -> Result<(), ResourceError> {
        Self::ensure_did(provider_did)?;
        Self::ensure_positive(amount_micro)?;
        let i = self
            .index_of(provider_did)
            .ok_or(ResourceError::StakeAccountNotFound)?;
        let acc = &mut self.accounts[i];
        if amount_micro > acc.available_micro {
            return Err(ResourceError::InsufficientFreeStake {
                requested: amount_micro,
                available: acc.available_micro,
            });
        }
        let new_available = acc
            .available_micro
            .checked_sub(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        let new_withdrawn = acc
            .withdrawn_micro
            .checked_add(amount_micro)
            .ok_or(ResourceError::ArithmeticOverflow)?;
        acc.available_micro = new_available;
        acc.withdrawn_micro = new_withdrawn;
        Ok(())
    }

    /// 全表守恒：每个账户恒有 deposited == available + frozen + slashed + withdrawn。
    pub fn invariant_holds(&self) -> bool {
        self.accounts.iter().all(StakeAccount::conserves)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "did:nau:provider";

    #[test]
    fn deposit_freeze_unfreeze_settle_path_conserves() {
        let mut l = StakeLedger::new();
        l.deposit(P, 1_000).unwrap();
        l.deposit(P, 500).unwrap(); // 可多次累计
        let acc = l.get(P).unwrap();
        assert_eq!(acc.deposited(), 1_500);
        assert_eq!(acc.available(), 1_500);
        // 冻结 900。
        l.freeze(P, 900).unwrap();
        let acc = l.get(P).unwrap();
        assert_eq!(acc.available(), 600);
        assert_eq!(acc.frozen(), 900);
        assert_eq!(acc.deposited(), 1_500); // 总额不变
                                            // 解冻 200。
        l.unfreeze(P, 200).unwrap();
        let acc = l.get(P).unwrap();
        assert_eq!(acc.available(), 800);
        assert_eq!(acc.frozen(), 700);
        // 正常提取 300 可用。
        l.withdraw(P, 300).unwrap();
        let acc = l.get(P).unwrap();
        assert_eq!(acc.available(), 500);
        assert_eq!(acc.withdrawn(), 300);
        // 1500 == available500 + frozen700 + slashed0 + withdrawn300
        assert!(acc.conserves());
        assert!(l.invariant_holds());
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn freeze_failclosed_when_free_insufficient_leaves_buckets_untouched() {
        let mut l = StakeLedger::new();
        l.deposit(P, 100).unwrap();
        l.freeze(P, 60).unwrap(); // available40 frozen60
        let before = l.get(P).unwrap().clone();
        // 想再冻 50，但只剩 40 可用 → 拒绝。
        assert!(matches!(
            l.freeze(P, 50),
            Err(ResourceError::InsufficientFreeStake {
                requested: 50,
                available: 40
            })
        ));
        // 失败后分桶与失败前逐字节一致（无半笔移动）。
        assert_eq!(l.get(P).unwrap(), &before);
        assert!(l.invariant_holds());
    }

    #[test]
    fn unfreeze_cannot_exceed_frozen() {
        let mut l = StakeLedger::new();
        l.deposit(P, 100).unwrap();
        l.freeze(P, 30).unwrap();
        assert!(matches!(
            l.unfreeze(P, 31),
            Err(ResourceError::UnfreezeExceedsFrozen {
                attempted: 31,
                frozen: 30
            })
        ));
        // 恰好 30 可解冻。
        l.unfreeze(P, 30).unwrap();
        assert_eq!(l.get(P).unwrap().frozen(), 0);
        assert_eq!(l.get(P).unwrap().available(), 100);
        assert!(l.invariant_holds());
    }

    #[test]
    fn slash_only_consumes_frozen_and_is_terminal() {
        let mut l = StakeLedger::new();
        l.deposit(P, 1_000).unwrap();
        l.freeze(P, 400).unwrap();
        // 罚没 250：frozen 400→150，slashed 0→250。
        l.slash(P, 250).unwrap();
        let acc = l.get(P).unwrap();
        assert_eq!(acc.frozen(), 150);
        assert_eq!(acc.slashed(), 250);
        assert_eq!(acc.available(), 600);
        // slashed 不会因后续解冻/存入回流：解冻只影响 frozen/available。
        l.unfreeze(P, 150).unwrap();
        l.deposit(P, 100).unwrap();
        let acc = l.get(P).unwrap();
        assert_eq!(acc.slashed(), 250); // 终局累计不变
        assert_eq!(acc.available(), 850);
        assert_eq!(acc.deposited(), 1_100);
        // 1100 == 850 + 0 + 250 + 0
        assert!(acc.conserves());
        assert!(l.invariant_holds());
    }

    #[test]
    fn slash_cannot_touch_available_balance() {
        let mut l = StakeLedger::new();
        l.deposit(P, 1_000).unwrap();
        // 一分钱没冻 → 即使账上有 1000 可用，也不能罚没。
        assert!(matches!(
            l.slash(P, 1),
            Err(ResourceError::SlashExceedsFrozen {
                attempted: 1,
                frozen: 0
            })
        ));
        // 冻 100 后想罚 101 同样拒绝，可用余额不受影响。
        l.freeze(P, 100).unwrap();
        let before = l.get(P).unwrap().clone();
        assert!(matches!(
            l.slash(P, 101),
            Err(ResourceError::SlashExceedsFrozen {
                attempted: 101,
                frozen: 100
            })
        ));
        assert_eq!(l.get(P).unwrap(), &before);
        assert_eq!(l.get(P).unwrap().available(), 900);
        assert!(l.invariant_holds());
    }

    #[test]
    fn zero_amount_empty_and_unknown_account_rejected() {
        let mut l = StakeLedger::new();
        // 空/空白 DID：存也拒绝。
        assert!(matches!(
            l.deposit("  ", 10),
            Err(ResourceError::EmptyProviderDid)
        ));
        // 零额：各操作统一拒绝。
        l.deposit(P, 100).unwrap();
        assert!(matches!(
            l.deposit(P, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        assert!(matches!(
            l.freeze(P, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        assert!(matches!(
            l.unfreeze(P, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        assert!(matches!(
            l.slash(P, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        assert!(matches!(
            l.withdraw(P, 0),
            Err(ResourceError::NonPositiveQuantity)
        ));
        // 从未开户的主体：任何非存入操作具名 StakeAccountNotFound（区别于余额为 0）。
        for op in ["freeze", "unfreeze", "slash", "withdraw"] {
            let r = match op {
                "freeze" => l.freeze("did:nau:ghost", 1),
                "unfreeze" => l.unfreeze("did:nau:ghost", 1),
                "slash" => l.slash("did:nau:ghost", 1),
                _ => l.withdraw("did:nau:ghost", 1),
            };
            assert!(
                matches!(r, Err(ResourceError::StakeAccountNotFound)),
                "{op}"
            );
        }
        assert!(l.get("did:nau:ghost").is_none());
        assert!(l.invariant_holds());
    }

    #[test]
    fn multiple_providers_are_isolated_and_withdraw_respects_freeze() {
        let mut l = StakeLedger::new();
        l.deposit("did:nau:a", 1_000).unwrap();
        l.deposit("did:nau:b", 2_000).unwrap();
        l.freeze("did:nau:a", 400).unwrap();
        l.freeze("did:nau:b", 500).unwrap();
        // 两账户互不串账。
        let a = l.get("did:nau:a").unwrap();
        let b = l.get("did:nau:b").unwrap();
        assert_eq!((a.available(), a.frozen()), (600, 400));
        assert_eq!((b.available(), b.frozen()), (1_500, 500));
        // a 不能提取冻结的 400：提 700（>可用600）拒绝。
        assert!(matches!(
            l.withdraw("did:nau:a", 700),
            Err(ResourceError::InsufficientFreeStake {
                requested: 700,
                available: 600
            })
        ));
        // b 罚没全部冻结 500 后，可用 1500 完好。
        l.slash("did:nau:b", 500).unwrap();
        let b = l.get("did:nau:b").unwrap();
        assert_eq!((b.available(), b.frozen(), b.slashed()), (1_500, 0, 500));
        assert!(l.invariant_holds());
        // 全表守恒逐账户求和。
        let sum_deposited: u128 = l.accounts().iter().map(|a| a.deposited()).sum();
        assert_eq!(sum_deposited, 3_000);
    }
}
