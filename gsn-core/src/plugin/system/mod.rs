//! 系统插件（T0，Ring 0）—— 随内核发布、进程内、不可热插拔
//!
//! # 有哪些
//!
//! | 插件 id | 职责 |
//! |---|---|
//! | [`SYS_IDENTITY`] | DID 身份与 Ed25519（铸造/解析/指纹） |
//! | [`SYS_NET`] | 网络传输（libp2p 宿主装配） |
//! | [`SYS_STORAGE`] | 存储（账本/CRDT/纠删码） |
//! | [`SYS_CHAIN`] | 链上锚定 / EVM |
//! | [`SYS_AUSEC`] | AUSec 弹性计算：执行后端选择/就绪（v3.7.0 起） |
//! | [`SYS_RESOURCE_MARKET`] | 面向 Agent 的资源市场：确定性记账内核/状态机（v3.8.0 起） |
//!
//! T0 以 [`crate::plugin::runtime::native::NativeRuntime`] 进程内承载，
//! 处理器是内核的真实函数，不经过隔离。

use crate::identity::{Did, Keypair};
use crate::plugin::manifest::{
    Capabilities, ManifestSignature, PluginInfo, PluginLimits, PluginManifest,
};
use crate::plugin::runtime::native::NativeRuntime;
use crate::storage::persist::PersistentStore;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

/// T0 系统插件接线的宿主句柄（daemon 启动时构造；某字段为 `None` 表示
/// 该能力在本进程不可用，对应方法不注册 —— 不编造运行状态）。
#[derive(Clone, Default)]
pub struct SystemHandles {
    /// 持久化存储（同步 SQLite）：SYS_STORAGE / SYS_CHAIN 用。
    pub store: Option<Arc<PersistentStore>>,
    /// 网络命令句柄（异步）：SYS_NET 用。
    pub peer_cmd_tx: Option<crate::node::PeerCmdTx>,
}

impl SystemHandles {
    /// 接线存储句柄。
    pub fn with_store(mut self, store: Arc<PersistentStore>) -> Self {
        self.store = Some(store);
        self
    }
    /// 接线网络句柄。
    pub fn with_peer(mut self, tx: crate::node::PeerCmdTx) -> Self {
        self.peer_cmd_tx = Some(tx);
        self
    }
}

/// 系统插件：身份。
pub const SYS_IDENTITY: &str = "com.twinsearth.sys.identity";
/// 系统插件：网络。
pub const SYS_NET: &str = "com.twinsearth.sys.net";
/// 系统插件：存储。
pub const SYS_STORAGE: &str = "com.twinsearth.sys.storage";
/// 系统插件：链。
pub const SYS_CHAIN: &str = "com.twinsearth.sys.chain";
/// 系统插件：AUSec 弹性计算（v3.7.0 起）。
pub const SYS_AUSEC: &str = crate::ausec::AUSEC_PLUGIN;
/// 系统插件：面向 Agent 的资源市场确定性记账内核（v3.8.0 起）。
pub const SYS_RESOURCE_MARKET: &str = crate::economy::resource::RESOURCE_MARKET_PLUGIN;
/// 系统插件：结算路由（v3.9.0 起，只决策不动钱）。
pub const SYS_PAYMENT_ROUTER: &str = crate::economy::payment::PAYMENT_ROUTER_PLUGIN;

/// 全部系统插件 id（构建/装配顺序）。
pub fn system_ids() -> Vec<&'static str> {
    vec![
        SYS_IDENTITY,
        SYS_NET,
        SYS_STORAGE,
        SYS_CHAIN,
        SYS_AUSEC,
        SYS_RESOURCE_MARKET,
        SYS_PAYMENT_ROUTER,
    ]
}

