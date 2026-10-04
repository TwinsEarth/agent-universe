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
//! v3.7.4 新增 [`memory`]：内存配额池 + **共享额度记账**（同一内容寻址只读块跨沙盒
//! 只计一次并集）+ 两级超卖准入（committed 物理硬闸、nominal 超卖比上限闸）。纯
//! 确定性记账，跨平台可测；virtio-pmem/DAX/DAMON/balloon 等 Linux-MicroVM 专有原语
//! 按设计在 v3.7.6 声明、无原语平台具名拒绝。经 PMB `memory_status` /
//! `memory_admit` 暴露只读查询。
//!
//! 后续小版本：3.7.5 等待期保内存+空闲优先回收、3.7.6 回收统计+OS 原语具名拒绝、
//! 3.7.7–3.7.9 CPU 优先级调度；3.8.x Agent 委员会 + pack_diff/轨迹分叉；3.9.x
//! Agent 安全组织。

pub mod backend;
pub mod blockstore;
pub mod cpu;
pub mod image;
pub mod memory;
pub mod primitives;
pub mod seed;

pub use backend::{
    backend_for_tier, detect_features, primitive_availability, readiness, ExecutionBackend,
    OsIsolation, Platform, PlatformFeatures, PrimitiveAvailability, Readiness, RiskGrade,
};
pub use blockstore::{
    BlockFetchError, BlockSource, BlockSourceKind, BlockStats, BlockStore, BlockStoreError,
    LocalDirBlockSource, SharedChunkCache, UdosRemoteBlockSource,
};
pub use cpu::{
    arbitrate_priority, parse_latency as parse_cpu_latency, parse_tier as parse_cpu_tier,
    CpuClassSummary, CpuError, CpuPriorityModel, CpuSandboxRequest, EffectiveCpuEntry,
    DEFAULT_WEIGHT as CPU_DEFAULT_WEIGHT, MAX_WEIGHT as CPU_MAX_WEIGHT,
    PRIORITY_SENSITIVE as CPU_PRIORITY_SENSITIVE, PRIORITY_TOLERANT as CPU_PRIORITY_TOLERANT,
};
pub use image::{
    build_manifest, digest_hex, ChunkEntry, ChunkManifest, ManifestError, MAX_CHUNK_SIZE,
};
pub use memory::{
    ActivityState, Admission, IdleReclaimer, LatencyClass, MemoryError, MemoryPool,
    MemorySandboxRequest, OvercommitRatio, PoolStatus, ReclaimLedger, ReclaimPlan, ReclaimStats,
    ReclaimStep, SandboxIdleObservation, SharedRef,
};
pub use primitives::{
    memory_primitive_status, MemoryPrimitive, MemoryPrimitiveState, MemoryPrimitiveStatus,
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
/// PMB 方法：v3.7.4 内存配额池状态（committed/nominal/共享去重/超卖口径）只读查询。
pub const METHOD_MEMORY_STATUS: &str = "memory_status";
/// PMB 方法：v3.7.4 给定现有沙盒集合，对候选申请做两级准入纯决策（不持久化）。
pub const METHOD_MEMORY_ADMIT: &str = "memory_admit";
/// PMB 方法：v3.7.5 等待期保内存 + 空闲优先回收的确定性回收计划（纯建议、无副作用）。
pub const METHOD_IDLE_RECLAIM: &str = "idle_reclaim_plan";
/// PMB 方法：v3.7.6 回收/超卖统计只读汇总（由历史回收计划 + 当前账/观测确定性重算，
/// 入账即做越界/溢出 fail-closed 校验）。
pub const METHOD_RECLAIM_STATS: &str = "reclaim_stats";
/// PMB 方法：v3.7.6 Linux-MicroVM 专有内存原语（virtio-pmem/DAX/DAMON/balloon）
/// 的诚实门：非 Linux 具名拒绝；Linux 仅声明、can_enforce=false（执行器未接线）。
pub const METHOD_MEMORY_PRIMITIVE: &str = "memory_primitive_status";
/// PMB 方法：v3.7.7 CPU 两级优先级模型（时延敏感/容忍 + 优先级与权重），纯确定性
/// 仲裁；低信任级自我提级 Sensitive fail-closed；不调任何 OS 调度原语。
pub const METHOD_CPU_PRIORITY: &str = "cpu_priority";

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

/// 从 JSON 描述构造并装配内存池：`{physical_bytes, overcommit_times?|overcommit_permille?,
/// sandboxes?: [MemorySandboxRequest...]}`。重复准入/校验错误照常返回（fail-closed）。
fn pool_from_input(input: &serde_json::Value) -> Result<MemoryPool, String> {
    let physical = input
        .get("physical_bytes")
        .and_then(|v| v.as_u64())
        .ok_or("缺少 physical_bytes（正整数，字节）")?;
    // 超卖比：默认 1×；优先用精确千分点，否则用「倍」。
    let ratio = if let Some(p) = input.get("overcommit_permille").and_then(|v| v.as_u64()) {
        OvercommitRatio::from_permille(p).map_err(|e| e.to_string())?
    } else {
        let times = input
            .get("overcommit_times")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);
        OvercommitRatio::from_permille(times * 1000).map_err(|e| e.to_string())?
    };
    let mut pool = MemoryPool::new(physical, ratio).map_err(|e| e.to_string())?;
    if let Some(arr) = input.get("sandboxes").and_then(|v| v.as_array()) {
        for (i, v) in arr.iter().enumerate() {
            let req: MemorySandboxRequest = serde_json::from_value(v.clone())
                .map_err(|e| format!("sandboxes[{i}] 不是合法 MemorySandboxRequest: {e}"))?;
            // 现有集合必须全部成立，否则输入自相矛盾（fail-closed）。
            pool.admit(req)
                .map_err(|e| format!("sandboxes[{i}] 无法准入: {e}"))?;
        }
    }
    Ok(pool)
}

