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
//! 后续小版本：3.7.3 P2P 种子健康 + 每块 Ed25519 锚定、3.7.4–3.7.6 内存共享记账、
//! 3.7.7–3.7.9 CPU 优先级调度；3.8.x Agent 委员会 + pack_diff/轨迹分叉；3.9.x
//! Agent 安全组织。

pub mod backend;
pub mod blockstore;
pub mod image;

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
}
