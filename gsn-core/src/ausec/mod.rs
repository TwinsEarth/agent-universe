//! AUSec（Agent Universe Elastic Compute）弹性计算基础设施。
//!
//! 定位与设计见 `docs/ausec/AUSEC-DESIGN.md`。v3.7.0 交付基座：
//!
//! - [`backend`]：四执行后端（FnCall/Container/MicroVM/FullVm）、分级→后端映射、
//!   平台原语可行性 + 执行器接线状态的**诚实**就绪模型；
//! - 系统插件 [`AUSEC_PLUGIN`]：经 PMB/宿主 `call` 暴露 `status` 与
//!   `select_backend` 两个只读查询。
//!
//! v3.7.1 新增 [`image`]：内容寻址镜像块清单（偏移/长度/sha256/顺序）的构建、
//! 结构完整性基线与按需取块后的内容校验；经 PMB `manifest_validate` 作为块清单
//! 入场闸。
//!
//! v3.7.2 新增 [`blockstore`]：`BlockStore` 本地仅存元数据 + 按需取块（缺块即取、
//! 命中复用、按内容地址只读共享），`BlockSource` 契约含真实本地种子源与具名但不
//! 伪造传输的 UDOS 远端源。
//!
//! v3.7.3 新增 [`seed`]：种子/副本健康度的确定性计算（0→0%、1→100%、10→1000%，
//! 按最弱块决定整镜像健康度）+ 每块 Ed25519 发布者锚定（绑定
//! image/index/offset/length/sha256，防槽位迁移），`AttestedSeedSource` 包在
//! `BlockSource` 外做真实生产 fail-closed 校验（缺证明/坏签名/不受信一律拒绝）。
//! 经 PMB `seed_health` / `chunk_attestation_verify` 暴露只读查询。
//!
//! 后续小版本：3.7.4–3.7.6 内存共享记账、
//! 3.7.7–3.7.9 CPU 优先级调度；3.8.x Agent 委员会 + pack_diff/轨迹分叉；3.9.x
//! Agent 安全组织。

pub mod backend;
pub mod blockstore;
pub mod image;
pub mod seed;

pub use backend::{
    backend_for_tier, detect_features, primitive_availability, readiness, ExecutionBackend,
    OsIsolation, Platform, PlatformFeatures, PrimitiveAvailability, Readiness, RiskGrade,
};
pub use blockstore::{
    BlockFetchError, BlockSource, BlockSourceKind, BlockStats, BlockStore, BlockStoreError,
    LocalDirBlockSource, SharedChunkCache, UdosRemoteBlockSource,
};
pub use image::{
    build_manifest, digest_hex, ChunkEntry, ChunkManifest, ManifestError, MAX_CHUNK_SIZE,
};
pub use seed::{
    chunk_attestation_message, sign_chunk, sign_chunk_with_keypair, verify_chunk_attestation,
    verify_trusted_chunk, AttestationError, AttestedSeedSource, ChunkSeedHealth, ImageSeedHealth,
    PublisherId, ReplicaLedger, TrustedPublishers, ATTESTATION_DOMAIN,
};

use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::runtime::native::NativeRuntime;
use crate::plugin::tier::Tier;

/// AUSec 系统插件 id（T0，随内核进程内注册，不可热插拔）。
pub const AUSEC_PLUGIN: &str = "com.twinsearth.sys.ausec";

/// PMB 方法：查询 AUSec 基座状态（平台特征 + 四后端真实就绪度）。
pub const METHOD_STATUS: &str = "status";
/// PMB 方法：按插件分级/风险选择默认后端，并给出当前平台就绪度。
pub const METHOD_SELECT_BACKEND: &str = "select_backend";
/// PMB 方法：v3.7.1 内容寻址镜像块清单入场校验（结构完整性基线）。
pub const METHOD_MANIFEST_VALIDATE: &str = "manifest_validate";
/// PMB 方法：v3.7.3 种子/副本健康度确定性计算（不联网，不伪造对端）。
pub const METHOD_SEED_HEALTH: &str = "seed_health";
/// PMB 方法：v3.7.3 单块发布者锚定签名 + 信任集合只读校验。
pub const METHOD_CHUNK_ATTESTATION_VERIFY: &str = "chunk_attestation_verify";

