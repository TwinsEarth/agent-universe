//! 语义化版本与本项目的 npm↔Rust 双轨版本号换算。
//!
//! 版本权威源是仓库根 `VERSION`（npm 形态 `X.Y.Z`）；gsn-core crate 使用
//! `0.X.(Y*10+Z)` 的 Rust 形态（见 `scripts/bump-version.sh`）。例如：
//!
//! ```text
//! 3.6.0  <->  0.3.60
//! 3.6.1  <->  0.3.61
//! 3.10.3 <->  0.3.103
//! ```
//!
//! 约束：该编码只在补丁号 `Z < 10` 时无歧义（`(Y, Z)` 与 `NN` 一一对应）。
//! 若某条版本线的补丁号达到 10，必须先改编码方案，而不是静默产生碰撞。

use std::cmp::Ordering;

use super::UpdateError;

/// npm 形态的语义化版本 `X.Y.Z`（非负整数，无预发布/构建元数据）。
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemVer {
    /// 解析 `X.Y.Z`；拒绝多余段、空白、非数字与前导符号。
    pub fn parse(s: &str) -> Result<Self, UpdateError> {
        let s = s.trim();
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(UpdateError::BadVersion(s.to_string()));
        }
        let mut nums = [0u64; 3];
        for (i, p) in parts.iter().enumerate() {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return Err(UpdateError::BadVersion(s.to_string()));
            }
            // 拒绝前导零（"03" 非法，单独的 "0" 合法），避免 3.6.1 与 3.6.01 混淆。
            if p.len() > 1 && p.starts_with('0') {
                return Err(UpdateError::BadVersion(s.to_string()));
            }
            nums[i] = p
                .parse::<u64>()
                .map_err(|_| UpdateError::BadVersion(s.to_string()))?;
        }
        Ok(SemVer {
            major: nums[0],
            minor: nums[1],
            patch: nums[2],
        })
    }

    /// 编译期注入的当前 gsn-core crate 版本（Rust 形态），换算回 npm 形态。
    pub fn current() -> Result<Self, UpdateError> {
        npm_from_rust(env!("CARGO_PKG_VERSION"))
    }

    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Greater
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// npm `X.Y.Z` -> Rust `0.X.(Y*10+Z)`。补丁号必须 < 10，否则编码会碰撞。
pub fn npm_to_rust(v: &SemVer) -> Result<String, UpdateError> {
    if v.patch >= 10 {
        return Err(UpdateError::PatchOutOfRange(v.patch));
    }
    let encoded = v
        .minor
        .checked_mul(10)
        .and_then(|n| n.checked_add(v.patch))
        .ok_or(UpdateError::VersionOverflow)?;
    Ok(format!("0.{}.{}", v.major, encoded))
}

/// Rust `0.X.NN` -> npm `X.(NN/10).(NN%10)`。只接受前导为 0 的三段 Rust 形态。
pub fn npm_from_rust(rust_ver: &str) -> Result<SemVer, UpdateError> {
    let p = SemVer::parse(rust_ver)?;
    if p.major != 0 {
        return Err(UpdateError::BadRustVersion(rust_ver.to_string()));
    }
    // Rust 第二段是 npm major（X）；第三段 NN = npm minor*10 + patch。
    let major = p.minor;
    let minor = p.patch / 10;
    let patch = p.patch % 10;
    Ok(SemVer {
        major,
        minor,
        patch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_orders() {
        let a = SemVer::parse("3.6.0").unwrap();
        let b = SemVer::parse("3.6.1").unwrap();
        let c = SemVer::parse("3.10.0").unwrap();
        assert!(b.is_newer_than(&a));
        assert!(c.is_newer_than(&b));
        assert_eq!(
            a,
            SemVer {
                major: 3,
                minor: 6,
                patch: 0
            }
        );
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "3.6", "3.6.x", "v3.6.1", "3.6.1.0", "", "03.6.1", "3..1", "-1.2.3",
        ] {
            assert!(SemVer::parse(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn mapping_roundtrip() {
        for (npm, rust) in [
            ("3.6.0", "0.3.60"),
            ("3.6.1", "0.3.61"),
            ("3.10.3", "0.3.103"),
            ("2.9.2", "0.2.92"),
        ] {
            let v = SemVer::parse(npm).unwrap();
            assert_eq!(npm_to_rust(&v).unwrap(), rust);
            assert_eq!(npm_from_rust(rust).unwrap(), v);
        }
    }

    #[test]
    fn rejects_patch_ten_and_nonzero_lead() {
        let v = SemVer::parse("3.6.10").unwrap();
        assert!(matches!(
            npm_to_rust(&v),
            Err(UpdateError::PatchOutOfRange(10))
        ));
        assert!(npm_from_rust("1.3.60").is_err());
    }

    // 旧实现回归点：若有人把 crate 版本直接当 npm 版本比较（"0.3.60" 当作三段
    // npm 版本），会得出 major=0，通道过滤会把全部 3.x 版本判为跨大版本而永不更新。
    #[test]
    fn current_is_decoded_from_rust_not_compared_raw() {
        let cur = SemVer::current().expect("crate version decodes");
        // gsn-core 0.3.NN -> npm 3.x，而不是 npm 0.x。
        assert_eq!(cur.major, 3);
    }
}
