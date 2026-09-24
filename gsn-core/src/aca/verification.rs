//! 五级验证分层
//!
//! L0 抽样 → L1 冗余 2-of-3 → L2 TEE → L3 zkML → L4 委员会仲裁

use serde::{Deserialize, Serialize};

/// 验证级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationLevel {
    /// L0：自报 + 5-10% 抽样
    L0Sample,
    /// L1：冗余 2-of-3
    L1Redundant,
    /// L2：TEE 可信执行
    L2TEE,
    /// L3：zkML + 乐观挑战
    L3ZkML,
    /// L4：委员会仲裁 + 质押罚没
    L4Committee,
}

impl VerificationLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            VerificationLevel::L0Sample => "L0",
            VerificationLevel::L1Redundant => "L1",
            VerificationLevel::L2TEE => "L2",
            VerificationLevel::L3ZkML => "L3",
            VerificationLevel::L4Committee => "L4",
        }
    }

    /// 相对执行成本倍率
    pub fn cost_multiplier(&self) -> f64 {
        match self {
            VerificationLevel::L0Sample => 0.1,
            VerificationLevel::L1Redundant => 2.0,
            VerificationLevel::L2TEE => 1.15,
            VerificationLevel::L3ZkML => 500.0,
            VerificationLevel::L4Committee => 100.0,
        }
    }

    /// 最终性（秒，越大越慢）
    pub fn finality_seconds(&self) -> u64 {
        match self {
            VerificationLevel::L0Sample => 1,
            VerificationLevel::L1Redundant => 120,
            VerificationLevel::L2TEE => 5,
            VerificationLevel::L3ZkML => 3600,
            VerificationLevel::L4Committee => 604800,
        }
    }

    /// 适用场景描述
    pub fn description(&self) -> &'static str {
        match self {
            VerificationLevel::L0Sample => "翻译、摘要、标注",
            VerificationLevel::L1Redundant => "代码审查、数据分析",
            VerificationLevel::L2TEE => "医疗、金融、隐私数据",
            VerificationLevel::L3ZkML => "高价值、可形式化验证",
            VerificationLevel::L4Committee => "争议任务、协议升级",
        }
    }
}

/// 验证策略
#[derive(Debug, Clone)]
pub struct VerificationPolicy {
    /// 任务价值（GSP）
    pub task_value: u64,
    /// 欺诈概率估计（0.0-1.0）
    pub fraud_probability: f64,
    /// 已有的冗余验证者数
    pub verifier_count: u32,
}

impl VerificationPolicy {
    pub fn new(task_value: u64, fraud_probability: f64) -> Self {
        Self {
            task_value,
            fraud_probability,
            verifier_count: 0,
        }
    }

    /// 经济决策：选择最优验证级别
    /// C_verify(level) <= V_task * P_fraud
    pub fn choose_level(&self) -> VerificationLevel {
        let expected_loss = self.task_value as f64 * self.fraud_probability;

        // 按成本从低到高选择第一个可接受的
        // L0: 成本 = 0.1 * base
        // L1: 成本 = 2.0 * base
        // L2: 成本 = 1.15 * base
        // L3: 成本 = 500 * base
        // L4: 成本 = 100 * base

        if expected_loss < 1.0 {
            VerificationLevel::L0Sample
        } else if expected_loss < 10.0 {
            VerificationLevel::L1Redundant
        } else if expected_loss < 100.0 {
            VerificationLevel::L2TEE
        } else if expected_loss < 1000.0 {
            VerificationLevel::L3ZkML
        } else {
            VerificationLevel::L4Committee
        }
    }

    /// 所需最少验证者数
    pub fn required_verifiers(&self, level: VerificationLevel) -> u32 {
        match level {
            VerificationLevel::L0Sample => 1,
            VerificationLevel::L1Redundant => 3, // 2-of-3
            VerificationLevel::L2TEE => 1,
            VerificationLevel::L3ZkML => 1,
            VerificationLevel::L4Committee => 7, // 委员会
        }
    }
}

/// 验证结果
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub level: VerificationLevel,
    pub passed: bool,
    pub verifiers: u32,
    pub verifications_needed: u32,
    pub cost: u64,
    pub duration_secs: u64,
    pub notes: String,
}

impl VerificationResult {
    pub fn pass(level: VerificationLevel, verifiers: u32, cost: u64) -> Self {
        Self {
            level,
            passed: true,
            verifiers,
            verifications_needed: verifiers,
            cost,
            duration_secs: level.finality_seconds(),
            notes: String::new(),
        }
    }

    pub fn fail(level: VerificationLevel, verifiers: u32, notes: String) -> Self {
        Self {
            level,
            passed: false,
            verifiers,
            verifications_needed: verifiers,
            cost: 0,
            duration_secs: level.finality_seconds(),
            notes,
        }
    }
}
