//! 更新通道（track）与目标版本选择策略。纯逻辑、无网络，便于穷举单测。
//!
//! v3.6.1 语义（用户拍板，贴合插件化版本定义）：
//! - **Minor 通道（默认）**：自动跟到**同一大版本内最新「中版本」基线** `x.Y.0`，
//!   不追小版本（patch）。当前中版本线内出再多补丁都不自动更新——小版本只影响
//!   单个/多个插件，由更激进的通道或人工决定。
//! - **Patch 通道（白名单：先锋队员 & 贡献者）**：自动跟到同一大版本内**最新小版本**
//!   `x.Y.Z`。
//! - **手动**：可升级到任意版本，**包括跨大版本**。
//! - **任何自动通道都绝不跨大版本**：存在更高 major 时只在结果里提示，不自动选。

use serde_json::Value;

use super::semver::SemVer;
use super::UpdateError;

/// 自动更新通道。
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Track {
    /// 默认：跟最新中版本基线（x.Y.0）。
    Minor,
    /// 白名单：跟最新小版本（x.Y.Z）。
    Patch,
}

/// 一次更新检查的结论。
#[derive(Clone, Debug)]
pub struct Decision {
    /// 按当前通道应自动更新到的目标；None 表示无需/不应自动更新。
    pub target: Option<SemVer>,
    /// 存在更高大版本（自动通道不会跨过去，仅供提示手动升级）。
    pub newer_major: Option<SemVer>,
    /// 参与评估的同大版本候选数量。
    pub considered: usize,
}

/// 从 npm packument（registry 文档 JSON）中按通道选目标。
///
/// `packument` 需含对象型 `versions`（键为版本字符串）。预发布版本因
/// [`SemVer::parse`] 不接受 `-` 元数据而天然被排除。
pub fn select_target(
    packument: &Value,
    current: &SemVer,
    track: Track,
) -> Result<Decision, UpdateError> {
    let versions_obj = packument
        .get("versions")
        .and_then(Value::as_object)
        .ok_or_else(|| UpdateError::Registry("packument 缺少 versions 对象".to_string()))?;

    let mut same_major: Vec<SemVer> = Vec::new();
    let mut newer_major: Option<SemVer> = None;
    for key in versions_obj.keys() {
        // 只接受稳定的 X.Y.Z；预发布/畸形键被忽略（不污染选择）。
        let Ok(v) = SemVer::parse(key) else {
            continue;
        };
        match v.major.cmp(&current.major) {
            std::cmp::Ordering::Equal => same_major.push(v),
            std::cmp::Ordering::Greater => {
                newer_major = Some(match newer_major {
                    Some(prev) if prev.is_newer_than(&v) => prev,
                    _ => v,
                });
            }
            std::cmp::Ordering::Less => {}
        }
    }
    let considered = same_major.len();
    let candidates: Vec<&SemVer> = same_major
        .iter()
        .filter(|v| v.is_newer_than(current))
        .collect();

    let target = match track {
        Track::Patch => candidates.iter().max().copied().cloned(),
        Track::Minor => {
            // 最新中版本号 Y（在比当前更新的候选里取最大 minor）。
            let latest_minor = candidates.iter().map(|v| v.minor).max();
            match latest_minor {
                // 最新 minor 仍是当前中版本线 => 默认通道不追补丁 => 不更新。
                Some(y) if y == current.minor => None,
                Some(y) => {
                    // 优先落到该中版本线的基线 x.Y.0；若该线未发布 .0（异常但容忍），
                    // 退而取该线上最高补丁，保证总有可达目标。
                    let baseline = SemVer {
                        major: current.major,
                        minor: y,
                        patch: 0,
                    };
                    if same_major.contains(&baseline) {
                        Some(baseline)
                    } else {
                        candidates
                            .iter()
                            .filter(|v| v.minor == y)
                            .max()
                            .copied()
                            .cloned()
                    }
                }
                None => None,
            }
        }
    };

    Ok(Decision {
        target,
        newer_major,
        considered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(versions: &[&str]) -> Value {
        let mut map = serde_json::Map::new();
        for v in versions {
            map.insert((*v).to_string(), json!({}));
        }
        json!({ "versions": Value::Object(map) })
    }

    #[test]
    fn minor_track_jumps_to_new_minor_baseline_not_patches() {
        let d = doc(&[
            "3.6.0", "3.6.4", "3.7.0", "3.7.2", "3.8.0", "3.8.5", "4.0.0",
        ]);
        let cur = SemVer::parse("3.6.1").unwrap();
        let dec = select_target(&d, &cur, Track::Minor).unwrap();
        // 跳到最新中版本基线 3.8.0，而不是 3.8.5；不跨到 4.0.0。
        assert_eq!(dec.target.unwrap().to_string(), "3.8.0");
        assert_eq!(dec.newer_major.unwrap().to_string(), "4.0.0");
    }

    #[test]
    fn minor_track_does_not_chase_patches_on_current_line() {
        let d = doc(&["3.6.0", "3.6.9"]);
        let cur = SemVer::parse("3.6.0").unwrap();
        let dec = select_target(&d, &cur, Track::Minor).unwrap();
        assert!(dec.target.is_none(), "默认通道不得在当前中版本线追补丁");
    }

    #[test]
    fn patch_track_takes_latest_patch_same_major() {
        let d = doc(&["3.6.0", "3.6.4", "3.6.11", "4.0.0"]);
        let cur = SemVer::parse("3.6.0").unwrap();
        let dec = select_target(&d, &cur, Track::Patch).unwrap();
        assert_eq!(dec.target.unwrap().to_string(), "3.6.11");
        assert_eq!(dec.newer_major.unwrap().to_string(), "4.0.0");
    }

    #[test]
    fn never_auto_crosses_major_when_only_major_newer_exists() {
        let d = doc(&["3.6.0", "4.0.0", "4.1.2"]);
        let cur = SemVer::parse("3.6.0").unwrap();
        for track in [Track::Minor, Track::Patch] {
            let dec = select_target(&d, &cur, track).unwrap();
            assert!(dec.target.is_none(), "{track:?} 不得自动跨大版本");
            assert!(dec.newer_major.is_some());
        }
    }

    #[test]
    fn up_to_date_yields_none() {
        let d = doc(&["3.6.0"]);
        let cur = SemVer::parse("3.6.0").unwrap();
        assert!(select_target(&d, &cur, Track::Patch)
            .unwrap()
            .target
            .is_none());
    }

    #[test]
    fn ignores_prerelease_and_malformed() {
        let d = doc(&["3.6.0", "3.7.0-rc.1", "not-a-version", "3.7.0"]);
        let cur = SemVer::parse("3.6.0").unwrap();
        let dec = select_target(&d, &cur, Track::Minor).unwrap();
        assert_eq!(dec.target.unwrap().to_string(), "3.7.0");
    }

    // 旧实现回归点：若直接读 dist-tags.latest（4.0.0）就自动更新，会跨大版本。
    #[test]
    fn dist_tag_latest_is_not_blindly_trusted() {
        let d = json!({
            "dist-tags": { "latest": "4.0.0" },
            "versions": { "3.6.0": {}, "4.0.0": {} }
        });
        let cur = SemVer::parse("3.6.0").unwrap();
        let dec = select_target(&d, &cur, Track::Patch).unwrap();
        assert!(dec.target.is_none());
        assert_eq!(dec.newer_major.unwrap().to_string(), "4.0.0");
    }
}
