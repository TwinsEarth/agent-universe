//! v2.7.9: 沙箱安全边界测试
//!
//! 网络策略落地（默认拒绝/白名单/裸IP/通配）、审计日志（append/过滤/落盘）、
//! 权限检查（scope）、证据分级（门槛）。

use gsn_core::sandbox::config::NetworkPolicy;
use gsn_core::sandbox::error::SandboxError;
use gsn_core::sandbox::security::{
    audit_entry, is_raw_ip, AuditLog, EvidenceGrade, NetworkGuard, PermissionChecker,
};

fn policy_with_egress(list: &[&str]) -> NetworkPolicy {
    NetworkPolicy {
        allow_egress: true,
        egress_allowlist: list.iter().map(|s| s.to_string()).collect(),
        allow_ingress: false,
        deny_raw_ip: false,
    }
}

// ---------- 网络 ----------

#[test]
fn v279_egress_denied_by_default() {
    let p = NetworkPolicy::default();
    let g = NetworkGuard::new(&p);
    assert!(matches!(
        g.check_egress("api.example.com", 443),
        Err(SandboxError::NetworkDenied(_))
    ));
}

#[test]
fn v279_egress_allowed_when_whitelisted() {
    let p = policy_with_egress(&["api.example.com:443"]);
    NetworkGuard::new(&p)
        .check_egress("api.example.com", 443)
        .unwrap();
}

#[test]
fn v279_egress_denied_for_non_whitelisted() {
    let p = policy_with_egress(&["api.example.com:443"]);
    let g = NetworkGuard::new(&p);
    assert!(g.check_egress("evil.com", 443).is_err());
    // 端口不对也拒绝
    assert!(g.check_egress("api.example.com", 80).is_err());
}

#[test]
fn v279_egress_wildcard_port() {
    let p = policy_with_egress(&["api.example.com:*"]);
    NetworkGuard::new(&p)
        .check_egress("api.example.com", 8080)
        .unwrap();
}

#[test]
fn v279_egress_wildcard_subdomain() {
    let p = policy_with_egress(&["*.example.com:443"]);
    NetworkGuard::new(&p)
        .check_egress("sub.example.com", 443)
        .unwrap();
    assert!(NetworkGuard::new(&p).check_egress("other.net", 443).is_err());
}

#[test]
fn v279_deny_raw_ip() {
    let p = NetworkPolicy {
        allow_egress: true,
        egress_allowlist: vec!["*:443".to_string()],
        allow_ingress: false,
        deny_raw_ip: true,
    };
    let g = NetworkGuard::new(&p);
    assert!(g.check_egress("1.2.3.4", 443).is_err());
    // 域名放行
    g.check_egress("example.com", 443).unwrap();
}

#[test]
fn v279_raw_ip_detection() {
    assert!(is_raw_ip("1.2.3.4"));
    assert!(is_raw_ip("[2001:db8::1]"));
    assert!(!is_raw_ip("example.com"));
}

// ---------- 证据分级 ----------

#[test]
fn v279_evidence_grade_ordering() {
    assert!(EvidenceGrade::Verified.meets(EvidenceGrade::Unverified));
    assert!(!EvidenceGrade::Unverifiable.meets(EvidenceGrade::Verified));
    assert_eq!(EvidenceGrade::Verified.label(), "verified");
}

// ---------- 审计 ----------

#[test]
fn v279_audit_log_append_and_filter() {
    let mut log = AuditLog::new(None);
    log.append(audit_entry(
        "sb1", "did:x", "exec", "main.py", "ok", EvidenceGrade::Verified,
    ))
    .unwrap();
    log.append(audit_entry(
        "sb2", "did:y", "exec", "main.js", "ok", EvidenceGrade::Unverified,
    ))
    .unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log.for_sandbox("sb1").len(), 1);
    assert_eq!(log.for_sandbox("sb1")[0].target, "main.py");
}

#[test]
fn v279_audit_log_persists_to_file() {
    let path = std::env::temp_dir().join("au-audit-v279.jsonl");
    let _ = std::fs::remove_file(&path);
    {
        let mut log = AuditLog::new(Some(path.clone()));
        log.append(audit_entry(
            "sb1", "did:x", "exec", "main.py", "ok", EvidenceGrade::Verified,
        ))
        .unwrap();
    }
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("main.py"));
    assert!(content.contains("verified"));
}

// ---------- 权限 ----------

#[test]
fn v279_permission_check_allows_with_scope() {
    let held = vec!["files:read".to_string(), "mcp:call".to_string()];
    PermissionChecker::check(&held, "files:read").unwrap();
    assert!(PermissionChecker::check(&held, "admin").is_err());
}

#[test]
fn v279_permission_wildcard_scope() {
    let held = vec!["*".to_string()];
    PermissionChecker::check(&held, "anything").unwrap();
}

#[test]
fn v279_permission_missing_denied() {
    let held: Vec<String> = vec![];
    assert!(matches!(
        PermissionChecker::check(&held, "files:write"),
        Err(SandboxError::IsolationViolation(_))
    ));
}
