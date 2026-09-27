//! 结算引擎（Settlement Engine，精确整数版 v2.5.8）
//!
//! 资金模型（对齐 JS、杜绝铸币）：
//! - 普通余额账户：`{did}`
//! - 质押锁定账户：`__stake__:{did}`（注册时从自有余额转入，可被罚没）
//! - 任务托管账户：`__escrow__:{task_id}`（发布任务时从需求方余额锁定）
//!
//! 守恒（精确相等，无容差）：
//!   Σ 所有账户余额 == 累计充值(total_deposits) − 累计罚没(total_slashed)
//!
//! 资金只有两个改变总规模的入口/出口：
//! - deposit：充值（唯一资金入口）
//! - slash：罚没（资金退出系统）
//! transfer / 托管 / 结算都只是账户间搬运，不改变总余额。

use crate::marketplace::money::Money;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// 结算原因
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementReason {
    Completed,
    Rejected,
    Slashed,
    Refunded,
}

/// 结算记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementRecord {
    pub task_id: String,
    pub from_account: String,
    pub to_account: String,
    pub amount: Money,
    pub reason: SettlementReason,
    pub timestamp: u64,
}

/// 守恒报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConservationReport {
    pub conserved: bool,
    pub total_deposits: Money,
    pub total_paid: Money,
    pub total_slashed: Money,
    pub balance_sum: Money,
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 结算引擎
pub struct SettlementEngine {
    balances: HashMap<String, Money>,
    records: Vec<SettlementRecord>,
    paid_tasks: HashSet<String>,
    total_deposits: Money,
    total_paid: Money,
    total_slashed: Money,
}

impl SettlementEngine {
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
            records: Vec::new(),
            paid_tasks: HashSet::new(),
            total_deposits: Money::ZERO,
            total_paid: Money::ZERO,
            total_slashed: Money::ZERO,
        }
    }

    /// 充值：系统**唯一**的资金入口
    pub fn deposit(&mut self, account: &str, amount: Money) -> Result<(), String> {
        if amount.is_negative() {
            return Err("充值金额不能为负".to_string());
        }
        let bal = self.balance(account);
        self.balances
            .insert(account.to_string(), bal.checked_add(amount)?);
        self.total_deposits = self.total_deposits.checked_add(amount)?;
        Ok(())
    }

    /// 查询余额（不存在即为 0）
    pub fn balance(&self, account: &str) -> Money {
        self.balances.get(account).copied().unwrap_or(Money::ZERO)
    }

    /// 内部转账：要求 from 有足额资金，不改变总余额
    pub fn transfer(&mut self, from: &str, to: &str, amount: Money) -> Result<(), String> {
        if !amount.is_positive() {
            return Err("转账金额必须为正".to_string());
        }
        let fb = self.balance(from);
        if fb < amount {
            return Err(format!(
                "账户 {} 余额不足：有 {}，需 {}",
                from, fb, amount
            ));
        }
        let tb = self.balance(to);
        self.balances
            .insert(from.to_string(), fb.checked_sub(amount)?);
        self.balances
            .insert(to.to_string(), tb.checked_add(amount)?);
        Ok(())
    }

    /// 结算（带防重复）。payer 通常是托管账户 `__escrow__:{task_id}`
    pub fn settle(
        &mut self,
        task_id: &str,
        payer: &str,
        payee: &str,
        amount: Money,
        reason: SettlementReason,
    ) -> Result<Money, String> {
        if self.paid_tasks.contains(task_id) {
            return Ok(Money::ZERO);
        }

        match reason {
            SettlementReason::Rejected => {
                self.records.push(SettlementRecord {
                    task_id: task_id.to_string(),
                    from_account: payer.to_string(),
                    to_account: payee.to_string(),
                    amount: Money::ZERO,
                    reason,
                    timestamp: now_ts(),
                });
                self.paid_tasks.insert(task_id.to_string());
                Ok(Money::ZERO)
            }
            SettlementReason::Completed => {
                // payer（托管账户）必须有足额；发布即托管，故必有
                self.transfer(payer, payee, amount)?;
                self.total_paid = self.total_paid.checked_add(amount)?;
                self.records.push(SettlementRecord {
                    task_id: task_id.to_string(),
                    from_account: payer.to_string(),
                    to_account: payee.to_string(),
                    amount,
                    reason,
                    timestamp: now_ts(),
                });
                self.paid_tasks.insert(task_id.to_string());
                Ok(amount)
            }
            other => Err(format!("settle 不支持原因 {:?}", other)),
        }
    }

    /// 罚没：资金从指定账户**退出系统**
    pub fn slash(&mut self, account: &str, amount: Money) -> Result<Money, String> {
        if !amount.is_positive() {
            return Err("罚没金额必须为正".to_string());
        }
        let bal = self.balance(account);
        if bal < amount {
            return Err(format!(
                "账户 {} 余额不足，无法罚没 {}（有 {}）",
                account, amount, bal
            ));
        }
        self.balances
            .insert(account.to_string(), bal.checked_sub(amount)?);
        self.total_slashed = self.total_slashed.checked_add(amount)?;
        self.records.push(SettlementRecord {
            task_id: format!("slash:{}", account),
            from_account: account.to_string(),
            to_account: String::new(),
            amount,
            reason: SettlementReason::Slashed,
            timestamp: now_ts(),
        });
        Ok(amount)
    }

    /// 退款：托管/锁定账户 → 原账户（内部转账并留痕）
    pub fn refund(
        &mut self,
        task_id: &str,
        escrow: &str,
        to: &str,
        amount: Money,
    ) -> Result<(), String> {
        if amount.is_positive() {
            self.transfer(escrow, to, amount)?;
            self.records.push(SettlementRecord {
                task_id: task_id.to_string(),
                from_account: escrow.to_string(),
                to_account: to.to_string(),
                amount,
                reason: SettlementReason::Refunded,
                timestamp: now_ts(),
            });
        }
        Ok(())
    }

    /// 全量扫描：重新对所有账户求和，与「充值−罚没」精确比对
    pub fn conservation_check(&self) -> ConservationReport {
        let mut sum = Money::ZERO;
        for b in self.balances.values() {
            sum = match sum.checked_add(*b) {
                Ok(v) => v,
                Err(_) => return self.report_with(sum, false),
            };
        }
        let expected = self
            .total_deposits
            .checked_sub(self.total_slashed)
            .unwrap_or(Money::ZERO);
        let conserved = sum == expected;
        self.report_with(sum, conserved)
    }

    fn report_with(&self, balance_sum: Money, conserved: bool) -> ConservationReport {
        ConservationReport {
            conserved,
            total_deposits: self.total_deposits,
            total_paid: self.total_paid,
            total_slashed: self.total_slashed,
            balance_sum,
        }
    }

    /// 独立全量审计（与 conservation_check 同算法；v2.5.9 提升为独立审计器）
    pub fn audit_full_scan(&self) -> ConservationReport {
        self.conservation_check()
    }

    pub fn records(&self) -> &[SettlementRecord] {
        &self.records
    }
}

