//! # v3.6.1 自动更新子系统
//!
//! 每次 daemon 启动联网到 **npm registry** 检查版本（版本权威源），并按通道决定
//! 是否自动更新；同时更新 **daemon 二进制**（GitHub Release 资产）与 **npm 包**。
//!
//! ## 通道语义
//! - 默认 **Minor 通道**：自动跟到同一大版本内最新中版本基线 `x.Y.0`，不追补丁；
//! - 白名单（先锋队员 & 贡献者，仓库 `config/pioneers.txt`）**Patch 通道**：
//!   自动跟到最新小版本 `x.Y.Z`；
//! - `gsn update [版本]` 手动更新：任意版本，**允许跨大版本**；
//! - 任何自动通道都**绝不跨大版本**。
//!
//! ## 环境变量
//! - `GSN_AUTO_UPDATE=0|off|false`：只检查提示，不自动安装（默认自动安装）；
//! - `GSN_NO_UPDATE_CHECK=1`：完全不联网检查（离线环境）；
//! - `GSN_UPDATE_TRACK=minor|patch`：显式指定本机通道；
//! - `GSN_PIONEERS_FILE=path`：追加一份本地先锋名单。
//!
//! ## 诚实边界
//! - daemon 二进制安装后在下次启动生效；正在运行的进程不会被强制重启。
//! - npm 为全局安装（`npm i -g`），在无全局写权限/无 npm 的环境会失败并告警，
//!   但不影响 daemon 二进制本身的更新。
//! - Windows 下若 daemon 正在运行，exe 被占用，更新会明确报错并要求先停止节点。

pub mod error;
pub mod https;
pub mod installer;
pub mod policy;
pub mod registry;
pub mod semver;
pub mod whitelist;

pub use error::UpdateError;
pub use policy::{select_target, Decision, Track};
pub use semver::SemVer;

use std::time::Duration;

use registry::{
    asset_urls, fetch_expected_sha256, fetch_packument, DEFAULT_MAX_BINARY, DEFAULT_MAX_PACKUMENT,
    DEFAULT_TIMEOUT,
};

/// 子模块：把安装目标（当前正在运行的可执行文件路径）隔离出来，便于测试与审计。
fn current_executable() -> Result<std::path::PathBuf, UpdateError> {
    std::env::current_exe().map_err(|e| UpdateError::IoPath("<current_exe>".to_string(), e.kind()))
}

/// 一次「检查」的高层结论（供 CLI、daemon 日志与状态端点共用）。
#[derive(Clone, Debug)]
pub enum CheckOutcome {
    /// 已是通道内最新。
    UpToDate {
        current: SemVer,
        newer_major: Option<SemVer>,
    },
    /// 有可更新版本（是否已自动安装见 installed）。
    Available {
        current: SemVer,
        target: SemVer,
        track: Track,
        installed: bool,
        newer_major: Option<SemVer>,
        npm_result: Option<String>,
    },
    /// 检查被关闭（GSN_NO_UPDATE_CHECK=1）。
    CheckDisabled,
    /// 检查失败（网络/解析），不影响启动。
    Failed(String),
}

/// daemon 启动用的配置。
#[derive(Clone, Debug)]
pub struct AutoUpdateConfig {
    /// 是否允许自动安装（关闭则只检查提示）。
    pub auto_install: bool,
    /// 是否一并更新 npm JS 包。
    pub update_npm: bool,
    pub timeout: Duration,
}

