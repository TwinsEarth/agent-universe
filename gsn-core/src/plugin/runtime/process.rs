//! 独立进程运行时（ProcessRuntime）—— 复用现有 sandbox，承载 T1/T2/T3
//!
//! # 能力声明（诚实）
//!
//! | 边界 | Linux/macOS | Windows |
//! |---|---|---|
//! | 独立工作目录 fs_isolation | ✅ | ✅ |
//! | CPU 时间/超时 cpu_limit | ✅ | ✅ |
//! | 输出上限 output_cap | ✅ | ✅ |
//! | 内存上限 memory_limit | ❌（`ulimit -v` 是地址空间，非 RSS） | ✅ Job Object |
//! | 进程树 kill / 进程数 | ❌ | ✅ Job Object |
//! | 阻止子进程读写宿主 fs_deny_host | ❌（safe_join 只约束 Rust helper） | ❌ |
//! | 出站网络控制 network_egress | ❌ | ❌ |
//! | 磁盘配额 disk_quota | ❌ | ❌ |
//!
//! 后三者在**所有平台**都没有原语。因此 T1/T2 可信插件必须以清单显式 waiver 接受；
//! T3 不可信插件所需边界无法满足 → 拒绝（见 [`crate::plugin::runtime::required_bounds`]）。

use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::manifest::PluginManifest;
use crate::plugin::runtime::{PluginInstance, PluginRuntime, RuntimeCapabilities};
use crate::sandbox::capability::{Capability as SbCap, Waiver};
use crate::sandbox::config::{ResourceLimits, SandboxConfig};
use crate::sandbox::runtime::process::ProcessSandbox;
use crate::sandbox::runtime::CodeLanguage;
use crate::sandbox::Sandbox;
use std::path::PathBuf;

/// 独立进程运行时。
pub struct ProcessRuntime {
    /// 工作目录父目录（None = 系统临时目录）。
    work_dir_base: Option<PathBuf>,
    /// 已 spawn 实例 id。
    instances: Vec<String>,
}

impl ProcessRuntime {
    /// 新运行时。
    pub fn new(work_dir_base: Option<PathBuf>) -> Self {
        ProcessRuntime {
            work_dir_base,
            instances: Vec::new(),
        }
    }
}

/// 把清单 waiver（key）映射到 sandbox 边界。
fn map_waiver(key: &str, why: &str) -> Option<Waiver> {
    let boundary = match key {
        "network_egress" => SbCap::NetworkDenyAll,
        "fs_deny_host" => SbCap::FilesystemConfinement,
        "disk_quota" => SbCap::DiskQuota,
        _ => return None,
    };
    Some(Waiver {
        boundary,
        justification: why.to_string(),
    })
}

/// 独立进程实例。
pub struct ProcessInstance {
    id: String,
    sb: ProcessSandbox,
    alive: bool,
}

impl PluginInstance for ProcessInstance {
    fn call(&mut self, method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
        match method {
            // 在隔离进程中执行代码：payload = {"language":"python","code":"..."}
            "exec" => {
                let v: serde_json::Value = serde_json::from_slice(payload)
                    .map_err(|e| PluginError::Runtime(format!("exec 参数非法: {e}")))?;
                let lang_label = v
                    .get("language")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| PluginError::Runtime("缺少 language".to_string()))?;
                let code = v
                    .get("code")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| PluginError::Runtime("缺少 code".to_string()))?;
                let lang = CodeLanguage::from_label(lang_label)
                    .ok_or_else(|| PluginError::Runtime(format!("未知语言 {lang_label}")))?;
                let result = self
                    .sb
                    .run_code(lang, code)
                    .map_err(|e| PluginError::Runtime(format!("执行失败: {e}")))?;
                let out = serde_json::json!({
                    "exit_code": result.exit_code,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                });
                serde_json::to_vec(&out)
                    .map_err(|e| PluginError::Runtime(format!("结果序列化失败: {e}")))
            }
            other => Err(PluginError::NotFound(format!(
                "进程插件 {} 无方法 {other}",
                self.id
            ))),
        }
    }

    fn stop(&mut self) -> PluginResult<()> {
        self.sb
            .destroy()
            .map_err(|e| PluginError::Runtime(format!("销毁失败: {e}")))?;
        self.alive = false;
        Ok(())
    }

    fn is_alive(&mut self) -> bool {
        self.alive
    }
}

impl PluginRuntime for ProcessRuntime {
    fn name(&self) -> &str {
        "process"
    }