/// 构造一个 T0 系统插件清单（随内核，无外部签名）。
pub fn system_manifest(id: &str, version: &str) -> PluginManifest {
    PluginManifest {
        plugin: PluginInfo {
            name: id.to_string(),
            version: version.to_string(),
            abi: "3.0".to_string(),
            entry: "native".to_string(),
            publisher: "twinsearth".to_string(),
            module_sha256: String::new(),
        },
        capabilities: Capabilities::default(),
        limits: PluginLimits::default(),
        waivers: BTreeMap::new(),
        signature: ManifestSignature::default(),
    }
}

/// 本构建内随内核打包的全部 T0 系统插件清单。
pub fn bundled_manifests(version: &str) -> Vec<PluginManifest> {
    system_ids()
        .into_iter()
        .map(|id| system_manifest(id, version))
        .collect()
}

/// 把 Display 错误统一转成运行时类型化错误（处理器闭包用）。
fn rt_err<E: std::fmt::Display>(e: E) -> crate::plugin::error::PluginError {
    crate::plugin::error::PluginError::Runtime(e.to_string())
}

/// 在同步处理器中阻塞等待一个异步网络查询。
///
/// 系统插件处理器经 [`crate::plugin::host::PluginHost::call`] 调用，
/// 而编排器/插件 API 的 host.call 都在 `spawn_blocking` 线程上执行；
/// 该线程不驱动 runtime，故 `Handle::block_on` 不会自死锁。
fn block_on_net<T>(fut: impl std::future::Future<Output = T>) -> Result<T, String> {
    let h = tokio::runtime::Handle::try_current().map_err(|e| format!("异步运行时不可用: {e}"))?;
    Ok(h.block_on(fut))
}

