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
    // Windows：Job Object 强制墙钟超时与内存/进程数，不强制 CPU 时间/句柄数。
    if cfg!(windows) {
        w.insert(
            "cpu_limit".to_string(),
            "official trusted code; Windows Job Object enforces wall-clock, not CPU time"
                .to_string(),
        );
        w.insert(
            "open_file_limit".to_string(),
            "official trusted code; Windows Job Object has no handle-count limit".to_string(),
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

/// market-settle 插件 entry：移植自 `marketplace/settlement.rs::independent_audit`。
const SETTLE_ENTRY: &str = r#"# market-settle official plugin (T1)
# ported from marketplace/settlement.rs SettlementEngine::independent_audit

def status(_payload):
    return {"plugin": "com.twinsearth.official.market-settle",
            "tier": "official", "methods": ["status", "audit"]}

def audit(payload):
    records = payload.get("records", [])
    balances = payload.get("balances", {})
    # 独立重放流水：只信任流水，不信任当前余额/聚合。
    expected = {}
    deposits = 0
    slashed = 0
    for r in records:
        reason = r.get("reason")
        amount = int(r.get("amount", 0))
        frm = r.get("from_account", "")
        to = r.get("to_account", "")
        if reason == "Deposited":
            expected[to] = expected.get(to, 0) + amount
            deposits += amount
        elif reason == "Slashed":
            expected[frm] = expected.get(frm, 0) - amount
            slashed += amount
        else:
            # Completed/Refunded/Staked/Escrowed/Rejected/DuplicateWork: from -> to 搬运
            if amount > 0:
                expected[frm] = expected.get(frm, 0) - amount
                expected[to] = expected.get(to, 0) + amount
    # 逐账户比对（重放账户 ∪ 当前账户），覆盖 ghost/缺失/篡改。
    accounts = set()
    accounts.update(expected.keys())
    accounts.update(balances.keys())
    mismatches = []
    for a in accounts:
        exp = expected.get(a, 0)
        act = int(balances.get(a, 0))
        if exp != act:
            mismatches.append({"account": a, "expected": exp, "actual": act})
    mismatches.sort(key=lambda m: m["account"])
    expected_total = deposits - slashed
    actual_total = 0
    for v in balances.values():
        actual_total += int(v)
    # 若调用方提供引擎聚合（total_deposits/total_slashed），一并校验聚合一致。
    aggregate_matches = True
    if "total_deposits" in payload or "total_slashed" in payload:
        aggregate_matches = (deposits == int(payload.get("total_deposits", deposits))
                             and slashed == int(payload.get("total_slashed", slashed)))
    passed = (not mismatches) and expected_total == actual_total and aggregate_matches
    return {"passed": passed,
            "replayed_records": len(records),
            "replayed_deposits": deposits,
            "replayed_slashed": slashed,
            "expected_total": expected_total,
            "actual_total": actual_total,
            "aggregate_matches": aggregate_matches,
            "mismatches": mismatches}
"#;

/// scheduler-task 插件 entry：移植自 `scheduler/router.rs::assign_task`（边界安全）。
const ROUTER_ENTRY: &str = r#"# scheduler-task official plugin (T1)
# ported from scheduler/router.rs TaskRouter::assign_task

def status(_payload):
    return {"plugin": "com.twinsearth.official.scheduler-task",
            "tier": "official", "methods": ["status", "route"]}

def route(payload):
    nodes = payload.get("nodes", [])
    max_concurrent = int(payload.get("max_concurrent", 1))
    budget = int(payload.get("budget", 0))
    if not nodes:
        return {"selected": None, "reason": "no_candidates"}
    if max_concurrent <= 0:
        # 无容量定义（原 Rust load/max_concurrent 会除零）：显式判为无容量。
        return {"selected": None, "reason": "no_capacity"}
    best = None
    best_score = None
    for n in nodes:
        did = n.get("did")
        load = float(n.get("load", 0))
        if load >= max_concurrent:
            continue
        latency = float(n.get("latency_ms", 100))
        load_penalty = (load / max_concurrent) * 0.3
        latency_penalty = min(latency / 1000.0, 1.0) * 0.2
        score = 1.0 - load_penalty - latency_penalty
        if best_score is None or score > best_score:
            best_score = score
            best = {"did": did, "latency_ms": latency}
    if best is None:
        return {"selected": None, "reason": "all_saturated"}
    # 估算成本：预算在全部候选间平分（len(nodes) 非空，整数除法安全）。
    estimated_cost = budget // len(nodes)
    return {"selected": best["did"], "score": best_score,
            "latency_ms": best["latency_ms"], "estimated_cost": estimated_cost}
"#;

/// agent-card 插件 entry：校验 `MarketAgentCard` 字段（`marketplace/agent_card.rs`）。
const CARD_ENTRY: &str = r#"# agent-card official plugin (T1)
# validates MarketAgentCard fields (marketplace/agent_card.rs)

def status(_payload):
    return {"plugin": "com.twinsearth.official.agent-card",
            "tier": "official", "methods": ["status", "validate"]}

def validate(payload):
    card = payload.get("card", payload)
    errors = []
    agent_id = str(card.get("agent_id", ""))
    if not agent_id:
        errors.append("agent_id is required")
    elif not agent_id.startswith("did:"):
        errors.append("agent_id must be a DID (start with 'did:')")
    if not str(card.get("name", "")):
        errors.append("name is required")
    if not str(card.get("version", "")):
        errors.append("version is required")
    if not isinstance(card.get("skills", []), list):
        errors.append("skills must be a list")
    try:
        rep = float(card.get("reputation_score", 0))
        if rep < 0.0 or rep > 1.0:
            errors.append("reputation_score must be in [0,1]")
    except (TypeError, ValueError):
        errors.append("reputation_score must be a number")
    try:
        sr = float(card.get("success_rate", 0))
        if sr < 0.0 or sr > 1.0:
            errors.append("success_rate must be in [0,1]")
    except (TypeError, ValueError):
        errors.append("success_rate must be a number")
    try:
        stake = int(card.get("stake", 0))
        if stake < 0:
            errors.append("stake must be non-negative")
    except (TypeError, ValueError):
        errors.append("stake must be an integer")
    return {"valid": len(errors) == 0, "errors": errors}
"#;

/// swarm-emergence 插件 entry：移植自 `swarm/emergence.rs::EmergenceDetector::detect`。
///
/// 输入：`history`（每条 `[timestamp, throughput, latency]`）、`threshold`、`window_size`；
/// 输出：最近窗口相对更早窗口的两类涌现信号——
/// 吞吐量增长超阈值 → `collaboration`、延迟下降超阈值 → `load_balancing`。
const EMERGENCE_ENTRY: &str = r#"# swarm-emergence official plugin (T1)
# detects emergent swarm behavior (swarm/emergence.rs EmergenceDetector)

def status(_payload):
    return {"plugin": "com.twinsearth.official.swarm-emergence",
            "tier": "official", "methods": ["status", "detect"]}

def detect(payload):
    history = payload.get("history", [])
    threshold = float(payload.get("threshold", 0.5))
    window = int(payload.get("window_size", 3))
    empty = {"signals": [], "window_size": window}
    if window <= 0 or len(history) < window:
        return empty
    n = len(history)
    # recent = 最后 window 个；older = 之前最多 window 个（可能不足 window，与 Rust 对齐）。
    recent = history[n - window:]
    older = history[max(0, n - window * 2):n - window]
    if not older:
        return empty
    signals = []
    # 吞吐量增长（Collaboration）
    rt = sum(float(r[1]) for r in recent) / len(recent)
    ot = sum(float(r[1]) for r in older) / len(older)
    if ot > 0.0:
        growth = (rt - ot) / ot
        if growth > threshold:
            signals.append({"signal_type": "collaboration", "strength": growth,
                            "description": "吞吐量增长 %.1f%%，检测到协同涌现" % (growth * 100.0)})
    # 延迟下降（LoadBalancing）
    rl = sum(float(r[2]) for r in recent) / len(recent)
    ol = sum(float(r[2]) for r in older) / len(older)
    if ol > 0.0:
        improvement = (ol - rl) / ol
        if improvement > threshold:
            signals.append({"signal_type": "load_balancing", "strength": improvement,
                            "description": "延迟下降 %.1f%%，检测到负载均衡涌现" % (improvement * 100.0)})
    return {"signals": signals, "window_size": window,
            "recent_throughput": rt, "older_throughput": ot,
            "recent_latency": rl, "older_latency": ol}
"#;

/// agent-skill 插件 entry：移植自 `marketplace/mod.rs::discover_by_skill`。
///
/// 输入：`skill`（技能标签）、`agents`（卡片列表，每个含 `agent_id`、`skills`）；
/// 先按卡片声明的技能构建反向索引（对齐 `register_agent` 的 `skill_index` 构建），
/// 再做**精确**（忽略大小写/首尾空白）标签匹配，返回所有声明该技能的卡片。
const SKILL_ENTRY: &str = r#"# agent-skill official plugin (T1)
# discovers agents by skill tag (marketplace/mod.rs discover_by_skill)

def status(_payload):
    return {"plugin": "com.twinsearth.official.agent-skill",
            "tier": "official", "methods": ["status", "discover"]}

def discover(payload):
    skill = payload.get("skill", "")
    agents = payload.get("agents", [])
    target = str(skill).strip().lower()
    result = {"skill": skill, "agents": [], "count": 0}
    if not target:
        return result
    # 构建技能反向索引（对齐 register_agent：skill_index[skill_lower].push(agent_id)）
    index = {}
    for a in agents:
        aid = a.get("agent_id")
        if aid is None:
            aid = a.get("id")
        if aid is None:
            continue
        for s in a.get("skills", []):
            key = str(s).strip().lower()
            if key not in index:
                index[key] = []
            if aid not in index[key]:
                index[key].append(aid)
    # 精确匹配查找（对齐 discover_by_skill：skill_index.get(skill.to_lowercase())）
    ids = index.get(target, [])
    matched = []
    for a in agents:
        aid = a.get("agent_id")
        if aid is None:
            aid = a.get("id")
        if aid in ids:
            matched.append(a)
    result["agents"] = matched
    result["count"] = len(matched)
    return result
"#;

/// chain-anchor 插件 entry：离线移植自 `contracts/src/AgentCardAnchor.sol`（无 RPC，
/// 不声称真实上链）。内嵌纯 Python keccak256（domain 0x01），实现：
/// - `anchor`：仅授权锚定者、双 hash 非空、同 cidHash 首写后不可变（不可覆盖）；
/// - `verify`：已锚定（`anchoredAt>0`）且 agentDidHash 一致（双校验）；
/// - 另有 `get_anchor` 与 owner 管理的 `add_anchorer` / `remove_anchorer`。
///
/// 离线无 `block.timestamp`，`anchoredAt` 由调用方传入单调时间戳。
const ANCHOR_ENTRY: &str = r#"# chain-anchor official plugin (T1)
# offline port of contracts/src/AgentCardAnchor.sol (no RPC; does NOT claim real on-chain)

# ---- pure-python keccak256 (Ethereum; domain byte 0x01) ----
_MASK = (1 << 64) - 1
_RC = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
    0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
    0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008]
_ROT = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14]]


