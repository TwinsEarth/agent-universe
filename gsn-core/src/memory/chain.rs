//! v5.0.9：记忆完整性哈希链验证。
//!
//! `PersistentMemoryStore::verify_chain` 对当前文件与全部滚动文件逐文件验证
//! （每文件独立成链：首条 `prev_hash=None`，轮转产生新链段）：
//! - 条目自洽：`entry.hash == SHA-256(canonical(entry) || "|" || entry.prev_hash)`；
//! - 链衔接：非首条 `entry.prev_hash == 前一条.hash`；
//! - 旧格式（v5.0.9 之前无 `hash` 字段）条目计为 `unsigned_entries`，跳过自洽校验
//!   （其 `prev_hash` 必为 `None`，不破坏后续链）；
//! - 损坏行（JSON 解析失败）计为 `corrupt_lines`：一旦存在，该文件链完整性
//!   无法证实，`ok=false` 且 `broken_at` 指向首个损坏行。

use super::persistent::MemoryEntry;
use crate::MemoryError;
use std::path::Path;

/// 单文件哈希链验证报告。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChainFileReport {
    /// 文件路径（展示用）。
    pub path: String,
    /// 该文件成功解析的记忆条目数。
    pub entries: usize,
    /// 无 `hash` 字段的旧格式条目数（跳过自洽校验）。
    pub unsigned_entries: usize,
    /// JSON 解析失败的行数。
    pub corrupt_lines: usize,
    /// 首个链断点（0 起）；`None` 表示未发现断点。
    pub broken_at: Option<usize>,
    /// 是否通过：无损坏行、无链断点、且 `entries>0` 或文件为空（空文件视为 ok）。
    pub ok: bool,
}

/// 验证单个 JSONL 记忆文件。
pub fn verify_chain_file(path: &Path) -> Result<ChainFileReport, MemoryError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ChainFileReport {
                path: path.display().to_string(),
                entries: 0,
                unsigned_entries: 0,
                corrupt_lines: 0,
                broken_at: None,
                ok: true,
            });
        }
        Err(e) => return Err(MemoryError::Io(e.to_string())),
    };
    let mut report = ChainFileReport {
        path: path.display().to_string(),
        entries: 0,
        unsigned_entries: 0,
        corrupt_lines: 0,
        broken_at: None,
        ok: true,
    };
    let mut prev_hash: Option<String> = None;
    for (i, line) in content.lines().enumerate() {
        let entry: MemoryEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => {
                report.corrupt_lines += 1;
                if report.broken_at.is_none() {
                    report.broken_at = Some(i);
                    report.ok = false;
                }
                // 损坏行之后链无法证实：持续标记（首断点已记录）。
                prev_hash = None;
                continue;
            }
        };
        report.entries += 1;
        // 链衔接：非首条必须指向前一条的 hash（前一条存在且已签名时）。
        if let Some(p) = &prev_hash {
            if entry.prev_hash.as_deref() != Some(p.as_str()) {
                report.broken_at = Some(i);
                report.ok = false;
            }
        } else if report.corrupt_lines == 0 && report.entries > 1 {
            // 前一条存在但无 hash（旧格式）且本条带 prev_hash：衔接断裂。
            if entry.prev_hash.is_some() {
                report.broken_at = Some(i);
                report.ok = false;
            }
        }
        // 自洽校验（仅已签名条目）。
        if let Some(h) = &entry.hash {
            let expected = match entry.compute_hash() {
                Ok(e) => Some(e),
                Err(_) => {
                    report.broken_at = Some(i);
                    report.ok = false;
                    None
                }
            };
            if let Some(exp) = expected {
                if exp != *h {
                    report.broken_at = Some(i);
                    report.ok = false;
                }
                prev_hash = Some(h.clone());
            }
        } else {
            report.unsigned_entries += 1;
            prev_hash = None;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PersistentMemoryStore;
    use std::io::Write;

    fn temp_store(dir: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("au-chain-{dir}-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn entry(tid: &str, gate: &str) -> MemoryEntry {
        MemoryEntry::new(tid).gate(gate)
    }

    #[test]
    fn appended_entries_form_valid_chain() {
        let p = temp_store("valid");
        let store = PersistentMemoryStore::open(&p);
        store.append(entry("t", "ci")).unwrap();
        store.append(entry("t", "test")).unwrap();
        store.append(entry("t2", "ci")).unwrap();
        let reports = store.verify_chain().unwrap();
        assert_eq!(reports.len(), 1);
        assert!(reports[0].ok, "{reports:?}");
        assert_eq!(reports[0].entries, 3);
        assert_eq!(reports[0].unsigned_entries, 0);
        assert_eq!(reports[0].corrupt_lines, 0);
        // 逐条核对 prev 衔接
        let all = store.load_all().unwrap();
        assert!(all[0].prev_hash.is_none());
        assert_eq!(all[1].prev_hash.as_deref(), all[0].hash.as_deref());
        assert_eq!(all[2].prev_hash.as_deref(), all[1].hash.as_deref());
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn tamper_detected() {
        let p = temp_store("tamper");
        let store = PersistentMemoryStore::open(&p);
        store.append(entry("t", "ci")).unwrap();
        store.append(entry("t", "test")).unwrap();
        // 篡改第一条内容（改 gate），不动 hash：链校验必须报断点。
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        let content = std::fs::read_to_string(&p).unwrap();
        let first = content.lines().next().unwrap().to_string();
        let tampered = first.replace("\"gate\":\"ci\"", "\"gate\":\"CI\"");
        f.write_all(b"\n").unwrap();
        drop(f);
        let mut new_content = content.replace(&first, &tampered);
        new_content.pop(); // 去掉追加的空行
        std::fs::write(&p, new_content).unwrap();
        let reports = store.verify_chain().unwrap();
        assert!(!reports[0].ok);
        assert!(reports[0].broken_at.is_some(), "{reports:?}");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn corrupt_line_marks_unverifiable() {
        let p = temp_store("corrupt");
        let store = PersistentMemoryStore::open(&p);
        store.append(entry("t", "ci")).unwrap();
        let mut content = std::fs::read_to_string(&p).unwrap();
        content.push_str("{not-json}\n");
        std::fs::write(&p, content).unwrap();
        let reports = store.verify_chain().unwrap();
        assert!(!reports[0].ok);
        assert_eq!(reports[0].corrupt_lines, 1);
        assert_eq!(reports[0].broken_at, Some(1));
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn missing_file_is_ok_empty() {
        let p = temp_store("missing");
        let store = PersistentMemoryStore::open(&p);
        let reports = store.verify_chain().unwrap();
        assert_eq!(reports.len(), 1);
        assert!(reports[0].ok);
        assert_eq!(reports[0].entries, 0);
    }

    #[test]
    fn old_format_unsigned_entries_skipped() {
        let p = temp_store("oldfmt");
        // 手工写一条 v5.0.8 旧格式（无 hash/prev_hash 字段）。
        std::fs::write(
            &p,
            "{\"at\":\"2026-10-15T00:00:00Z\",\"task_id\":\"t\",\"gate\":\"ci\"}\n",
        )
        .unwrap();
        let store = PersistentMemoryStore::open(&p);
        // 在其后追加一条新格式：prev_hash 应为 None（旧格式无 hash），自身成链。
        store.append(entry("t", "test")).unwrap();
        let reports = store.verify_chain().unwrap();
        assert!(reports[0].ok, "{reports:?}");
        assert_eq!(reports[0].unsigned_entries, 1);
        std::fs::remove_file(&p).ok();
    }
}
