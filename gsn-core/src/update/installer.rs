//! 下载产物的校验与原子安装，以及 npm 包更新。
//!
//! 安全要点：
//! - 二进制安装前**必须**通过 sha256 校验（摘要来自发布资产 `<bin>.sha256`，
//!   与二进制同一次 Release、走同一 https）。不匹配即 fail-closed，绝不落地运行。
//! - 先写同目录临时文件、fsync、再原子 rename，避免半成品覆盖正在使用的二进制。
//! - Unix 可直接 rename 覆盖正在运行的可执行文件（旧 inode 继续运行到退出）；
//!   Windows 先把旧文件改名为 `.old` 再放入新文件，旧文件被占用时明确报错，
//!   绝不静默保留旧版本却报告成功。
//! - daemon 二进制安装后**不强制重启**正在运行的节点（避免打断 P2P/结算），
//!   新版本在下次启动生效；`gsn update` 会明确提示。

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::semver::SemVer;
use super::UpdateError;

/// 计算字节的小写十六进制 sha256。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// 校验下载内容：期望摘要（小写 hex）必须与实际逐字节相等。
pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<(), UpdateError> {
    let actual = sha256_hex(bytes);
    let expected = expected_hex.trim().to_ascii_lowercase();
    if actual != expected {
        return Err(UpdateError::ChecksumMismatch { expected, actual });
    }
    Ok(())
}

/// 把已校验的字节原子安装到 `dest`。
pub fn install_binary(bytes: &[u8], dest: &Path) -> Result<(), UpdateError> {
    let parent = dest.parent().ok_or_else(|| {
        UpdateError::IoPath(dest.display().to_string(), std::io::ErrorKind::NotFound)
    })?;
    fs::create_dir_all(parent)
        .map_err(|e| UpdateError::IoPath(parent.display().to_string(), e.kind()))?;

    let tmp = tmp_path(dest);
    // 写入临时文件（覆盖可能残留的半成品）。
    {
        let mut f = File::create(&tmp)
            .map_err(|e| UpdateError::IoPath(tmp.display().to_string(), e.kind()))?;
        f.write_all(bytes)
            .map_err(|e| UpdateError::IoPath(tmp.display().to_string(), e.kind()))?;
        f.sync_all()
            .map_err(|e| UpdateError::IoPath(tmp.display().to_string(), e.kind()))?;
    }
    set_executable(&tmp)?;

    rename_into_place(&tmp, dest)?;

    // Windows：上一版可能留下 dest.old（旧进程退出后才解锁）。尽力清理，
    // 清理失败不影响本次安装正确性（下次更新会再试）。
    #[cfg(windows)]
    {
        let old = old_path(dest);
        let _ = fs::remove_file(old);
    }
    Ok(())
}

/// 与 dest 同目录的临时文件名（跨进程唯一，避免并发更新互相踩踏）。
fn tmp_path(dest: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!(
        ".{}.new-{pid}-{nanos}",
        dest.file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "gsn-daemon".to_string())
    );
    dest.with_file_name(name)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), UpdateError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|e| UpdateError::IoPath(path.display().to_string(), e.kind()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)
        .map_err(|e| UpdateError::IoPath(path.display().to_string(), e.kind()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), UpdateError> {
    Ok(())
}

#[cfg(unix)]
fn rename_into_place(tmp: &Path, dest: &Path) -> Result<(), UpdateError> {
    // Unix：rename 可原子覆盖正在运行的可执行文件（旧 inode 保留到进程退出）。
    fs::rename(tmp, dest).map_err(|e| UpdateError::IoPath(dest.display().to_string(), e.kind()))
}

#[cfg(windows)]
fn old_path(dest: &Path) -> PathBuf {
    let mut o = dest.as_os_str().to_owned();
    o.push(".old");
    PathBuf::from(o)
}

#[cfg(windows)]
fn rename_into_place(tmp: &Path, dest: &Path) -> Result<(), UpdateError> {
    // Windows 不能覆盖正在执行的 exe：先把旧文件改名 .old，再放入新文件。
    if dest.exists() {
        let old = old_path(dest);
        let _ = fs::remove_file(&old);
        if let Err(e) = fs::rename(dest, &old) {
            // 典型：PermissionDenied（旧 daemon 仍在运行，文件被锁）。
            return Err(match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    UpdateError::LockedBinary(dest.display().to_string())
                }
                _ => UpdateError::IoPath(dest.display().to_string(), e.kind()),
            });
        }
    }
    fs::rename(tmp, dest).map_err(|e| UpdateError::IoPath(dest.display().to_string(), e.kind()))
}

// 仅用于在无 dest 父目录时给出 NotFound 类型，避免在签名里散落 io::Error。

/// npm 子命令参数（纯函数，便于单测）。全局安装指定版本的 JS SDK。
pub fn npm_args(version: &SemVer) -> Vec<String> {
    vec![
        "install".to_string(),
        "-g".to_string(),
        "--no-fund".to_string(),
        "--no-audit".to_string(),
        format!("@twinsearth/agent-universe@{version}"),
    ]
}

/// 执行 npm 全局更新。超时后 kill 子进程。npm 缺失/失败返回类型化错误，
/// 调用方据此决定是否仅作为非致命告警（daemon 二进制更新才是权威）。
pub fn update_npm(version: &SemVer, timeout: Duration) -> Result<String, UpdateError> {
    let program = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let args = npm_args(version);
    let mut child = Command::new(program)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| UpdateError::Command(format!("启动 {program} 失败: {e}")))?;

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(format!("npm 包已更新到 {version}"));
                }
                return Err(UpdateError::Command(format!(
                    "{program} {} 退出状态 {status}",
                    args.join(" ")
                )));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(UpdateError::Command(format!(
                        "npm 更新超时（{}s），已终止",
                        timeout.as_secs()
                    )));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(UpdateError::Command(format!("等待 npm 失败: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_accepts_match_and_rejects_tamper() {
        let data = b"hello daemon";
        let good = sha256_hex(data);
        assert!(verify_sha256(data, &good).is_ok());
        let mut tampered = data.to_vec();
        tampered[0] ^= 0xff;
        assert!(matches!(
            verify_sha256(&tampered, &good),
            Err(UpdateError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn install_roundtrip_and_replaces_existing() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("gsn-update-test-{}-{}", std::process::id(), nanos));
        fs::create_dir_all(&dir).unwrap();
        let dest = dir.join(if cfg!(windows) {
            "gsn-daemon.exe"
        } else {
            "gsn-daemon"
        });

        install_binary(b"v1-bytes", &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"v1-bytes");
        // 第二次安装应原子替换，且不残留临时文件。
        install_binary(b"v2-bytes-longer", &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"v2-bytes-longer");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".new-"))
            .collect();
        assert!(leftovers.is_empty(), "残留临时文件: {leftovers:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn npm_args_pin_exact_version() {
        let v = SemVer::parse("3.6.1").unwrap();
        let a = npm_args(&v);
        assert_eq!(a.last().unwrap(), "@twinsearth/agent-universe@3.6.1");
        assert!(a.contains(&"-g".to_string()));
    }
}