def _rol(x, n):
    if n == 0:
        return x
    return ((x << n) | (x >> (64 - n))) & _MASK


def keccak256(data):
    rate = 136
    msg = bytearray(data)
    msg.append(0x01)
    while len(msg) % rate != 0:
        msg.append(0)
    msg[-1] |= 0x80
    S = [[0] * 5 for _ in range(5)]
    for off in range(0, len(msg), rate):
        for i in range(rate // 8):
            x, y = i % 5, i // 5
            S[x][y] ^= int.from_bytes(msg[off + 8 * i:off + 8 * i + 8], "little")
        for rnd in range(24):
            C = [S[x][0] ^ S[x][1] ^ S[x][2] ^ S[x][3] ^ S[x][4] for x in range(5)]
            D = [C[(x - 1) % 5] ^ _rol(C[(x + 1) % 5], 1) for x in range(5)]
            for x in range(5):
                for y in range(5):
                    S[x][y] ^= D[x]
            B = [[0] * 5 for _ in range(5)]
            for x in range(5):
                for y in range(5):
                    B[y][(2 * x + 3 * y) % 5] = _rol(S[x][y], _ROT[x][y])
            for x in range(5):
                for y in range(5):
                    S[x][y] = B[x][y] ^ ((~B[(x + 1) % 5][y]) & B[(x + 2) % 5][y])
            S[0][0] ^= _RC[rnd]
    out = b""
    for i in range(rate // 8):
        x, y = i % 5, i // 5
        out += S[x][y].to_bytes(8, "little")
    return out[:32]


def _hash_hex(value):
    if isinstance(value, str):
        value = value.encode("utf-8")
    return keccak256(value).hex()


def status(_payload):
    return {"plugin": "com.twinsearth.official.chain-anchor",
            "tier": "official",
            "methods": ["status", "anchor", "verify", "get_anchor",
                        "add_anchorer", "remove_anchorer"]}


def anchor(payload):
    cid = payload.get("cid")
    agent_did = payload.get("agent_did")
    anchorer = payload.get("anchorer", "")
    authorized = list(payload.get("authorized_anchorers", []) or [])
    anchors = dict(payload.get("anchors", {}) or {})
    timestamp = int(payload.get("timestamp", 0))
    if cid is None or str(cid) == "":
        return {"ok": False, "error": "empty cid", "anchors": anchors}
    if agent_did is None or str(agent_did) == "":
        return {"ok": False, "error": "empty did", "anchors": anchors}
    cid_hash = _hash_hex(cid)
    agent_did_hash = _hash_hex(agent_did)
    if anchorer not in authorized:
        return {"ok": False, "error": "not authorized anchorer", "anchors": anchors}
    if cid_hash in anchors:
        return {"ok": False, "error": "anchor already exists; immutable", "anchors": anchors}
    record = {"cidHash": cid_hash, "agentDidHash": agent_did_hash,
              "anchoredAt": timestamp, "anchorer": anchorer}
    anchors[cid_hash] = record
    return {"ok": True, "anchor": record, "anchors": anchors,
            "cidHash": cid_hash, "agentDidHash": agent_did_hash}


def verify(payload):
    cid = payload.get("cid")
    agent_did = payload.get("agent_did")
    anchors = payload.get("anchors", {}) or {}
    if cid is None or str(cid) == "" or agent_did is None or str(agent_did) == "":
        return {"valid": False, "error": "empty cid or did"}
    cid_hash = _hash_hex(cid)
    agent_did_hash = _hash_hex(agent_did)
    a = anchors.get(cid_hash)
    valid = (a is not None and int(a.get("anchoredAt", 0)) > 0
             and a.get("agentDidHash") == agent_did_hash)
    return {"valid": valid, "cidHash": cid_hash, "agentDidHash": agent_did_hash}


def get_anchor(payload):
    cid = payload.get("cid")
    anchors = payload.get("anchors", {}) or {}
    if cid is None or str(cid) == "":
        return {"anchor": None}
    return {"anchor": anchors.get(_hash_hex(cid))}


def add_anchorer(payload):
    actor = payload.get("actor", "")
    owner = payload.get("owner", "")
    target = payload.get("anchorer", "")
    authorized = list(payload.get("authorized_anchorers", []) or [])
    if actor != owner:
        return {"ok": False, "error": "not owner"}
    if target and target not in authorized:
        authorized.append(target)
    return {"ok": True, "authorized_anchorers": authorized}


def remove_anchorer(payload):
    actor = payload.get("actor", "")
    owner = payload.get("owner", "")
    target = payload.get("anchorer", "")
    authorized = list(payload.get("authorized_anchorers", []) or [])
    if actor != owner:
        return {"ok": False, "error": "not owner"}
    if target in authorized:
        authorized.remove(target)
    return {"ok": True, "authorized_anchorers": authorized}
"#;

const BRIDGE_ENTRY: &str = r#"# chain-bridge official plugin (T1)
# offline port of contracts/src/ReputationRegistry.sol (off-chain reputation on-chain anchor)

# ---- pure-python keccak256 (Ethereum; domain byte 0x01) ----
_MASK = (1 << 64) - 1
_RC = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
    0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
    0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008]
_ROT = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14]]


