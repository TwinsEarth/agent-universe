//! 审核 Agent（Auditor，v5.0.0 骨架）
//!
//! 审核节点行为、记忆条目与任务产出，持有 [`SecurityEngine`]（违规上报/封禁）。
//! v5.0.0 提供角色定义、消息入口与 SecurityEngine 接线；v5.0.4 起隐私扫描规则下沉到
//! `memory::privacy` 并接线进持久化写入路径（fail-closed），本 Agent 复用同一套规则。
//! 行为/产出审核在后续版本。
//!
//! 隐私策略对齐 FLEET §4：记忆档案不存 prompt / secret，命中即 **fail-closed 拒绝**。

use crate::memory::privacy::scan_entry_for_secret;
use crate::memory::MemoryEntry;
use crate::security::{SecurityEngine, SecurityFlag};

/// 审核结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditVerdict {
    /// 允许。
    Allow,
    /// 放行但标记（低风险观察项）。
    Flag,
    /// 拒绝（fail-closed）。
    Reject,
}

/// 单条审核发现。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFinding {
    pub subject: String,
    pub code: String,
    pub verdict: AuditVerdict,
    pub detail: String,
}

/// 审核 Agent。
pub struct AuditorAgent {
    id: String,
    security: SecurityEngine,
    findings: Vec<AuditFinding>,
}

impl AuditorAgent {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            security: SecurityEngine::new(),
            findings: Vec::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn findings(&self) -> &[AuditFinding] {
        &self.findings
    }

    pub fn security(&self) -> &SecurityEngine {
        &self.security
    }

    pub fn findings_count(&self) -> usize {
        self.findings.len()
    }

    fn record(&mut self, f: AuditFinding) -> AuditVerdict {
        let v = f.verdict;
        self.findings.push(f);
        v
    }

    /// 审核一条记忆条目：fail-closed —— argv / 结论 / tail 中任一字段含疑似凭据或私钥即拒绝。
    ///
    /// 扫描规则复用 `memory::privacy`，与持久化写入路径 [`PersistentMemoryStore::append`]
    /// 使用同一套特征，保证「Agent 审核」与「存储层 fail-closed」结论一致。
    ///
    /// [`PersistentMemoryStore::append`]: crate::memory::PersistentMemoryStore::append
    pub fn audit_memory_entry(&mut self, e: &MemoryEntry) -> AuditVerdict {
        if let Some(hit) = scan_entry_for_secret(e) {
            return self.record(AuditFinding {
                subject: e.task_id.clone(),
                code: "AUDIT-SECRET-001".to_string(),
                verdict: AuditVerdict::Reject,
                detail: format!(
                    "记忆条目 {} 包含疑似敏感凭据（{}），拒绝写入档案",
                    hit.field, hit.kind
                ),
            });
        }
        AuditVerdict::Allow
    }

    /// 上报节点行为：委托 SecurityEngine；返回上报后该节点是否已被封禁。
    pub fn report_behavior(&mut self, node_id: &str, flag: SecurityFlag) -> bool {
        self.security.report_behavior(node_id.to_string(), flag);
        self.security.is_banned(node_id)
    }

    /// 直接封禁节点。
    pub fn ban(&mut self, node_id: &str) {
        self.security.ban_node(node_id.to_string());
    }

    pub fn is_banned(&self, node_id: &str) -> bool {
        self.security.is_banned(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::privacy::contains_aws_access_key;

    fn entry_with(field: &str) -> MemoryEntry {
        MemoryEntry::new("t1").conclusion(field)
    }

    #[test]
    fn clean_entry_allowed() {
        let mut a = AuditorAgent::new("auditor");
        let v = a.audit_memory_entry(&entry_with("verify-join passes on this tree"));
        assert_eq!(v, AuditVerdict::Allow);
        assert_eq!(a.findings_count(), 0);
    }

    #[test]
    fn detects_github_pat() {
        let mut a = AuditorAgent::new("auditor");
        let v = a.audit_memory_entry(&entry_with(
            "token github_pat_11COYUIPI0abcdef0123456789 leaked",
        ));
        assert_eq!(v, AuditVerdict::Reject);
        assert_eq!(a.findings()[0].code, "AUDIT-SECRET-001");
    }

    #[test]
    fn detects_private_key() {
        let mut a = AuditorAgent::new("auditor");
        let v = a.audit_memory_entry(&entry_with_owned(
            "key: -----BEGIN PRIVATE KEY-----\nMIIE...",
        ));
        assert_eq!(v, AuditVerdict::Reject);
    }

    fn entry_with_owned(s: &str) -> MemoryEntry {
        MemoryEntry::new("t2").stdout_tail(s)
    }

    #[test]
    fn detects_aws_key() {
        assert!(contains_aws_access_key("AKIAIOSFODNN7EXAMPLE"));
        assert!(!contains_aws_access_key("not a key"));
    }

    #[test]
    fn behavior_reporting_wires_security() {
        let mut a = AuditorAgent::new("auditor");
        // 每次 report 扣分；连续多次最终触发封禁。
        let mut banned = false;
        for _ in 0..10 {
            banned = a.report_behavior("node-1", SecurityFlag::InvalidSignature);
        }
        assert!(banned);
        assert!(a.is_banned("node-1"));
    }

    #[test]
    fn direct_ban() {
        let mut a = AuditorAgent::new("auditor");
        a.ban("node-2");
        assert!(a.is_banned("node-2"));
    }
}
