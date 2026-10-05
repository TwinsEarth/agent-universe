//! v3.9.5 ERC-4337（账户抽象）**Paymaster 纯确定性赞助决策内核**。
//!
//! # 只决策是否代付 gas，不广播、不代付、不持钥
//!
//! ERC-4337 让一个没有 ETH 余额的智能账户也能发起 UserOperation：由链上 Paymaster
//! 在 `validatePaymasterUserOp` 阶段为其作保，EntryPoint 从 Paymaster 存款里扣 gas。
//! 本模块只实现**与环境无关、可复算的那一半**：
//!
//! - UserOperation 气体/费用字段的结构校验（整数 raw wei，零浮点）；
//! - v0.6 拼接式 `paymasterAndData`（20 字节地址 + data）与 v0.7 分离式 `paymaster` / `paymasterData` 的纯解析、对齐与一致性核对；
//! - 按 `(preVerificationGas + verificationGasLimit + callGasLimit + paymasterVerificationGasLimit + paymasterPostOpGasLimit) * maxFeePerGas` 做**整数、checked** 的最大 gas 成本估算（只上估，不下估）；
//! - 在一份显式 [`SponsorPolicy`]（开关 / 链 / 时间窗 / 白名单 / 单笔上限 / 累计预算）下给出**赞助或具名拒绝**的确定性决策，预算守恒。
//!
//! 它**不**：
//!
//! - 连接 bundler / EntryPoint / 任意 EVM RPC，不提交、不广播、不聚合 UserOperation；
//! - 持有/生成 secp256k1 或 Paymaster 签名私钥，不产出 `validatePaymasterUserOp` 签名——任何签发请求一律 [`PaymasterError::PaymasterSignerNotConfigured`] 具名 fail-closed，绝不伪造；
//! - 真的垫付/划转 gas、不在链上质押/充值 Paymaster 存款；
//! - 读系统时钟——时间窗判定的 `now_unix` 由调用方显式传入（保持纯函数可测）；
//! - 校验 EntryPoint 合约地址、链上 nonce 或存款余额（无 RPC，调用方取证后自行保证）。
//!
//! 金额一律链上最小单位 **wei**（整数），白名单匹配按 20 字节地址常量时间比较；
//! 零 syscall、零 unsafe、零 IO，所有累加走 checked，溢出具名拒绝、无 panic 路径。

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::x402::EvmAddress;

// ───────────────────────────────────────────────────────────────────────────
// hex 工具（裸字节解析，供 v0.6 paymasterAndData / v0.7 paymasterData 使用）
// ───────────────────────────────────────────────────────────────────────────

/// 剥 `0x/0X` 后按偶数长度 ASCII hex 解码为字节；非法字符或奇数长度返回 None。
pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let h = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
    if h.len() % 2 != 0 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = Vec::with_capacity(h.len() / 2);
    let bytes = h.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = u8::from_str_radix(std::str::from_utf8(&bytes[i..i + 2]).ok()?, 16).ok()?;
        out.push(hi);
        i += 2;
    }
    Some(out)
}

/// 解析 v0.6 拼接式 `paymasterAndData`：前 20 字节是 paymaster 地址，其余是不透明 data。
///
/// data 可以为空（恰好 20 字节）；少于 20 字节或 hex 非法具名拒绝。
pub fn parse_paymaster_and_data(hex: &str) -> Result<(EvmAddress, Vec<u8>), PaymasterError> {
    let bytes = hex_decode(hex).ok_or(PaymasterError::BadPaymasterDataHex)?;
    if bytes.len() < 20 {
        return Err(PaymasterError::PaymasterDataTooShort {
            bytes_len: bytes.len(),
        });
    }
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes[..20]);
    Ok((EvmAddress(addr), bytes[20..].to_vec()))
}

// ───────────────────────────────────────────────────────────────────────────
// 领域错误
// ───────────────────────────────────────────────────────────────────────────

