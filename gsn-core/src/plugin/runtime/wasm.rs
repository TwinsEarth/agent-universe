//! WASM 运行时（WasmRuntime）—— 本构建中类型化拒绝
//!
//! # 为什么是拒绝而不是实现
//!
//! 设计上，WASM/WASI 沙箱是理想的默认隔离层：线性内存、能力式 host API、CPU 计量、
//! 跨平台一致。但本构建的依赖树中**没有 wasmtime**（无 WASM 运行时），因此本会话
//! 无法加载、验证、运行任何 WASM 插件。
//!
//! 按「不假装能跑」的原则，这里对所有级别返回具名的
//! [`crate::plugin::error::PluginError::Runtime`]，并明确告诉使用者：
//! 需要启用 `wasm` 特性（引入 wasmtime）后才能承载 WASM 插件。
//!
//! 这是**能力缺口的诚实标注**，不是「默默把 WASM 插件放进不受限进程」。

use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::manifest::PluginManifest;
use crate::plugin::runtime::{PluginInstance, PluginRuntime, RuntimeCapabilities};

/// 拒绝原因（单一来源）。
const REASON: &str = "WASM 运行时（wasmtime）未在本构建中编译：无法加载/验证/运行 WASM 插件。\
     请启用 wasm 特性引入 wasmtime 后再加载；在此之前不要把 WASM 插件降级到进程后端。";

/// WASM 运行时占位（本构建不可用）。
#[derive(Debug, Default, Clone, Copy)]
pub struct WasmRuntime;

impl WasmRuntime {
    pub fn new() -> Self {
        WasmRuntime
    }
}

impl PluginRuntime for WasmRuntime {
    fn name(&self) -> &str {
        "wasm"
    }

    fn declares(&self) -> RuntimeCapabilities {
        // 实际未编译：诚实返回全 false，不声称任何边界。
        RuntimeCapabilities::default()
    }

    fn supports(&self, _manifest: &PluginManifest) -> PluginResult<()> {
        // 任何级别都无法承载。
        Err(PluginError::Runtime(REASON.to_string()))
    }

    fn spawn(&mut self, _manifest: &PluginManifest) -> PluginResult<Box<dyn PluginInstance>> {
        Err(PluginError::Runtime(REASON.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::{Capabilities, ManifestSignature, PluginInfo, PluginLimits};
    use std::collections::BTreeMap;

    fn manifest() -> PluginManifest {
        PluginManifest {
            plugin: PluginInfo {
                name: "com.twinsearth.official.x".to_string(),
                version: "3.0.0".to_string(),
                abi: "3.0".to_string(),
                entry: "x".to_string(),
                publisher: String::new(),
                module_sha256: String::new(),
            },
            capabilities: Capabilities::default(),
            limits: PluginLimits::default(),
            waivers: BTreeMap::new(),
            signature: ManifestSignature::default(),
        }
    }

    #[test]
    fn supports_always_errors() {
        let rt = WasmRuntime::new();
        assert!(rt.supports(&manifest()).is_err());
    }

    #[test]
    fn spawn_always_errors() {
        let mut rt = WasmRuntime::new();
        assert!(rt.spawn(&manifest()).is_err());
    }

    #[test]
    fn declares_nothing() {
        assert_eq!(
            WasmRuntime::new().declares(),
            RuntimeCapabilities::default()
        );
    }
}