/// `memory_status` 入参：`{physical_bytes, overcommit_times?|overcommit_permille?,
/// sandboxes?: [...]}`，返回当前池的 committed/nominal/去重节省/超卖口径快照。
/// 纯确定性：只按调用方给出的沙盒申请重算，不读真实 OS 计数。
fn memory_status(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let s = pool_from_input(&input)?.status();
    Ok(serde_json::json!({
        "physical_bytes": s.physical_bytes,
        "max_overcommit_permille": s.max_overcommit_permille,
        "sandbox_count": s.sandbox_count,
        "committed_bytes": s.committed_bytes,
        "nominal_bytes": s.nominal_bytes,
        "shared_dedup_saving": s.shared_dedup_saving,
        "nominal_ceiling": s.nominal_ceiling,
        "committed_utilization_permille": s.committed_utilization_permille,
        "observed_overcommit_permille": s.observed_overcommit_permille,
        "distinct_shared_contents": s.distinct_shared_contents,
        "note": "确定性记账：committed=独占之和+只读共享并集(同一内容全机计一次)；virtio-pmem/DAX/DAMON/balloon 为 Linux 专有原语, 3.7.6 具名拒绝, 此处非 OS 实测",
    }))
}

/// `memory_admit` 入参：同 `memory_status`，另加 `candidate: MemorySandboxRequest`。
/// 返回候选准入后的投影 `Admission`；被物理硬闸或超卖闸拒绝则返回错误字符串
/// （纯决策，不持久化任何全局状态）。
fn memory_admit(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let cand_val = input
        .get("candidate")
        .cloned()
        .ok_or("缺少 candidate（MemorySandboxRequest 对象）")?;
    let candidate: MemorySandboxRequest = serde_json::from_value(cand_val)
        .map_err(|e| format!("candidate 不是合法 MemorySandboxRequest: {e}"))?;
    let a = pool_from_input(&input)?
        .project(&candidate)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "admitted": a.admitted,
        "committed_bytes": a.committed_bytes,
        "nominal_bytes": a.nominal_bytes,
        "shared_dedup_saving": a.shared_dedup_saving,
        "committed_utilization_permille": a.committed_utilization_permille,
        "observed_overcommit_permille": a.observed_overcommit_permille,
        "nominal_ceiling": a.nominal_ceiling,
    }))
}