/// Paymaster 决策内核的类型化拒绝（不静默降级、不代付、不伪造签名）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymasterError {
    /// 赞助策略显式关闭。
    PolicyDisabled,
    /// UserOperation 的 sender 是零地址。
    ZeroSender,
    /// 策略 paymaster 是零地址（策略本身非法）。
    ZeroPaymaster,
    /// UserOperation 链 id 与策略链 id 不一致（禁止跨链作保）。
    ChainMismatch { op_chain: u64, policy_chain: u64 },
    /// maxFeePerGas 必须为正。
    ZeroMaxFeePerGas,
    /// maxPriorityFeePerGas 必须为正。
    ZeroMaxPriorityFeePerGas,
    /// maxPriorityFeePerGas 不得大于 maxFeePerGas。
    PriorityFeeExceedsMax { priority_wei: u128, max_wei: u128 },
    /// preVerificationGas 必须为正。
    ZeroPreVerificationGas,
    /// verificationGasLimit 必须为正。
    ZeroVerificationGas,
    /// paymasterAndData / paymasterData 不是合法偶数长度 hex。
    BadPaymasterDataHex,
    /// v0.6 paymasterAndData 不足 20 字节（装不下地址）。
    PaymasterDataTooShort { bytes_len: usize },
    /// 声明了 paymasterData 却没有 paymaster 地址，或两种形态都没提供 paymaster。
    NoPaymasterProvided,
    /// 生效 paymaster（v0.6/v0.7 解析或与策略比对）与策略指定的不一致。
    PaymasterMismatch { effective: String, policy: String },
    /// 策略限额非法：单笔上限或累计预算为 0。
    PolicyLimitsInvalid,
    /// 策略时间窗自身非法（valid_after >= valid_until）。
    InvalidPolicyWindow { valid_after: u64, valid_until: u64 },
    /// 当前时间早于策略生效时间。
    PolicyNotYetValid { now: u64, valid_after: u64 },
    /// 当前时间已达到/超过策略失效时间。
    PolicyExpired { now: u64, valid_until: u64 },
    /// sender 不在白名单且策略未显式放开任意发送者。
    SenderNotWhitelisted,
    /// 估算单笔最大 gas 成本超过策略单笔上限。
    PerOpGasCostExceeded { estimated_wei: u128, cap_wei: u128 },
    /// spent_before + 本次估算超过累计预算。
    BudgetExceeded {
        estimated_wei: u128,
        spent_before_wei: u128,
        budget_wei: u128,
    },
    /// checked 整数运算溢出。
    ArithmeticOverflow,
    /// 本版没有 Paymaster 宿主签名后端——fail-closed，绝不伪造链上担保签名。
    PaymasterSignerNotConfigured,
}

impl std::fmt::Display for PaymasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymasterError::PolicyDisabled => {
                write!(f, "PAYMASTER_POLICY_DISABLED: sponsorship policy is disabled")
            }
            PaymasterError::ZeroSender => {
                write!(f, "PAYMASTER_ZERO_SENDER: UserOperation sender must be non-zero")
            }
            PaymasterError::ZeroPaymaster => {
                write!(f, "PAYMASTER_ZERO_PAYMASTER: policy paymaster must be non-zero")
            }
            PaymasterError::ChainMismatch {
                op_chain,
                policy_chain,
            } => write!(
                f,
                "PAYMASTER_CHAIN_MISMATCH: op chain {op_chain} != policy chain {policy_chain}"
            ),
            PaymasterError::ZeroMaxFeePerGas => {
                write!(f, "PAYMASTER_ZERO_MAX_FEE: maxFeePerGas must be > 0")
            }
            PaymasterError::ZeroMaxPriorityFeePerGas => {
                write!(f, "PAYMASTER_ZERO_PRIORITY_FEE: maxPriorityFeePerGas must be > 0")
            }
            PaymasterError::PriorityFeeExceedsMax {
                priority_wei,
                max_wei,
            } => write!(
                f,
                "PAYMASTER_PRIORITY_EXCEEDS_MAX: priority {priority_wei} > max {max_wei}"
            ),
            PaymasterError::ZeroPreVerificationGas => {
                write!(f, "PAYMASTER_ZERO_PRE_VERIFICATION_GAS: preVerificationGas must be > 0")
            }
            PaymasterError::ZeroVerificationGas => {
                write!(f, "PAYMASTER_ZERO_VERIFICATION_GAS: verificationGasLimit must be > 0")
            }
            PaymasterError::BadPaymasterDataHex => {
                write!(f, "PAYMASTER_BAD_DATA_HEX: paymaster(And)Data must be even-length 0x hex")
            }
            PaymasterError::PaymasterDataTooShort { bytes_len } => write!(
                f,
                "PAYMASTER_DATA_TOO_SHORT: v0.6 paymasterAndData needs >=20 bytes, got {bytes_len}"
            ),
            PaymasterError::NoPaymasterProvided => write!(
                f,
                "PAYMASTER_NOT_PROVIDED: a sponsored UserOperation must name its paymaster"
            ),
            PaymasterError::PaymasterMismatch { effective, policy } => write!(
                f,
                "PAYMASTER_MISMATCH: effective {effective} != policy paymaster {policy}"
            ),
            PaymasterError::PolicyLimitsInvalid => write!(
                f,
                "PAYMASTER_POLICY_LIMITS_INVALID: per-op cap and total budget must both be > 0"
            ),
            PaymasterError::InvalidPolicyWindow {
                valid_after,
                valid_until,
            } => write!(
                f,
                "PAYMASTER_INVALID_POLICY_WINDOW: require valid_after < valid_until, got {valid_after}/{valid_until}"
            ),
            PaymasterError::PolicyNotYetValid { now, valid_after } => write!(
                f,
                "PAYMASTER_NOT_YET_VALID: now {now} < valid_after {valid_after}"
            ),
            PaymasterError::PolicyExpired { now, valid_until } => {
                write!(f, "PAYMASTER_EXPIRED: now {now} >= valid_until {valid_until}")
            }
            PaymasterError::SenderNotWhitelisted => write!(
                f,
                "PAYMASTER_SENDER_NOT_WHITELISTED: sender is not in the policy whitelist"
            ),
            PaymasterError::PerOpGasCostExceeded {
                estimated_wei,
                cap_wei,
            } => write!(
                f,
                "PAYMASTER_PER_OP_CAP_EXCEEDED: estimated {estimated_wei} > per-op cap {cap_wei}"
            ),
            PaymasterError::BudgetExceeded {
                estimated_wei,
                spent_before_wei,
                budget_wei,
            } => write!(
                f,
                "PAYMASTER_BUDGET_EXCEEDED: spent {spent_before_wei} + estimated {estimated_wei} > budget {budget_wei}"
            ),
            PaymasterError::ArithmeticOverflow => {
                write!(f, "PAYMASTER_ARITHMETIC_OVERFLOW: checked integer computation overflowed")
            }
            PaymasterError::PaymasterSignerNotConfigured => write!(
                f,
                "PAYMASTER_SIGNER_NOT_CONFIGURED: no paymaster host signer backend (refusing to fabricate an on-chain sponsorship signature)"
            ),
        }
    }
}

