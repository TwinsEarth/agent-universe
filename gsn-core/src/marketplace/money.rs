//! 精确整数货币（Exact Integer Money）
//!
//! v2.5.8: 账本金额一律使用最小货币单位的 **i64 整数**，
//! 彻底取代 f64 浮点金额。浮点在 0.1 + 0.2、跨语言序列化、
//! 容差比较等场景会产生不可对账的误差；整数 + 精确相等
//! 才能让「守恒」成为可机械判定的硬不变量。
//!
//! 跨语言对齐：
//! - Rust：Money(i64)，序列化为裸整数
//! - JS：金额为 Number.isSafeInteger 的安全整数
//! - JSON 线上表示：整数（无小数点）

use serde::{Deserialize, Serialize};
use std::fmt;

/// 精确整数金额（最小货币单位）
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Money(pub i64);

impl Money {
    /// 零
    pub const ZERO: Money = Money(0);
    /// 市场最低质押（100）
    pub const MIN_STAKE: Money = Money(100);

    pub fn new(v: i64) -> Self {
        Money(v)
    }

    pub fn as_i64(self) -> i64 {
        self.0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// 检查加法（溢出即错，不静默回绕）
    pub fn checked_add(self, other: Money) -> Result<Money, String> {
        self.0
            .checked_add(other.0)
            .map(Money)
            .ok_or_else(|| "金额加法溢出".to_string())
    }

    /// 检查减法（下溢即错）
    pub fn checked_sub(self, other: Money) -> Result<Money, String> {
        self.0
            .checked_sub(other.0)
            .map(Money)
            .ok_or_else(|| "金额减法下溢".to_string())
    }

    /// 取较小者
    pub fn min(self, other: Money) -> Money {
        if self <= other { self } else { other }
    }
}

impl Default for Money {
    fn default() -> Self {
        Money::ZERO
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_is_exact_integer() {
        let a = Money::new(100);
        let b = Money::new(50);
        assert_eq!(a.checked_add(b).unwrap(), Money::new(150));
        assert_eq!(a.checked_sub(b).unwrap(), Money::new(50));
        // 精确相等，无容差
        assert_ne!(a, Money::new(101));
    }

    #[test]
    fn money_rejects_negative_deposit_logic() {
        assert!(Money::new(-1).is_negative());
        assert!(!Money::new(0).is_positive());
    }

    #[test]
    fn money_overflow_detected() {
        assert!(Money::new(i64::MAX).checked_add(Money::new(1)).is_err());
        assert!(Money::new(i64::MIN).checked_sub(Money::new(1)).is_err());
        // 注意：0-1 在整数运算中合法得到 -1（由 deposit/transfer 业务层拒绝负数），
        // 并非算术溢出；checked_* 只负责检测真正的溢出。
        assert_eq!(Money::new(0).checked_sub(Money::new(1)).unwrap(), Money::new(-1));
    }

    #[test]
    fn money_serializes_as_bare_integer() {
        let m = Money::new(100);
        assert_eq!(serde_json::to_string(&m).unwrap(), "100");
        let back: Money = serde_json::from_str("100").unwrap();
        assert_eq!(back, m);
    }
}