/// `idle_reclaim_plan` 入参：`{target_bytes, observations:[SandboxIdleObservation]}`。
/// 返回确定性回收计划（只从各沙盒申报的 reclaimable_idle_bytes 取，永不触碰
/// reserved_bytes、永不释放沙盒）；空闲页不足时 `sufficient=false` 并给出缺口。
/// 纯建议，不修改任何配额账或真实内存。
fn idle_reclaim_plan(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let target = input
        .get("target_bytes")
        .and_then(|v| v.as_u64())
        .ok_or("缺少 target_bytes（u64 字节）")?;
    let obs_val = input
        .get("observations")
        .cloned()
        .unwrap_or(serde_json::json!([]));
    let observations: Vec<SandboxIdleObservation> = serde_json::from_value(obs_val)
        .map_err(|e| format!("observations 不是合法 SandboxIdleObservation 数组: {e}"))?;
    let plan = IdleReclaimer::plan(&observations, target).map_err(|e| e.to_string())?;
    serde_json::to_value(&plan).map_err(|e| format!("ReclaimPlan 序列化失败: {e}"))
}

/// `reclaim_stats` 入参：
/// `{records?: [ReclaimPlan...], observations?: [SandboxIdleObservation...],
///    physical_bytes?, overcommit_times?|overcommit_permille?, sandboxes?: [...]}`。
///
/// 每条历史回收计划在入账时做越界/溢出 fail-closed 校验（0 字节步骤、步骤合计≠声明、
/// 声明>目标、shortfall 不自洽、求和溢出）；统计全部由已入账记录 + 可选当前池/观测
/// 确定性重算。**不读 OS、不持久化、不伪造回收。**
fn reclaim_stats(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut ledger = ReclaimLedger::new();
    if let Some(arr) = input.get("records").and_then(|v| v.as_array()) {
        for (i, v) in arr.iter().enumerate() {
            let plan: ReclaimPlan = serde_json::from_value(v.clone())
                .map_err(|e| format!("records[{i}] 不是合法 ReclaimPlan: {e}"))?;
            ledger
                .record(plan)
                .map_err(|e| format!("records[{i}] 账实不符被拒绝: {e}"))?;
        }
    }

    let observations: Vec<SandboxIdleObservation> = match input.get("observations") {
        None => Vec::new(),
        Some(serde_json::Value::Null) => Vec::new(),
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| format!("observations 不是合法 SandboxIdleObservation 数组: {e}"))?,
    };

    // 仅当给出 physical_bytes 时才构造配额池；否则纯历史/观测口径。
    let pool = if input.get("physical_bytes").is_some() {
        Some(pool_from_input(&input)?)
    } else {
        None
    };

    let stats = ledger
        .summarize(pool.as_ref(), &observations)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&stats).map_err(|e| format!("ReclaimStats 序列化失败: {e}"))
}