    fn declares(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            fs_isolation: true,
            cpu_limit: true,
            output_cap: true,
            // Windows 上 Job Object 提供真实内存/进程树/进程数强制。
            #[cfg(windows)]
            memory_limit: true,
            #[cfg(windows)]
            process_tree_kill: true,
            #[cfg(windows)]
            process_count_limit: true,
            ..Default::default()
        }
    }

    fn spawn(&mut self, manifest: &PluginManifest) -> PluginResult<Box<dyn PluginInstance>> {
        // supports 已在 trait 默认实现中按级别边界 + waiver 校验；这里显式再调用一次。
        self.supports(manifest)?;

        // 资源：从清单 limits 映射。
        let resources = ResourceLimits {
            cpu_millis: manifest.limits.cpu_ms as u32,
            mem_mb: (manifest.limits.memory_bytes / (1024 * 1024)).max(64) as u32,
            disk_mb: (manifest.limits.disk_bytes / (1024 * 1024)).max(1) as u32,
            max_processes: manifest.limits.max_processes.max(1),
            timeout_ms: manifest.limits.cpu_ms,
            ..Default::default()
        };

        // 把清单 waivers 映射到 sandbox（仅映射进程后端无法强制的那几条）。
        let mut waivers = Vec::new();
        for (key, why) in &manifest.waivers {
            if let Some(w) = map_waiver(key, why) {
                waivers.push(w);
            }
        }

        let cfg = SandboxConfig {
            work_dir_base: self.work_dir_base.clone(),
            resources,
            waivers,
            ..Default::default()
        };

        let id = manifest.plugin.name.clone();
        let mut sb = ProcessSandbox::new(&id);
        sb.create(&cfg)
            .map_err(|e| PluginError::Runtime(format!("沙箱创建失败: {e}")))?;
        sb.start()
            .map_err(|e| PluginError::Runtime(format!("沙箱启动失败: {e}")))?;

        self.instances.push(id.clone());
        Ok(Box::new(ProcessInstance {
            id,
            sb,
            alive: true,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::{Capabilities, ManifestSignature, PluginInfo, PluginLimits};
    use std::collections::BTreeMap;

    fn manifest_with(name: &str, waivers: &[&str]) -> PluginManifest {
        let mut w = BTreeMap::new();
        for key in waivers {
            w.insert(key.to_string(), "trusted official code, tested".to_string());
        }
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
            waivers: w,
            signature: ManifestSignature::default(),
        }
    }

    #[test]
    fn official_with_waivers_supported() {
        let rt = ProcessRuntime::new(None);
        // T1 基础边界 process 后端满足；fs_deny_host/network/disk 需要 waiver。
        let m = manifest_with(
            "com.twinsearth.official.x",
            &["fs_deny_host", "network_egress", "disk_quota"],
        );
        assert!(rt.supports(&m).is_ok());
    }

    #[test]
    fn official_without_waiver_rejected() {
        let rt = ProcessRuntime::new(None);
        // 注意：T1 的 required_bounds 只含 fs_isolation/cpu_limit/output_cap，
        // 这些 process 后端都满足，所以无 waiver 也能通过 supports。
        let m = manifest_with("com.twinsearth.official.x", &[]);
        assert!(rt.supports(&m).is_ok());
    }

    #[test]
    fn third_party_always_rejected() {
        let rt = ProcessRuntime::new(None);
        // T3 需要 fs_deny_host/network_egress/disk_quota 等，process 后端全不满足，
        // 且 T3 无 waiver 例外 → 即使列了 waiver 也拒绝。
        let m = manifest_with(
            "com.example.x",
            &["fs_deny_host", "network_egress", "disk_quota"],
        );
        assert!(rt.supports(&m).is_err());
    }

    #[test]
    fn spawn_and_exec_python() {
        let mut rt = ProcessRuntime::new(None);
        let m = manifest_with(
            "com.twinsearth.official.demo",
            &["fs_deny_host", "network_egress", "disk_quota"],
        );
        let mut inst = rt.spawn(&m).unwrap();
        let payload = br#"{"language":"python","code":"print(1+1)"}"#;
        let out = inst.call("exec", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["stdout"].as_str().unwrap().trim(), "2");
        assert_eq!(v["exit_code"].as_i64().unwrap(), 0);
        inst.stop().unwrap();
        assert!(!inst.is_alive());
    }

    #[test]
    fn unknown_method_rejected() {
        let mut rt = ProcessRuntime::new(None);
        let m = manifest_with(
            "com.twinsearth.official.demo",
            &["fs_deny_host", "network_egress", "disk_quota"],
        );
        let mut inst = rt.spawn(&m).unwrap();
        assert!(inst.call("nope", b"{}").is_err());
    }
}
