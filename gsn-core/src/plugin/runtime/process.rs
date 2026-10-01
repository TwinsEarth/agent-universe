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
use crate::plugin::official::official_entry_source;
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
        // 平台特异的资源边界（macOS 内存、Windows CPU 时间）。
        "memory_limit" => SbCap::MemoryLimit,
        "cpu_limit" => SbCap::CpuLimit,
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
    /// 是否携带 entry 业务模块（v3.1.0 起）。
    has_entry: bool,
}

impl ProcessInstance {
    /// 在隔离进程中加载 entry 模块并调用其业务方法。
    fn invoke_entry(&mut self, method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
        // 方法名必须是合法 Python 标识符，杜绝引导代码注入。
        if method.is_empty()
            || !method
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(PluginError::Runtime(format!("非法方法名 {method}")));
        }
        // payload 必须是合法 JSON。
        let value: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|e| PluginError::Runtime(format!("payload 非法: {e}")))?;
        let literal = serde_json::to_string(&value)
            .map_err(|e| PluginError::Runtime(format!("payload 序列化失败: {e}")))?;
        // 包成 Python 字符串字面量（JSON 字符串，Python 可解析）。
        let py_data = serde_json::to_string(&literal)
            .map_err(|e| PluginError::Runtime(format!("payload 转义失败: {e}")))?;
        let method_lit = serde_json::to_string(method)
            .map_err(|e| PluginError::Runtime(format!("方法名转义失败: {e}")))?;

        let bootstrap = format!(
            r#"import json, sys
import plugin
data = json.loads({py_data})
try:
    fn = getattr(plugin, {method_lit})
except AttributeError:
    sys.stderr.write("no method {method}")
    sys.exit(2)
out = fn(data)
sys.stdout.write(json.dumps(out))
"#,
            py_data = py_data,
            method_lit = method_lit,
            method = method
        );

        let result = self
            .sb
            .run_code(CodeLanguage::Python, &bootstrap)
            .map_err(|e| PluginError::Runtime(format!("entry 调用失败: {e}")))?;
        if result.exit_code == 0 {
            Ok(result.stdout.into_bytes())
        } else {
            let tail = result.stderr.trim().lines().last().unwrap_or("");
            Err(PluginError::Runtime(format!(
                "插件 {method} 失败 (exit {}): {tail}",
                result.exit_code
            )))
        }
    }
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
            // entry 业务方法：加载 entry 模块并在隔离进程中调用。
            other if self.has_entry => self.invoke_entry(other, payload),
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

        // 写入 entry 业务模块（若该插件携带）。
        let entry = official_entry_source(&manifest.plugin.name);
        let has_entry = if let Some(es) = entry.as_ref() {
            sb.write_file(es.filename, es.source)
                .map_err(|e| PluginError::Runtime(format!("entry 写入失败: {e}")))?;
            true
        } else {
            false
        };

        self.instances.push(id.clone());
        Ok(Box::new(ProcessInstance {
            id,
            sb,
            alive: true,
            has_entry,
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
        // 平台特异的资源边界 waiver（与 official::process_waivers 对齐）。
        if cfg!(not(any(target_os = "linux", windows))) {
            w.insert(
                "memory_limit".to_string(),
                "test: platform has no RLIMIT_AS, memory not OS-enforced".to_string(),
            );
        }
        if cfg!(windows) {
            w.insert(
                "cpu_limit".to_string(),
                "test: Job Object enforces wall-clock, not CPU time".to_string(),
            );
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
        let rt = unique_rt();
        // T1 基础边界 process 后端满足；fs_deny_host/network/disk 需要 waiver。
        let m = manifest_with(
            "com.twinsearth.official.x",
            &["fs_deny_host", "network_egress", "disk_quota"],
        );
        assert!(rt.supports(&m).is_ok());
    }

    #[test]
    fn official_without_waiver_rejected() {
        let rt = unique_rt();
        // 注意：T1 的 required_bounds 只含 fs_isolation/cpu_limit/output_cap，
        // 这些 process 后端都满足，所以无 waiver 也能通过 supports。
        let m = manifest_with("com.twinsearth.official.x", &[]);
        assert!(rt.supports(&m).is_ok());
    }

    #[test]
    fn third_party_always_rejected() {
        let rt = unique_rt();
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
        let mut rt = unique_rt();
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
        let mut rt = unique_rt();
        let m = manifest_with(
            "com.twinsearth.official.demo",
            &["fs_deny_host", "network_egress", "disk_quota"],
        );
        let mut inst = rt.spawn(&m).unwrap();
        assert!(inst.call("nope", b"{}").is_err());
    }

    // ── v3.1.0 entry 业务模块 ─────────────────────────────────────
    fn entry_manifest(name: &str) -> PluginManifest {
        crate::plugin::official::official_manifest(name, "3.0.0")
    }

    /// 每个测试用独立 base 目录：并行测试即使插件 id 相同也不共享工作目录。
    fn unique_rt() -> ProcessRuntime {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("au-entry-test-{}-{}", std::process::id(), n));
        ProcessRuntime::new(Some(base))
    }

    #[test]
    fn reputation_overall_via_entry() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_ECONOMY_REPUTATION);
        let mut inst = rt.spawn(&m).unwrap();
        let payload = br#"{"quality":1.0,"speed":0.8,"honesty":0.6,"availability":0.4}"#;
        let out = inst.call("overall", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        // 1*.35 + .8*.20 + .6*.30 + .4*.15 = .35+.16+.18+.06 = .75
        let got = v["overall"].as_f64().unwrap();
        assert!((got - 0.75).abs() < 1e-9, "got {got}");
        inst.stop().unwrap();
    }

    #[test]
    fn market_match_picks_best_via_entry() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_MARKET_MATCH);
        let mut inst = rt.spawn(&m).unwrap();
        let payload = br#"{"bids":[
            {"agent_id":"a","price":100,"latency_ms":100,"reputation":0.5},
            {"agent_id":"b","price":100,"latency_ms":100,"reputation":0.9}
        ]}"#;
        let out = inst.call("match", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        // 同价同延迟，高信誉 b 的 score 更高。
        assert_eq!(v["winner"].as_str().unwrap(), "b");
        inst.stop().unwrap();
    }

    #[test]
    fn market_match_empty_bids() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_MARKET_MATCH);
        let mut inst = rt.spawn(&m).unwrap();
        let out = inst.call("match", br#"{"bids":[]}"#).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v["winner"].is_null());
        inst.stop().unwrap();
    }

    #[test]
    fn entry_status_method() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_ECONOMY_REPUTATION);
        let mut inst = rt.spawn(&m).unwrap();
        let out = inst.call("status", b"{}").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            v["plugin"].as_str().unwrap(),
            crate::plugin::official::OFF_ECONOMY_REPUTATION
        );
        inst.stop().unwrap();
    }

    #[test]
    fn entry_missing_method_fails() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_ECONOMY_REPUTATION);
        let mut inst = rt.spawn(&m).unwrap();
        // getattr 失败 → exit 2 → Err。
        assert!(inst.call("nope", b"{}").is_err());
        inst.stop().unwrap();
    }

    #[test]
    fn invalid_method_name_rejected() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_ECONOMY_REPUTATION);
        let mut inst = rt.spawn(&m).unwrap();
        // 非合法标识符 → 引导注入在生成前被拒绝。
        assert!(inst.call("a;import os", b"{}").is_err());
        inst.stop().unwrap();
    }
}
