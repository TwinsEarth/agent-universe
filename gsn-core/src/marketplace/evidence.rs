//! 证据分级（Evidence Grade）
//!
//! 每个数字、结果、指标都必须标注证据等级
//! - Verified: 可复跑测试验证
//! - CpuProto: 原型阶段运行结果
//! - Unverified: 未验证 / [RESULT NEEDED]

use serde::{Deserialize, Serialize};

/// 证据等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceGrade {
    /// 可复跑测试验证
    Verified,
    /// 原型阶段运行结果
    CpuProto,
    /// 未验证 / [RESULT NEEDED]
    Unverified,
}

impl EvidenceGrade {
    pub fn label(&self) -> &'static str {
        match self {
            EvidenceGrade::Verified => "verified",
            EvidenceGrade::CpuProto => "cpu-proto",
            EvidenceGrade::Unverified => "unverified",
        }
    }

    /// 是否可信（用于结算门禁）
    pub fn is_trustworthy(&self) -> bool {
        matches!(self, EvidenceGrade::Verified | EvidenceGrade::CpuProto)
    }
}

impl std::fmt::Display for EvidenceGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl Default for EvidenceGrade {
    fn default() -> Self {
        EvidenceGrade::Unverified
    }
}