/// 把就绪状态转成诚实的 JSON：区分 ready / executor_not_wired / needs_probe /
/// unsupported，并带 `can_run_now` 布尔与平台原语细节。
fn readiness_json(
    platform: Platform,
    features: PlatformFeatures,
    backend: ExecutionBackend,
) -> serde_json::Value {
    let ready = readiness(platform, features, backend);
    let can_run_now = ready.can_run_now();
    let (state, detail) = match ready {
        Readiness::Ready => ("ready", "平台原语具备且执行器已接线".to_string()),
        Readiness::ExecutorNotWired { primitive: p } => (
            "executor_not_wired",
            format!(
                "平台{}，但本仓该后端执行器尚未接线（v3.7.0 仅交付选择/声明）",
                primitive_label(&p)
            ),
        ),
        Readiness::NeedsProbe { precondition } => ("needs_probe", precondition),
        Readiness::Unsupported { reason } => ("unsupported", reason),
    };
    serde_json::json!({
        "state": state,
        "can_run_now": can_run_now,
        "detail": detail,
        "executor_wired": backend.executor_wired(),
        "os_isolation": serde_json::to_value(backend.os_isolation()).unwrap_or(serde_json::Value::Null),
        "startup_hint": backend.startup_hint(),
    })
}

fn primitive_label(p: &PrimitiveAvailability) -> &'static str {
    match p {
        PrimitiveAvailability::Available => "原语可用",
        PrimitiveAvailability::NeedsProbe { .. } => "可能支持但需前置探测",
        PrimitiveAvailability::Unsupported { .. } => "无对应原语",
    }
}

/// 构造 `status` 查询结果（每次调用实时探测平台特征）。
pub fn status_payload() -> serde_json::Value {
    let platform = Platform::current();
    let features = detect_features();
    let backends: Vec<serde_json::Value> = ExecutionBackend::all()
        .iter()
        .map(|b| {
            serde_json::json!({
                "backend": b.as_str(),
                "readiness": readiness_json(platform, features, *b),
            })
        })
        .collect();
    serde_json::json!({
        "plugin": AUSEC_PLUGIN,
        "module": "ausec",
        "core_version": env!("CARGO_PKG_VERSION"),
        "platform": serde_json::to_value(platform).unwrap_or(serde_json::Value::Null),
        "features": serde_json::to_value(features).unwrap_or(serde_json::Value::Null),
        "backends": backends,
        "note": "startup_hint 为设计目标/外部资料数量级，非本仓实测；平台原语可用不等于执行器已接线",
    })
}

/// 处理 `select_backend`：入参 `{plugin_id, risk?}`，返回默认后端与就绪度。
fn select_backend(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let plugin_id = input
        .get("plugin_id")
        .and_then(|v| v.as_str())
        .ok_or("缺少 plugin_id（字符串）")?;
    let risk = match input.get("risk").and_then(|v| v.as_str()) {
        None | Some("standard") => RiskGrade::Standard,
        Some("elevated") => RiskGrade::Elevated,
        Some(other) => return Err(format!("非法 risk={other}（仅 standard/elevated）")),
    };
    let tier = Tier::from_name(plugin_id);
    // 注：Tier::from_name 不产生 Blacklist（黑名单由 Blacklist 模块在加载闸门精确
    // 匹配，黑名单插件根本到不了调度器）。Blacklist→None 的不可调度契约由
    // backend_for_tier 纯函数 + backend.rs 单测锁定，这里处理的是可加载的分级。
    let backend = backend_for_tier(tier, risk).ok_or("无法为该插件分级选择执行后端")?;
    let platform = Platform::current();
    let features = detect_features();
    Ok(serde_json::json!({
        "plugin_id": plugin_id,
        "tier": tier.as_str(),
        "risk": if risk == RiskGrade::Elevated { "elevated" } else { "standard" },
        "backend": backend.as_str(),
        "readiness": readiness_json(platform, features, backend),
    }))
}

/// 字节桥：`status`（忽略负载，只读）。
fn handle_status(_method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
    serde_json::to_vec(&status_payload())
        .map_err(|e| PluginError::Runtime(format!("AUSec status 序列化失败: {e}")))
}

/// PMB `manifest_validate` 入参解析与校验：入参 `{manifest: <ChunkManifest>}`。
///
/// 这是 v3.7.1 块清单的唯一入场闸：v3.7.2 BlockStore 在接受任何远端/本地清单前
/// 都经此路径，结构不合法（空洞/重叠/乱序/长度/摘要形状/总长不符）具名拒绝。
fn manifest_validate(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let manifest_val = input
        .get("manifest")
        .cloned()
        .ok_or("缺少 manifest（ChunkManifest 对象）")?;
    let raw =
        serde_json::to_vec(&manifest_val).map_err(|e| format!("manifest 重新序列化失败: {e}"))?;
    let manifest = ChunkManifest::parse_and_validate(&raw).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "valid": true,
        "image": manifest.image,
        "chunk_size": manifest.chunk_size,
        "chunks": manifest.chunks.len(),
        "total_length": manifest.total_length,
    }))
}

