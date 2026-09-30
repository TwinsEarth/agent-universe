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
//!
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
    /// 重复劳动（返工后提交与上次完全相同的结果），付 0
    DuplicateWork,
    Slashed,
    Refunded,
    /// 充值留痕（v2.5.9：让唯一资金入口也进入只追加流水，独立审计可从流水完整重放）
    Deposited,
    /// 质押锁定（did → __stake__:did）
    Staked,
    /// 任务托管锁定（requester → __escrow__:task）
    Escrowed,
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

/// 单账户账实不符
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountMismatch {
    pub account: String,
    /// 从流水独立重放得到的应有余额
    pub expected: Money,
    /// 引擎当前余额
    pub actual: Money,
}

/// 独立审计报告（v2.5.9）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    /// 审计是否通过
    pub passed: bool,
    /// 重放的流水条数
    pub replayed_records: usize,
    /// 从流水独立重放的累计充值
    pub replayed_deposits: Money,
    /// 从流水独立重放的累计罚没
    pub replayed_slashed: Money,
    /// 重放得到的系统应有总额（充值−罚没）
    pub expected_total: Money,
    /// 当前所有账户余额求和
    pub actual_total: Money,
    /// 重放聚合是否与引擎自维护的 total_deposits / total_slashed 一致
    pub aggregate_matches: bool,
    /// 逐账户账实不符明细
    pub mismatches: Vec<AccountMismatch>,
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 从只追加流水重放得到的账本（v2.6.1：审计与持久化恢复共用）
#[derive(Debug)]
pub struct ReplayedLedger {
    pub balances: HashMap<String, Money>,
    pub deposits: Money,
    pub slashed: Money,
}

