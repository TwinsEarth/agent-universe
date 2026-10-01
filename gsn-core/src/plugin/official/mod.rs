//! 官方插件（T1，Ring 1）—— 由 TwinsEarth 开发、经审计、可热插拔
//!
//! # 有哪些
//!
//! | 插件 id | 职责 | 对应历史能力 |
//! |---|---|---|
//! | [`OFF_AGENT_CARD`] | AgentCard 注册/管理 | v2.1.0 |
//! | [`OFF_AGENT_SKILL`] | Skill 注册/发现 | v2.1.0 |
//! | [`OFF_SWARM_EMERGENCE`] | 群体智能涌现检测 | v2.2.0 |
//! | [`OFF_ECONOMY_REPUTATION`] | 多维信誉 | v2.2.0 |
//! | [`OFF_MARKET_MATCH`] | 市场匹配 | v2.3.0 |
//! | [`OFF_MARKET_SETTLE`] | BFT-lite QA 结算 | v2.3.0 |
//! | [`OFF_SCHEDULER_TASK`] | 任务调度 | v2.3.0 |
//! | [`OFF_CHAIN_ANCHOR`] | 链上锚定 | v2.1.0 |
//! | [`OFF_CHAIN_BRIDGE`] | 跨链信誉桥 | v2.2.0 |
//!
//! T1 以 [`crate::plugin::runtime::process::ProcessRuntime`] 承载（可信代码），
//! 清单显式声明进程后端无法强制的三条边界的 waiver；waiver 被签名、记审计。

use crate::plugin::manifest::{
    Capabilities, ManifestSignature, PluginInfo, PluginLimits, PluginManifest,
};
use std::collections::BTreeMap;

/// AgentCard。
pub const OFF_AGENT_CARD: &str = "com.twinsearth.official.agent-card";
/// Skill。
pub const OFF_AGENT_SKILL: &str = "com.twinsearth.official.agent-skill";
/// 涌现。
pub const OFF_SWARM_EMERGENCE: &str = "com.twinsearth.official.swarm-emergence";
/// 信誉。
pub const OFF_ECONOMY_REPUTATION: &str = "com.twinsearth.official.economy-reputation";
/// 市场匹配。
pub const OFF_MARKET_MATCH: &str = "com.twinsearth.official.market-match";
/// 结算。
pub const OFF_MARKET_SETTLE: &str = "com.twinsearth.official.market-settle";
/// 调度。
pub const OFF_SCHEDULER_TASK: &str = "com.twinsearth.official.scheduler-task";
/// 锚定。
pub const OFF_CHAIN_ANCHOR: &str = "com.twinsearth.official.chain-anchor";
/// 桥。
pub const OFF_CHAIN_BRIDGE: &str = "com.twinsearth.official.chain-bridge";

/// 全部官方插件 id。
pub fn official_ids() -> Vec<&'static str> {
    vec![
        OFF_AGENT_CARD,
        OFF_AGENT_SKILL,
        OFF_SWARM_EMERGENCE,
        OFF_ECONOMY_REPUTATION,
        OFF_MARKET_MATCH,
        OFF_MARKET_SETTLE,
        OFF_SCHEDULER_TASK,
        OFF_CHAIN_ANCHOR,
        OFF_CHAIN_BRIDGE,
    ]
}

/// 进程后端在所有平台都无法强制的边界（T1 可信，显式 waiver）。
fn process_waivers() -> BTreeMap<String, String> {
    let mut w = BTreeMap::new();
    w.insert(
        "fs_deny_host".to_string(),
        "official trusted code; host boundary accepted (safe_join on Rust helpers)".to_string(),
    );
    w.insert(
        "network_egress".to_string(),
        "official trusted code; network is mediated via PMB sys.net, no direct sockets".to_string(),
    );
    w.insert(
        "disk_quota".to_string(),
        "official trusted code; process backend has no FS quota primitive".to_string(),
    );
    // 平台特异的资源边界：与 sandbox::capability::process_declaration 对齐——
    // 该平台 process 后端无法强制的那一条，由 T1 可信构建链路显式接受并记审计。
    // macOS 及其他非 Linux/Windows 的 Unix：内核无 RLIMIT_AS，内存不可强制。
    if cfg!(not(any(target_os = "linux", windows))) {
        w.insert(
            "memory_limit".to_string(),
            "official trusted code; no RLIMIT_AS on this platform, memory is not OS-enforced"
                .to_string(),
        );
    }
    // Windows：Job Object 强制墙钟超时，不强制 CPU 时间。
    if cfg!(windows) {
        w.insert(
            "cpu_limit".to_string(),
            "official trusted code; Windows Job Object enforces wall-clock, not CPU time"
                .to_string(),
        );
    }
    w
}

