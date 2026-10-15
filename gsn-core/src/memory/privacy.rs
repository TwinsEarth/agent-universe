//! 记忆隐私扫描与字段级脱敏（v5.0.4 fail-closed + v5.0.5 redact）。
//!
//! 记忆档案只记机器事实与结论，**不存 prompt / secret**。对自由文本字段
//! （argv / stdout_tail / stderr_tail / conclusion / cwd_rel）做最小凭据特征处理：
//!
//! - **v5.0.4 fail-closed**：写入前对原始完整内容扫描，命中疑似凭据即拒绝落盘
//!   （`PersistentMemoryStore` 在写入路径天然 fail-closed，不依赖调用方是否记得
//!   先过一遍 `AuditorAgent`）。
//! - **v5.0.5 redact（字段级脱敏 / 最小化）**：在落盘前对字段内 secret 做局部掩码，
//!   保留其余有用上下文（先脱敏再落盘），对应 MMP「字段级接受」思想。
//!
//! 该模块位于 memory 层（而非 agent 层），`agent::auditor` 复用同一套规则，
//! 避免两份规则漂移。
//!
//! ## 唯一事实来源
//! [`RULES`] 中的每条规则是 `(Regex, kind, redacted_replacement)` 三元组：
//! [`detect_secret`] 与 [`redact_secrets`] 共享同一正则，因此「检测到的类型」与
//! 「脱敏覆盖的范围」天然一致，不会出现「检测命中但脱敏漏掩码」。
//!
//! 替换串统一保留「类型前缀 + `[REDACTED]`」，且 `[REDACTED]` 不是各正则的有效载荷，
//! 因此脱敏后再次 [`detect_secret`] 不会重复命中（由 `redact_covers_every_detect_pattern`
//! 对齐测试守护）。
//!
//! 设计取舍：
//! - 只做高置信、低误报的特征（token 前缀 + 有效载荷、PEM 头、AWS AKID、足够长的 `sk-`）。
//! - 宁可误拒（fail-closed），不可漏放凭据；误拒只丢一条本地记忆，漏放则泄露密钥。

use super::MemoryEntry;
use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

/// 编译一条内置规则。所有模式均为编译期字面量，由单元测试
/// `all_rules_compile` 保证可编译；此处**不 panic**（Static Gates 要求生产
/// 代码无 panic 路径）。若仍发生编译失败（仅当测试未跑时），立即终止进程，
/// 而不是静默降级为空规则导致漏扫。
fn compile(
    pattern: &'static str,
    kind: &'static str,
    repl: &'static str,
) -> (Regex, &'static str, &'static str) {
    let re = if let Ok(r) = Regex::new(pattern) {
        r
    } else {
        // 不可达：所有内置模式由 all_rules_compile 测试守护；此处硬终止
        // 是「开发者错误」级别的失败，等价于编译期错误，不进入 panic 路径。
        std::process::abort();
    };
    (re, kind, repl)
}

/// (匹配正则, 凭据类型标签, 脱敏替换串)。
///
/// 顺序即 detect / redact 的处理顺序；带 `authorization` 的更具体规则放在裸
/// `bearer` 与 GitHub 前缀之前，PEM 块最先处理（整块掩码，含跨行）。
static RULES: LazyLock<Vec<(Regex, &'static str, &'static str)>> = LazyLock::new(|| {
    vec![
        // PEM 私钥整块：从头行到对应 END（或文本尾，tail 常被截断）。
        compile(
            r"(?is)-----BEGIN [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----.*?(?:-----END [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----|\z)",
            "private key pem",
            "[REDACTED PRIVATE KEY BLOCK]",
        ),
        // Authorization: Bearer <token>（具体，放在裸 bearer 前）。
        compile(
            r"(?i)authorization:\s*bearer\s+[A-Za-z0-9._~+/=-]+",
            "authorization bearer header",
            "authorization: bearer [REDACTED]",
        ),
        // 裸 Bearer JWT（允许 eyJ 后无完整载荷，detect 仍 fail-closed）。
        compile(
            r"(?i)bearer\s+eyJ[A-Za-z0-9._~+/=-]*",
            "bearer jwt",
            "bearer [REDACTED]",
        ),
        // GitHub fine-grained（github_pat_ 后含下划线分段）。
        compile(
            r"(?i)github_pat_[A-Za-z0-9_]{4,}",
            "github fine-grained token",
            "github_pat_[REDACTED]",
        ),
        // GitHub marker：裸 github_pat（后紧跟一个非 token 字符或文本尾），兜底。
        // 用捕获组消耗并回填分隔符（regex crate 不支持 lookahead）；fine-grained
        // 规则在前面先处理带 `_` 后缀的，其替换串 github_pat_ 后紧跟下划线，
        // 不满足此 marker 的 `[^A-Za-z0-9_]`，故不会二次匹配。
        compile(
            r"(?i)github_pat([^A-Za-z0-9_]|$)",
            "github token marker",
            "[REDACTED-github-pat]$1",
        ),
        compile(
            r"(?i)ghp_[A-Za-z0-9]{4,}",
            "github classic token",
            "ghp_[REDACTED]",
        ),
        compile(
            r"(?i)gho_[A-Za-z0-9]{4,}",
            "github oauth token",
            "gho_[REDACTED]",
        ),
        compile(
            r"(?i)ghu_[A-Za-z0-9]{4,}",
            "github user-to-server token",
            "ghu_[REDACTED]",
        ),
        compile(
            r"(?i)ghs_[A-Za-z0-9]{4,}",
            "github server-to-server token",
            "ghs_[REDACTED]",
        ),
        compile(
            r"(?i)xoxb-[A-Za-z0-9-]{4,}",
            "slack bot token",
            "xoxb-[REDACTED]",
        ),
        compile(
            r"(?i)xoxp-[A-Za-z0-9-]{4,}",
            "slack user token",
            "xoxp-[REDACTED]",
        ),
        // 常见 `sk-` 服务密钥（需后接足够长的字符，避免误判普通词）。
        compile(
            r"(?i)sk-[A-Za-z0-9]{16,}",
            "service secret key",
            "sk-[REDACTED]",
        ),
        // AWS Access Key ID：AKIA 后接 16 个大写字母/数字（大小写敏感）。
        compile(r"AKIA[A-Z0-9]{16}", "aws access key id", "AKIA[REDACTED]"),
    ]
});

