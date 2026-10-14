//! 审核 Agent（Auditor，v5.0.0 骨架）
//!
//! 审核节点行为、记忆条目与任务产出，持有 [`SecurityEngine`]（违规上报/封禁）。
//! v5.0.0 提供角色定义、消息入口、隐私 fail-closed 最小扫描（凭据特征）与
//! SecurityEngine 接线；完整隐私规则在 v5.0.4，行为/产出审核在 v5.0.5。
//!
//! 隐私策略对齐 FLEET §4：记忆档案不存 prompt / secret，命中即 **fail-closed 拒绝**。

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

    /// 审核一条记忆条目：fail-closed —— 结论/tail 中任一字段含疑似凭据或私钥即拒绝。
    pub fn audit_memory_entry(&mut self, e: &MemoryEntry) -> AuditVerdict {
        for field in sensitive_fields(e) {
            if let Some(hit) = detect_secret(field) {
                return self.record(AuditFinding {
                    subject: e.task_id.clone(),
                    code: "AUDIT-SECRET-001".to_string(),
                    verdict: AuditVerdict::Reject,
                    detail: format!("记忆条目包含疑似敏感凭据（{hit}），拒绝写入档案"),
                });
            }
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

fn sensitive_fields(e: &MemoryEntry) -> Vec<&str> {
    let mut v = Vec::new();
    if let Some(s) = &e.conclusion {
        v.push(s.as_str());
    }
    if let Some(s) = &e.stdout_tail {
        v.push(s.as_str());
    }
    if let Some(s) = &e.stderr_tail {
        v.push(s.as_str());
    }
    v
}

/// 最小凭据特征扫描（v5.0.0 骨架；v5.0.4 扩展完整规则集）。
/// 返回命中的类型标签。
fn detect_secret(s: &str) -> Option<&'static str> {
    /// (小写匹配串, 类型标签)
    const PATTERNS: &[(&str, &str)] = &[
        ("github_pat_", "github fine-grained token"),
        ("ghp_", "github classic token"),
        ("gho_", "github oauth token"),
        ("ghu_", "github user-to-server token"),
        ("ghs_", "github server-to-server token"),
        ("-----begin private key-----", "pkcs8 private key"),
        ("-----begin openssh private key-----", "openssh private key"),
        ("-----begin rsa private key-----", "rsa private key"),
        ("-----begin ec private key-----", "ec private key"),
        ("bearer eyj", "bearer jwt"),
    ];
    let lower = s.to_ascii_lowercase();
    for (needle, label) in PATTERNS {
        if lower.contains(needle) {
            return Some(*label);
        }
    }
    // 常见 `sk-` 服务密钥（需后接足够长的字符，避免误判普通词）。
    if let Some(idx) = lower.find("sk-") {
        let rest = &lower[idx + 3..];
        if rest.chars().filter(|c| c.is_ascii_alphanumeric()).count() >= 16 {
            return Some("service secret key");
        }
    }
    if contains_aws_access_key(s) {
        return Some("aws access key id");
    }
    None
}

/// AWS Access Key ID：`AKIA` 后接 16 个大写字母/数字（总长 20）。
fn contains_aws_access_key(s: &str) -> bool {
    let bytes = s.as_bytes();
    for w in bytes.windows(20) {
        if w.starts_with(b"AKIA")
            && w[4..]
                .iter()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

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
