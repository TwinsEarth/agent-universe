//! 先锋队员 & 贡献者白名单与本机更新通道判定。
//!
//! 白名单以仓库内置文件 `config/pioneers.txt` 为权威来源，编译期嵌入
//! （`include_str!`），离线可判定，更新走 PR；另允许操作员用
//! `GSN_PIONEERS_FILE` 追加一份本地名单。匹配键是本机稳定的 libp2p Peer ID。
//!
//! 注意：白名单只决定**本机自动更新的激进程度**（minor 基线 vs 最新补丁），
//! 不授予任何网络/经济特权，因此「自称先锋」并不构成安全越权；操作员仍可用
//! `GSN_UPDATE_TRACK` 显式指定通道。

use crate::net::peer::load_or_create_identity;

use super::policy::Track;
use super::UpdateError;

/// 编译期嵌入的仓库内置白名单文本。
const EMBEDDED_PIONEERS: &str = include_str!("../../../config/pioneers.txt");

/// 解析名单文本为去空白、去注释的 Peer ID 集合（保留重复无意义，调用方自行去重判断）。
pub fn parse_entries(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// 读取额外本地名单文件（GSN_PIONEERS_FILE）。文件未设置/不存在 => 空名单；
/// 显式设置了却读取失败 => 返回错误（显式配置不应被静默吞成空表）。
pub fn load_extra_entries() -> Result<Vec<String>, UpdateError> {
    match std::env::var("GSN_PIONEERS_FILE") {
        Ok(path) if !path.trim().is_empty() => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| UpdateError::IoPath(path.clone(), e.kind()))?;
            Ok(parse_entries(&text))
        }
        _ => Ok(Vec::new()),
    }
}

/// 计算本机 libp2p Peer ID（base58）。身份密钥持久化在 `~/.gsn/identity.key`。
pub fn local_peer_id() -> Result<String, UpdateError> {
    let kp = load_or_create_identity()
        .map_err(|e| UpdateError::Identity(format!("加载本地身份失败: {e:?}")))?;
    Ok(libp2p::identity::PeerId::from(kp.public()).to_string())
}

/// 判断给定 Peer ID 是否在白名单（内置名单 ∪ 额外名单）。
pub fn is_pioneer(peer_id: &str) -> Result<bool, UpdateError> {
    let target = peer_id.trim();
    if parse_entries(EMBEDDED_PIONEERS).iter().any(|e| e == target) {
        return Ok(true);
    }
    Ok(load_extra_entries()?.iter().any(|e| e == target))
}

/// 解析本机自动更新通道。
///
/// 优先级：显式参数 > `GSN_UPDATE_TRACK` > 白名单(Patch) > 默认(Minor)。
/// 无法识别的 `GSN_UPDATE_TRACK` 值返回错误（typo 绝不能静默选中更激进通道）。
pub fn resolve_track(explicit: Option<Track>) -> Result<Track, UpdateError> {
    if let Some(t) = explicit {
        return Ok(t);
    }
    match std::env::var("GSN_UPDATE_TRACK") {
        Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "patch" => Ok(Track::Patch),
            "minor" => Ok(Track::Minor),
            other => Err(UpdateError::BadTrack(other.to_string())),
        },
        Err(_) => {
            let pid = local_peer_id()?;
            if is_pioneer(&pid)? {
                Ok(Track::Patch)
            } else {
                Ok(Track::Minor)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_ignoring_blank_and_comments() {
        let text = "# header\n\n  12D3abc  \n# comment\n12D3def\n";
        assert_eq!(parse_entries(text), vec!["12D3abc", "12D3def"]);
    }

    #[test]
    fn embedded_roster_is_present() {
        // 内置名单文件必须随二进制编译进来；哪怕当前为空名单，头部注释应存在。
        assert!(EMBEDDED_PIONEERS.contains("先锋队员"));
        assert!(parse_entries(EMBEDDED_PIONEERS)
            .iter()
            .all(|e| !e.starts_with('#')));
    }

    #[test]
    fn explicit_track_wins_and_typo_is_rejected() {
        std::env::remove_var("GSN_UPDATE_TRACK");
        assert_eq!(resolve_track(Some(Track::Patch)).unwrap(), Track::Patch);
        std::env::set_var("GSN_UPDATE_TRACK", "MINOR");
        assert_eq!(resolve_track(None).unwrap(), Track::Minor);
        std::env::set_var("GSN_UPDATE_TRACK", "pattch");
        assert!(matches!(resolve_track(None), Err(UpdateError::BadTrack(_))));
        std::env::remove_var("GSN_UPDATE_TRACK");
    }
}