impl std::error::Error for PaymasterError {}

// ───────────────────────────────────────────────────────────────────────────
// UserOperation 气体/费用字段（ERC-4337 决策子集，单位 wei / gas 整数）
// ───────────────────────────────────────────────────────────────────────────

/// ERC-4337 UserOperation 中与 gas 担保决策相关的字段（v0.7 命名；v0.6 无两个
/// paymaster 气体字段时填 0 即可）。全部为链上整数，无浮点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOperationGas {
    pub pre_verification_gas: u128,
    pub verification_gas_limit: u128,
    /// 可以为 0（仅部署/授权而无执行调用的 UserOperation）。
    pub call_gas_limit: u128,
    /// v0.7 paymaster 验证气体；v0.6 形态填 0。
    pub paymaster_verification_gas_limit: u128,
    /// v0.7 postOp 气体；v0.6 形态填 0。
    pub paymaster_post_op_gas_limit: u128,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
}

impl UserOperationGas {
    /// 结构校验：费用为正、priority 不超过 max、必有的两个气体下限为正。
    pub fn validate(&self) -> Result<(), PaymasterError> {
        if self.max_fee_per_gas == 0 {
            return Err(PaymasterError::ZeroMaxFeePerGas);
        }
        if self.max_priority_fee_per_gas == 0 {
            return Err(PaymasterError::ZeroMaxPriorityFeePerGas);
        }
        if self.max_priority_fee_per_gas > self.max_fee_per_gas {
            return Err(PaymasterError::PriorityFeeExceedsMax {
                priority_wei: self.max_priority_fee_per_gas,
                max_wei: self.max_fee_per_gas,
            });
        }
        if self.pre_verification_gas == 0 {
            return Err(PaymasterError::ZeroPreVerificationGas);
        }
        if self.verification_gas_limit == 0 {
            return Err(PaymasterError::ZeroVerificationGas);
        }
        Ok(())
    }

    /// 五类气体上限之和（checked）。这是最坏情况下 EntryPoint 可能计费的气体量。
    pub fn total_gas(&self) -> Result<u128, PaymasterError> {
        self.pre_verification_gas
            .checked_add(self.verification_gas_limit)
            .and_then(|v| v.checked_add(self.call_gas_limit))
            .and_then(|v| v.checked_add(self.paymaster_verification_gas_limit))
            .and_then(|v| v.checked_add(self.paymaster_post_op_gas_limit))
            .ok_or(PaymasterError::ArithmeticOverflow)
    }