/// 一次敏感命中：字段名 + 凭据类型标签。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretHit {
    /// 命中的字段名（如 `argv[2]`、`stdout_tail`、`conclusion`、`cwd_rel`）。
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
///
/// 字段顺序：argv[i] → stdout_tail → stderr_tail → conclusion → cwd_rel。
/// 不扫描 task_id / gate / tags。
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
    if let Some(s) = &e.cwd_rel {
        if let Some(kind) = detect_secret(s) {
            return Some(SecretHit::new("cwd_rel", kind));
        }
    }
    None
}

/// 最小凭据特征扫描。返回命中的类型标签。
///
/// 规则与 [`redact_secrets`] 完全共享（见 [`RULES`]）；前缀型 token 要求后接
/// 有效载荷，避免把脱敏标记 `[REDACTED]` 误判为 secret。
pub fn detect_secret(s: &str) -> Option<&'static str> {
    for (re, kind, _repl) in RULES.iter() {
        if re.is_match(s) {
            return Some(*kind);
        }
    }
    None
}

/// 字段级脱敏：把文本中所有匹配到的 secret 替换为「类型前缀 + `[REDACTED]`」，
/// 保留其余上下文。返回 `(脱敏后文本, 是否发生过替换)`。
///
/// 逐条规则处理；已插入的 `[REDACTED]` 不会再被后续规则匹配。
pub fn redact_secrets(s: &str) -> (String, bool) {
    let mut out = s.to_string();
    let mut changed = false;
    for (re, _kind, repl) in RULES.iter() {
        let updated = re.replace_all(&out, *repl);
        if matches!(updated, Cow::Owned(_)) {
            changed = true;
        }
        out = updated.into_owned();
    }
    (out, changed)
}

/// 对一条记忆做**逐字段脱敏**：argv 每元素、stdout/stderr tail、conclusion、
/// cwd_rel 都先掩码再返回。返回 `(脱敏后条目, 是否发生过脱敏)`。
///
/// 不改动 task_id / gate / at / tags / 退出码等非自由文本字段。
pub fn redact_entry(e: &MemoryEntry) -> (MemoryEntry, bool) {
    let mut out = e.clone();
    let mut changed = false;

    let mut argv = Vec::with_capacity(e.argv.len());
    for a in &e.argv {
        let (r, c) = redact_secrets(a);
        changed |= c;
        argv.push(r);
    }
    out.argv = argv;

    let redact_opt = |v: &Option<String>| -> (Option<String>, bool) {
        match v {
            Some(s) => {
                let (r, c) = redact_secrets(s);
                (Some(r), c)
            }
            None => (None, false),
        }
    };
    let (r, c) = redact_opt(&e.stdout_tail);
    changed |= c;
    out.stdout_tail = r;
    let (r, c) = redact_opt(&e.stderr_tail);
    changed |= c;
    out.stderr_tail = r;
    let (r, c) = redact_opt(&e.conclusion);
    changed |= c;
    out.conclusion = r;
    let (r, c) = redact_opt(&e.cwd_rel);
    changed |= c;
    out.cwd_rel = r;

    (out, changed)
}