impl Default for AutoUpdateConfig {
    fn default() -> Self {
        let auto_install = !matches_flag_off(&std::env::var("GSN_AUTO_UPDATE").unwrap_or_default());
        let update_npm = std::env::var("GSN_UPDATE_NPM").as_deref() != Ok("0");
        Self {
            auto_install,
            update_npm,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

fn matches_flag_off(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "0" | "off" | "false" | "no"
    )
}

/// 是否完全跳过联网检查。
pub fn check_disabled() -> bool {
    std::env::var("GSN_NO_UPDATE_CHECK").as_deref() == Ok("1")
}

/// 只读检查：解析本机通道、抓取 packument、给出通道决策。阻塞，调用方应在
/// `spawn_blocking` 中运行。
pub fn check(track: Option<Track>) -> Result<(SemVer, Decision, Track), UpdateError> {
    let current = SemVer::current()?;
    let resolved = whitelist::resolve_track(track)?;
    let doc = fetch_packument(DEFAULT_TIMEOUT, DEFAULT_MAX_PACKUMENT)?;
    let decision = select_target(&doc, &current, resolved)?;
    Ok((current, decision, resolved))
}

/// 手动/自动执行一次更新到 `target`：下载二进制→sha256 校验→原子替换→npm 更新。
/// 阻塞函数；npm 失败不致命，随返回值报告。
pub fn perform_update(
    target: &SemVer,
    do_npm: bool,
    timeout: Duration,
) -> Result<Option<String>, UpdateError> {
    let (bin_url, _sha_url) = asset_urls(target)?;
    let expected = fetch_expected_sha256(target, timeout)?;
    let bytes = https::https_get(&bin_url, timeout, DEFAULT_MAX_BINARY)?;
    installer::verify_sha256(&bytes, &expected)?;
    let dest = current_executable()?;
    installer::install_binary(&bytes, &dest)?;

    let npm_result = if do_npm {
        match installer::update_npm(target, timeout) {
            Ok(msg) => Some(msg),
            // npm 不可用/权限不足不致命：daemon 二进制已是权威更新。
            Err(e) => Some(format!("npm 更新未完成（不影响 daemon）: {e}")),
        }
    } else {
        None
    };
    Ok(npm_result)
}

/// daemon 启动后台检查的完整流程（阻塞体）。永不 panic，失败只转为 `Failed`。
pub fn run_startup_check(cfg: &AutoUpdateConfig) -> CheckOutcome {
    if check_disabled() {
        return CheckOutcome::CheckDisabled;
    }
    let (current, decision, track) = match check(None) {
        Ok(v) => v,
        Err(e) => return CheckOutcome::Failed(e.to_string()),
    };
    let Some(target) = decision.target else {
        return CheckOutcome::UpToDate {
            current,
            newer_major: decision.newer_major,
        };
    };

    if !cfg.auto_install {
        return CheckOutcome::Available {
            current,
            target,
            track,
            installed: false,
            newer_major: decision.newer_major,
            npm_result: None,
        };
    }

    match perform_update(&target, cfg.update_npm, cfg.timeout) {
        Ok(npm_result) => CheckOutcome::Available {
            current,
            target,
            track,
            installed: true,
            newer_major: decision.newer_major,
            npm_result,
        },
        Err(e) => CheckOutcome::Failed(format!("更新到 {target} 失败: {e}")),
    }
}

/// 人类可读的一行摘要（daemon 横幅 / CLI 输出复用）。
pub fn summarize(o: &CheckOutcome) -> String {
    match o {
        CheckOutcome::UpToDate {
            current,
            newer_major,
        } => match newer_major {
            Some(m) => format!("自动更新：当前 v{current} 已是本大版本最新；另有新大版本 v{m}（不自动跨版本，可用 `gsn update {m}` 手动升级）"),
            None => format!("自动更新：当前 v{current} 已是最新"),
        },
        CheckOutcome::Available {
            current,
            target,
            installed: true,
            npm_result,
            ..
        } => {
            let mut s = format!("自动更新：已安装 v{target}（当前运行 v{current}，重启 daemon 后生效）");
            if let Some(n) = npm_result {
                s.push_str(&format!("；{n}"));
            }
            s
        }
        CheckOutcome::Available {
            current,
            target,
            installed: false,
            ..
        } => format!("自动更新：发现 v{target}（当前 v{current}），自动安装已关闭；可用 `gsn update {target}` 手动更新"),
        CheckOutcome::CheckDisabled => "自动更新：已通过 GSN_NO_UPDATE_CHECK=1 关闭检查".to_string(),
        CheckOutcome::Failed(m) => format!("自动更新：检查失败（不影响启动）：{m}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_off_parsing() {
        for on in ["", "1", "true", "yes", "anything"] {
            assert!(!matches_flag_off(on), "{on:?} 不应判为关闭");
        }
        for off in ["0", "off", "FALSE", " No "] {
            assert!(matches_flag_off(off), "{off:?} 应判为关闭");
        }
    }
}