/// `memory_primitive_status` 入参：`{primitive?: "virtio_pmem"|"dax"|"damon"|"balloon",
/// platform?: "linux"|"macos"|"windows"|"other"}`；缺省 primitive 返回四原语、平台用
/// 编译期当前平台（platform 覆盖仅用于跨平台确定性核查）。
fn memory_primitive_query(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let platform = match input.get("platform").and_then(|v| v.as_str()) {
        None => Platform::current(),
        Some("linux") => Platform::Linux,
        Some("macos") => Platform::MacOs,
        Some("windows") => Platform::Windows,
        Some("other") => Platform::Other,
        Some(other) => return Err(format!("非法 platform={other}")),
    };
    let prims = match input.get("primitive").and_then(|v| v.as_str()) {
        None => MemoryPrimitive::all().to_vec(),
        Some(name) => vec![MemoryPrimitive::parse(name)
            .ok_or_else(|| format!("未知内存原语 {name}（仅 virtio_pmem/dax/damon/balloon）"))?],
    };
    let statuses: Vec<MemoryPrimitiveStatus> = prims
        .into_iter()
        .map(|p| memory_primitive_status(platform, p))
        .collect();
    Ok(serde_json::json!({
        "plugin": AUSEC_PLUGIN,
        "module": "ausec",
        "platform": serde_json::to_value(platform).unwrap_or(serde_json::Value::Null),
        "primitives": statuses,
        "note": "virtio-pmem/DAX/DAMON/balloon 为 Linux-MicroVM 专有原语；v3.7.6 非 Linux 具名拒绝，Linux 仅声明、执行器未接线，can_enforce 恒 false，不代表已真实共享/回收内存",
    }))
}

/// `cpu_priority` 入参：
/// `{capacity_millis: u64, sandboxes: [{sandbox_id, tier?: "system"|"official"|
/// "certified"|"third_party"(缺省=最小信任), latency?: "sensitive"|"tolerant",
/// weight?: u32, requested_millis: u64}]}`。
///
/// tier 缺省按 `third_party`（最小信任默认，绝不默认给 sensitive）；低信任级自报
/// sensitive 由 [`arbitrate_priority`] fail-closed 拒绝。返回仲裁后的确定性模型。
fn cpu_priority_query(input: serde_json::Value) -> Result<serde_json::Value, String> {
    let capacity = input
        .get("capacity_millis")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "缺少 capacity_millis（必须为正整数 u64）".to_string())?;

    let arr = input
        .get("sandboxes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "缺少 sandboxes 数组".to_string())?;

    let mut requests = Vec::with_capacity(arr.len());
    for (i, v) in arr.iter().enumerate() {
        let sandbox_id = v
            .get("sandbox_id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| format!("sandboxes[{i}] 缺少 sandbox_id 字符串"))?
            .to_string();
        // tier 缺省 → third_party（最小信任，fail-safe 默认）。
        let tier = match v.get("tier") {
            None | Some(serde_json::Value::Null) => Tier::ThirdParty,
            Some(serde_json::Value::String(s)) => {
                parse_cpu_tier(s).map_err(|e| format!("sandboxes[{i}].tier 非法: {e}"))?
            }
            Some(_) => return Err(format!("sandboxes[{i}].tier 必须是字符串")),
        };
        let latency = match v.get("latency") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) => Some(
                parse_cpu_latency(s).map_err(|e| format!("sandboxes[{i}].latency 非法: {e}"))?,
            ),
            Some(_) => return Err(format!("sandboxes[{i}].latency 必须是字符串")),
        };
        let weight = match v.get("weight") {
            None | Some(serde_json::Value::Null) => None,
            Some(w) => Some(
                w.as_u64()
                    .filter(|n| *n <= u64::from(CPU_MAX_WEIGHT))
                    .ok_or_else(|| format!("sandboxes[{i}].weight 非法或越界"))?
                    as u32,
            ),
        };
        let requested_millis = v
            .get("requested_millis")
            .and_then(|x| x.as_u64())
            .ok_or_else(|| format!("sandboxes[{i}] 缺少 requested_millis（正整数）"))?;
        requests.push(CpuSandboxRequest {
            sandbox_id,
            tier,
            latency,
            weight,
            requested_millis,
        });
    }

    let model = arbitrate_priority(capacity, &requests).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "plugin": AUSEC_PLUGIN,
        "module": "ausec",
        "model": model,
        "note": "v3.7.7 仅交付两级优先级与权重的确定性仲裁，不调用 cgroup/sched_setaffinity 等 OS 调度原语、不做真实限速；竞争下的配额分配在 v3.7.8，突发准入在 v3.7.9。tier 缺省按 third_party；低信任级自报 sensitive 一律拒绝。",
    }))
}