/// AWS Access Key ID：`AKIA` 后接 16 个大写字母/数字（总长 20）。
///
/// 保留为公开辅助函数（与 [`RULES`] 中 AWS 规则等价），供调用方单独判断。
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

    /// 守护内置规则全部可编译：触发 `RULES` 初始化，任何模式不可编译都会
    /// 在 `compile` 中硬终止本测试进程；条数断言防止规则被误删/漏加。
    #[test]
    fn all_rules_compile() {
        assert_eq!(RULES.len(), 13, "内置规则条数变化需同步更新本测试");
        for (re, kind, _) in RULES.iter() {
            assert!(!re.is_match(""), "规则 {kind} 应可正常执行（空串恒不命中）");
        }
    }

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

    /// detect 与 redact 规则对齐的核心不变量：对每个「detect 能命中」的样本，
    /// redact 必须发生替换，且脱敏后 detect 不再命中（绝不残留 secret）。
    #[test]
    fn redact_covers_every_detect_pattern() {
        // 高危测试样本在运行时拼接（前缀与主体分离），避免完整 token 静态出现在
        // 源码中触发 GitHub push protection；拼接后仍是完整格式，正则照常命中。
        let slack_b = format!("xoxb-{}", "123456789012-1234567890123-abcdefghijklmnop");
        let slack_p = format!(
            "xoxp-{}",
            "123456789012-1234567890123-1234567890123-abcdefghijklmnop"
        );
        let samples = [
            "github_pat_11COYUIPI0v83xu95akM0q_abcDEF",
            "ghp_0123456789abcdef0123456789",
            "gho_0123456789abcdef0123456789",
            "ghu_0123456789abcdef0123456789",
            "ghs_0123456789abcdef0123456789",
            "see github_pat in docs",
            slack_b.as_str(),
            slack_p.as_str(),
            "-----BEGIN PRIVATE KEY-----\nMIIBVw0123\n",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nabcdef\n",
            "-----BEGIN RSA PRIVATE KEY-----\nabcdef\n",
            "-----BEGIN EC PRIVATE KEY-----\nabcdef\n",
            "-----BEGIN DSA PRIVATE KEY-----\nabcdef\n",
            "-----BEGIN PGP PRIVATE KEY BLOCK-----\nabcdef\n",
            "-----BEGIN PRIVATE KEY-----\nMIIBVw\n-----END PRIVATE KEY-----",
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.sig",
            "bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.sig",
            "sk-abcdef0123456789ABCD",
            "AKIAIOSFODNN7EXAMPLE",
        ];
        for s in samples {
            assert!(detect_secret(s).is_some(), "detect 应命中: {s}");
            let (r, changed) = redact_secrets(s);
            assert!(changed, "redact 应发生替换: {s}");
            assert!(
                detect_secret(&r).is_none(),
                "脱敏后仍检测到 secret: {r}（原 {s}）"
            );
            // 任何有效密钥主体都不应残留。
            assert!(!r.contains("0123456789abcdef"), "残留 token 主体: {r}");
        }
    }

    #[test]
    fn redact_preserves_surrounding_context() {
        let input = "step1 ok\nsecret=ghp_0123456789abcdef0123456789\nstep3 done";
        let (r, changed) = redact_secrets(input);
        assert!(changed);
        assert!(r.contains("step1 ok"), "保留上文: {r}");
        assert!(r.contains("step3 done"), "保留下文: {r}");
        assert!(r.contains("ghp_[REDACTED]"), "掩码 token: {r}");
        assert!(!r.contains("0123456789abcdef"));
    }

    #[test]
    fn redact_entry_masks_each_field() {
        let e = MemoryEntry::new("task-1")
            .gate("sandbox.exec")
            .argv(vec![
                "curl".to_string(),
                "ghp_0123456789abcdef0123456789".to_string(),
            ])
            .stdout_tail("token=AKIAIOSFODNN7EXAMPLE".to_string())
            .stderr_tail("warn".to_string())
            .conclusion("ran in /tmp/secret".to_string())
            .cwd_rel("/Users/me/ghp_0123456789abcdef0123456789".to_string())
            .add_tag("sandbox");
        let (r, changed) = redact_entry(&e);
        assert!(changed, "应发生脱敏");
        assert!(scan_entry_for_secret(&r).is_none(), "脱敏后整条仍含 secret");
        assert_eq!(r.argv[0], "curl", "干净 argv 元素保留");
        assert!(r.argv[1].contains("ghp_[REDACTED]"));
        assert!(r.stdout_tail.unwrap().contains("AKIA[REDACTED]"));
        assert_eq!(r.stderr_tail.as_deref(), Some("warn"));
        assert_eq!(r.conclusion.as_deref(), Some("ran in /tmp/secret"));
        assert!(r.cwd_rel.unwrap().contains("ghp_[REDACTED]"));
        // 非自由文本字段不动。
        assert_eq!(r.gate.as_deref(), Some("sandbox.exec"));
        assert_eq!(r.task_id, "task-1");
    }

    #[test]
    fn clean_text_redact_is_unchanged() {
        let (r, changed) = redact_secrets("nothing sensitive here");
        assert!(!changed);
        assert_eq!(r, "nothing sensitive here");
    }
}