impl Default for SettlementEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_and_exact_balance() {
        let mut e = SettlementEngine::new();
        e.deposit("u1", Money::new(100)).unwrap();
        assert_eq!(e.balance("u1"), Money::new(100));
        assert!(e.conservation_check().conserved);
    }

    #[test]
    fn negative_deposit_rejected() {
        let mut e = SettlementEngine::new();
        assert!(e.deposit("u1", Money::new(-1)).is_err());
    }

    #[test]
    fn transfer_requires_funds_no_minting() {
        let mut e = SettlementEngine::new();
        // 未充值即转账 → 失败（系统不铸币）
        assert!(e.transfer("a", "b", Money::new(10)).is_err());
        e.deposit("a", Money::new(10)).unwrap();
        e.transfer("a", "b", Money::new(10)).unwrap();
        assert_eq!(e.balance("b"), Money::new(10));
        assert_eq!(e.balance("a"), Money::ZERO);
        assert!(e.conservation_check().conserved);
    }

    #[test]
    fn slash_removes_value_from_system() {
        let mut e = SettlementEngine::new();
        e.deposit("a", Money::new(100)).unwrap();
        e.slash("a", Money::new(30)).unwrap();
        assert_eq!(e.balance("a"), Money::new(70));
        let r = e.conservation_check();
        assert!(r.conserved);
        assert_eq!(r.total_slashed, Money::new(30));
    }

    #[test]
    fn conservation_detects_mismatch() {
        let mut e = SettlementEngine::new();
        e.deposit("a", Money::new(100)).unwrap();
        // 手工制造账实不符
        e.balances.insert("ghost".to_string(), Money::new(5));
        assert!(!e.conservation_check().conserved);
    }
}