/// 字节桥：`memory_status`。
fn handle_memory_status(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| PluginError::Manifest(format!("memory_status 负载非合法 JSON: {e}")))?;
    let out = memory_status(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec memory_status 序列化失败: {e}")))
}

/// 字节桥：`memory_admit`。
fn handle_memory_admit(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| PluginError::Manifest(format!("memory_admit 负载非合法 JSON: {e}")))?;
    let out = memory_admit(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec memory_admit 序列化失败: {e}")))
}

/// 字节桥：`idle_reclaim_plan`（v3.7.5 纯建议）。
fn handle_idle_reclaim(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| PluginError::Manifest(format!("idle_reclaim_plan 负载非合法 JSON: {e}")))?;
    let out = idle_reclaim_plan(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec idle_reclaim_plan 序列化失败: {e}")))
}

/// 字节桥：`reclaim_stats`（v3.7.6 只读统计汇总）。
fn handle_reclaim_stats(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| PluginError::Manifest(format!("reclaim_stats 负载非合法 JSON: {e}")))?;
    let out = reclaim_stats(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec reclaim_stats 序列化失败: {e}")))
}

/// 字节桥：`memory_primitive_status`（v3.7.6 Linux 专有原语诚实门）。
fn handle_memory_primitive(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = if payload.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(payload).map_err(|e| {
            PluginError::Manifest(format!("memory_primitive_status 负载非合法 JSON: {e}"))
        })?
    };
    let out = memory_primitive_query(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec memory_primitive_status 序列化失败: {e}")))
}

/// 字节桥：`cpu_priority`（v3.7.7 CPU 两级优先级 + 权重确定性仲裁）。
fn handle_cpu_priority(_method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
    let input: serde_json::Value = if payload.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(payload)
            .map_err(|e| PluginError::Manifest(format!("cpu_priority 负载非合法 JSON: {e}")))?
    };
    let out = cpu_priority_query(input).map_err(PluginError::Runtime)?;
    serde_json::to_vec(&out)
        .map_err(|e| PluginError::Runtime(format!("AUSec cpu_priority 序列化失败: {e}")))
}

