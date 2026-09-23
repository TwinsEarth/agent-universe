//! 结算引擎 & 守恒账本（Contribution Ledger）
//!
//! 守恒不变量：
//! balance_sum = total_paid - total_slashed
//! total_paid ≤ total_budget

use serde::{Deserialize, Serialize};

/// 结算原因
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettlementReason {
    /// 正常完成，全额支付
    Completed,
    /// 已付过，付 0
    AlreadyPaid,
    /// 重复劳动，付 0
    DuplicateWork,
    /// 验收不通过，付 0
    Rejected,
}

/// 结算记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementRecord {
    pub task_id: String,
    pub payer: String,
    pub payee: String,
    pub amount: f64,
    pub reason: SettlementReason,
    pub timestamp: u64,
}

/// 守恒检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConservationReport {
    pub total_budget: f64,
    pub total_paid: f64,
    pub total_slashed: f64,
    pub balance_sum: f64,
    pub conserved: bool,
}

/// 结算引擎
#[derive(Debug, Clone)]
pub struct SettlementEngine {
    /// 账户余额
    balances: std::collections::HashMap<String, f64>,
    /// 结算记录
    records: Vec<SettlementRecord>,
    /// 已结算任务（防重复支付）
    settled_tasks: std::collections::HashSet<String>,
    /// 总预算
    total_budget: f64,
    /// 总罚没
    total_slashed: f64,
}

impl SettlementEngine {
    pub fn new() -> Self {
        Self {
            balances: std::collections::HashMap::new(),
            records: Vec::new(),
            settled_tasks: std::collections::HashSet::new(),
            total_budget: 0.0,
            total_slashed: 0.0,
        }
    }

    /// 充值
    pub fn deposit(&mut self, account: &str, amount: f64) {
        *self.balances.entry(account.to_string()).or_insert(0.0) += amount;
        self.total_budget += amount;
    }

    /// 获取余额
    pub fn balance(&self, account: &str) -> f64 {
        *self.balances.get(account).unwrap_or(&0.0)
    }

    /// 结算任务
    pub fn settle(
        &mut self,
        task_id: &str,
        payer: &str,
        payee: &str,
        amount: f64,
        reason: SettlementReason,
    ) -> Result<f64, String> {
        // 已付过 → 付 0
        if self.settled_tasks.contains(task_id) {
            self.records.push(SettlementRecord {
                task_id: task_id.to_string(),
                payer: payer.to_string(),
                payee: payee.to_string(),
                amount: 0.0,
                reason: SettlementReason::AlreadyPaid,
                timestamp: Self::now(),
            });
            return Ok(0.0);
        }

        let actual_amount = match reason {
            SettlementReason::Completed => amount,
            SettlementReason::AlreadyPaid
            | SettlementReason::DuplicateWork
            | SettlementReason::Rejected => 0.0,
        };

        if actual_amount > 0.0 {
            // 检查付款方余额
            let payer_balance = self.balance(payer);
            if payer_balance < actual_amount {
                return Err(format!(
                    "余额不足：{} 有 {}，需要 {}",
                    payer, payer_balance, actual_amount
                ));
            }

            // 扣款
            *self.balances.entry(payer.to_string()).or_insert(0.0) -= actual_amount;
            // 加款
            *self.balances.entry(payee.to_string()).or_insert(0.0) += actual_amount;
        }

        self.settled_tasks.insert(task_id.to_string());
        self.records.push(SettlementRecord {
            task_id: task_id.to_string(),
            payer: payer.to_string(),
            payee: payee.to_string(),
            amount: actual_amount,
            reason,
            timestamp: Self::now(),
        });

        Ok(actual_amount)
    }

    /// 罚没
    pub fn slash(&mut self, account: &str, amount: f64) -> Result<f64, String> {
        let balance = self.balance(account);
        if balance < amount {
            return Err(format!(
                "罚没失败：{} 余额 {}，需罚没 {}",
                account, balance, amount
            ));
        }
        *self.balances.entry(account.to_string()).or_insert(0.0) -= amount;
        self.total_slashed += amount;
        Ok(amount)
    }

    /// 守恒检查
    ///
    /// 资金守恒：
    /// - 充值增加系统总余额
    /// - 支付是账户间转账，系统总余额不变
    /// - 罚没将资金移出系统，总余额减少
    ///
    /// 不变量：balance_sum = total_budget - total_slashed
    /// 约束：total_paid ≤ total_budget
    pub fn conservation_check(&self) -> ConservationReport {
        let total_paid: f64 = self
            .records
            .iter()
            .filter(|r| r.reason == SettlementReason::Completed)
            .map(|r| r.amount)
            .sum();

        let balance_sum: f64 = self.balances.values().sum();

        // 守恒：当前所有余额 = 总预算 - 总罚没
        let expected_sum = self.total_budget - self.total_slashed;
        let conserved = (balance_sum - expected_sum).abs() < 0.001
            && total_paid <= self.total_budget + 0.001;

        ConservationReport {
            total_budget: self.total_budget,
            total_paid,
            total_slashed: self.total_slashed,
            balance_sum,
            conserved,
        }
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn total_paid(&self) -> f64 {
        self.records
            .iter()
            .filter(|r| r.reason == SettlementReason::Completed)
            .map(|r| r.amount)
            .sum()
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl Default for SettlementEngine {
    fn default() -> Self {
        Self::new()
    }
}
