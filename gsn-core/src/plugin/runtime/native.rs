//! T0 进程内运行时（NativeRuntime）—— 仅系统插件，不隔离
//!
//! 系统插件是内核组成部分，随内核在同一进程、同一地址空间运行，因此本运行时
//! **不提供任何隔离**；这正是只有 T0 能用它的原因。
//!
//! 系统插件以「方法处理器」形式注册：`(plugin_id, method) → handler`。

use crate::plugin::error::PluginResult;
use crate::plugin::manifest::PluginManifest;
use crate::plugin::runtime::{Bound, PluginInstance, PluginRuntime, RuntimeCapabilities};
use crate::plugin::tier::Tier;
use std::collections::BTreeMap;
use std::sync::Arc;

/// 处理器：接收 method + JSON 负载，返回 JSON 结果字节。
///
/// 用 `Arc` 以便 spawn 时共享同一份处理器（Box 无法 clone）。
type Handler = Arc<dyn Fn(&str, &[u8]) -> PluginResult<Vec<u8>> + Send + Sync>;

/// T0 进程内运行时。
pub struct NativeRuntime {
    /// 插件 id → 该插件的处理器集合（method → handler）。
    handlers: BTreeMap<String, BTreeMap<String, Handler>>,
    /// 存活实例 id。
    alive: Vec<String>,
}

impl NativeRuntime {
    pub fn new() -> Self {
        NativeRuntime {
            handlers: BTreeMap::new(),
            alive: Vec::new(),
        }
    }

    /// 注册一个系统插件的方法处理器（构建期由宿主调用）。
    pub fn register_handler<F>(&mut self, plugin_id: &str, method: &str, handler: F)
    where
        F: Fn(&str, &[u8]) -> PluginResult<Vec<u8>> + Send + Sync + 'static,
    {
        self.handlers
            .entry(plugin_id.to_string())
            .or_default()
            .insert(method.to_string(), Arc::new(handler));
    }
}

impl Default for NativeRuntime {
    fn default() -> Self {
        NativeRuntime::new()
    }
}

/// T0 进程内实例。
pub struct NativeInstance {
    id: String,
    handlers: BTreeMap<String, Handler>,
    alive: bool,
}

impl PluginInstance for NativeInstance {
    fn call(&mut self, method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
        let handler = self.handlers.get(method).ok_or_else(|| {
            crate::plugin::error::PluginError::NotFound(format!(
                "系统插件 {} 无方法 {method}",
                self.id
            ))
        })?;
        handler(method, payload)
    }

    fn stop(&mut self) -> PluginResult<()> {
        self.alive = false;
        Ok(())
    }

    fn is_alive(&mut self) -> bool {
        self.alive
    }
}

impl PluginRuntime for NativeRuntime {
    fn name(&self) -> &str {
        "native"
    }

    fn declares(&self) -> RuntimeCapabilities {
        // T0 不隔离：除了 output cap 外，所有边界都 false。
        RuntimeCapabilities {
            output_cap: true,
            ..Default::default()
        }
    }

    fn supports(&self, manifest: &PluginManifest) -> PluginResult<()> {
        let tier = Tier::from_name(&manifest.plugin.name);
        if tier != Tier::System {
            return Err(crate::plugin::error::PluginError::Runtime(format!(
                "native 运行时不隔离，仅承载 T0 系统插件；{} 是 {} 级别",
                manifest.plugin.name,
                tier.as_str()
            )));
        }
        // T0 不要求边界。
        Ok(())
    }

    fn spawn(&mut self, manifest: &PluginManifest) -> PluginResult<Box<dyn PluginInstance>> {
        self.supports(manifest)?;
        let id = manifest.plugin.name.clone();
        let handlers = self.handlers.get(&id).cloned().ok_or_else(|| {
            crate::plugin::error::PluginError::NotFound(format!("系统插件 {id} 未注册处理器"))
        })?;
        self.alive.push(id.clone());
        Ok(Box::new(NativeInstance {
            id,
            handlers,
            alive: true,
        }))
    }
}

/// 用 Bound 避免 unused 警告（supports 内部通过 trait 实现，这里仅引用）。
#[allow(dead_code)]
const _BOUND_REF: Bound = Bound::FsIsolation;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::{Capabilities, ManifestSignature, PluginInfo, PluginLimits};
    use std::collections::BTreeMap as Map;

    fn manifest(name: &str) -> PluginManifest {
        PluginManifest {
            plugin: PluginInfo {
                name: name.to_string(),
                version: "3.0.0".to_string(),
                abi: "3.0".to_string(),
                entry: "x".to_string(),
                publisher: String::new(),
                module_sha256: String::new(),
            },
            capabilities: Capabilities::default(),
            limits: PluginLimits::default(),
            waivers: Map::new(),
            signature: ManifestSignature::default(),
        }
    }

    #[test]
    fn only_system_tier_accepted() {
        let rt = NativeRuntime::new();
        assert!(rt.supports(&manifest("com.twinsearth.sys.x")).is_ok());
        assert!(rt.supports(&manifest("com.twinsearth.official.x")).is_err());
        assert!(rt.supports(&manifest("com.example.x")).is_err());
    }

    #[test]
    fn call_registered_handler() {
        let mut rt = NativeRuntime::new();
        rt.register_handler("com.twinsearth.sys.echo", "echo", |_m, p| Ok(p.to_vec()));
        let mut inst = rt.spawn(&manifest("com.twinsearth.sys.echo")).unwrap();
        let out = inst.call("echo", br#"{"hello":1}"#).unwrap();
        assert_eq!(out, br#"{"hello":1}"#);
        assert!(inst.is_alive());
        inst.stop().unwrap();
        assert!(!inst.is_alive());
    }

    #[test]
    fn unknown_method_not_found() {
        let mut rt = NativeRuntime::new();
        rt.register_handler("com.twinsearth.sys.echo", "echo", |_m, _p| Ok(vec![]));
        let mut inst = rt.spawn(&manifest("com.twinsearth.sys.echo")).unwrap();
        assert!(inst.call("missing", b"{}").is_err());
    }
}