/// 向 T0 进程内运行时注册 AUSec 系统插件的处理器（由系统插件装配流程调用）。
pub fn register(rt: &mut NativeRuntime) {
    // memory_status / memory_admit：v3.7.4 内存共享额度记账 + 两级超卖准入（只读决策）。
    rt.register_handler(AUSEC_PLUGIN, METHOD_MEMORY_STATUS, handle_memory_status);
    rt.register_handler(AUSEC_PLUGIN, METHOD_MEMORY_ADMIT, handle_memory_admit);
    // idle_reclaim_plan：v3.7.5 等待期保内存 + 空闲优先回收（只读纯建议）。
    rt.register_handler(AUSEC_PLUGIN, METHOD_IDLE_RECLAIM, handle_idle_reclaim);
    // reclaim_stats：v3.7.6 回收/超卖统计只读汇总（入账即 fail-closed 校验）。
    rt.register_handler(AUSEC_PLUGIN, METHOD_RECLAIM_STATS, handle_reclaim_stats);
    // memory_primitive_status：v3.7.6 Linux 专有内存原语诚实门（非 Linux 具名拒绝）。
    rt.register_handler(
        AUSEC_PLUGIN,
        METHOD_MEMORY_PRIMITIVE,
        handle_memory_primitive,
    );
    // cpu_priority：v3.7.7 CPU 两级优先级 + 权重确定性仲裁（只读，低信任自提级拒绝）。
    rt.register_handler(AUSEC_PLUGIN, METHOD_CPU_PRIORITY, handle_cpu_priority);
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

    #[test]
    fn pm_memory_status_dedup_and_oversubscription() {
        // 物理 1000、50×：两沙盒各独占 100、映射同一只读块 60。
        let out = memory_status(serde_json::json!({
            "physical_bytes": 1000,
            "overcommit_times": 50,
            "sandboxes": [
                {"sandbox_id": "a", "private_bytes": 100, "shared": [{"id": "x", "bytes": 60}]},
                {"sandbox_id": "b", "private_bytes": 100, "shared": [{"id": "x", "bytes": 60}]},
            ],
        }))
        .unwrap();
        assert_eq!(out["sandbox_count"], 2);
        assert_eq!(out["committed_bytes"], 260); // 200 独占 + 60 并集
        assert_eq!(out["nominal_bytes"], 320); // 共享重复计
        assert_eq!(out["shared_dedup_saving"], 60);
        assert_eq!(out["committed_utilization_permille"], 260);
        assert_eq!(out["observed_overcommit_permille"], 1230); // 320/260*1000
        assert_eq!(out["nominal_ceiling"], 50000);
        assert_eq!(out["distinct_shared_contents"], 1);
    }

    #[test]
    fn pm_memory_admit_gates_fail_closed() {
        // 物理闸：只放 a（committed 160），候选 b(100+x) 把 committed 推到 260 > 250。
        let err_phys = memory_admit(serde_json::json!({
            "physical_bytes": 250,
            "overcommit_times": 50,
            "sandboxes": [
                {"sandbox_id": "a", "private_bytes": 100, "shared": [{"id": "x", "bytes": 60}]},
            ],
            "candidate": {"sandbox_id": "b", "private_bytes": 100,
                          "shared": [{"id": "x", "bytes": 60}]},
        }))
        .unwrap_err();
        assert!(err_phys.contains("物理内存硬闸"), "got: {err_phys}");

        // 名义超卖闸：同一大块被 3 个沙盒映射，2× 超卖；第 3 个名义 2700 > 2000，
        // 但 committed 仅 900（未触物理闸），因此必须精确命中超卖闸。
        let err_oc = memory_admit(serde_json::json!({
            "physical_bytes": 1000,
            "overcommit_times": 2,
            "sandboxes": [
                {"sandbox_id": "a", "private_bytes": 0, "shared": [{"id": "big", "bytes": 900}]},
                {"sandbox_id": "b", "private_bytes": 0, "shared": [{"id": "big", "bytes": 900}]},
            ],
            "candidate": {"sandbox_id": "c", "private_bytes": 0,
                          "shared": [{"id": "big", "bytes": 900}]},
        }))
        .unwrap_err();
        assert!(err_oc.contains("超卖上限闸"), "got: {err_oc}");

        // 准入通过时返回完整投影。
        let ok = memory_admit(serde_json::json!({
            "physical_bytes": 1000,
            "overcommit_times": 2,
            "sandboxes": [
                {"sandbox_id": "a", "private_bytes": 0, "shared": [{"id": "big", "bytes": 900}]},
            ],
            "candidate": {"sandbox_id": "b", "private_bytes": 0,
                          "shared": [{"id": "big", "bytes": 900}]},
        }))
        .unwrap();
        assert_eq!(ok["admitted"], true);
        assert_eq!(ok["committed_bytes"], 900);
        assert_eq!(ok["nominal_bytes"], 1800);
        assert_eq!(ok["shared_dedup_saving"], 900);
    }

    #[test]
    fn pm_idle_reclaim_plan_waits_preserves_and_shortfall() {
        // 空闲优先：Waiting/Tolerant/空闲久的先收；保留内存不动；缺口如实返回。
        let out = idle_reclaim_plan(serde_json::json!({
            "target_bytes": 120,
            "observations": [
                {"sandbox_id": "run", "state": "Running", "idle_ms": 5,
                 "latency": "Sensitive", "reserved_bytes": 900, "reclaimable_idle_bytes": 100},
                {"sandbox_id": "wait", "state": "Waiting", "idle_ms": 9000,
                 "latency": "Tolerant", "reserved_bytes": 5000, "reclaimable_idle_bytes": 30},
            ],
        }))
        .unwrap();
        // 先收 wait 的 30，仍缺 90；不向 run 之外伸手——run 还有 100，所以应继续收 run 90 足额。
        assert_eq!(out["reclaimed_bytes"], 120);
        assert_eq!(out["sufficient"], true);
        assert_eq!(out["shortfall_bytes"], 0);
        assert_eq!(out["steps"][0]["sandbox_id"], "wait");
        assert_eq!(out["steps"][0]["bytes"], 30);
        assert_eq!(out["steps"][1]["sandbox_id"], "run");
        assert_eq!(out["steps"][1]["bytes"], 90);

        // 保留内存不被回收：只有 wait（保留 5000、空闲 30），target=100 → 回收 30、缺口 70。
        let short = idle_reclaim_plan(serde_json::json!({
            "target_bytes": 100,
            "observations": [
                {"sandbox_id": "wait", "state": "Waiting", "idle_ms": 9000,
                 "latency": "Tolerant", "reserved_bytes": 5000, "reclaimable_idle_bytes": 30},
            ],
        }))
        .unwrap();
        assert_eq!(short["reclaimed_bytes"], 30);
        assert_eq!(short["shortfall_bytes"], 70);
        assert_eq!(short["sufficient"], false);

        // 无观测（缺省空数组）：不报错，返回不足额 + 全额缺口（无沙盒可回收）。
        let none = idle_reclaim_plan(serde_json::json!({"target_bytes": 10})).unwrap();
        assert_eq!(none["sufficient"], false);
        assert_eq!(none["reclaimed_bytes"], 0);
        assert_eq!(none["shortfall_bytes"], 10);
        // 缺 target_bytes 才 fail-closed。
        assert!(idle_reclaim_plan(serde_json::json!({})).is_err());
    }

    #[test]
    fn pm_reclaim_stats_accumulates_and_rejects_forged_records() {
        // 合法：两条计划（60 不足 + 40 足额），累计 100/140。
        let ok = reclaim_stats(serde_json::json!({
            "records": [
                {"target_bytes": 100, "reclaimed_bytes": 60, "shortfall_bytes": 40,
                 "sufficient": false,
                 "steps": [{"sandbox_id": "a", "bytes": 60, "reason": "idle"}]},
                {"target_bytes": 40, "reclaimed_bytes": 40, "shortfall_bytes": 0,
                 "sufficient": true,
                 "steps": [{"sandbox_id": "b", "bytes": 40, "reason": "idle"}]},
            ],
        }))
        .unwrap();
        assert_eq!(ok["records"], 2);
        assert_eq!(ok["cumulative_reclaimed_bytes"], 100);
        assert_eq!(ok["cumulative_target_bytes"], 140);
        assert_eq!(ok["distinct_reclaimed_sandboxes"], 2);
        assert_eq!(ok["sufficient_records"], 1);
        assert_eq!(ok["insufficient_records"], 1);

        // 伪造记录（步骤合计≠声明）必须经 PM 入口 fail-closed。
        let forged = reclaim_stats(serde_json::json!({
            "records": [
                {"target_bytes": 100, "reclaimed_bytes": 60, "shortfall_bytes": 40,
                 "sufficient": false,
                 "steps": [{"sandbox_id": "a", "bytes": 40, "reason": "idle"}]},
            ],
        }));
        assert!(forged.is_err());

        // 0 字节步骤拒绝。
        let zero = reclaim_stats(serde_json::json!({
            "records": [
                {"target_bytes": 0, "reclaimed_bytes": 0, "shortfall_bytes": 0,
                 "sufficient": true,
                 "steps": [{"sandbox_id": "a", "bytes": 0, "reason": "x"}]},
            ],
        }));
        assert!(zero.is_err());

        // 观测字节溢出拒绝。
        let ovf = reclaim_stats(serde_json::json!({
            "observations": [
                {"sandbox_id": "a", "state": "Waiting", "idle_ms": 1, "latency": "Tolerant",
                 "reserved_bytes": u64::MAX, "reclaimable_idle_bytes": 1},
            ],
        }));
        assert!(ovf.is_err());
    }

    #[test]
    fn pm_memory_primitive_named_reject_off_linux_and_declared_on_linux() {
        // 非 Linux：四原语全部 unsupported、can_enforce=false。
        for plat in ["macos", "windows", "other"] {
            let out = memory_primitive_query(serde_json::json!({"platform": plat})).unwrap();
            let arr = out["primitives"].as_array().unwrap();
            assert_eq!(arr.len(), 4);
            for s in arr {
                assert_eq!(s["state"], "unsupported", "{plat} {}", s["primitive"]);
                assert_eq!(s["can_enforce"], false);
            }
        }
        // Linux：declared_linux 但 can_enforce 仍 false（只声明、执行器未接线）。
        let lin = memory_primitive_query(serde_json::json!({"platform": "linux"})).unwrap();
        for s in lin["primitives"].as_array().unwrap() {
            assert_eq!(s["state"], "declared_linux");
            assert_eq!(s["can_enforce"], false);
        }
        // 单原语查询 + 连写别名。
        let one = memory_primitive_query(
            serde_json::json!({"primitive": "virtio-pmem", "platform": "macos"}),
        )
        .unwrap();
        assert_eq!(one["primitives"][0]["primitive"], "virtio_pmem");
        assert_eq!(one["primitives"][0]["state"], "unsupported");
        // 未知原语/平台 fail-closed。
        assert!(memory_primitive_query(serde_json::json!({"primitive": "ksm"})).is_err());
        assert!(memory_primitive_query(serde_json::json!({"platform": "plan9"})).is_err());
    }

    #[test]
    fn pm_cpu_priority_arbitrates_and_rejects_self_promotion() {
        // system 缺省 sensitive 排前；第三方缺省/显式 tolerant 在后。
        let ok = cpu_priority_query(serde_json::json!({
            "capacity_millis": 1000,
            "sandboxes": [
                {"sandbox_id": "bg", "tier": "third_party", "requested_millis": 500},
                {"sandbox_id": "core", "tier": "system", "requested_millis": 300},
                {"sandbox_id": "off", "requested_millis": 200}, // tier 缺省=third_party
            ],
        }))
        .unwrap();
        let entries = ok["model"]["entries"].as_array().unwrap();
        assert_eq!(entries[0]["sandbox_id"], "core");
        assert_eq!(entries[0]["effective_latency"], "Sensitive");
        assert_eq!(entries[1]["effective_latency"], "Tolerant");
        assert_eq!(entries[2]["effective_latency"], "Tolerant");
        assert_eq!(ok["model"]["sensitive"]["count"], 1);

        // 低信任级自报 sensitive：经 PM 入口 fail-closed。
        let err = cpu_priority_query(serde_json::json!({
            "capacity_millis": 1000,
            "sandboxes": [
                {"sandbox_id": "x", "tier": "certified", "latency": "sensitive",
                 "requested_millis": 100},
            ],
        }))
        .unwrap_err();
        assert!(err.contains("不得自我提升为时延敏感"), "got: {err}");

        // 缺 capacity、权重越界、非法 tier 全部拒绝。
        assert!(cpu_priority_query(serde_json::json!({"sandboxes": []})).is_err());
        assert!(cpu_priority_query(serde_json::json!({
            "capacity_millis": 1000,
            "sandboxes": [{"sandbox_id": "a", "tier": "system", "weight": 0,
                           "requested_millis": 1}],
        }))
        .is_err());
        assert!(cpu_priority_query(serde_json::json!({
            "capacity_millis": 1000,
            "sandboxes": [{"sandbox_id": "a", "tier": "kernel", "requested_millis": 1}],
        }))
        .is_err());
    }
}
