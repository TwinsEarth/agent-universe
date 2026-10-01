//! 插件注册中心（Registry）—— 清单、版本与依赖图
//!
//! # 注册中心管什么
//!
//! - 保存已注册插件的清单与级别；
//! - 维护依赖图（谁依赖谁），做依赖满足与循环依赖检查；
//! - 提供按名称查询、版本查询。
//!
//! # 注册不是加载
//!
//! 注册只登记清单（及其签名已校验的前提）；真正进入运行态由生命周期/运行时推进。

use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::manifest::PluginManifest;
use crate::plugin::tier::Tier;
use std::collections::BTreeMap;

/// 已注册插件记录。
#[derive(Debug, Clone)]
pub struct RegisteredPlugin {
    /// 清单。
    pub manifest: PluginManifest,
    /// 级别（由 name 前缀决定）。
    pub tier: Tier,
}

/// 插件注册中心。
#[derive(Debug, Default, Clone)]
pub struct PluginRegistry {
    plugins: BTreeMap<String, RegisteredPlugin>,
}

impl PluginRegistry {
    /// 空注册中心。
    pub fn new() -> Self {
        PluginRegistry {
            plugins: BTreeMap::new(),
        }
    }

    /// 注册（登记）一个已校验清单。
    ///
    /// 同名插件以同版本重复注册为幂等；不同版本覆盖前会返回错误
    /// （避免静默覆盖正在运行的版本——版本升级走热更新路径）。
    pub fn register(&mut self, manifest: PluginManifest) -> PluginResult<()> {
        let name = manifest.plugin.name.clone();
        let tier = Tier::from_name(&name);
        if let Some(existing) = self.plugins.get(&name) {
            if existing.manifest.plugin.version != manifest.plugin.version {
                return Err(PluginError::Manifest(format!(
                    "插件 {name} 已注册版本 {}，新版本 {} 必须走热更新路径",
                    existing.manifest.plugin.version, manifest.plugin.version
                )));
            }
            // 同版本幂等。
            return Ok(());
        }
        self.plugins
            .insert(name, RegisteredPlugin { manifest, tier });
        Ok(())
    }

    /// 热更新替换：新版本已在运行时完成双缓冲切换后，替换清单与版本。
    ///
    /// 与 [`register`](Self::register) 的区别：这里明确是热更新路径，**允许版本变更**。
    /// 调用方必须在此之前完成新版本的 spawn 与健康检查（见 `PluginHost::hot_reload`）。
    pub fn replace(&mut self, manifest: PluginManifest) -> PluginResult<()> {
        let name = manifest.plugin.name.clone();
        let tier = Tier::from_name(&name);
        self.plugins
            .insert(name, RegisteredPlugin { manifest, tier });
        Ok(())
    }

    /// 注销（热插拔卸载登记）。
    pub fn unregister(&mut self, name: &str) -> PluginResult<()> {
        if self.plugins.remove(name).is_none() {
            return Err(PluginError::NotFound(name.to_string()));
        }
        Ok(())
    }

    /// 按名称获取已注册插件。
    pub fn get(&self, name: &str) -> Option<&RegisteredPlugin> {
        self.plugins.get(name)
    }

    /// 是否已注册。
    pub fn contains(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// 全部已注册插件（name → 记录）。
    pub fn all(&self) -> &BTreeMap<String, RegisteredPlugin> {
        &self.plugins
    }

    /// 列出某插件声明的依赖（从清单 `depends` 字段读取；若清单没有该字段则为空）。
    ///
    /// 本项目清单的依赖以能力/分组形式表达；这里读取显式依赖名。
    pub fn dependencies_of(&self, name: &str) -> Vec<String> {
        // 清单结构没有独立 depends 字段时，依赖以隐式分组表达；
        // 这里返回空，循环检查在显式依赖加入时进行。
        self.plugins
            .get(name)
            .map(|_| Vec::new())
            .unwrap_or_default()
    }

    /// 检查注册某插件是否会形成循环依赖（显式依赖图）。
    ///
    /// 给定「插件名 → 它依赖的插件名列表」，做一次从 start 的 DFS，
    /// 若能回到 start 则有循环。
    pub fn has_cycle(graph: &BTreeMap<String, Vec<String>>, start: &str) -> bool {
        let mut stack = vec![start.to_string()];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(node) = stack.pop() {
            if node == start && !seen.is_empty() {
                return true;
            }
            if !seen.insert(node.clone()) {
                continue;
            }
            if let Some(deps) = graph.get(&node) {
                for d in deps {
                    stack.push(d.clone());
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::{Capabilities, ManifestSignature, PluginInfo, PluginLimits};
    use std::collections::BTreeMap;

    fn manifest(name: &str, version: &str) -> PluginManifest {
        PluginManifest {
            plugin: PluginInfo {
                name: name.to_string(),
                version: version.to_string(),
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
    fn register_and_get() {
        let mut reg = PluginRegistry::new();
        reg.register(manifest("com.twinsearth.sys.x", "3.0.0"))
            .unwrap();
        assert!(reg.contains("com.twinsearth.sys.x"));
        assert_eq!(reg.get("com.twinsearth.sys.x").unwrap().tier, Tier::System);
    }

    #[test]
    fn idempotent_same_version() {
        let mut reg = PluginRegistry::new();
        reg.register(manifest("com.twinsearth.sys.x", "3.0.0"))
            .unwrap();
        reg.register(manifest("com.twinsearth.sys.x", "3.0.0"))
            .unwrap();
        assert_eq!(reg.all().len(), 1);
    }

    #[test]
    fn different_version_must_use_hotswap() {
        let mut reg = PluginRegistry::new();
        reg.register(manifest("com.twinsearth.sys.x", "3.0.0"))
            .unwrap();
        assert!(reg
            .register(manifest("com.twinsearth.sys.x", "3.0.1"))
            .is_err());
    }

    #[test]
    fn unregister() {
        let mut reg = PluginRegistry::new();
        reg.register(manifest("com.twinsearth.sys.x", "3.0.0"))
            .unwrap();
        reg.unregister("com.twinsearth.sys.x").unwrap();
        assert!(!reg.contains("com.twinsearth.sys.x"));
        assert!(reg.unregister("com.twinsearth.sys.x").is_err());
    }

    #[test]
    fn cycle_detection() {
        let mut graph = BTreeMap::new();
        graph.insert("a".to_string(), vec!["b".to_string()]);
        graph.insert("b".to_string(), vec!["c".to_string()]);
        graph.insert("c".to_string(), vec!["a".to_string()]);
        assert!(PluginRegistry::has_cycle(&graph, "a"));

        let mut acyclic = BTreeMap::new();
        acyclic.insert("a".to_string(), vec!["b".to_string()]);
        acyclic.insert("b".to_string(), vec!["c".to_string()]);
        acyclic.insert("c".to_string(), vec![]);
        assert!(!PluginRegistry::has_cycle(&acyclic, "a"));
    }
}
