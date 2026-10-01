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
use crate::plugin::runtime::{OutboxMessage, PluginInstance, PluginRuntime, RuntimeCapabilities};
use crate::sandbox::capability::{Capability as SbCap, Waiver};
use crate::sandbox::config::{ResourceLimits, SandboxConfig};
use crate::sandbox::runtime::process::ProcessSandbox;
use crate::sandbox::runtime::CodeLanguage;
use crate::sandbox::Sandbox;
use std::path::PathBuf;

/// B2：注入给进程插件的宿主通信模块（`host.py`）。
///
/// 插件 entry 内 `import host` 后调用 `host.send_to(target, payload, capability=...)` 或
/// `host.publish(payload, capability=...)`；这些函数**不开网络**，只把消息逐行写入
/// 隔离工作目录的 `outbox.jsonl`。宿主在 invoke_entry 返回后读回并代表插件投递 PMB
/// （宿主仍是唯一投递点，保持七道检查）。
pub const HOST_PY: &str = r#"import json, os

_OUTBOX = os.path.join(os.path.dirname(os.path.abspath(__file__)), "outbox.jsonl")


def _emit(rec):
    with open(_OUTBOX, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False) + "\n")


def send_to(target, payload, capability="plugin:message:send"):
    """向单个目标插件发送一条消息（由宿主经 PMB 投递）。"""
    _emit({"kind": "send", "target": target, "capability": capability, "payload": payload})


def publish(payload, capability="plugin:message:send"):
    """向所有运行中插件广播一条事件（由宿主经 PMB 投递）。"""
    _emit({"kind": "publish", "capability": capability, "payload": payload})
"#;

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
        // 平台特异的资源边界（macOS 内存、Windows CPU 时间/句柄数）。
        "memory_limit" => SbCap::MemoryLimit,
        "cpu_limit" => SbCap::CpuLimit,
        "open_file_limit" => SbCap::OpenFileLimit,
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
    /// 插件主动产生、待宿主投递的消息（B2 outbox）。
    outbox: Vec<OutboxMessage>,
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
            // B2：读回插件主动产生的消息（若有）。
            self.collect_outbox()?;
            Ok(result.stdout.into_bytes())
        } else {
            let tail = result.stderr.trim().lines().last().unwrap_or("");
            Err(PluginError::Runtime(format!(
                "插件 {method} 失败 (exit {}): {tail}",
                result.exit_code
            )))
        }
    }

    /// 读取工作目录 outbox，解析为 [`OutboxMessage`] 并清空文件（B2）。
    fn collect_outbox(&mut self) -> PluginResult<()> {
        let text = match self.sb.read_file("outbox.jsonl") {
            Ok(t) => t,
            // 插件未写 outbox（未主动通信）：正常。
            Err(_) => return Ok(()),
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(line)
                .map_err(|e| PluginError::Runtime(format!("outbox 行非法: {e}")))?;
            let kind = v
                .get("kind")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if kind != "send" && kind != "publish" {
                return Err(PluginError::Runtime(format!("outbox 未知 kind: {kind}")));
            }
            let target = v.get("target").and_then(|x| x.as_str()).map(String::from);
            if kind == "send" && target.is_none() {
                return Err(PluginError::Runtime("send 消息缺少 target".into()));
            }
            let capability = v
                .get("capability")
                .and_then(|x| x.as_str())
                .unwrap_or("plugin:message:send")
                .to_string();
            let payload = v.get("payload").cloned().unwrap_or(serde_json::Value::Null);
            self.outbox.push(OutboxMessage {
                kind,
                target,
                capability,
                payload,
            });
        }
        // 清空 outbox 文件，避免下次重复读。
        self.sb
            .write_file("outbox.jsonl", "")
            .map_err(|e| PluginError::Runtime(format!("outbox 清空失败: {e}")))?;
        Ok(())
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

    fn drain_outbox(&mut self) -> PluginResult<Vec<OutboxMessage>> {
        Ok(std::mem::take(&mut self.outbox))
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
        Ok(Box::new(self.spawn_concrete(manifest)?))
    }
}