/// 构造一个 T1 官方插件清单（带进程后端 waiver）。
pub fn official_manifest(id: &str, version: &str) -> PluginManifest {
    PluginManifest {
        plugin: PluginInfo {
            name: id.to_string(),
            version: version.to_string(),
            abi: "3.0".to_string(),
            entry: "process".to_string(),
            publisher: "twinsearth".to_string(),
            module_sha256: String::new(),
        },
        capabilities: Capabilities::default(),
        limits: PluginLimits::default(),
        waivers: process_waivers(),
        signature: ManifestSignature::default(),
    }
}

/// 全部 T1 官方插件清单。
pub fn bundled_manifests(version: &str) -> Vec<PluginManifest> {
    official_ids()
        .into_iter()
        .map(|id| official_manifest(id, version))
        .collect()
}

// ── entry 业务模块（v3.1.0 起官方插件真正承载业务逻辑）──────────────
//
// 历史上这些算法写在单体 marketplace/ 里。v3.1.0 把它们作为插件 entry 模块
// 随插件承载：spawn 时写入隔离工作目录，call 时在隔离进程中加载并调用。

/// 插件 entry 模块（随插件承载的真实业务代码）。
pub struct EntrySource {
    /// 沙箱语言标签（python / javascript）。
    pub language: &'static str,
    /// 模块在工作目录中的文件名。
    pub filename: &'static str,
    /// 模块源码。
    pub source: &'static str,
}

/// economy-reputation 插件 entry：移植自 `marketplace/reputation.rs::overall`。
const REPUTATION_ENTRY: &str = r#"# economy-reputation official plugin (T1)
# ported from marketplace/reputation.rs MarketReputation::overall

def status(_payload):
    return {"plugin": "com.twinsearth.official.economy-reputation",
            "tier": "official", "methods": ["status", "overall"]}

def overall(payload):
    q = float(payload["quality"])
    s = float(payload["speed"])
    h = float(payload["honesty"])
    a = float(payload["availability"])
    score = q*0.35 + s*0.20 + h*0.30 + a*0.15
    return {"overall": score}
"#;

/// market-match 插件 entry：移植自 `marketplace/mod.rs::match_task`。
const MATCH_ENTRY: &str = r#"# market-match official plugin (T1)
# ported from marketplace/mod.rs Market::match_task

def status(_payload):
    return {"plugin": "com.twinsearth.official.market-match",
            "tier": "official", "methods": ["status", "match"]}

def match(payload):
    bids = payload.get("bids", [])
    if not bids:
        return {"winner": None, "reason": "no_bids"}
    best = None
    best_score = None
    for b in bids:
        price = float(b.get("price", 0))
        if price <= 0:
            continue
        rep = float(b.get("reputation", 0.5))
        latency = float(b.get("latency_ms", 0))
        # cost-performance = reputation / price; latency penalty
        cost = rep / price
        latency_penalty = 1.0 / (1.0 + latency / 1000.0)
        score = cost * latency_penalty
        if best_score is None or score > best_score:
            best_score = score
            best = b
    if best is None:
        return {"winner": None, "reason": "no_valid_bid"}
    return {"winner": best.get("agent_id"), "score": best_score}
"#;

/// 返回某官方插件的 entry 业务模块（无则该插件仍是通用 exec 承载）。
pub fn official_entry_source(name: &str) -> Option<EntrySource> {
    match name {
        OFF_ECONOMY_REPUTATION => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: REPUTATION_ENTRY,
        }),
        OFF_MARKET_MATCH => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: MATCH_ENTRY,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::process::ProcessRuntime;
    use crate::plugin::runtime::PluginRuntime;

    #[test]
    fn all_official_manifests_supported_by_process() {
        let rt = ProcessRuntime::new(None);
        let ms = bundled_manifests("3.0.0");
        assert_eq!(ms.len(), 9);
        for m in &ms {
            assert!(
                rt.supports(m).is_ok(),
                "官方插件 {} 应被 process 后端承载",
                m.plugin.name
            );
        }
    }

    #[test]
    fn waivers_present_for_process_limits() {
        let m = official_manifest(OFF_MARKET_MATCH, "3.0.0");
        assert!(m.waivers.contains_key("fs_deny_host"));
        assert!(m.waivers.contains_key("network_egress"));
        assert!(m.waivers.contains_key("disk_quota"));
    }

    #[test]
    fn entry_source_for_business_plugins() {
        assert!(official_entry_source(OFF_ECONOMY_REPUTATION).is_some());
        assert!(official_entry_source(OFF_MARKET_MATCH).is_some());
        // 其余插件暂为通用 exec 承载。
        assert!(official_entry_source(OFF_AGENT_CARD).is_none());
        assert!(official_entry_source(OFF_CHAIN_BRIDGE).is_none());
    }
}