    /// 最大 gas 成本 = 总气体上限 × maxFeePerGas（整数上估，先校验字段）。
    pub fn estimated_max_gas_cost(&self) -> Result<u128, PaymasterError> {
        self.validate()?;
        self.total_gas()?
            .checked_mul(self.max_fee_per_gas)
            .ok_or(PaymasterError::ArithmeticOverflow)
    }
}

/// 用于赞助决策的 UserOperation 子集（不含 callData/factoryData 等不参与纯决策的字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOperation {
    pub sender: EvmAddress,
    pub nonce: u128,
    pub chain_id: u64,
    pub gas: UserOperationGas,
    /// v0.6 拼接形态：`0x` + 20B paymaster 地址 + 任意 data（与 v0.7 字段二选一，或同时给且地址一致）。
    pub paymaster_and_data_hex: Option<String>,
    /// v0.7 分离形态的 paymaster 地址。
    pub paymaster: Option<EvmAddress>,
    /// v0.7 分离形态的不透明 paymasterData（hex；可空）。
    pub paymaster_data_hex: Option<String>,
}

impl UserOperation {
    /// 解析出本次操作实际生效的 paymaster 地址与 data。
    ///
    /// - 只给 v0.6：解析 `paymasterAndData`；
    /// - 只给 v0.7：取 `paymaster`（data 可空）；
    /// - 两者都给：地址必须一致，否则 [`PaymasterError::PaymasterMismatch`]；
    /// - 只给 data 不给地址、或两者都没给：具名拒绝。
    pub fn effective_paymaster(&self) -> Result<(EvmAddress, Vec<u8>), PaymasterError> {
        let v06 = match &self.paymaster_and_data_hex {
            Some(h) => Some(parse_paymaster_and_data(h)?),
            None => None,
        };
        let v07 = match (&self.paymaster, &self.paymaster_data_hex) {
            (Some(addr), Some(dh)) => Some((
                *addr,
                hex_decode(dh).ok_or(PaymasterError::BadPaymasterDataHex)?,
            )),
            (Some(addr), None) => Some((*addr, Vec::new())),
            (None, Some(_)) => return Err(PaymasterError::NoPaymasterProvided),
            (None, None) => None,
        };
        match (v06, v07) {
            (Some(a), Some(b)) => {
                if a.0 == b.0 {
                    Ok(a)
                } else {
                    Err(PaymasterError::PaymasterMismatch {
                        effective: a.0.to_hex(),
                        policy: b.0.to_hex(),
                    })
                }
            }
            (Some(a), None) => Ok(a),
            (None, Some(b)) => Ok(b),
            (None, None) => Err(PaymasterError::NoPaymasterProvided),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 赞助策略 + 决策
// ───────────────────────────────────────────────────────────────────────────

/// 一份显式的 Paymaster 赞助策略。默认值不会“自动放开”——`enabled`、白名单与
/// `allow_any_sender` 都需调用方显式设置；内核只做按策略的确定性判定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SponsorPolicy {
    /// 唯一允许作保的链上 Paymaster 地址（UserOperation 必须指向它）。
    pub paymaster: EvmAddress,
    pub chain_id: u64,
    pub enabled: bool,
    /// 仅当显式为 true 时跳过白名单（默认 false = 只赞助白名单内 sender）。
    pub allow_any_sender: bool,
    /// 被赞助 sender 的 20 字节地址集合；空集合 + allow_any_sender=false 即拒绝所有人。
    pub whitelist: BTreeSet<[u8; 20]>,
    /// 单笔 UserOperation 最大可接受的 gas 成本上估（wei），必须 > 0。
    pub per_op_max_gas_cost_wei: u128,
    /// 累计赞助预算上限（wei），必须 > 0；调用方负责把累计 spent 持久化后回传。
    pub total_budget_wei: u128,
    /// 策略生效（含）的 unix 秒。
    pub valid_after: u64,
    /// 策略失效（不含）的 unix 秒。
    pub valid_until: u64,
}

/// 一次“同意赞助”的确定性结论。注意它**不是**链上担保，也不含签名。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SponsorshipApproval {
    pub decision: String,
    pub paymaster: String,
    pub sender: String,
    pub chain_id: u64,
    pub estimated_max_gas_cost_wei: u128,
    pub spent_before_wei: u128,
    pub budget_remaining_after_wei: u128,
    /// 始终 false：本内核不产出 Paymaster 担保签名。
    pub produces_paymaster_signature: bool,
    /// 始终 false：本内核不连接 bundler、不中继上链。
    pub relays_user_operation: bool,
}

impl SponsorPolicy {
    /// 按策略对一个 UserOperation 做纯确定性赞助判定。
    ///
    /// `spent_before_wei` 是调用方记账的历史累计赞助额（内核无状态、不持久化）；
    /// `now_unix` 显式传入（内核不读时钟）。任何一条不满足都返回类型化错误
    /// （fail-closed），绝不会在越限/越权时同意。
    pub fn decide_sponsorship(
        &self,
        uo: &UserOperation,
        spent_before_wei: u128,
        now_unix: u64,
    ) -> Result<SponsorshipApproval, PaymasterError> {
        if !self.enabled {
            return Err(PaymasterError::PolicyDisabled);
        }
        if self.paymaster.is_zero() {
            return Err(PaymasterError::ZeroPaymaster);
        }
        if uo.sender.is_zero() {
            return Err(PaymasterError::ZeroSender);
        }
        if uo.chain_id != self.chain_id {
            return Err(PaymasterError::ChainMismatch {
                op_chain: uo.chain_id,
                policy_chain: self.chain_id,
            });
        }
        if self.per_op_max_gas_cost_wei == 0 || self.total_budget_wei == 0 {
            return Err(PaymasterError::PolicyLimitsInvalid);
        }
        if self.valid_after >= self.valid_until {
            return Err(PaymasterError::InvalidPolicyWindow {
                valid_after: self.valid_after,
                valid_until: self.valid_until,
            });
        }
        if now_unix < self.valid_after {
            return Err(PaymasterError::PolicyNotYetValid {
                now: now_unix,
                valid_after: self.valid_after,
            });
        }
        if now_unix >= self.valid_until {
            return Err(PaymasterError::PolicyExpired {
                now: now_unix,
                valid_until: self.valid_until,
            });
        }

        // UserOperation 必须指向本策略的 paymaster（v0.6/v0.7 先做一致性解析）。
        let (effective, _data) = uo.effective_paymaster()?;
        if effective != self.paymaster {
            return Err(PaymasterError::PaymasterMismatch {
                effective: effective.to_hex(),
                policy: self.paymaster.to_hex(),
            });
        }

        // 结构校验 + 整数上估。
        let estimated = uo.gas.estimated_max_gas_cost()?;
        if estimated > self.per_op_max_gas_cost_wei {
            return Err(PaymasterError::PerOpGasCostExceeded {
                estimated_wei: estimated,
                cap_wei: self.per_op_max_gas_cost_wei,
            });
        }

        // 白名单（除非显式放开任意发送者）。
        if !self.allow_any_sender && !self.whitelist.contains(&uo.sender.0) {
            return Err(PaymasterError::SenderNotWhitelisted);
        }

        // 累计预算守恒（checked）。
        let new_spent = spent_before_wei
            .checked_add(estimated)
            .ok_or(PaymasterError::ArithmeticOverflow)?;
        if new_spent > self.total_budget_wei {
            return Err(PaymasterError::BudgetExceeded {
                estimated_wei: estimated,
                spent_before_wei,
                budget_wei: self.total_budget_wei,
            });
        }
        let remaining = self
            .total_budget_wei
            .checked_sub(new_spent)
            .ok_or(PaymasterError::ArithmeticOverflow)?;

        Ok(SponsorshipApproval {
            decision: "approved".to_string(),
            paymaster: self.paymaster.to_hex(),
            sender: uo.sender.to_hex(),
            chain_id: self.chain_id,
            estimated_max_gas_cost_wei: estimated,
            spent_before_wei,
            budget_remaining_after_wei: remaining,
            produces_paymaster_signature: false,
            relays_user_operation: false,
        })
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 签名边界：本版只预览、不签发（与 x402_sign 同样的 fail-closed 姿态）
// ───────────────────────────────────────────────────────────────────────────

/// Paymaster 担保签名预览：始终声明未配置签名后端、不返回任何签名。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymasterSignPreview {
    pub paymaster_signer_configured: bool,
    pub signature_hex: Option<String>,
}

pub fn preview_paymaster_signature() -> PaymasterSignPreview {
    PaymasterSignPreview {
        paymaster_signer_configured: false,
        signature_hex: None,
    }
}

/// 真正产出 `validatePaymasterUserOp` 担保签名——本版无宿主签名后端，具名拒绝。
pub fn sign_paymaster_data() -> Result<(), PaymasterError> {
    Err(PaymasterError::PaymasterSignerNotConfigured)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(hex: &str) -> EvmAddress {
        EvmAddress::from_hex(hex).expect("test address")
    }

    const PM: &str = "0x1111111111111111111111111111111111111111";
    const SENDER: &str = "0x2222222222222222222222222222222222222222";
    const OTHER: &str = "0x3333333333333333333333333333333333333333";

    fn good_gas() -> UserOperationGas {
        UserOperationGas {
            pre_verification_gas: 50_000,
            verification_gas_limit: 100_000,
            call_gas_limit: 200_000,
            paymaster_verification_gas_limit: 50_000,
            paymaster_post_op_gas_limit: 30_000,
            max_fee_per_gas: 1_000_000_000,        // 1 gwei
            max_priority_fee_per_gas: 100_000_000, // 0.1 gwei
        }
        // total gas = 430_000；estimated = 430_000 * 1e9 = 4.3e14
    }

    fn good_uo() -> UserOperation {
        UserOperation {
            sender: addr(SENDER),
            nonce: 7,
            chain_id: 8453,
            gas: good_gas(),
            paymaster_and_data_hex: Some(format!("0x{}", "11".repeat(20))),
            paymaster: None,
            paymaster_data_hex: None,
        }
    }

    fn good_policy() -> SponsorPolicy {
        let mut wl = BTreeSet::new();
        wl.insert(addr(SENDER).0);
        SponsorPolicy {
            paymaster: addr(PM),
            chain_id: 8453,
            enabled: true,
            allow_any_sender: false,
            whitelist: wl,
            per_op_max_gas_cost_wei: 1_000_000_000_000_000, // 1e15
            total_budget_wei: 10_000_000_000_000_000,       // 1e16
            valid_after: 1_000,
            valid_until: 2_000_000_000,
        }
    }

    #[test]
    fn gas_validate_accepts_good_and_rejects_each_bad_field() {
        assert!(good_gas().validate().is_ok());
        let mut g = good_gas();
        g.max_fee_per_gas = 0;
        assert!(matches!(
            g.validate(),
            Err(PaymasterError::ZeroMaxFeePerGas)
        ));
        let mut g = good_gas();
        g.max_priority_fee_per_gas = 0;
        assert!(matches!(
            g.validate(),
            Err(PaymasterError::ZeroMaxPriorityFeePerGas)
        ));
        let mut g = good_gas();
        g.max_priority_fee_per_gas = g.max_fee_per_gas + 1;
        assert!(matches!(
            g.validate(),
            Err(PaymasterError::PriorityFeeExceedsMax { .. })
        ));
        let mut g = good_gas();
        g.pre_verification_gas = 0;
        assert!(matches!(
            g.validate(),
            Err(PaymasterError::ZeroPreVerificationGas)
        ));
        let mut g = good_gas();
        g.verification_gas_limit = 0;
        assert!(matches!(
            g.validate(),
            Err(PaymasterError::ZeroVerificationGas)
        ));
        // call_gas_limit 允许为 0。
        let mut g = good_gas();
        g.call_gas_limit = 0;
        assert!(g.validate().is_ok());
    }

    #[test]
    fn total_gas_and_estimated_cost_are_integer_and_hand_computed() {
        let g = good_gas();
        assert_eq!(g.total_gas().unwrap(), 430_000);
        // 430_000 * 1_000_000_000 = 430_000_000_000_000
        assert_eq!(g.estimated_max_gas_cost().unwrap(), 430_000_000_000_000);
        // call_gas=0 时相应减少。
        let mut g2 = good_gas();
        g2.call_gas_limit = 0;
        assert_eq!(g2.total_gas().unwrap(), 230_000);
    }

    #[test]
    fn estimated_cost_overflow_is_named_not_panic() {
        let mut g = good_gas();
        g.max_fee_per_gas = u128::MAX;
        g.max_priority_fee_per_gas = u128::MAX;
        assert!(matches!(
            g.estimated_max_gas_cost(),
            Err(PaymasterError::ArithmeticOverflow)
        ));
    }

    #[test]
    fn parse_paymaster_and_data_splits_address_and_tail() {
        let hex = format!("0x{}{}", "11".repeat(20), "deadbeef");
        let (a, data) = parse_paymaster_and_data(&hex).unwrap();
        assert_eq!(a, addr(PM));
        assert_eq!(data, vec![0xde, 0xad, 0xbe, 0xef]);
        // 恰好 20 字节：data 为空也合法。
        let (a2, data2) = parse_paymaster_and_data(&format!("0x{}", "11".repeat(20))).unwrap();
        assert_eq!(a2, addr(PM));
        assert!(data2.is_empty());
    }

    #[test]
    fn parse_paymaster_and_data_rejects_bad_hex_odd_and_short() {
        assert!(matches!(
            parse_paymaster_and_data("0xzz"),
            Err(PaymasterError::BadPaymasterDataHex)
        ));
        assert!(matches!(
            parse_paymaster_and_data("0xabc"),
            Err(PaymasterError::BadPaymasterDataHex)
        ));
        // 19 字节。
        assert!(matches!(
            parse_paymaster_and_data(&format!("0x{}", "11".repeat(19))),
            Err(PaymasterError::PaymasterDataTooShort { bytes_len: 19 })
        ));
    }

    #[test]
    fn hex_decode_handles_prefixes_and_even_length() {
        assert_eq!(hex_decode("0x0a0b").unwrap(), vec![0x0a, 0x0b]);
        assert_eq!(hex_decode("0X0A0B").unwrap(), vec![0x0a, 0x0b]);
        assert!(hex_decode("0x0").is_none());
        assert!(hex_decode("nope").is_none());
    }

    #[test]
    fn effective_paymaster_v06_v07_equal_mismatch_and_missing() {
        // 仅 v0.6。
        let (e, _) = good_uo().effective_paymaster().unwrap();
        assert_eq!(e, addr(PM));

        // 仅 v0.7（data 可空）。
        let mut uo = good_uo();
        uo.paymaster_and_data_hex = None;
        uo.paymaster = Some(addr(PM));
        uo.paymaster_data_hex = None;
        let (e2, d2) = uo.effective_paymaster().unwrap();
        assert_eq!(e2, addr(PM));
        assert!(d2.is_empty());

        // v0.6/v0.7 都给且地址一致 → 以 v0.6 为准且带 data。
        let mut uo = good_uo();
        uo.paymaster = Some(addr(PM));
        uo.paymaster_data_hex = Some("0xdeadbeef".to_string());
        uo.paymaster_and_data_hex = Some(format!("0x{}{}", "11".repeat(20), "cafe"));
        let (e3, d3) = uo.effective_paymaster().unwrap();
        assert_eq!(e3, addr(PM));
        assert_eq!(d3, vec![0xca, 0xfe]);

        // 地址不一致 → 拒绝。
        let mut uo = good_uo();
        uo.paymaster = Some(addr(OTHER));
        assert!(matches!(
            uo.effective_paymaster(),
            Err(PaymasterError::PaymasterMismatch { .. })
        ));

        // 只有 data 没有地址 → 拒绝。
        let mut uo = good_uo();
        uo.paymaster_and_data_hex = None;
        uo.paymaster = None;
        uo.paymaster_data_hex = Some("0xaa".to_string());
        assert!(matches!(
            uo.effective_paymaster(),
            Err(PaymasterError::NoPaymasterProvided)
        ));

        // 两者都没有 → 拒绝。
        let mut uo = good_uo();
        uo.paymaster_and_data_hex = None;
        assert!(matches!(
            uo.effective_paymaster(),
            Err(PaymasterError::NoPaymasterProvided)
        ));
    }

    #[test]
    fn decide_happy_path_approves_with_remaining() {
        let ap = good_policy()
            .decide_sponsorship(&good_uo(), 0, 1_000_000)
            .unwrap();
        assert_eq!(ap.decision, "approved");
        assert_eq!(ap.estimated_max_gas_cost_wei, 430_000_000_000_000);
        assert_eq!(ap.spent_before_wei, 0);
        assert_eq!(
            ap.budget_remaining_after_wei,
            10_000_000_000_000_000u128 - 430_000_000_000_000
        );
        assert!(!ap.produces_paymaster_signature);
        assert!(!ap.relays_user_operation);
    }

    #[test]
    fn decide_spending_exactly_to_budget_is_approved_then_overflows() {
        let mut p = good_policy();
        p.total_budget_wei = 430_000_000_000_000; // 恰好一笔。
        let ap = p.decide_sponsorship(&good_uo(), 0, 1_000_000).unwrap();
        assert_eq!(ap.budget_remaining_after_wei, 0);
        // 已有一笔 spent 后再来一笔 → 超预算。
        assert!(matches!(
            p.decide_sponsorship(&good_uo(), 430_000_000_000_000, 1_000_000),
            Err(PaymasterError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn decide_denies_disabled_zeros_chain_and_window() {
        let mut p = good_policy();
        p.enabled = false;
        assert!(matches!(
            p.decide_sponsorship(&good_uo(), 0, 1_000_000),
            Err(PaymasterError::PolicyDisabled)
        ));

        let mut p = good_policy();
        p.paymaster = EvmAddress::ZERO;
        assert!(matches!(
            p.decide_sponsorship(&good_uo(), 0, 1_000_000),
            Err(PaymasterError::ZeroPaymaster)
        ));

        let mut uo = good_uo();
        uo.sender = EvmAddress::ZERO;
        assert!(matches!(
            good_policy().decide_sponsorship(&uo, 0, 1_000_000),
            Err(PaymasterError::ZeroSender)
        ));

        let mut uo = good_uo();
        uo.chain_id = 1;
        assert!(matches!(
            good_policy().decide_sponsorship(&uo, 0, 1_000_000),
            Err(PaymasterError::ChainMismatch { .. })
        ));

        let mut p = good_policy();
        p.per_op_max_gas_cost_wei = 0;
        assert!(matches!(
            p.decide_sponsorship(&good_uo(), 0, 1_000_000),
            Err(PaymasterError::PolicyLimitsInvalid)
        ));

        let mut p = good_policy();
        p.valid_after = 2_000_000_000;
        p.valid_until = 2_000_000_000;
        assert!(matches!(
            p.decide_sponsorship(&good_uo(), 0, 1_000_000),
            Err(PaymasterError::InvalidPolicyWindow { .. })
        ));

        assert!(matches!(
            good_policy().decide_sponsorship(&good_uo(), 0, 999),
            Err(PaymasterError::PolicyNotYetValid { .. })
        ));
        assert!(matches!(
            good_policy().decide_sponsorship(&good_uo(), 0, 2_000_000_000),
            Err(PaymasterError::PolicyExpired { .. })
        ));
    }

    #[test]
    fn decide_denies_wrong_paymaster_bad_gas_and_per_op_cap() {
        // UserOperation 指向别的 paymaster。
        let mut uo = good_uo();
        uo.paymaster_and_data_hex = Some(format!("0x{}", "33".repeat(20)));
        assert!(matches!(
            good_policy().decide_sponsorship(&uo, 0, 1_000_000),
            Err(PaymasterError::PaymasterMismatch { .. })
        ));

        // 气体字段非法（priority>max）也不能因白名单而放行。
        let mut uo = good_uo();
        uo.gas.max_priority_fee_per_gas = uo.gas.max_fee_per_gas + 1;
        assert!(good_policy().decide_sponsorship(&uo, 0, 1_000_000).is_err());

        // 单笔上估超过策略单笔上限。
        let mut p = good_policy();
        p.per_op_max_gas_cost_wei = 1_000;
        assert!(matches!(
            p.decide_sponsorship(&good_uo(), 0, 1_000_000),
            Err(PaymasterError::PerOpGasCostExceeded { .. })
        ));
    }

    #[test]
    fn whitelist_default_denies_and_explicit_allow_any_sender_overrides() {
        // 空白名单 + 不放开任意 → 即使是 SENDER 也拒（好策略里白名单含 SENDER，这里清空）。
        let mut p = good_policy();
        p.whitelist.clear();
        assert!(matches!(
            p.decide_sponsorship(&good_uo(), 0, 1_000_000),
            Err(PaymasterError::SenderNotWhitelisted)
        ));
        // 显式放开任意发送者后通过。
        p.allow_any_sender = true;
        assert!(p.decide_sponsorship(&good_uo(), 0, 1_000_000).is_ok());
    }

    #[test]
    fn cumulative_spent_counts_across_ops() {
        let p = good_policy();
        // 第一笔后 spent=4.3e14，剩余可再容纳。
        let first = p.decide_sponsorship(&good_uo(), 0, 1_000_000).unwrap();
        let second = p
            .decide_sponsorship(&good_uo(), 430_000_000_000_000, 1_000_000)
            .unwrap();
        assert_eq!(second.spent_before_wei, 430_000_000_000_000);
        assert_eq!(
            second.budget_remaining_after_wei,
            first.budget_remaining_after_wei - 430_000_000_000_000
        );
    }

    #[test]
    fn preview_has_no_signature_and_sign_is_fail_closed() {
        let pv = preview_paymaster_signature();
        assert!(!pv.paymaster_signer_configured);
        assert_eq!(pv.signature_hex, None);
        assert!(matches!(
            sign_paymaster_data(),
            Err(PaymasterError::PaymasterSignerNotConfigured)
        ));
        assert!(sign_paymaster_data()
            .unwrap_err()
            .to_string()
            .contains("PAYMASTER_SIGNER_NOT_CONFIGURED"));
    }
}