def _rol(x, n):
    if n == 0:
        return x
    return ((x << n) | (x >> (64 - n))) & _MASK


def keccak256(data):
    rate = 136
    msg = bytearray(data)
    msg.append(0x01)
    while len(msg) % rate != 0:
        msg.append(0)
    msg[-1] |= 0x80
    S = [[0] * 5 for _ in range(5)]
    for off in range(0, len(msg), rate):
        for i in range(rate // 8):
            x, y = i % 5, i // 5
            S[x][y] ^= int.from_bytes(msg[off + 8 * i:off + 8 * i + 8], "little")
        for rnd in range(24):
            C = [S[x][0] ^ S[x][1] ^ S[x][2] ^ S[x][3] ^ S[x][4] for x in range(5)]
            D = [C[(x - 1) % 5] ^ _rol(C[(x + 1) % 5], 1) for x in range(5)]
            for x in range(5):
                for y in range(5):
                    S[x][y] ^= D[x]
            B = [[0] * 5 for _ in range(5)]
            for x in range(5):
                for y in range(5):
                    B[y][(2 * x + 3 * y) % 5] = _rol(S[x][y], _ROT[x][y])
            for x in range(5):
                for y in range(5):
                    S[x][y] = B[x][y] ^ ((~B[(x + 1) % 5][y]) & B[(x + 2) % 5][y])
            S[0][0] ^= _RC[rnd]
    out = b""
    for i in range(rate // 8):
        x, y = i % 5, i // 5
        out += S[x][y].to_bytes(8, "little")
    return out[:32]


def _hash_hex(value):
    if isinstance(value, str):
        value = value.encode("utf-8")
    return keccak256(value).hex()


def _state(payload):
    return {
        "owner": payload.get("owner", ""),
        "verifiers": dict(payload.get("verifiers", {}) or {}),
        "epochs": dict(payload.get("epochs", {}) or {}),
        "agent_epochs": dict(payload.get("agent_epochs", {}) or {}),
        "final_snapshots": dict(payload.get("final_snapshots", {}) or {}),
    }


def _verifier_count(st):
    return len(st["verifiers"])


def _epoch_key(agent_did_hash, epoch):
    return agent_did_hash + ":" + str(epoch)


def _median(vals):
    a = list(vals)
    for i in range(1, len(a)):
        key = a[i]
        j = i - 1
        while j >= 0 and a[j] > key:
            a[j + 1] = a[j]
            j -= 1
        a[j + 1] = key
    return a[len(a) // 2]


def status(_payload):
    return {"plugin": "com.twinsearth.official.chain-bridge",
            "tier": "official",
            "methods": ["status", "add_verifier", "remove_verifier",
                        "record", "finalize", "get_latest", "submission_count"]}


def add_verifier(payload):
    st = _state(payload)
    actor = payload.get("actor", "")
    target = payload.get("verifier", "")
    if actor != st["owner"]:
        return {"ok": False, "error": "not owner"}
    if target is None or str(target) == "":
        return {"ok": False, "error": "zero verifier"}
    if target in st["verifiers"]:
        return {"ok": False, "error": "already verifier"}
    if _verifier_count(st) >= 32:
        return {"ok": False, "error": "verifier cap reached"}
    st["verifiers"][target] = True
    out = {"ok": True, "verifier": target}
    out.update(st)
    return out


def remove_verifier(payload):
    st = _state(payload)
    actor = payload.get("actor", "")
    target = payload.get("verifier", "")
    if actor != st["owner"]:
        return {"ok": False, "error": "not owner"}
    if target not in st["verifiers"]:
        return {"ok": False, "error": "not verifier"}
    del st["verifiers"][target]
    out = {"ok": True, "verifier": target}
    out.update(st)
    return out


def record(payload):
    st = _state(payload)
    verifier = payload.get("verifier", "")
    agent_did = payload.get("agent_did", "")
    epoch = int(payload.get("epoch", 0))
    scores = {
        "quality": int(payload.get("quality", 0)),
        "speed": int(payload.get("speed", 0)),
        "honesty": int(payload.get("honesty", 0)),
        "availability": int(payload.get("availability", 0)),
    }
    if verifier not in st["verifiers"]:
        return {"ok": False, "error": "not verifier"}
    if not all(0 <= v <= 10000 for v in scores.values()):
        return {"ok": False, "error": "score out of bps range"}
    agent_hash = _hash_hex(agent_did)
    ek = _epoch_key(agent_hash, epoch)
    ep = st["epochs"].get(ek)
    if ep is not None and verifier in ep["scores"]:
        prev = ep["scores"][verifier]
        if prev != scores:
            return {"ok": False, "error": "conflicting resubmission"}
        out = {"ok": True, "idempotent": True}
        out.update(st)
        return out
    if ep is None:
        ep = {"submitters": [], "scores": {}, "finalized": False}
        st["epochs"][ek] = ep
    ep["submitters"].append(verifier)
    ep["scores"][verifier] = scores
    out = {"ok": True, "agentDidHash": agent_hash, "epochKey": ek}
    out.update(st)
    return out


def required_quorum(st):
    n = _verifier_count(st)
    if n == 0:
        return None
    return n // 2 + 1


def finalize(payload):
    st = _state(payload)
    agent_did = payload.get("agent_did", "")
    epoch = int(payload.get("epoch", 0))
    timestamp = int(payload.get("timestamp", 0))
    agent_hash = _hash_hex(agent_did)
    ek = _epoch_key(agent_hash, epoch)
    ep = st["epochs"].get(ek)
    if ep is None:
        return {"ok": False, "error": "no submissions"}
    if ep["finalized"]:
        return {"ok": False, "error": "already finalized"}
    q = required_quorum(st)
    if q is None:
        return {"ok": False, "error": "no verifiers"}
    if len(ep["submitters"]) < q:
        return {"ok": False, "error": "quorum not reached"}
    subs = ep["submitters"]
    snap = {
        "agentDidHash": agent_hash,
        "quality": _median([ep["scores"][v]["quality"] for v in subs]),
        "speed": _median([ep["scores"][v]["speed"] for v in subs]),
        "honesty": _median([ep["scores"][v]["honesty"] for v in subs]),
        "availability": _median([ep["scores"][v]["availability"] for v in subs]),
        "epoch": epoch,
        "finalizedAt": timestamp,
    }
    ep["finalized"] = True
    snaps = st["final_snapshots"].setdefault(agent_hash, {})
    snaps[str(epoch)] = snap
    ae = st["agent_epochs"].setdefault(agent_hash, [])
    ae.append(epoch)
    out = {"ok": True, "snapshot": snap}
    out.update(st)
    return out


def get_latest(payload):
    st = _state(payload)
    agent_did = payload.get("agent_did", "")
    agent_hash = _hash_hex(agent_did)
    epochs = st["agent_epochs"].get(agent_hash, [])
    if not epochs:
        return {"snapshot": {"agentDidHash": agent_hash, "quality": 0, "speed": 0,
                             "honesty": 0, "availability": 0, "epoch": 0,
                             "finalizedAt": 0}}
    last = epochs[-1]
    return {"snapshot": st["final_snapshots"][agent_hash][str(last)]}


def submission_count(payload):
    st = _state(payload)
    agent_did = payload.get("agent_did", "")
    epoch = int(payload.get("epoch", 0))
    agent_hash = _hash_hex(agent_did)
    ep = st["epochs"].get(_epoch_key(agent_hash, epoch))
    return {"count": 0 if ep is None else len(ep["submitters"])}
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
        OFF_MARKET_SETTLE => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: SETTLE_ENTRY,
        }),
        OFF_SCHEDULER_TASK => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: ROUTER_ENTRY,
        }),
        OFF_AGENT_CARD => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: CARD_ENTRY,
        }),
        OFF_SWARM_EMERGENCE => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: EMERGENCE_ENTRY,
        }),
        OFF_AGENT_SKILL => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: SKILL_ENTRY,
        }),
        OFF_CHAIN_ANCHOR => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: ANCHOR_ENTRY,
        }),
        OFF_CHAIN_BRIDGE => Some(EntrySource {
            language: "python",
            filename: "plugin.py",
            source: BRIDGE_ENTRY,
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
        // 架构合规关卡（v3.4.2 全局审核）：每个官方插件必须承载真实业务 entry。
        // 自动遍历 official_ids()，而非逐个硬编码——未来新增官方插件若漏配 entry
        // （或 official_entry_source 漏写 match 臂），本测试立即失败。
        let ids = official_ids();
        assert_eq!(ids.len(), 9, "官方插件数量应为 9（9/9 业务化）");
        for id in ids {
            let src = official_entry_source(id);
            assert!(src.is_some(), "官方插件 {id} 缺少业务 entry source");
            let s = src.unwrap();
            assert_eq!(s.language, "python");
            assert!(!s.filename.is_empty(), "{id} entry 文件名缺失");
            assert!(!s.source.is_empty(), "{id} entry 源码为空");
        }
    }
}
