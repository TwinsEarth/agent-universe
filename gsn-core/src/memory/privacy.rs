//! 记忆隐私扫描（v5.0.4，FLEET §4 fail-closed）。
//!
//! 记忆档案只记机器事实与结论，**不存 prompt / secret**。写入前对自由文本字段
//! （argv / stdout_tail / stderr_tail / conclusion）做最小凭据特征扫描，命中即拒绝落盘。
//!
//! 该模块位于 memory 层（而非 agent 层），使 [`super::PersistentMemoryStore`] 在写入
//! 路径上天然 fail-closed，不依赖调用方是否记得先过一遍 [`AuditorAgent`]；
//! `agent::auditor` 复用同一套特征，避免两份规则漂移。
//!
//! 设计取舍：
//! - 只做高置信、低误报的特征（token 前缀、PEM 头、AWS AKID、足够长的 `sk-`）。
//! - 宁可误拒（fail-closed），不可漏放凭据；误拒只丢一条本地记忆，漏放则泄露密钥。
//!
//! [`AuditorAgent`]: crate::agent::AuditorAgent

use super::MemoryEntry;

/// 一次敏感命中：字段名 + 凭据类型标签。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretHit {
    /// 命中的字段名（如 `argv[2]`、`stdout_tail`、`conclusion`）。
    pub field: String,
    /// 凭据类型标签（如 `github classic token`）。
    pub kind: &'static str,
}

impl SecretHit {
    fn new(field: impl Into<String>, kind: &'static str) -> Self {
        Self {
            field: field.into(),
            kind,
        }
    }
}

/// 扫描一条记忆的全部自由文本字段，返回首个敏感命中。
pub fn scan_entry_for_secret(e: &MemoryEntry) -> Option<SecretHit> {
    for (i, a) in e.argv.iter().enumerate() {
        if let Some(kind) = detect_secret(a) {
            return Some(SecretHit::new(format!("argv[{i}]"), kind));
        }
    }
    if let Some(s) = &e.stdout_tail {
        if let Some(kind) = detect_secret(s) {
            return Some(SecretHit::new("stdout_tail", kind));
        }
    }
    if let Some(s) = &e.stderr_tail {
        if let Some(kind) = detect_secret(s) {
            return Some(SecretHit::new("stderr_tail", kind));
        }
    }
    if let Some(s) = &e.conclusion {
        if let Some(kind) = detect_secret(s) {
            return Some(SecretHit::new("conclusion", kind));
        }
    }
    None
}

/// 最小凭据特征扫描。返回命中的类型标签。
///
/// 大小写不敏感（PEM 头、token 前缀均按小写匹配）。
pub fn detect_secret(s: &str) -> Option<&'static str> {
    /// (小写匹配串, 类型标签)
    const PATTERNS: &[(&str, &str)] = &[
        ("github_pat_", "github fine-grained token"),
        ("ghp_", "github classic token"),
        ("gho_", "github oauth token"),
        ("ghu_", "github user-to-server token"),
        ("ghs_", "github server-to-server token"),
        ("github_pat", "github token marker"),
        ("xoxb-", "slack bot token"),
        ("xoxp-", "slack user token"),
        ("-----begin private key-----", "pkcs8 private key"),
        ("-----begin openssh private key-----", "openssh private key"),
        ("-----begin rsa private key-----", "rsa private key"),
        ("-----begin ec private key-----", "ec private key"),
        ("-----begin dsa private key-----", "dsa private key"),
        ("-----begin pgp private key block-----", "pgp private key"),
        ("bearer eyj", "bearer jwt"),
        ("authorization: bearer ", "authorization bearer header"),
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
pub fn contains_aws_access_key(s: &str) -> bool {
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

    #[test]
    fn clean_entry_has_no_secret() {
        let e = MemoryEntry::new("t").conclusion("verify-join passes on this tree");
        assert!(scan_entry_for_secret(&e).is_none());
    }

    #[test]
    fn detects_in_argv() {
        let e = MemoryEntry::new("t").argv(vec![
            "curl".to_string(),
            "-H".to_string(),
            "Authorization: Bearer ghp_0123456789abcdef0123456789".to_string(),
        ]);
        let hit = scan_entry_for_secret(&e).expect("应命中");
        assert_eq!(hit.field, "argv[2]");
    }

    #[test]
    fn detects_in_stdout_and_conclusion() {
        let e = MemoryEntry::new("t").stdout_tail("token=github_pat_abcdefghij0123456789");
        assert_eq!(
            scan_entry_for_secret(&e).map(|h| h.field),
            Some("stdout_tail".to_string())
        );
        let e2 = MemoryEntry::new("t").conclusion("AKIAIOSFODNN7EXAMPLE leaked");
        assert!(scan_entry_for_secret(&e2).is_some());
    }

    #[test]
    fn short_sk_is_not_flagged() {
        // 不足 16 个字母数字，避免误判普通词。
        assert!(detect_secret("value sk-abc").is_none());
        // 足够长则命中。
        assert!(detect_secret("sk-abcdef0123456789ABCD").is_some());
    }

    #[test]
    fn private_key_headers_detected() {
        for hdr in [
            "-----BEGIN PRIVATE KEY-----",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "-----BEGIN RSA PRIVATE KEY-----",
        ] {
            assert!(detect_secret(hdr).is_some(), "{hdr}");
        }
    }
}