/// 字节桥：`manifest_validate`，入参 JSON `{manifest: <ChunkManifest>}`。
fn handle_manifest_validate(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| PluginError::Manifest(format!("manifest_validate 负载非合法 JSON: {e}")))?;
    let out = manifest_validate(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec manifest_validate 序列化失败: {e}")))
}

/// 字节桥：`select_backend`，入参 JSON `{plugin_id, risk?}`。
fn handle_select(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| PluginError::Manifest(format!("select_backend 负载非合法 JSON: {e}")))?;
    let out = select_backend(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec select_backend 序列化失败: {e}")))
}

/// `seed_health` 入参 `{manifest, replica_counts: {"<index>": n, ...}}`。
///
/// 纯确定性：只按调用方给出的副本观测计数计算健康度，不联网、不推断对端。
fn seed_health(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let manifest_val = input
        .get("manifest")
        .cloned()
        .ok_or("缺少 manifest（ChunkManifest 对象）")?;
    let raw =
        serde_json::to_vec(&manifest_val).map_err(|e| format!("manifest 重新序列化失败: {e}"))?;
    let manifest = ChunkManifest::parse_and_validate(&raw).map_err(|e| e.to_string())?;

    let mut counts = std::collections::HashMap::new();
    if let Some(obj) = input.get("replica_counts").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            let idx: usize = k
                .parse()
                .map_err(|_| format!("replica_counts 的键必须是块序号数字，得到 {k}"))?;
            let n: u32 = v
                .as_u64()
                .ok_or_else(|| format!("replica_counts[{k}] 必须是非负整数"))?
                .try_into()
                .map_err(|_| format!("replica_counts[{k}] 超出 u32"))?;
            counts.insert(idx, n);
        }
    }
    let h = ImageSeedHealth::for_manifest(&manifest, &counts);
    Ok(serde_json::json!({
        "image": manifest.image,
        "chunks_total": h.chunks_total,
        "chunks_available": h.chunks_available,
        "availability_permille": h.availability_permille,
        "min_replicas": h.min_replicas,
        "mean_replicas_permille": h.mean_replicas_permille,
        "health_percent": h.health_percent,
        "fully_available": h.fully_available,
        "fully_seeded_1000pct": h.fully_seeded_1000pct,
        "note": "确定性计算：health_percent=最弱块副本数*100（0→0%,1→100%,10→1000%）；1000% 口径源自外部 DSEC 报道非本仓复测",
    }))
}

/// 字节桥：`seed_health`。
fn handle_seed_health(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| PluginError::Manifest(format!("seed_health 负载非合法 JSON: {e}")))?;
    let out = seed_health(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec seed_health 序列化失败: {e}")))
}

/// `chunk_attestation_verify` 入参：
/// `{image, entry: <ChunkEntry>, publisher: <hex32B>, signature: <hex64B>,
///   trusted_publishers: [<hex32B>...]}`。
///
/// 只读安全校验：先验 Ed25519 锚定（绑定 image/index/offset/length/sha256），
/// 再查信任集合；任一不过返回错误（fail-closed）。
fn chunk_attestation_verify(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let image = input
        .get("image")
        .and_then(|v| v.as_str())
        .ok_or("缺少 image（字符串）")?;
    let entry_val = input
        .get("entry")
        .cloned()
        .ok_or("缺少 entry（ChunkEntry 对象）")?;
    let entry: ChunkEntry =
        serde_json::from_value(entry_val).map_err(|e| format!("entry 不是合法 ChunkEntry: {e}"))?;
    let publisher_hex = input
        .get("publisher")
        .and_then(|v| v.as_str())
        .ok_or("缺少 publisher（32 字节公钥的 hex）")?;
    let signature_hex = input
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or("缺少 signature（64 字节签名的 hex）")?;

    let publisher_bytes =
        hex::decode(publisher_hex).map_err(|e| format!("publisher hex 解码失败: {e}"))?;
    let publisher = PublisherId::from_bytes(&publisher_bytes).map_err(|e| e.to_string())?;
    let signature =
        hex::decode(signature_hex).map_err(|e| format!("signature hex 解码失败: {e}"))?;

    let mut trusted = TrustedPublishers::new();
    let arr = input
        .get("trusted_publishers")
        .and_then(|v| v.as_array())
        .ok_or("缺少 trusted_publishers（hex 公钥数组；空数组=谁都不信）")?;
    for (i, v) in arr.iter().enumerate() {
        let hx = v
            .as_str()
            .ok_or_else(|| format!("trusted_publishers[{i}] 必须是 hex 字符串"))?;
        let b =
            hex::decode(hx).map_err(|e| format!("trusted_publishers[{i}] hex 解码失败: {e}"))?;
        trusted.add(PublisherId::from_bytes(&b).map_err(|e| e.to_string())?);
    }

    verify_trusted_chunk(image, &entry, &publisher, &signature, &trusted)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"valid": true, "index": entry.index, "publisher": publisher_hex}))
}

