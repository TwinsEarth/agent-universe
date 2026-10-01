//! 黑名单 —— 指纹库、取证、举报、申诉与解封
//!
//! # 黑名单命中即禁止加载
//!
//! 在清单校验进入 `VERIFIED` 之前检查黑名单；匹配则转入 `QUARANTINED` / 拒绝。
//!
//! # 匹配方式：精确匹配
//!
//! 按插件 `name` 精确匹配，或按模块 SHA-256 精确匹配（载荷被改名后仍能命中）。
//!
//! # 解封
//!
//! 黑名单条目不因「改名」而解除；唯一解除方式是发布新版本并通过完整审核流程
//! （`appeal` 记录申诉，`unblock` 仅在新模块摘要通过后生效）。

use serde::{Deserialize, Serialize};

/// 黑名单原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlacklistReason {
    /// 恶意代码。
    Malware,
    /// 违反政策。
    PolicyViolation,
    /// 发布者密钥被吊销。
    KeyRevoked,
    /// 社区举报（经人工复核）。
    CommunityReport,
    /// 运行时行为异常（越界/未声明网络/进程注入）。
    RuntimeAbuse,
}

impl BlacklistReason {
    pub fn as_str(self) -> &'static str {
        match self {
            BlacklistReason::Malware => "malware",
            BlacklistReason::PolicyViolation => "policy_violation",
            BlacklistReason::KeyRevoked => "key_revoked",
            BlacklistReason::CommunityReport => "community_report",
            BlacklistReason::RuntimeAbuse => "runtime_abuse",
        }
    }
}

/// 黑名单条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlacklistEntry {
    /// 插件名（精确匹配）。
    pub plugin_name: String,
    /// 模块 SHA-256（可选，精确匹配）。
    pub module_sha256: Option<String>,
    /// 原因。
    pub reason: BlacklistReason,
    /// 加入时间（Unix 秒）。
    pub blacklisted_at: u64,
    /// 证据（CID 或证据描述/报告位置）。
    pub evidence: String,
    /// 申诉记录（可空）。
    #[serde(default)]
    pub appeal: Option<Appeal>,
}

/// 申诉记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appeal {
    /// 申诉说明。
    pub note: String,
    /// 申诉时间（Unix 秒）。
    pub filed_at: u64,
    /// 修复后的新版本/模块摘要。
    pub replacement_module_sha256: Option<String>,
}

/// 黑名单库。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blacklist {
    entries: Vec<BlacklistEntry>,
}

impl Blacklist {
    /// 空黑名单。
    pub fn new() -> Self {
        Blacklist {
            entries: Vec::new(),
        }
    }

    /// 加入黑名单（幂等：同名 + 同模块摘要不重复添加）。
    pub fn add(&mut self, entry: BlacklistEntry) {
        let dup = self
            .entries
            .iter()
            .any(|e| e.plugin_name == entry.plugin_name && e.module_sha256 == entry.module_sha256);
        if !dup {
            self.entries.push(entry);
        }
    }

    /// 精确匹配：名称命中，或模块摘要命中（任一即视为黑名单）。
    pub fn matches(&self, name: &str, module_sha256: &str) -> Option<&BlacklistEntry> {
        self.entries.iter().find(|e| {
            e.plugin_name == name
                || e.module_sha256.as_deref() == Some(module_sha256) && !module_sha256.is_empty()
        })
    }

    /// 是否黑名单（便捷布尔）。
    pub fn is_blacklisted(&self, name: &str, module_sha256: &str) -> bool {
        self.matches(name, module_sha256).is_some()
    }

    /// 记录申诉（不解除黑名单）。
    pub fn file_appeal(&mut self, name: &str, module_sha256: &str, appeal: Appeal) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| {
            e.plugin_name == name
                || e.module_sha256.as_deref() == Some(module_sha256) && !module_sha256.is_empty()
        }) {
            entry.appeal = Some(appeal);
            true
        } else {
            false
        }
    }

    /// 解封：仅当提供了「新模块摘要」且它不等于被拉黑的模块摘要时，
    /// 移除对应条目（表示新版本通过完整审核）。
    ///
    /// 返回是否成功解封。
    pub fn unblock_with_new_module(&mut self, name: &str, new_module_sha256: &str) -> bool {
        let pos = self.entries.iter().position(|e| {
            e.plugin_name == name && e.module_sha256.as_deref() != Some(new_module_sha256)
        });
        if let Some(i) = pos {
            self.entries.remove(i);
            true
        } else {
            false
        }
    }

    /// 全部条目（只读，供审计/查询）。
    pub fn entries(&self) -> &[BlacklistEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, digest: Option<&str>) -> BlacklistEntry {
        BlacklistEntry {
            plugin_name: name.to_string(),
            module_sha256: digest.map(|s| s.to_string()),
            reason: BlacklistReason::Malware,
            blacklisted_at: 1000,
            evidence: "report-1".to_string(),
            appeal: None,
        }
    }

    #[test]
    fn matches_by_name() {
        let mut bl = Blacklist::new();
        bl.add(entry("com.evil.x", None));
        assert!(bl.is_blacklisted("com.evil.x", ""));
        assert!(!bl.is_blacklisted("com.evil.y", ""));
    }

    #[test]
    fn matches_by_module_digest_after_rename() {
        let mut bl = Blacklist::new();
        bl.add(entry("com.evil.x", Some("deadbeef")));
        // 改了名，但同一模块摘要仍命中。
        assert!(bl.is_blacklisted("com.evil.renamed", "deadbeef"));
    }

    #[test]
    fn idempotent_add() {
        let mut bl = Blacklist::new();
        bl.add(entry("com.evil.x", None));
        bl.add(entry("com.evil.x", None));
        assert_eq!(bl.entries().len(), 1);
    }

    #[test]
    fn appeal_does_not_unblock() {
        let mut bl = Blacklist::new();
        bl.add(entry("com.evil.x", Some("old")));
        let ok = bl.file_appeal(
            "com.evil.x",
            "old",
            Appeal {
                note: "fixed".to_string(),
                filed_at: 1100,
                replacement_module_sha256: Some("new".to_string()),
            },
        );
        assert!(ok);
        // 申诉后仍在黑名单。
        assert!(bl.is_blacklisted("com.evil.x", "old"));
    }

    #[test]
    fn unblock_only_with_new_module() {
        let mut bl = Blacklist::new();
        bl.add(entry("com.evil.x", Some("old")));
        // 用同一旧摘要不能解封。
        assert!(!bl.unblock_with_new_module("com.evil.x", "old"));
        // 用新摘要可解封。
        assert!(bl.unblock_with_new_module("com.evil.x", "new"));
        assert!(!bl.is_blacklisted("com.evil.x", "old"));
    }
}
