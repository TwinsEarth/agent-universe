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
//!
//! T0 以 [`crate::plugin::runtime::native::NativeRuntime`] 进程内承载，
//! 处理器是内核的真实函数，不经过隔离。

use crate::identity::{Did, Keypair};
use crate::plugin::manifest::{
    Capabilities, ManifestSignature, PluginInfo, PluginLimits, PluginManifest,
};
use crate::plugin::runtime::native::NativeRuntime;
use std::collections::BTreeMap;

/// 系统插件：身份。
pub const SYS_IDENTITY: &str = "com.twinsearth.sys.identity";
/// 系统插件：网络。
pub const SYS_NET: &str = "com.twinsearth.sys.net";
/// 系统插件：存储。
pub const SYS_STORAGE: &str = "com.twinsearth.sys.storage";
/// 系统插件：链。
pub const SYS_CHAIN: &str = "com.twinsearth.sys.chain";

/// 全部系统插件 id（构建/装配顺序）。
pub fn system_ids() -> Vec<&'static str> {
    vec![SYS_IDENTITY, SYS_NET, SYS_STORAGE, SYS_CHAIN]
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

/// 把系统插件的真实处理器注册到 T0 运行时。
///
/// 目前 [`SYS_IDENTITY`] 提供真实可用的方法；其余 T0 插件的能力由宿主装配
/// （网络守护进程、存储句柄）在 [`crate::plugin::host::PluginHost`] 中接线。
pub fn register_handlers(rt: &mut NativeRuntime) {
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

    // 其余 T0 系统插件：注册真实的 status 方法，声明其职责。
    // 真实的网络/存储/链句柄由宿主在装配时接线（daemon 启动时），这里返回
    // 代码里真实存在的职责声明，不编造运行状态。
    for (id, declared) in [
        (
            SYS_NET,
            &["libp2p-transport", "kademlia-dht", "gossipsub"][..],
        ),
        (SYS_STORAGE, &["crdt", "erasure-coding"][..]),
        (SYS_CHAIN, &["evm-light-client", "chain-anchor"][..]),
    ] {
        let id_owned = id.to_string();
        let declared_owned = declared.to_vec();
        rt.register_handler(id, "status", move |_m, _p| {
            serde_json::to_vec(&serde_json::json!({
                "plugin": id_owned,
                "declared_capabilities": declared_owned,
            }))
            .map_err(rt_err)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::PluginRuntime;

    #[test]
    fn bundled_manifests_are_system_tier() {
        let ms = bundled_manifests("3.0.0");
        assert_eq!(ms.len(), 4);
        for m in &ms {
            assert!(m.plugin.name.starts_with("com.twinsearth.sys."));
        }
    }

    #[test]
    fn mint_and_parse_did_roundtrip() {
        let mut rt = NativeRuntime::new();
        register_handlers(&mut rt);
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
        register_handlers(&mut rt);
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
        register_handlers(&mut rt);
        let mut inst = rt.spawn(&system_manifest(SYS_IDENTITY, "3.0.0")).unwrap();
        let p = serde_json::to_vec(&serde_json::json!({"did": "not-a-did"})).unwrap();
        let r = inst.call("parse_did", &p).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&r).unwrap();
        assert_eq!(v["valid"], false);
    }
}