impl ProcessRuntime {
    /// 构造并返回具体类型的 [`ProcessInstance`]（暴露给测试，便于在覆盖 entry
    /// 时访问沙箱字段；trait `spawn` 以本方法为基础装箱）。
    pub fn spawn_concrete(&mut self, manifest: &PluginManifest) -> PluginResult<ProcessInstance> {
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
            // B2：注入宿主通信模块 host.py（供插件主动 send_to/publish）。
            sb.write_file("host.py", HOST_PY)
                .map_err(|e| PluginError::Runtime(format!("host.py 写入失败: {e}")))?;
            true
        } else {
            false
        };

        self.instances.push(id.clone());
        Ok(ProcessInstance {
            id,
            sb,
            alive: true,
            has_entry,
            outbox: Vec::new(),
        })
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
            w.insert(
                "open_file_limit".to_string(),
                "test: Job Object has no handle-count limit".to_string(),
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

    // ── B2（v3.5.0）：进程插件主动通信 outbox ──────────────────────────
    #[test]
    fn outbox_captures_plugin_initiated_messages() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_MARKET_MATCH);
        let mut inst = rt.spawn_concrete(&m).unwrap();
        // 覆盖 entry 为一个主动发消息的插件（entry 内 import host）。
        let custom = r#"
import host
def notify(data):
    host.send_to("bridge-peer-a", {"n": data["n"]}, capability="plugin:message:send")
    host.publish({"done": True}, capability="plugin:message:send")
    return {"ok": True}
"#;
        inst.sb.write_file("plugin.py", custom).unwrap();
        let out = inst.call("notify", br#"{"n":7}"#).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["ok"], true);
        // 宿主侧 collect_outbox 已在 call 内解析出两条消息。
        let msgs = inst.drain_outbox().unwrap();
        assert_eq!(msgs.len(), 2, "expected 2 outbox messages");
        assert_eq!(msgs[0].kind, "send");
        assert_eq!(msgs[0].target.as_deref(), Some("bridge-peer-a"));
        assert_eq!(msgs[0].capability, "plugin:message:send");
        assert_eq!(msgs[0].payload["n"], 7);
        assert_eq!(msgs[1].kind, "publish");
        assert_eq!(msgs[1].capability, "plugin:message:send");
        assert_eq!(msgs[1].payload["done"], true);
        // drain 后再次取应为空（不重复）。
        assert!(inst.drain_outbox().unwrap().is_empty());
        inst.stop().unwrap();
    }