/// 字节桥：`chunk_attestation_verify`。
fn handle_chunk_attestation_verify(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = serde_json::from_slice(payload).map_err(|e| {
        PluginError::Manifest(format!("chunk_attestation_verify 负载非合法 JSON: {e}"))
    })?;
    let out = chunk_attestation_verify(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out).map_err(|e| {
        PluginError::Runtime(format!("AUSec chunk_attestation_verify 序列化失败: {e}"))
    })
}

/// 向 T0 进程内运行时注册 AUSec 系统插件的处理器（由系统插件装配流程调用）。
pub fn register(rt: &mut NativeRuntime) {
    // status：无参，只读。
    rt.register_handler(AUSEC_PLUGIN, METHOD_STATUS, handle_status);
    // select_backend：只读选择/就绪查询。
    rt.register_handler(AUSEC_PLUGIN, METHOD_SELECT_BACKEND, handle_select);
    // manifest_validate：v3.7.1 镜像块清单入场校验。
    rt.register_handler(
        AUSEC_PLUGIN,
        METHOD_MANIFEST_VALIDATE,
        handle_manifest_validate,
    );
    // seed_health：v3.7.3 种子健康度确定性计算（只读）。
    rt.register_handler(AUSEC_PLUGIN, METHOD_SEED_HEALTH, handle_seed_health);
    // chunk_attestation_verify：v3.7.3 块发布者锚定签名校验（只读）。
    rt.register_handler(
        AUSEC_PLUGIN,
        METHOD_CHUNK_ATTESTATION_VERIFY,
        handle_chunk_attestation_verify,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_payload_lists_all_backends_and_is_honest() {
        let v = status_payload();
        assert_eq!(v["plugin"], AUSEC_PLUGIN);
        let backends = v["backends"].as_array().unwrap();
        assert_eq!(backends.len(), 4);
        let ids: Vec<&str> = backends
            .iter()
            .map(|b| b["backend"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["fncall", "container", "microvm", "fullvm"]);
        // v3.7.0：除 fncall 外都不得 can_run_now=true（防"承诺>事实"）。
        for b in backends {
            let can = b["readiness"]["can_run_now"].as_bool().unwrap();
            if b["backend"] == "fncall" {
                assert!(can, "fncall 应可用于进程内可信短任务");
            } else {
                assert!(!can, "{} 在 v3.7.0 不应可执行", b["backend"]);
                assert_eq!(b["readiness"]["state"], "executor_not_wired");
            }
        }
    }

    #[test]
    fn select_backend_via_pm_handler() {
        // 系统插件 → FnCall。
        let out = select_backend(serde_json::json!({"plugin_id": "com.twinsearth.sys.identity"}))
            .unwrap();
        assert_eq!(out["tier"], "system");
        assert_eq!(out["backend"], "fncall");
        assert_eq!(out["readiness"]["can_run_now"], true);

        // 官方插件 → Container；高风险第三方 → FullVm。
        let off =
            select_backend(serde_json::json!({"plugin_id": "com.twinsearth.official.market"}))
                .unwrap();
        assert_eq!(off["backend"], "container");
        let tp =
            select_backend(serde_json::json!({"plugin_id": "com.example.x", "risk": "elevated"}))
                .unwrap();
        assert_eq!(tp["tier"], "third_party");
        assert_eq!(tp["backend"], "fullvm");

        // 缺参/非法 risk 报错。
        assert!(select_backend(serde_json::json!({})).is_err());
        assert!(select_backend(serde_json::json!({"plugin_id":"x","risk":"crazy"})).is_err());
    }

    #[test]
    fn manifest_validate_admits_good_and_rejects_gap() {
        let data: Vec<u8> = (0..37u32).map(|i| (i % 251) as u8).collect();
        let m = build_manifest("img/pm", &data, 16).unwrap();

        // 合法清单经 PM 入口准入。
        let ok = manifest_validate(serde_json::json!({"manifest": m.clone()})).unwrap();
        assert_eq!(ok["valid"], true);
        assert_eq!(ok["chunks"], 3);
        assert_eq!(ok["total_length"], 37);

        // 缺 manifest 字段。
        assert!(manifest_validate(serde_json::json!({})).is_err());

        // 结构非法（偏移空洞）必须被 PM 入口拒绝。
        let mut bad = m;
        bad.chunks[1].offset += 1;
        assert!(manifest_validate(serde_json::json!({"manifest": bad})).is_err());
    }

    #[test]
    fn pm_seed_health_is_deterministic_and_weakest_wins() {
        let data: Vec<u8> = (0..33u32).map(|i| (i % 251) as u8).collect();
        let m = build_manifest("img/pmseed", &data, 16).unwrap(); // 3 块
                                                                  // 副本计数以字符串键给出：块0=12、块1=10、块2=10 → 短板 10 → 1000%。
        let ok = seed_health(serde_json::json!({
            "manifest": m,
            "replica_counts": {"0": 12, "1": 10, "2": 10},
        }))
        .unwrap();
        assert_eq!(ok["health_percent"], 1000);
        assert_eq!(ok["min_replicas"], 10);
        assert_eq!(ok["fully_seeded_1000pct"], true);
        assert_eq!(ok["fully_available"], true);

        // 缺一个块的计数（按 0）→ 短板 0 → 0% 且不可用。
        let m2 = build_manifest("img/pmseed2", &data, 16).unwrap();
        let weak = seed_health(serde_json::json!({
            "manifest": m2,
            "replica_counts": {"0": 5, "1": 5},
        }))
        .unwrap();
        assert_eq!(weak["health_percent"], 0);
        assert_eq!(weak["fully_available"], false);

        // 非法键/非数字必须报错。
        assert!(seed_health(serde_json::json!({
            "manifest": build_manifest("img/x", &data, 16).unwrap(),
            "replica_counts": {"abc": 1},
        }))
        .is_err());
    }

    #[test]
    fn pm_chunk_attestation_verify_good_slot_move_untrusted() {
        use crate::ausec::seed::sign_chunk_with_keypair;
        use crate::identity::Keypair;

        let data: Vec<u8> = (0..40u32).map(|i| (i % 251) as u8).collect();
        let image = "img/pmat";
        let m = build_manifest(image, &data, 16).unwrap();
        let mut seed = [0u8; 32];
        for (i, b) in seed.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(3).wrapping_add(1);
        }
        let kp = Keypair::from_seed(&seed);
        let pk_hex = hex::encode(kp.public_key());

        // 对块0签名。
        let sig0_hex = hex::encode(sign_chunk_with_keypair(&kp, image, &m.chunks[0]));

        // 合法：签名匹配块0 + 发布者在信任集合。
        let ok = chunk_attestation_verify(serde_json::json!({
            "image": image,
            "entry": m.chunks[0].clone(),
            "publisher": pk_hex,
            "signature": sig0_hex,
            "trusted_publishers": [pk_hex],
        }))
        .unwrap();
        assert_eq!(ok["valid"], true);

        // 槽位迁移：用块0签名验块1 → 必须失败。
        assert!(chunk_attestation_verify(serde_json::json!({
            "image": image,
            "entry": m.chunks[1].clone(),
            "publisher": pk_hex,
            "signature": sig0_hex,
            "trusted_publishers": [pk_hex],
        }))
        .is_err());

        // 发布者不在信任集合（自洽签名也拒绝）。
        assert!(chunk_attestation_verify(serde_json::json!({
            "image": image,
            "entry": m.chunks[0].clone(),
            "publisher": pk_hex,
            "signature": sig0_hex,
            "trusted_publishers": [],
        }))
        .is_err());

        // 信任集合非空但不含该发布者。
        let other = {
            let mut s2 = [9u8; 32];
            s2[0] = 1;
            let kp2 = Keypair::from_seed(&s2);
            hex::encode(kp2.public_key())
        };
        assert!(chunk_attestation_verify(serde_json::json!({
            "image": image,
            "entry": m.chunks[0].clone(),
            "publisher": pk_hex,
            "signature": sig0_hex,
            "trusted_publishers": [other],
        }))
        .is_err());
    }
}
