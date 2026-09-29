//! 沙箱安全边界（v2.7.9）
//!
//! 把 v2.7.6 的策略类型落到可调用的检查：
//! - [`NetworkGuard`]：出站网络策略落地（白名单匹配、裸 IP 拒绝、默认拒绝）；
//! - [`AuditLog`]：关键操作审计（append-only，可落盘）；
//! - [`PermissionChecker`]：Agent 对 scope 的授权检查；
//! - [`EvidenceGrade`]：证据分级标签，随数据/结果流动。

use super::config::NetworkPolicy;
use super::error::SandboxError;
use serde::{Deserialize, Serialize};

// ---------- 证据分级 ----------

/// 证据分级（三级标签，随数据流动）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceGrade {
    /// 未验证/不可信（最低）
    Unverifiable = 0,
    /// 单方声明 / 未独立验证
    Unverified = 1,
    /// 已独立验证 / 多源一致（最高）
    Verified = 2,
}

impl EvidenceGrade {
    pub fn label(&self) -> &'static str {
        match self {
            EvidenceGrade::Unverifiable => "unverifiable",
            EvidenceGrade::Unverified => "unverified",
            EvidenceGrade::Verified => "verified",
        }
    }

    /// 是否达到结算/放行要求的门槛
    pub fn meets(&self, required: EvidenceGrade) -> bool {
        *self >= required
    }
}

// ---------- 网络策略落地 ----------

/// 网络策略检查器
pub struct NetworkGuard<'a> {
    policy: &'a NetworkPolicy,
}

impl<'a> NetworkGuard<'a> {
    pub fn new(policy: &'a NetworkPolicy) -> Self {
        Self { policy }
    }

    /// 检查是否允许向 host:port 发起出站连接
    pub fn check_egress(&self, host: &str, port: u16) -> Result<(), SandboxError> {
        if !self.policy.allow_egress {
            return Err(SandboxError::NetworkDenied(format!(
                "沙箱默认禁止出站网络: {host}:{port}"
            )));
        }
        if self.policy.deny_raw_ip && is_raw_ip(host) {
            return Err(SandboxError::NetworkDenied(format!(
                "策略拒绝裸 IP（要求域名）: {host}"
            )));
        }
        // 白名单：至少一条匹配
        let matched = self
            .policy
            .egress_allowlist
            .iter()
            .any(|entry| hostport_matches(entry, host, port));
        if !matched {
            return Err(SandboxError::NetworkDenied(format!(
                "目标不在出站白名单: {host}:{port}"
            )));
        }
        Ok(())
    }
}

/// 判断 host 是否为裸 IP（v4 或 v6）
pub fn is_raw_ip(host: &str) -> bool {
    let h = host.trim_start_matches('[').trim_end_matches(']');
    h.parse::<std::net::Ipv4Addr>().is_ok() || h.parse::<std::net::Ipv6Addr>().is_ok()
}

/// 白名单条目匹配 host:port，支持 `*` / `:*` 通配
fn hostport_matches(entry: &str, host: &str, port: u16) -> bool {
    let entry = entry.trim();
    if let Some((ehost, eport)) = entry.rsplit_once(':') {
        // 端口段
        let port_ok = eport == "*" || eport.parse::<u16>().map(|p| p == port).unwrap_or(false);
        port_ok && host_matches(ehost, host)
    } else {
        // 只给了 host，任意端口
        host_matches(entry, host)
    }
}

/// host 匹配，支持前缀/后缀通配 `*`
fn host_matches(pattern: &str, host: &str) -> bool {
    let p = pattern.trim();
    if p == "*" {
        return true;
    }
    if p.starts_with("*.") {
        // *.example.com 匹配 sub.example.com 与 example.com
        let suffix = &p[1..]; // .example.com
        return host.ends_with(suffix) || host == &p[2..];
    }
    p == host
}

// ---------- 审计日志 ----------

/// 一条审计记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp_ms: u64,
    pub sandbox_id: String,
    pub agent_did: String,
    pub action: String,
    pub target: String,
    pub outcome: String,
    pub evidence_grade: EvidenceGrade,
}

/// 审计日志（append-only）
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    file: Option<std::path::PathBuf>,
}

impl AuditLog {
    pub fn new(file: Option<std::path::PathBuf>) -> Self {
        Self {
            entries: Vec::new(),
            file,
        }
    }

    /// 追加一条记录（不可删改）；若配置了文件则追加落盘
    pub fn append(&mut self, entry: AuditEntry) -> Result<(), SandboxError> {
        if let Some(path) = &self.file {
            let line = serde_json::to_string(&entry)
                .map_err(|e| SandboxError::Internal(e.to_string()))?;
            use std::io::Write;
            use std::fs::OpenOptions;
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| SandboxError::Internal(e.to_string()))?;
            writeln!(f, "{line}").map_err(|e| SandboxError::Internal(e.to_string()))?;
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 按 sandbox 过滤的记录
    pub fn for_sandbox(&self, sandbox_id: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.sandbox_id == sandbox_id)
            .collect()
    }
}

// ---------- 权限检查 ----------

/// 权限检查器：把 Agent 持有的 scope 与操作所需 scope 比对
pub struct PermissionChecker;

impl PermissionChecker {
    /// 检查 held_scopes 是否包含 required scope（支持 `*` 全权）
    pub fn check(held_scopes: &[String], required: &str) -> Result<(), SandboxError> {
        let ok = held_scopes.iter().any(|s| s == "*" || s == required);
        if ok {
            Ok(())
        } else {
            Err(SandboxError::IsolationViolation(format!(
                "缺少权限 scope: {required}"
            )))
        }
    }
}

// ---------- 审计便利构造 ----------

/// 构造一条审计记录
pub fn audit_entry(
    sandbox_id: &str,
    agent_did: &str,
    action: &str,
    target: &str,
    outcome: &str,
    grade: EvidenceGrade,
) -> AuditEntry {
    AuditEntry {
        timestamp_ms: now_ms(),
        sandbox_id: sandbox_id.to_string(),
        agent_did: agent_did.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        outcome: outcome.to_string(),
        evidence_grade: grade,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