/// 把系统插件的真实处理器注册到 T0 运行时。
///
/// 目前 [`SYS_IDENTITY`] 提供真实可用的方法；其余 T0 插件的能力由宿主装配
/// （网络守护进程、存储句柄）在 [`crate::plugin::host::PluginHost`] 中接线。
pub fn register_handlers(rt: &mut NativeRuntime, handles: &SystemHandles) {
    // mint_did：铸造新身份 → {did, public_key}。
    // 私钥由 keyring/宿主保存，这里只返回公钥与 DID（不把私钥泄出进程）。
    rt.register_handler(SYS_IDENTITY, "mint_did", |_m, payload| {
        // 允许从可选 seed 派生；无 seed 则随机生成。
        let v: serde_json::Value = if payload.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_slice(payload).unwrap_or(serde_json::json!({}))
        };
        let kp = match v.get("seed") {
            Some(s) => {
                let seed_bytes =
                    hex::decode(s.as_str().ok_or_else(|| rt_err("seed 必须是 hex 字符串"))?)
                        .map_err(rt_err)?;
                if seed_bytes.len() != 32 {
                    return Err(rt_err("seed 必须为 32 字节"));
                }
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&seed_bytes);
                Keypair::from_seed(&seed)
            }
            None => Keypair::generate(),
        };
        let did = Did::from_public_key(kp.public_key());
        serde_json::to_vec(&serde_json::json!({
            "did": did.as_str(),
            "public_key": hex::encode(kp.public_key()),
        }))
        .map_err(rt_err)
    });

    // parse_did：解析并校验 → {valid, method, identifier}。
    rt.register_handler(SYS_IDENTITY, "parse_did", |_m, payload| {
        let v: serde_json::Value = serde_json::from_slice(payload).map_err(rt_err)?;
        let s = v
            .get("did")
            .and_then(|x| x.as_str())
            .ok_or_else(|| rt_err("缺少 did"))?;
        match Did::parse(s) {
            Ok(d) => serde_json::to_vec(&serde_json::json!({
                "valid": true,
                "method": d.method(),
                "identifier": d.identifier(),
            }))
            .map_err(rt_err),
            Err(e) => {
                serde_json::to_vec(&serde_json::json!({"valid": false, "error": e})).map_err(rt_err)
            }
        }
    });

    // fingerprint：从公钥算 8 字节指纹。
    rt.register_handler(SYS_IDENTITY, "fingerprint", |_m, payload| {
        let v: serde_json::Value = serde_json::from_slice(payload).map_err(rt_err)?;
        let pk_hex = v
            .get("public_key")
            .and_then(|x| x.as_str())
            .ok_or_else(|| rt_err("缺少 public_key"))?;
        let pk = hex::decode(pk_hex).map_err(rt_err)?;
        serde_json::to_vec(&serde_json::json!({
            "fingerprint": Did::fingerprint(&pk),
        }))
        .map_err(rt_err)
    });

    // ===== SYS_NET：网络（真实 daemon peer 句柄） =====
    let net_declared = ["libp2p-transport", "kademlia-dht", "gossipsub"];
    rt.register_handler(SYS_NET, "status", move |_m, _p| {
        serde_json::to_vec(&serde_json::json!({
            "plugin": SYS_NET,
            "declared_capabilities": net_declared,
        }))
        .map_err(rt_err)
    });
    if let Some(peer_tx) = &handles.peer_cmd_tx {
        // peer_info：本机 peer 信息。
        let tx = peer_tx.clone();
        rt.register_handler(SYS_NET, "peer_info", move |_m, _p| {
            let (reply, rx) = tokio::sync::oneshot::channel();
            block_on_net(tx.send(crate::node::PeerCommand::GetInfo { reply }))
                .map_err(rt_err)?
                .map_err(|_| rt_err("网络 actor 不可达"))?;
            let info = block_on_net(rx)
                .map_err(rt_err)?
                .map_err(|_| rt_err("peer_info 无响应"))?;
            serde_json::to_vec(&info).map_err(rt_err)
        });
        // list_peers：已连接对等节点。
        let tx = peer_tx.clone();
        rt.register_handler(SYS_NET, "list_peers", move |_m, _p| {
            let (reply, rx) = tokio::sync::oneshot::channel();
            block_on_net(tx.send(crate::node::PeerCommand::ListPeers { reply }))
                .map_err(rt_err)?
                .map_err(|_| rt_err("网络 actor 不可达"))?;
            let peers = block_on_net(rx)
                .map_err(rt_err)?
                .map_err(|_| rt_err("list_peers 无响应"))?;
            serde_json::to_vec(&serde_json::json!({ "peers": peers, "count": peers.len() }))
                .map_err(rt_err)
        });
        // nat_status：AutoNAT 检测的 NAT 状态（不猜测）。
        let tx = peer_tx.clone();
        rt.register_handler(SYS_NET, "nat_status", move |_m, _p| {
            let (reply, rx) = tokio::sync::oneshot::channel();
            block_on_net(tx.send(crate::node::PeerCommand::NatStatus { reply }))
                .map_err(rt_err)?
                .map_err(|_| rt_err("网络 actor 不可达"))?;
            let nat = block_on_net(rx)
                .map_err(rt_err)?
                .map_err(|_| rt_err("nat_status 无响应"))?;
            serde_json::to_vec(&serde_json::json!({ "nat_type": nat })).map_err(rt_err)
        });
    }

    // ===== SYS_STORAGE：存储（真实 store 句柄，同步查询） =====
    let storage_declared = ["crdt", "erasure-coding"];
    rt.register_handler(SYS_STORAGE, "status", move |_m, _p| {
        serde_json::to_vec(&serde_json::json!({
            "plugin": SYS_STORAGE,
            "declared_capabilities": storage_declared,
        }))
        .map_err(rt_err)
    });
    if let Some(store) = &handles.store {
        // stats：各表计数。
        let s = store.clone();
        rt.register_handler(SYS_STORAGE, "stats", move |_m, _p| {
            serde_json::to_vec(&serde_json::json!({
                "agents": s.agent_count().map_err(rt_err)?,
                "tasks": s.task_count().map_err(rt_err)?,
                "relays": s.relay_count().map_err(rt_err)?,
                "healthy_relays": s.healthy_relay_count().map_err(rt_err)?,
                "ledger_records": s.ledger_count().map_err(rt_err)?,
            }))
            .map_err(rt_err)
        });
        // list_agents：持久化的 agent。
        let s = store.clone();
        rt.register_handler(SYS_STORAGE, "list_agents", move |_m, _p| {
            let agents = s.load_agents().map_err(rt_err)?;
            let arr: Vec<Value> = agents
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "agent_id": a.agent_id, "name": a.name, "skills": a.skills,
                        "stake": a.stake, "reputation": a.reputation, "created_at": a.created_at,
                    })
                })
                .collect();
            serde_json::to_vec(&serde_json::json!({ "agents": arr, "count": arr.len() }))
                .map_err(rt_err)
        });
        // list_tasks：持久化的 task。
        let s = store.clone();
        rt.register_handler(SYS_STORAGE, "list_tasks", move |_m, _p| {
            let tasks = s.load_tasks().map_err(rt_err)?;
            let arr: Vec<Value> = tasks
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "task_id": t.task_id, "goal": t.goal, "state": t.state,
                        "owner": t.owner, "budget": t.budget, "winner_price": t.winner_price,
                        "verification_policy": t.verification_policy, "requester": t.requester,
                        "deadline": t.deadline, "created_at": t.created_at,
                    })
                })
                .collect();
            serde_json::to_vec(&serde_json::json!({ "tasks": arr, "count": arr.len() }))
                .map_err(rt_err)
        });
    }

    // ===== SYS_CHAIN：链上锚定（本地锚定记录；RPC 离线，诚实标注） =====
    let chain_declared = ["evm-light-client", "chain-anchor"];
    rt.register_handler(SYS_CHAIN, "status", move |_m, _p| {
        serde_json::to_vec(&serde_json::json!({
            "plugin": SYS_CHAIN,
            "declared_capabilities": chain_declared,
            "rpc": "offline",
        }))
        .map_err(rt_err)
    });
    if let Some(store) = &handles.store {
        // record_anchor：把一条锚定证明存入本地记录（kv_meta: chain_anchors）。
        let s = store.clone();
        rt.register_handler(SYS_CHAIN, "record_anchor", move |_m, payload| {
            let v: Value = serde_json::from_slice(payload).map_err(rt_err)?;
            let anchor = v.get("anchor").cloned().unwrap_or(v);
            let mut list: Vec<Value> = s
                .get_meta("chain_anchors")
                .map_err(rt_err)?
                .and_then(|x| serde_json::from_str(&x).ok())
                .unwrap_or_default();
            list.push(anchor);
            s.set_meta(
                "chain_anchors",
                &serde_json::to_string(&list).map_err(rt_err)?,
            )
            .map_err(rt_err)?;
            serde_json::to_vec(&serde_json::json!({ "recorded": true, "total": list.len() }))
                .map_err(rt_err)
        });
        // list_anchors：读取本地锚定记录。
        let s = store.clone();
        rt.register_handler(SYS_CHAIN, "list_anchors", move |_m, _p| {
            let list: Vec<Value> = s
                .get_meta("chain_anchors")
                .map_err(rt_err)?
                .and_then(|x| serde_json::from_str(&x).ok())
                .unwrap_or_default();
            serde_json::to_vec(&serde_json::json!({ "anchors": list, "count": list.len() }))
                .map_err(rt_err)
        });
    }

    // ===== SYS_AUSEC：弹性计算后端选择/就绪（无外部句柄依赖，恒注册） =====
    // 处理器本身是只读选择/查询；真实执行后端未接线时由 readiness 诚实拒绝。
    crate::ausec::register(rt);

    // ===== SYS_RESOURCE_MARKET：资源市场确定性记账内核（v3.8.0 起，恒注册） =====
    // 基座只暴露只读状态；撮合/计量/托管/质押处理器在后续小版本接线，未接线不注册。
    crate::economy::resource::register(rt);

    // ===== SYS_PAYMENT_ROUTER：结算路由（v3.9.0 起，恒注册） =====
    // 只注册只读状态与确定性选路面；执行/签名/链上写在后续小版本接线，未接线不注册。
    crate::economy::payment::register(rt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::PluginRuntime;

    #[test]
    fn bundled_manifests_are_system_tier() {
        let ms = bundled_manifests("3.0.0");
        assert_eq!(ms.len(), system_ids().len());
        for m in &ms {
            assert!(m.plugin.name.starts_with("com.twinsearth.sys."));
        }
        // AUSec 必须在系统插件装配集中（v3.7.0 起）。
        assert!(system_ids().contains(&SYS_AUSEC));
    }

    #[test]
    fn ausec_system_plugin_status_and_select() {
        // 经系统插件真实装配 → spawn → call 路径验证 AUSec 处理器（不是孤立单测）。
        let mut rt = NativeRuntime::new();
        register_handlers(&mut rt, &SystemHandles::default());
        let mut inst = rt.spawn(&system_manifest(SYS_AUSEC, "3.0.0")).unwrap();

        let st: serde_json::Value =
            serde_json::from_slice(&inst.call("status", b"{}").unwrap()).unwrap();
        assert_eq!(st["plugin"], SYS_AUSEC);
        assert_eq!(st["backends"].as_array().unwrap().len(), 4);

        let sel = serde_json::to_vec(&serde_json::json!({
            "plugin_id": "com.twinsearth.sys.identity"
        }))
        .unwrap();
        let out: serde_json::Value =
            serde_json::from_slice(&inst.call("select_backend", &sel).unwrap()).unwrap();
        assert_eq!(out["backend"], "fncall");
    }

    #[test]
    fn resource_market_system_plugin_status() {
        // 经系统插件真实装配 → spawn → call 路径验证资源市场内核处理器。
        let mut rt = NativeRuntime::new();
        register_handlers(&mut rt, &SystemHandles::default());
        let mut inst = rt
            .spawn(&system_manifest(SYS_RESOURCE_MARKET, "3.0.0"))
            .unwrap();

        let st: serde_json::Value =
            serde_json::from_slice(&inst.call("resource_market_status", b"{}").unwrap()).unwrap();
        assert_eq!(st["plugin"], SYS_RESOURCE_MARKET);
        assert_eq!(st["introduced_in"], "v3.8.0");
        // 记账单位是整数 micro，且本版本诚实标注不挂法币/链。
        assert_eq!(st["accounting_unit"]["integer_only"], true);
        assert_eq!(st["accounting_unit"]["fiat_pegged"], false);
        assert_eq!(st["accounting_unit"]["chain_settled"], false);
        // 四类闲置资源都在目录中。
        assert_eq!(st["resource_kinds"].as_array().unwrap().len(), 4);
        // 托管结算守恒内核（v3.8.5）+ 快照商品化版税（v3.8.6）已就绪；QA/审判驱动
        // 罚没与链上支付仍未强制。
        assert_eq!(st["enforceable"]["escrow_settlement"], true);
        assert_eq!(st["enforceable"]["snapshot_royalty"], true);
        assert_eq!(st["enforceable"]["reputation_scoring"], true);
        assert_eq!(st["enforceable"]["bft_lite_qa"], true);
        assert_eq!(st["enforceable"]["dynamic_pricing"], true);
        assert_eq!(st["enforceable"]["order_batching"], true);
        assert_eq!(st["enforceable"]["bootstrap_gate"], true);
        assert_eq!(st["enforceable"]["stake_slash"], false);
        assert_eq!(st["enforceable"]["onchain_payment"], false);

        // 未接线方法不注册（NotFound），不伪造可用。
        assert!(inst.call("offer_ask_submit", b"{}").is_err());
    }

    #[test]
    fn payment_router_system_plugin_status() {
        // 经系统插件真实装配 → spawn → call 路径验证结算/签名处理器（v3.9.0 路由 +
        // v3.9.1 宿主签名闸门）。
        let mut rt = NativeRuntime::new();
        register_handlers(&mut rt, &SystemHandles::default());
        let mut inst = rt
            .spawn(&system_manifest(SYS_PAYMENT_ROUTER, "3.0.0"))
            .unwrap();

        let st: serde_json::Value =
            serde_json::from_slice(&inst.call("payment_router_status", b"{}").unwrap()).unwrap();
        assert_eq!(st["plugin"], SYS_PAYMENT_ROUTER);
        assert_eq!(st["introduced_in"], "v3.9.0");
        assert_eq!(st["signing_introduced_in"], "v3.9.1");
        // 金额内部整数 micro，不挂法币。
        assert_eq!(st["amount_unit"]["integer_only"], true);
        assert_eq!(st["amount_unit"]["fiat_pegged"], false);
        // 三条结算轨道。
        assert_eq!(st["tracks"].as_array().unwrap().len(), 3);
        // 选路决策与签名 fail-closed 强制；资金/沙盒持钥/链上写动作诚实标注未提供。
        assert_eq!(st["enforceable"]["deterministic_route_decision"], true);
        assert_eq!(
            st["enforceable"]["fail_closed_when_track_unavailable"],
            true
        );
        assert_eq!(
            st["enforceable"]["fail_closed_when_signer_unconfigured"],
            true
        );
        assert_eq!(st["enforceable"]["fund_movement"], false);
        assert_eq!(st["enforceable"]["key_holding_in_sandbox"], false);
        assert_eq!(
            st["enforceable"]["host_only_signing_private_key_isolation"],
            true
        );
        assert_eq!(
            st["enforceable"]["sandbox_direct_transaction_signing"],
            false
        );
        assert_eq!(st["enforceable"]["transaction_broadcast"], false);
        assert_eq!(st["enforceable"]["onchain_anchor_write"], false);
        assert_eq!(st["provided"]["payment_router_status"], true);
        assert_eq!(st["provided"]["wallet_sign_preview"], true);
        assert_eq!(st["provided"]["lightning_node_connection"], false);
        assert_eq!(st["signing"]["private_key_enters_sandbox"], false);
        assert_eq!(
            st["signing"]["production_default_host_key_configured"],
            false
        );
        // v3.9.1 起声明只读选路 + 宿主受限签名（2 个）；执行/链上写仅登记后续。
        assert_eq!(st["capabilities_declared"].as_array().unwrap().len(), 2);

        // v3.9.1：wallet_sign_preview 已注册且对合法意图可用（不产出签名）。
        let digest = serde_json::Value::Array(vec![serde_json::Value::Number(7.into()); 32]);
        let intent = serde_json::json!({
            "track": "evm_x402",
            "domain": "x402.agent-universe.local",
            "message_digest": digest,
            "spend_cap_micro": 1000u64,
            "nonce": 1u64,
            "auth_only": false
        });
        let payload = serde_json::to_vec(&intent).unwrap();
        let prev: serde_json::Value =
            serde_json::from_slice(&inst.call("wallet_sign_preview", &payload).unwrap()).unwrap();
        assert_eq!(prev["host_key_configured"], false);
        assert!(prev.get("signature_hex").is_none());

        // wallet_sign 已注册，但生产默认未配置密钥 → 具名 SignerNotConfigured（NotFound
        // 之外的运行时类型化拒绝），不伪造签名；空/坏负载同样拒绝。
        let sign_err = inst.call("wallet_sign", &payload).unwrap_err().to_string();
        assert!(
            sign_err.contains("PAYMENT_SIGNER_NOT_CONFIGURED"),
            "got {sign_err}"
        );
        assert!(inst.call("wallet_sign", b"{}").is_err());
        // pay_execute 尚未接线（NotFound），不伪造可用。
        assert!(inst.call("pay_execute", b"{}").is_err());
    }

    #[test]
    fn mint_and_parse_did_roundtrip() {
        let mut rt = NativeRuntime::new();
        register_handlers(&mut rt, &SystemHandles::default());
        let mut inst = rt.spawn(&system_manifest(SYS_IDENTITY, "3.0.0")).unwrap();
        let minted = inst.call("mint_did", b"{}").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&minted).unwrap();
        let did = v["did"].as_str().unwrap();
        assert!(did.starts_with("did:nau:"));

        // parse 该 did。
        let parse_payload = serde_json::to_vec(&serde_json::json!({"did": did})).unwrap();
        let parsed = inst.call("parse_did", &parse_payload).unwrap();
        let pv: serde_json::Value = serde_json::from_slice(&parsed).unwrap();
        assert_eq!(pv["valid"], true);
        assert_eq!(pv["method"], "nau");
    }

    #[test]
    fn mint_did_from_seed_is_deterministic() {
        let mut rt = NativeRuntime::new();
        register_handlers(&mut rt, &SystemHandles::default());
        let mut inst = rt.spawn(&system_manifest(SYS_IDENTITY, "3.0.0")).unwrap();
        let seed = "01".repeat(32);
        let p = serde_json::to_vec(&serde_json::json!({"seed": seed})).unwrap();
        let a = inst.call("mint_did", &p).unwrap();
        let b = inst.call("mint_did", &p).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn parse_bad_did_reports_invalid() {
        let mut rt = NativeRuntime::new();
        register_handlers(&mut rt, &SystemHandles::default());
        let mut inst = rt.spawn(&system_manifest(SYS_IDENTITY, "3.0.0")).unwrap();
        let p = serde_json::to_vec(&serde_json::json!({"did": "not-a-did"})).unwrap();
        let r = inst.call("parse_did", &p).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&r).unwrap();
        assert_eq!(v["valid"], false);
    }

    // ── B3：系统插件真实句柄接线 ──
    fn tmp_db_path(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gsn_sys_b3_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("gsn.db")
    }

    fn sample_agent() -> crate::storage::persist::StoredAgent {
        crate::storage::persist::StoredAgent {
            agent_id: "did:nau:bob".into(),
            name: "Bob".into(),
            skills: "python".into(),
            stake: 100,
            reputation: 0.7,
            created_at: "2026-01-01".into(),
            card_json: None,
        }
    }

    fn sample_task() -> crate::storage::persist::StoredTask {
        crate::storage::persist::StoredTask {
            task_id: "t-1".into(),
            goal: "do".into(),
            state: "Open".into(),
            owner: None,
            budget: 500,
            created_at: "2026-01-01".into(),
            winner_price: None,
            verification_policy: "None".into(),
            requester: "did:nau:req".into(),
            deadline: 0,
            spec_json: None,
        }
    }

    #[test]
    fn storage_stats_and_lists_reflect_store() {
        use crate::plugin::host::PluginHost;
        let store = Arc::new(PersistentStore::open(tmp_db_path("storage")).unwrap());
        store.upsert_agent(&sample_agent()).unwrap();
        store.upsert_task(&sample_task()).unwrap();
        let mut host = PluginHost::new("3.4.5", None);
        host.boot_system(&SystemHandles::default().with_store(store.clone()))
            .unwrap();

        let sv: Value =
            serde_json::from_slice(&host.call(SYS_STORAGE, "stats", b"{}").unwrap()).unwrap();
        assert_eq!(sv["agents"], 1);
        assert_eq!(sv["tasks"], 1);

        let lav: Value =
            serde_json::from_slice(&host.call(SYS_STORAGE, "list_agents", b"{}").unwrap()).unwrap();
        assert_eq!(lav["count"], 1);
        assert_eq!(lav["agents"][0]["agent_id"], "did:nau:bob");

        let ltv: Value =
            serde_json::from_slice(&host.call(SYS_STORAGE, "list_tasks", b"{}").unwrap()).unwrap();
        assert_eq!(ltv["count"], 1);
        assert_eq!(ltv["tasks"][0]["task_id"], "t-1");
    }

    #[test]
    fn chain_record_and_list_anchors() {
        use crate::plugin::host::PluginHost;
        let store = Arc::new(PersistentStore::open(tmp_db_path("chain")).unwrap());
        let mut host = PluginHost::new("3.4.5", None);
        host.boot_system(&SystemHandles::default().with_store(store.clone()))
            .unwrap();

        let p1 = serde_json::to_vec(&serde_json::json!({
            "anchor": {"cid": "a"}, "tx": "0x1"
        }))
        .unwrap();
        let rv: Value =
            serde_json::from_slice(&host.call(SYS_CHAIN, "record_anchor", &p1).unwrap()).unwrap();
        assert_eq!(rv["recorded"], true);

        // 直传（无 anchor 包装）也接受。
        let p2 = serde_json::to_vec(&serde_json::json!({"cid": "b"})).unwrap();
        host.call(SYS_CHAIN, "record_anchor", &p2).unwrap();

        let lv: Value =
            serde_json::from_slice(&host.call(SYS_CHAIN, "list_anchors", b"{}").unwrap()).unwrap();
        assert_eq!(lv["count"], 2);

        // status 诚实标注 rpc offline。
        let stv: Value =
            serde_json::from_slice(&host.call(SYS_CHAIN, "status", b"{}").unwrap()).unwrap();
        assert_eq!(stv["rpc"], "offline");
    }

    #[test]
    fn methods_absent_without_handles() {
        use crate::plugin::host::PluginHost;
        let mut host = PluginHost::new("3.4.5", None);
        host.boot_system(&SystemHandles::default()).unwrap();
        // 未接线：对应方法不注册（NotFound）。
        assert!(host.call(SYS_CHAIN, "list_anchors", b"{}").is_err());
        assert!(host.call(SYS_NET, "peer_info", b"{}").is_err());
        // status 始终可用。
        assert!(host.call(SYS_NET, "status", b"{}").is_ok());
    }

    #[tokio::test]
    async fn net_methods_query_peer_handle() {
        use crate::node::{PeerCommand, PeerInfo};
        use crate::plugin::host::PluginHost;
        let (peer_tx, mut peer_rx) = tokio::sync::mpsc::channel::<PeerCommand>(16);
        // 消费 PeerCommand 的 mock swarm actor。
        tokio::spawn(async move {
            while let Some(cmd) = peer_rx.recv().await {
                match cmd {
                    PeerCommand::GetInfo { reply } => {
                        let _ = reply.send(PeerInfo {
                            peer_id: "12D3".into(),
                            connected: 3,
                            routing_entries: 5,
                            listen_addrs: vec!["/ip4/0.0.0.0/tcp/4001".into()],
                            bootstrapped: vec![],
                            nat_status: "Public".into(),
                        });
                    }
                    PeerCommand::ListPeers { reply } => {
                        let _ = reply.send(vec!["peer-a".into(), "peer-b".into()]);
                    }
                    PeerCommand::NatStatus { reply } => {
                        let _ = reply.send("Public".into());
                    }
                    _ => {}
                }
            }
        });

        let mut host = PluginHost::new("3.4.5", None);
        host.boot_system(&SystemHandles::default().with_peer(peer_tx))
            .unwrap();

        // 与真实编排器一致：host.call 在 spawn_blocking 线程执行。
        let (b, mut host) = tokio::task::spawn_blocking(move || {
            host.call(SYS_NET, "peer_info", b"{}").map(|b| (b, host))
        })
        .await
        .unwrap()
        .unwrap();
        let iv: Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(iv["peer_id"], "12D3");
        assert_eq!(iv["connected"], 3);

        let (b, mut host) = tokio::task::spawn_blocking(move || {
            host.call(SYS_NET, "list_peers", b"{}").map(|b| (b, host))
        })
        .await
        .unwrap()
        .unwrap();
        let pv: Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(pv["count"], 2);

        let b3 = tokio::task::spawn_blocking(move || host.call(SYS_NET, "nat_status", b"{}"))
            .await
            .unwrap()
            .unwrap();
        let nv: Value = serde_json::from_slice(&b3).unwrap();
        assert_eq!(nv["nat_type"], "Public");
    }
}