/// 只信任流水，逐笔有符号增量重放出账户余额与充值 / 罚没聚合。
/// checked 溢出返回 Err（独立于任何引擎自维护状态）。
pub fn replay_records(records: &[SettlementRecord]) -> Result<ReplayedLedger, String> {
    let mut balances: HashMap<String, Money> = HashMap::new();
    let mut deposits = Money::ZERO;
    let mut slashed = Money::ZERO;

    let mut apply = |acct: &str, delta: Money| -> Result<(), String> {
        let cur = balances.get(acct).copied().unwrap_or(Money::ZERO);
        let v = cur
            .checked_add(delta)
            .map_err(|_| "重放时账户金额溢出".to_string())?;
        balances.insert(acct.to_string(), v);
        Ok(())
    };

    for r in records {
        match r.reason {
            SettlementReason::Deposited => {
                apply(&r.to_account, r.amount)?;
                deposits = deposits
                    .checked_add(r.amount)
                    .map_err(|_| "重放时充值聚合溢出".to_string())?;
            }
            SettlementReason::Slashed => {
                apply(&r.from_account, Money::new(-r.amount.as_i64()))?;
                slashed = slashed
                    .checked_add(r.amount)
                    .map_err(|_| "重放时罚没聚合溢出".to_string())?;
            }
            _ => {
                // Completed / Refunded / Staked / Escrowed：from → to 搬运
                if r.amount.is_positive() {
                    apply(&r.from_account, Money::new(-r.amount.as_i64()))?;
                    apply(&r.to_account, r.amount)?;
                }
            }
        }
    }

    Ok(ReplayedLedger {
        balances,
        deposits,
        slashed,
    })
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

    /// 从持久化的只追加流水恢复引擎（v2.6.1，GAP §6.1）。
    ///
    /// 余额与充值 / 罚没聚合由 `replay_records` 独立重放，已结算任务集合与
    /// `total_paid` 从 Completed 流水重建。恢复出的引擎再跑 `independent_audit`
    /// 必须通过——保证账本落盘后重启不铸币、不丢账。
    pub fn restore(records: Vec<SettlementRecord>) -> Result<Self, String> {
        let replayed = replay_records(&records)?;
        let mut paid_tasks = HashSet::new();
        let mut total_paid = Money::ZERO;
        for r in &records {
            if r.reason == SettlementReason::Completed && r.amount.is_positive() {
                paid_tasks.insert(r.task_id.clone());
                total_paid = total_paid
                    .checked_add(r.amount)
                    .map_err(|_| "恢复时已付总额溢出".to_string())?;
            }
        }
        Ok(Self {
            balances: replayed.balances,
            records,
            paid_tasks,
            total_deposits: replayed.deposits,
            total_paid,
            total_slashed: replayed.slashed,
        })
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
        // v2.5.9：充值也进入只追加流水（from 系统外 "" → account）
        self.records.push(SettlementRecord {
            task_id: format!("deposit:{}", account),
            from_account: String::new(),
            to_account: account.to_string(),
            amount,
            reason: SettlementReason::Deposited,
            timestamp: now_ts(),
        });
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
            return Err(format!("账户 {} 余额不足：有 {}，需 {}", from, fb, amount));
        }
        let tb = self.balance(to);
        self.balances
            .insert(from.to_string(), fb.checked_sub(amount)?);
        self.balances
            .insert(to.to_string(), tb.checked_add(amount)?);
        Ok(())
    }

    /// 锁定（内部转账并留痕）：用于质押 `Staked` / 托管 `Escrowed`。
    /// 与 `transfer` 的区别是会进入只追加流水，独立审计可从流水重放。
    pub fn lock(
        &mut self,
        task_id: &str,
        from: &str,
        to: &str,
        amount: Money,
        reason: SettlementReason,
    ) -> Result<(), String> {
        self.transfer(from, to, amount)?;
        self.records.push(SettlementRecord {
            task_id: task_id.to_string(),
            from_account: from.to_string(),
            to_account: to.to_string(),
            amount,
            reason,
            timestamp: now_ts(),
        });
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
            SettlementReason::Rejected | SettlementReason::DuplicateWork => {
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

    /// 独立审计（v2.5.9，GAP §2.4）
    ///
    /// 与 `conservation_check` 的关键区别：**不信任**引擎自维护的 `balances`
    /// 与 `total_deposits / total_slashed` 聚合，而是只信任只追加、不可变的
    /// `records` 流水，逐笔独立重放出每个账户的应有余额，再与当前 `balances`
    /// 逐账户比对。这样即使增量聚合或余额被污染/算错，独立审计仍能发现。
    pub fn independent_audit(&self) -> AuditReport {
        let mut overflow = false;
        // 只信任流水独立重放（v2.6.1：与持久化恢复共用 replay_records）
        let (expected, deposits, slashed) = match replay_records(&self.records) {
            Ok(l) => (l.balances, l.deposits, l.slashed),
            Err(_) => {
                overflow = true;
                (HashMap::new(), Money::ZERO, Money::ZERO)
            }
        };

        // 逐账户比对（重放账户 ∪ 当前账户，覆盖 ghost / 缺失 / 篡改）
        let mut accounts: HashSet<String> = HashSet::new();
        for k in expected.keys() {
            accounts.insert(k.clone());
        }
        for k in self.balances.keys() {
            accounts.insert(k.clone());
        }
        let mut mismatches = Vec::new();
        for a in accounts {
            let exp = expected.get(&a).copied().unwrap_or(Money::ZERO);
            let act = self.balances.get(&a).copied().unwrap_or(Money::ZERO);
            if exp != act {
                mismatches.push(AccountMismatch {
                    account: a,
                    expected: exp,
                    actual: act,
                });
            }
        }
        mismatches.sort_by(|x, y| x.account.cmp(&y.account));

        let expected_total = match deposits.checked_sub(slashed) {
            Ok(v) => v,
            Err(_) => {
                overflow = true;
                Money::ZERO
            }
        };
        let mut actual_total = Money::ZERO;
        for b in self.balances.values() {
            match actual_total.checked_add(*b) {
                Ok(v) => actual_total = v,
                Err(_) => overflow = true,
            }
        }
        let aggregate_matches = deposits == self.total_deposits && slashed == self.total_slashed;
        let passed = !overflow
            && mismatches.is_empty()
            && expected_total == actual_total
            && aggregate_matches;

        AuditReport {
            passed,
            replayed_records: self.records.len(),
            replayed_deposits: deposits,
            replayed_slashed: slashed,
            expected_total,
            actual_total,
            aggregate_matches,
            mismatches,
        }
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

    #[test]
    fn independent_audit_passes_full_lifecycle() {
        let mut e = SettlementEngine::new();
        e.deposit("a", Money::new(200)).unwrap();
        // 质押锁定、托管锁定（均带流水）
        e.lock(
            "a",
            "a",
            "__stake__:a",
            Money::new(100),
            SettlementReason::Staked,
        )
        .unwrap();
        e.lock(
            "t1",
            "a",
            "__escrow__:t1",
            Money::new(50),
            SettlementReason::Escrowed,
        )
        .unwrap();
        let r = e.independent_audit();
        assert!(r.passed, "干净全流程应通过审计: {:?}", r.mismatches);
        assert_eq!(r.replayed_deposits, Money::new(200));
        assert_eq!(r.actual_total, Money::new(200));
    }

    #[test]
    fn independent_audit_catches_conservation_invisible_tampering() {
        let mut e = SettlementEngine::new();
        e.deposit("a", Money::new(100)).unwrap();
        // 守恒但账实不符：把 a 的钱拆给 b，流水里却没有这次搬运
        e.balances.insert("a".to_string(), Money::new(50));
        e.balances.insert("b".to_string(), Money::new(50));
        // 守恒检查被蒙蔽（总和仍为 100）
        assert!(e.conservation_check().conserved);
        // 独立审计必须失败，并给出两户明细
        let r = e.independent_audit();
        assert!(!r.passed);
        assert_eq!(r.mismatches.len(), 2);
        assert!(r.mismatches.iter().any(|m| {
            m.account == "a" && m.expected == Money::new(100) && m.actual == Money::new(50)
        }));
        assert!(r.mismatches.iter().any(|m| {
            m.account == "b" && m.expected == Money::ZERO && m.actual == Money::new(50)
        }));
    }

    #[test]
    fn independent_audit_detects_ghost_account() {
        let mut e = SettlementEngine::new();
        e.deposit("a", Money::new(100)).unwrap();
        e.balances.insert("ghost".to_string(), Money::new(10));
        assert!(!e.independent_audit().passed);
    }
}
