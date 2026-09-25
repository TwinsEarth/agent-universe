//! 群体记忆共享增强（v2.4.5）
//!
//! - `TransferBundle`：P10 六字段交接协议（Goal/Context/Done/Todo/Trace/Owner），机械校验。
//! - `EvidenceGrade`：P12 证据三级标签（verified / cpu-proto / unverified），随数据流动。
//! - `TraceLedger`：P12 哈希链审计轨迹，每条执行记录指向上一条。

use serde::{Deserialize, Serialize};

/// 证据分级（P12）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceGrade {
    /// 已被多节点交叉验证
    Verified,
    /// 仅 CPU 原型验证
    CpuProto,
    /// 未验证
    Unverified,
}

/// P10 交接包：六字段，机械校验缺一不可。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferBundle {
    pub goal: String,
    pub context: String,
    pub done: String,
    pub todo: String,
    pub trace: String,
    pub owner: String,
    pub grade: EvidenceGrade,
}

impl TransferBundle {
    /// 机械校验：六字段均非空，Owner 不得为空。失败不静默转移责任。
    pub fn validate(&self) -> Result<(), String> {
        if self.goal.is_empty() { return Err("goal empty".into()); }
        if self.context.is_empty() { return Err("context empty".into()); }
        if self.done.is_empty() { return Err("done empty".into()); }
        if self.todo.is_empty() { return Err("todo empty".into()); }
        if self.trace.is_empty() { return Err("trace empty".into()); }
        if self.owner.is_empty() { return Err("owner empty".into()); }
        Ok(())
    }
}

/// P12 审计轨迹：哈希链。
#[derive(Debug, Clone, Default)]
pub struct TraceLedger {
    chain: Vec<(String, EvidenceGrade)>,
    prev: String,
}

impl TraceLedger {
    pub fn new() -> Self {
        Self { chain: Vec::new(), prev: "genesis".into() }
    }

    /// 追加一条执行记录；新哈希 = prev + payload。
    pub fn append(&mut self, payload: &str, grade: EvidenceGrade) {
        let h = format!("{:016x}", {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut s = DefaultHasher::new();
            format!("{}{}", self.prev, payload).hash(&mut s);
            s.finish()
        });
        self.chain.push((h.clone(), grade));
        self.prev = h;
    }

    pub fn len(&self) -> usize { self.chain.len() }
    pub fn verified_count(&self) -> usize {
        self.chain.iter().filter(|(_, g)| *g == EvidenceGrade::Verified).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_rejects_missing_field() {
        let b = TransferBundle {
            goal: "do x".into(), context: "c".into(), done: "".into(),
            todo: "t".into(), trace: "tr".into(), owner: "o".into(),
            grade: EvidenceGrade::CpuProto,
        };
        assert!(b.validate().is_err());
    }

    #[test]
    fn bundle_accepts_complete() {
        let b = TransferBundle {
            goal: "g".into(), context: "c".into(), done: "d".into(),
            todo: "t".into(), trace: "tr".into(), owner: "o".into(),
            grade: EvidenceGrade::Verified,
        };
        assert!(b.validate().is_ok());
    }

    #[test]
    fn ledger_counts_verified() {
        let mut l = TraceLedger::new();
        l.append("task1", EvidenceGrade::Verified);
        l.append("task2", EvidenceGrade::Unverified);
        assert_eq!(l.len(), 2);
        assert_eq!(l.verified_count(), 1);
    }
}
