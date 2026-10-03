//! npm registry / GitHub Release 的只读抓取与资产定位。
//!
//! 版本权威源：npm registry 上 `@twinsearth/agent-universe` 的完整 packument
//! （含全部已发布版本，通道选择在 [`super::policy`] 完成；不盲信 `dist-tags.latest`）。
//! daemon 二进制资产位于同名 GitHub Release，按当前平台 triple 命名。

use std::time::Duration;

use serde_json::Value;

use super::https::https_get;
use super::semver::SemVer;
use super::UpdateError;

/// npm 包名（scope 需对 `/` 做百分号编码）。
pub const PACKAGE_NAME: &str = "@twinsearth/agent-universe";
const PACKUMENT_URL: &str = "https://registry.npmjs.org/@twinsearth%2Fagent-universe";
const GITHUB_BASE: &str = "https://github.com/TwinsEarth/agent-universe/releases/download";

/// packument 响应体上限：该包历史很短，2 MiB 足够并能挡住异常放大。
pub const DEFAULT_MAX_PACKUMENT: usize = 2 * 1024 * 1024;
/// daemon 二进制下载上限（512 MiB）。
pub const DEFAULT_MAX_BINARY: usize = 512 * 1024 * 1024;
/// 默认网络超时（连接与每次读）。
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(12);

/// 抓取并解析完整 packument。
pub fn fetch_packument(timeout: Duration, max_bytes: usize) -> Result<Value, UpdateError> {
    let body = https_get(PACKUMENT_URL, timeout, max_bytes)?;
    serde_json::from_slice(&body)
        .map_err(|e| UpdateError::Registry(format!("packument JSON 解析失败: {e}")))
}

/// 当前平台 daemon **原始可执行文件**的资产名。
///
/// 与 `.github/workflows/release.yml` 中 v3.6.1 起额外上传的逐平台原始二进制一一对应
/// （保留原 tar.gz/zip 供人工下载；自动更新走原始二进制以免引入解包依赖）。
/// 只覆盖发布矩阵真实存在的三个目标；其余平台明确报不支持，绝不猜测资产名。
pub fn platform_asset_name() -> Result<&'static str, UpdateError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("gsn-daemon-x86_64-unknown-linux-gnu"),
        ("macos", "aarch64") => Ok("gsn-daemon-aarch64-apple-darwin"),
        ("windows", "x86_64") => Ok("gsn-daemon-x86_64-pc-windows-msvc.exe"),
        (os, arch) => Err(UpdateError::UnsupportedTarget(format!("{os}/{arch}"))),
    }
}

/// 给定目标版本，返回（原始二进制 URL，sha256 摘要 URL）。
pub fn asset_urls(version: &SemVer) -> Result<(String, String), UpdateError> {
    let bin_name = platform_asset_name()?;
    let tag = format!("v{version}");
    let base = format!("{GITHUB_BASE}/{tag}");
    Ok((
        format!("{base}/{bin_name}"),
        format!("{base}/{bin_name}.sha256"),
    ))
}

/// 下载 sha256 清单并解析出其中的小写十六进制摘要（取首个空白分隔字段）。
pub fn fetch_expected_sha256(version: &SemVer, timeout: Duration) -> Result<String, UpdateError> {
    let (_, sha_url) = asset_urls(version)?;
    let body = https_get(&sha_url, timeout, 4096)?;
    let text = String::from_utf8(body)
        .map_err(|_| UpdateError::Checksum("sha256 清单非 UTF-8".to_string()))?;
    let first = text
        .split_whitespace()
        .next()
        .ok_or_else(|| UpdateError::Checksum("sha256 清单为空".to_string()))?;
    if first.len() != 64 || !first.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(UpdateError::Checksum(format!(
            "sha256 清单摘要非法: {first}"
        )));
    }
    Ok(first.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_current_platform_and_builds_urls() {
        let v = SemVer::parse("3.6.1").unwrap();
        let (bin, sha) = asset_urls(&v).unwrap();
        assert!(bin.contains("/download/v3.6.1/gsn-daemon-"));
        assert_eq!(sha, format!("{bin}.sha256"));
        assert!(bin.starts_with("https://github.com/TwinsEarth/"));
    }

    #[test]
    fn parses_sha_line() {
        // 通过 fetch_expected_sha256 的解析逻辑等价验证：取首个空白字段。
        let sample =
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  gsn-daemon-x\n";
        let got = sample
            .split_whitespace()
            .next()
            .unwrap()
            .to_ascii_lowercase();
        assert_eq!(got.len(), 64);
        assert!(got.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
