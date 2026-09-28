//! 时钟端口（GAP §4.8）
//!
//! 领域逻辑不直接调用 `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`：
//! 时钟早于 1970 时那条链会 panic。统一改走本模块：
//!
//! - [`SystemClock`]：真实系统时钟，早于 1970 时**饱和为 0**，不 panic；
//! - [`ManualClock`]：确定性时钟，供测试与可注入时间的上层使用；
//! - [`now_secs`]：便利函数，等价于 `SystemClock.now_secs()`。

use std::time::{SystemTime, UNIX_EPOCH};

/// 时钟端口：领域逻辑通过它取时间，不直接碰系统时钟。
pub trait Clock {
    /// 当前 unix 秒。
    fn now_secs(&self) -> u64;
}

/// 系统时钟：早于 1970 饱和为 0，绝不 panic。
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// 手动时钟：确定性测试 / 可注入时间。
#[derive(Debug, Clone)]
pub struct ManualClock {
    pub secs: u64,
}

impl ManualClock {
    pub fn new(secs: u64) -> Self {
        Self { secs }
    }
    pub fn set(&mut self, secs: u64) {
        self.secs = secs;
    }
    pub fn advance(&mut self, secs: u64) {
        self.secs = self.secs.saturating_add(secs);
    }
}

impl Clock for ManualClock {
    fn now_secs(&self) -> u64 {
        self.secs
    }
}

/// 便利：当前 unix 秒（系统时钟，饱和不 panic）。
pub fn now_secs() -> u64 {
    SystemClock.now_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_is_deterministic() {
        let mut c = ManualClock::new(1_000);
        assert_eq!(c.now_secs(), 1_000);
        c.advance(50);
        assert_eq!(c.now_secs(), 1_050);
        c.set(2_000);
        assert_eq!(c.now_secs(), 2_000);
    }

    #[test]
    fn system_clock_never_panics_and_is_reasonable() {
        // 不 panic，且至少是 2020 年以后的量级（防回归）
        let t = SystemClock.now_secs();
        assert!(t > 1_577_800_000, "系统时钟异常小: {t}");
        assert!(now_secs() > 1_577_800_000);
    }
}