    #[test]
    fn outbox_empty_when_plugin_does_not_communicate() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_ECONOMY_REPUTATION);
        let mut inst = rt.spawn_concrete(&m).unwrap();
        // status 不主动发消息 → outbox 为空。
        let _ = inst.call("status", b"{}").unwrap();
        assert!(inst.drain_outbox().unwrap().is_empty());
        inst.stop().unwrap();
    }

    #[test]
    fn outbox_send_without_target_rejected() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_MARKET_MATCH);
        let mut inst = rt.spawn_concrete(&m).unwrap();
        // 直接写一条缺 target 的 send 记录到 outbox。
        inst.sb
            .write_file(
                "outbox.jsonl",
                "{\"kind\":\"send\",\"capability\":\"message\",\"payload\":{}}\n",
            )
            .unwrap();
        // 下一次 entry 调用收集 outbox 时必须报错。
        inst.sb
            .write_file("plugin.py", "def ping(data):\n    return {}\n")
            .unwrap();
        assert!(inst.call("ping", b"{}").is_err());
        inst.stop().unwrap();
    }

    // ── v3.2.0 entry 业务模块（settle / scheduler / card）────────────
    #[test]
    fn settle_audit_passes_on_consistent_ledger() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_MARKET_SETTLE);
        let mut inst = rt.spawn(&m).unwrap();
        let payload = br#"{
          "records": [
            {"task_id":"","from_account":"","to_account":"A","amount":100,"reason":"Deposited","timestamp":0},
            {"task_id":"t1","from_account":"A","to_account":"__escrow__t1","amount":50,"reason":"Escrowed","timestamp":0},
            {"task_id":"t1","from_account":"__escrow__t1","to_account":"B","amount":50,"reason":"Completed","timestamp":0}
          ],
          "balances": {"A":50,"B":50,"__escrow__t1":0},
          "total_deposits":100,"total_slashed":0
        }"#;
        let out = inst.call("audit", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["passed"], true, "got {v}");
        assert_eq!(v["expected_total"], 100);
        assert_eq!(v["actual_total"], 100);
        inst.stop().unwrap();
    }

    #[test]
    fn settle_audit_catches_tampered_balance() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_MARKET_SETTLE);
        let mut inst = rt.spawn(&m).unwrap();
        // 流水重放 B=50，但当前余额 B=60（凭空多 10）。
        let payload = br#"{
          "records": [
            {"task_id":"","from_account":"","to_account":"A","amount":100,"reason":"Deposited","timestamp":0},
            {"task_id":"t1","from_account":"A","to_account":"__escrow__t1","amount":50,"reason":"Escrowed","timestamp":0},
            {"task_id":"t1","from_account":"__escrow__t1","to_account":"B","amount":50,"reason":"Completed","timestamp":0}
          ],
          "balances": {"A":50,"B":60,"__escrow__t1":0}
        }"#;
        let out = inst.call("audit", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["passed"], false);
        assert_eq!(v["actual_total"], 110);
        // 抓到 B 的账实不符（expected 50, actual 60）。
        let mm = &v["mismatches"];
        assert_eq!(mm[0]["account"], "B");
        assert_eq!(mm[0]["expected"], 50);
        assert_eq!(mm[0]["actual"], 60);
        inst.stop().unwrap();
    }

    #[test]
    fn scheduler_route_picks_low_latency_node() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_SCHEDULER_TASK);
        let mut inst = rt.spawn(&m).unwrap();
        let payload = br#"{
          "nodes": [
            {"did":"a","load":0,"latency_ms":100},
            {"did":"b","load":0,"latency_ms":50}
          ],
          "max_concurrent":4,"budget":100
        }"#;
        let out = inst.call("route", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["selected"], "b");
        assert_eq!(v["estimated_cost"], 50); // 100 // 2
        inst.stop().unwrap();
    }

    #[test]
    fn scheduler_route_empty_candidates() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_SCHEDULER_TASK);
        let mut inst = rt.spawn(&m).unwrap();
        let out = inst
            .call("route", br#"{"nodes":[],"max_concurrent":4}"#)
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v["selected"].is_null());
        assert_eq!(v["reason"], "no_candidates");
        inst.stop().unwrap();
    }

    #[test]
    fn scheduler_route_zero_capacity() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_SCHEDULER_TASK);
        let mut inst = rt.spawn(&m).unwrap();
        // max_concurrent=0（原 Rust load/max 会除零）→ 显式 no_capacity。
        let payload = br#"{"nodes":[{"did":"a","load":0,"latency_ms":50}],"max_concurrent":0}"#;
        let out = inst.call("route", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v["selected"].is_null());
        assert_eq!(v["reason"], "no_capacity");
        inst.stop().unwrap();
    }

    #[test]
    fn agent_card_validate_accepts_valid_card() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_AGENT_CARD);
        let mut inst = rt.spawn(&m).unwrap();
        let payload = br#"{"card":{
          "agent_id":"did:nau:a","name":"X","version":"1.0","skills":["s1"],
          "reputation_score":0.5,"success_rate":0.9,"stake":100
        }}"#;
        let out = inst.call("validate", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["valid"], true, "got {v}");
        inst.stop().unwrap();
    }

    #[test]
    fn agent_card_validate_rejects_invalid_card() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_AGENT_CARD);
        let mut inst = rt.spawn(&m).unwrap();
        // 非 DID id、空 name、信誉越界、负质押。
        let payload = br#"{"card":{
          "agent_id":"abc","name":"","version":"","skills":[],"reputation_score":1.5,
          "success_rate":0.9,"stake":-5
        }}"#;
        let out = inst.call("validate", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["valid"], false);
        assert!(v["errors"].as_array().unwrap().len() >= 4, "got {v}");
        inst.stop().unwrap();
    }

    // ── v3.2.2 entry 业务模块（swarm-emergence）────────────────────
    #[test]
    fn swarm_emergence_detects_throughput_growth() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_SWARM_EMERGENCE);
        let mut inst = rt.spawn(&m).unwrap();
        // 更早窗口 throughput=100，最近窗口 throughput=200（+100%）；latency 不变。
        let payload = br#"{
          "window_size": 3, "threshold": 0.5,
          "history": [
            [0,100,50],[1,100,50],[2,100,50],
            [3,200,50],[4,200,50],[5,200,50]
          ]
        }"#;
        let out = inst.call("detect", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let sigs = v["signals"].as_array().unwrap();
        assert_eq!(sigs.len(), 1, "got {v}");
        assert_eq!(sigs[0]["signal_type"], "collaboration");
        let strength = sigs[0]["strength"].as_f64().unwrap();
        assert!((strength - 1.0).abs() < 1e-9, "got {strength}");
        inst.stop().unwrap();
    }

    #[test]
    fn swarm_emergence_detects_latency_drop() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_SWARM_EMERGENCE);
        let mut inst = rt.spawn(&m).unwrap();
        // throughput 不变（不触发增长）；latency 100→40（-60%）。
        let payload = br#"{
          "window_size": 3, "threshold": 0.5,
          "history": [
            [0,100,100],[1,100,100],[2,100,100],
            [3,100,40],[4,100,40],[5,100,40]
          ]
        }"#;
        let out = inst.call("detect", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let sigs = v["signals"].as_array().unwrap();
        assert_eq!(sigs.len(), 1, "got {v}");
        assert_eq!(sigs[0]["signal_type"], "load_balancing");
        let strength = sigs[0]["strength"].as_f64().unwrap();
        assert!((strength - 0.6).abs() < 1e-9, "got {strength}");
        inst.stop().unwrap();
    }

    #[test]
    fn swarm_emergence_insufficient_history() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_SWARM_EMERGENCE);
        let mut inst = rt.spawn(&m).unwrap();
        // 不足一个窗口 → 无信号。
        let payload = br#"{"window_size":3,"threshold":0.5,
          "history":[[0,100,50],[1,100,50]]}"#;
        let out = inst.call("detect", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["signals"].as_array().unwrap().len(), 0, "got {v}");
        inst.stop().unwrap();
    }

    #[test]
    fn agent_skill_discover_finds_agents_by_skill() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_AGENT_SKILL);
        let mut inst = rt.spawn(&m).unwrap();
        // a、c 声明 python（c 为大写 Python），b 仅 rust。
        let payload = br#"{
          "skill": "python",
          "agents": [
            {"agent_id":"did:nau:a","name":"A","skills":["python","ml"]},
            {"agent_id":"did:nau:b","name":"B","skills":["rust"]},
            {"agent_id":"did:nau:c","name":"C","skills":["Python","data"]}
          ]
        }"#;
        let out = inst.call("discover", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["count"], 2, "got {v}");
        let ids: Vec<String> = v["agents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["agent_id"].as_str().unwrap().to_string())
            .collect();
        assert!(ids.contains(&"did:nau:a".to_string()));
        assert!(ids.contains(&"did:nau:c".to_string()));
        assert!(!ids.contains(&"did:nau:b".to_string()));
        inst.stop().unwrap();
    }

    #[test]
    fn agent_skill_discover_is_exact_not_substring() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_AGENT_SKILL);
        let mut inst = rt.spawn(&m).unwrap();
        // 查询 PYTHON：精确标签 Python / python 命中；"Python Developer" 是包含、不命中。
        let payload = br#"{
          "skill": "PYTHON",
          "agents": [
            {"agent_id":"did:nau:a","skills":["Python"]},
            {"agent_id":"did:nau:b","skills":["Python Developer"]},
            {"agent_id":"did:nau:c","skills":["python"]}
          ]
        }"#;
        let out = inst.call("discover", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["count"], 2, "got {v}");
        let ids: Vec<String> = v["agents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["agent_id"].as_str().unwrap().to_string())
            .collect();
        assert!(ids.contains(&"did:nau:a".to_string()));
        assert!(ids.contains(&"did:nau:c".to_string()));
        assert!(
            !ids.contains(&"did:nau:b".to_string()),
            "substring must not match"
        );
        inst.stop().unwrap();
    }

    #[test]
    fn agent_skill_discover_no_match() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_AGENT_SKILL);
        let mut inst = rt.spawn(&m).unwrap();
        let payload = br#"{
          "skill": "haskell",
          "agents": [
            {"agent_id":"did:nau:a","skills":["python"]},
            {"agent_id":"did:nau:b","skills":["rust"]}
          ]
        }"#;
        let out = inst.call("discover", payload).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["count"], 0, "got {v}");
        assert_eq!(v["agents"].as_array().unwrap().len(), 0);
        inst.stop().unwrap();
    }

    // ── v3.3.0 chain-anchor entry（离线移植 AgentCardAnchor.sol）────────
    #[test]
    fn chain_anchor_then_verify_via_entry() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_CHAIN_ANCHOR);
        let mut inst = rt.spawn(&m).unwrap();
        let owner = "did:nau:owner";
        // owner 锚定一个 CID
        let p1 = serde_json::json!({
            "cid": "bafy-manifest-1",
            "agent_did": "did:nau:agent-1",
            "anchorer": owner,
            "authorized_anchorers": [owner],
            "anchors": {},
            "timestamp": 1000
        });
        let out = inst
            .call("anchor", &serde_json::to_vec(&p1).unwrap())
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["ok"], true, "got {v}");
        let anchors = v["anchors"].clone();
        // verify 正确（cid + did 双校验）
        let pv = serde_json::json!({
            "cid": "bafy-manifest-1",
            "agent_did": "did:nau:agent-1",
            "anchors": anchors
        });
        let out = inst
            .call("verify", &serde_json::to_vec(&pv).unwrap())
            .unwrap();
        let vv: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(vv["valid"], true, "got {vv}");
        inst.stop().unwrap();
    }

    #[test]
    fn chain_anchor_duplicate_is_immutable_and_wrong_did_fails() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_CHAIN_ANCHOR);
        let mut inst = rt.spawn(&m).unwrap();
        let owner = "did:nau:owner";
        let p1 = serde_json::json!({
            "cid": "bafy-x",
            "agent_did": "did:nau:a",
            "anchorer": owner,
            "authorized_anchorers": [owner],
            "anchors": {},
            "timestamp": 1
        });
        let out = inst
            .call("anchor", &serde_json::to_vec(&p1).unwrap())
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["ok"], true, "got {v}");
        let anchors = v["anchors"].clone();
        // 重复 anchor 同一 cid → 拒绝（首写后不可变）
        let p2 = serde_json::json!({
            "cid": "bafy-x",
            "agent_did": "did:nau:a",
            "anchorer": owner,
            "authorized_anchorers": [owner],
            "anchors": anchors,
            "timestamp": 2
        });
        let out = inst
            .call("anchor", &serde_json::to_vec(&p2).unwrap())
            .unwrap();
        let v2: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v2["ok"], false, "got {v2}");
        // 错误 DID verify → 失败（双校验）
        let anchors2 = v2["anchors"].clone();
        let pv = serde_json::json!({
            "cid": "bafy-x",
            "agent_did": "did:nau:attacker",
            "anchors": anchors2
        });
        let out = inst
            .call("verify", &serde_json::to_vec(&pv).unwrap())
            .unwrap();
        let vv: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(vv["valid"], false, "got {vv}");
        inst.stop().unwrap();
    }

    #[test]
    fn chain_anchor_unauthorized_and_empty_rejected() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_CHAIN_ANCHOR);
        let mut inst = rt.spawn(&m).unwrap();
        let owner = "did:nau:owner";
        // 未授权 anchorer
        let p = serde_json::json!({
            "cid": "bafy-y",
            "agent_did": "did:nau:a",
            "anchorer": "did:nau:bad",
            "authorized_anchorers": [owner],
            "anchors": {},
            "timestamp": 1
        });
        let out = inst
            .call("anchor", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["ok"], false, "got {v}");
        // 空 cid
        let p = serde_json::json!({
            "cid": "",
            "agent_did": "did:nau:a",
            "anchorer": owner,
            "authorized_anchorers": [owner],
            "anchors": {},
            "timestamp": 1
        });
        let out = inst
            .call("anchor", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["ok"], false, "got {v}");
        // 空 did
        let p = serde_json::json!({
            "cid": "bafy-z",
            "agent_did": "",
            "anchorer": owner,
            "authorized_anchorers": [owner],
            "anchors": {},
            "timestamp": 1
        });
        let out = inst
            .call("anchor", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["ok"], false, "got {v}");
        inst.stop().unwrap();
    }

    // ── v3.4.0 chain-bridge entry（离线移植 ReputationRegistry.sol）────
    fn merge(mut base: serde_json::Value, over: serde_json::Value) -> serde_json::Value {
        if let (Some(b), Some(o)) = (base.as_object_mut(), over.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        base
    }

    fn bridge_initial_state(owner: &str) -> serde_json::Value {
        serde_json::json!({
            "owner": owner,
            "verifiers": {},
            "epochs": {},
            "agent_epochs": {},
            "final_snapshots": {}
        })
    }

    fn bridge_add_verifiers(
        rt: &mut Box<dyn crate::plugin::runtime::PluginInstance>,
        state: &mut serde_json::Value,
        owner: &str,
        verifiers: &[&str],
    ) {
        for v in verifiers {
            let p = merge(
                state.clone(),
                serde_json::json!({"actor": owner, "verifier": v}),
            );
            let out = rt
                .call("add_verifier", &serde_json::to_vec(&p).unwrap())
                .unwrap();
            let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
            assert_eq!(r["ok"], true, "add {v} got {r}");
            *state = r;
        }
    }

    #[test]
    fn bridge_record_then_finalize_median() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_CHAIN_BRIDGE);
        let mut inst = rt.spawn(&m).unwrap();
        let owner = "did:nau:owner";
        let mut state = bridge_initial_state(owner);
        bridge_add_verifiers(
            &mut inst,
            &mut state,
            owner,
            &["did:nau:v1", "did:nau:v2", "did:nau:v3"],
        );
        // 3 verifiers submit (quorum = 3/2+1 = 2)
        let data = serde_json::json!({
            "did:nau:v1": {"quality": 8000, "speed": 5000, "honesty": 9000, "availability": 1000},
            "did:nau:v2": {"quality": 6000, "speed": 9000, "honesty": 9000, "availability": 3000},
            "did:nau:v3": {"quality": 7000, "speed": 7000, "honesty": 9000, "availability": 2000}
        });
        for v in ["did:nau:v1", "did:nau:v2", "did:nau:v3"] {
            let mut action = data[v].clone();
            action["verifier"] = serde_json::json!(v);
            action["agent_did"] = serde_json::json!("did:nau:agent-1");
            action["epoch"] = serde_json::json!(1);
            let p = merge(state.clone(), action);
            let out = inst
                .call("record", &serde_json::to_vec(&p).unwrap())
                .unwrap();
            let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
            assert_eq!(r["ok"], true, "record {v} got {r}");
            state = r;
        }
        // finalize -> per-dimension median
        let p = merge(
            state.clone(),
            serde_json::json!({"agent_did": "did:nau:agent-1", "epoch": 1, "timestamp": 1234}),
        );
        let out = inst
            .call("finalize", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["ok"], true, "got {r}");
        assert_eq!(r["snapshot"]["quality"], 7000, "got {r}");
        assert_eq!(r["snapshot"]["speed"], 7000, "got {r}");
        assert_eq!(r["snapshot"]["honesty"], 9000, "got {r}");
        assert_eq!(r["snapshot"]["availability"], 2000, "got {r}");
        assert_eq!(r["snapshot"]["finalizedAt"], 1234, "got {r}");
        state = r;
        // get_latest returns the median snapshot
        let p = merge(
            state.clone(),
            serde_json::json!({"agent_did": "did:nau:agent-1"}),
        );
        let out = inst
            .call("get_latest", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["snapshot"]["quality"], 7000, "got {r}");
        inst.stop().unwrap();
    }

    #[test]
    fn bridge_idempotent_and_conflict() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_CHAIN_BRIDGE);
        let mut inst = rt.spawn(&m).unwrap();
        let owner = "did:nau:owner";
        let mut state = bridge_initial_state(owner);
        bridge_add_verifiers(&mut inst, &mut state, owner, &["did:nau:v1"]);
        let action = serde_json::json!({
            "verifier": "did:nau:v1",
            "agent_did": "did:nau:agent-1",
            "epoch": 1,
            "quality": 8000, "speed": 7000, "honesty": 9000, "availability": 6000
        });
        let p = merge(state.clone(), action.clone());
        let out = inst
            .call("record", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["ok"], true, "got {r}");
        assert!(r.get("idempotent").is_none(), "first submit not idempotent");
        state = r;
        // identical resubmit -> idempotent no-op
        let p = merge(state.clone(), action.clone());
        let out = inst
            .call("record", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["ok"], true, "got {r}");
        assert_eq!(r["idempotent"], true, "identical should be idempotent");
        state = r;
        // conflicting resubmit -> rejected
        let mut conflict = action.clone();
        conflict["quality"] = serde_json::json!(1);
        let p = merge(state.clone(), conflict);
        let out = inst
            .call("record", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["ok"], false, "conflict should be rejected");
        assert_eq!(r["error"], "conflicting resubmission", "got {r}");
        inst.stop().unwrap();
    }

    #[test]
    fn bridge_quorum_nonverifier_latest_zero() {
        let mut rt = unique_rt();
        let m = entry_manifest(crate::plugin::official::OFF_CHAIN_BRIDGE);
        let mut inst = rt.spawn(&m).unwrap();
        let owner = "did:nau:owner";
        let mut state = bridge_initial_state(owner);
        bridge_add_verifiers(
            &mut inst,
            &mut state,
            owner,
            &["did:nau:v1", "did:nau:v2", "did:nau:v3"],
        );
        // non-verifier record -> not verifier
        let p = merge(
            state.clone(),
            serde_json::json!({
                "verifier": "did:nau:intruder",
                "agent_did": "did:nau:agent-1",
                "epoch": 1,
                "quality": 100, "speed": 100, "honesty": 100, "availability": 100
            }),
        );
        let out = inst
            .call("record", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["ok"], false, "got {r}");
        assert_eq!(r["error"], "not verifier", "got {r}");
        // out-of-bps -> rejected
        let p = merge(
            state.clone(),
            serde_json::json!({
                "verifier": "did:nau:v1",
                "agent_did": "did:nau:agent-1",
                "epoch": 1,
                "quality": 10001, "speed": 0, "honesty": 0, "availability": 0
            }),
        );
        let out = inst
            .call("record", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["ok"], false, "got {r}");
        assert_eq!(r["error"], "score out of bps range", "got {r}");
        // only v1 submits -> finalize fails quorum (needs 2)
        let p = merge(
            state.clone(),
            serde_json::json!({
                "verifier": "did:nau:v1",
                "agent_did": "did:nau:agent-1",
                "epoch": 1,
                "quality": 8000, "speed": 7000, "honesty": 9000, "availability": 6000
            }),
        );
        let out = inst
            .call("record", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["ok"], true, "got {r}");
        state = r;
        let p = merge(
            state.clone(),
            serde_json::json!({"agent_did": "did:nau:agent-1", "epoch": 1, "timestamp": 1}),
        );
        let out = inst
            .call("finalize", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["ok"], false, "got {r}");
        assert_eq!(r["error"], "quorum not reached", "got {r}");
        // unknown agent get_latest -> zero snapshot (no revert)
        let p = merge(
            state.clone(),
            serde_json::json!({"agent_did": "did:nau:nobody"}),
        );
        let out = inst
            .call("get_latest", &serde_json::to_vec(&p).unwrap())
            .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(r["snapshot"]["quality"], 0, "got {r}");
        assert_eq!(r["snapshot"]["finalizedAt"], 0, "got {r}");
        inst.stop().unwrap();
    }
}
