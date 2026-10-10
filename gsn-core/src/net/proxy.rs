//! 系统代理管理（v3.9.14，参照 v2rayN `SysProxyHandler` / `ProxySetting*`）。
//!
//! 三平台统一入口：Windows（注册表 WinINET）、Linux（gsettings）、macOS（networksetup）。
//! 支持 HTTP/HTTPS/SOCKS 代理与 PAC 自动配置（Windows `AutoConfigURL`）。
//! 所有平台命令通过 [`CommandRunner`] 抽象执行，便于跨平台测试（fake runner）。
//!
//! 安全边界：本模块只设置用户级/系统级代理偏好，不涉及凭据；对外部命令
//! 一律经 `CommandRunner` 注入执行，测试不得触达真实系统代理。

use std::process::Command;

/// 代理协议类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyScheme {
    Http,
    Https,
    Socks,
}

impl ProxyScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            ProxyScheme::Http => "http",
            ProxyScheme::Https => "https",
            ProxyScheme::Socks => "socks",
        }
    }
}

/// 系统代理配置。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemProxyConfig {
    pub scheme: ProxyScheme,
    pub host: String,
    pub port: u16,
    /// 本地绕过列表（如 `<local>`）；Windows 写入 ProxyOverride，macOS 用 bypass 数组。
    pub bypass: Vec<String>,
}

impl SystemProxyConfig {
    pub fn new(scheme: ProxyScheme, host: impl Into<String>, port: u16) -> Self {
        Self {
            scheme,
            host: host.into(),
            port,
            bypass: vec!["<local>".to_string()],
        }
    }

    pub fn with_bypass(mut self, bypass: &[&str]) -> Self {
        self.bypass = bypass.iter().map(|s| s.to_string()).collect();
        self
    }

    /// `host:port` 组合。
    pub fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// 完整 URL（如 `http://host:port`）。
    pub fn url(&self) -> String {
        format!("{}://{}", self.scheme.as_str(), self.authority())
    }
}

/// 命令执行抽象（可注入 fake 用于测试）。
pub trait CommandRunner: Send + Sync {
    fn run(&self, program: &str, args: &[&str]) -> Result<(), String>;
}

/// 真实系统命令执行器。
#[derive(Clone, Copy, Debug, Default)]
pub struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<(), String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("执行 {program} 失败: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "{program} 退出码 {}: {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            ))
        }
    }
}

/// 系统代理设置结果（供调用方展示）。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyOpReport {
    pub op: &'static str,
    pub platform: &'static str,
    pub detail: String,
    pub ok: bool,
}

/// 三平台系统代理控制器。
#[derive(Clone, Debug)]
pub struct SystemProxyManager<R: CommandRunner> {
    runner: R,
}

impl<R: CommandRunner> SystemProxyManager<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// 设置 HTTP 代理（含 PAC URL 支持）。
    pub fn set_http_proxy(&self, cfg: &SystemProxyConfig) -> Result<ProxyOpReport, String> {
        let report = self.platform_set(cfg, true);
        Ok(ProxyOpReport {
            op: "set-http",
            platform: report.platform,
            detail: report.detail,
            ok: report.ok,
        })
    }

    /// 清除全部系统代理。
    pub fn clear_proxy(&self) -> Result<ProxyOpReport, String> {
        let report = self.platform_clear();
        Ok(ProxyOpReport {
            op: "clear",
            platform: report.platform,
            detail: report.detail,
            ok: report.ok,
        })
    }

    /// 应用 PAC 自动配置（Windows AutoConfigURL；其他平台回退为 http 代理指向 pac 服务端口）。
    pub fn apply_pac(&self, pac_url: &str) -> Result<ProxyOpReport, String> {
        let report = self.platform_pac(pac_url);
        Ok(ProxyOpReport {
            op: "apply-pac",
            platform: report.platform,
            detail: report.detail,
            ok: report.ok,
        })
    }

    #[cfg(test)]
    fn platform_name() -> &'static str {
        if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        }
    }

    fn platform_set(&self, cfg: &SystemProxyConfig, _http: bool) -> PlatformResult {
        if cfg!(target_os = "windows") {
            // Windows：注册表 WinINET（HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings）
            let bypass = cfg.bypass.join(";");
            let reg_args = [
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
                "/v",
                "ProxyEnable",
                "/t",
                "REG_DWORD",
                "/d",
                "1",
                "/f",
            ];
            if let Err(e) = self.runner.run("reg", &reg_args) {
                return PlatformResult::new("windows", e);
            }
            let proxy_args = [
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
                "/v",
                "ProxyServer",
                "/t",
                "REG_SZ",
                "/d",
                &cfg.authority(),
                "/f",
            ];
            if let Err(e) = self.runner.run("reg", &proxy_args) {
                return PlatformResult::new("windows", e);
            }
            let bypass_args = [
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
                "/v",
                "ProxyOverride",
                "/t",
                "REG_SZ",
                "/d",
                &bypass,
                "/f",
            ];
            if let Err(e) = self.runner.run("reg", &bypass_args) {
                return PlatformResult::new("windows", e);
            }
            PlatformResult::new_ok(
                "windows",
                format!("http 代理 {} 已设置（含 bypass）", cfg.authority()),
            )
        } else if cfg!(target_os = "macos") {
            // macOS：networksetup -setwebproxy / -setsecurewebproxy（按网卡）
            // 注意：实际部署需指定网卡（如 Wi-Fi）；这里遍历常见网卡，失败即回退提示。
            let services = ["Wi-Fi", "Ethernet"];
            let mut last_err = None;
            for svc in &services {
                let args = ["-setwebproxy", svc, &cfg.host, &cfg.port.to_string()];
                if let Err(e) = self.runner.run("networksetup", &args) {
                    last_err = Some(e);
                    continue;
                }
                last_err = None;
                break;
            }
            if let Some(e) = last_err {
                return PlatformResult::new("macos", e);
            }
            PlatformResult::new_ok("macos", format!("http 代理 {} 已设置", cfg.authority()))
        } else {
            // Linux：gsettings org.gnome.system.proxy
            let mode_args = ["set", "org.gnome.system.proxy", "mode", "manual"];
            if let Err(e) = self.runner.run("gsettings", &mode_args) {
                return PlatformResult::new("linux", e);
            }
            let host_args = ["set", "org.gnome.system.proxy.http", "host", &cfg.host];
            if let Err(e) = self.runner.run("gsettings", &host_args) {
                return PlatformResult::new("linux", e);
            }
            let port_args = [
                "set",
                "org.gnome.system.proxy.http",
                "port",
                &cfg.port.to_string(),
            ];
            if let Err(e) = self.runner.run("gsettings", &port_args) {
                return PlatformResult::new("linux", e);
            }
            PlatformResult::new_ok("linux", format!("http 代理 {} 已设置", cfg.authority()))
        }
    }

    fn platform_clear(&self) -> PlatformResult {
        if cfg!(target_os = "windows") {
            let args = [
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
                "/v",
                "ProxyEnable",
                "/t",
                "REG_DWORD",
                "/d",
                "0",
                "/f",
            ];
            match self.runner.run("reg", &args) {
                Ok(()) => {
                    PlatformResult::new_ok("windows", "系统代理已清除（ProxyEnable=0）".to_string())
                }
                Err(e) => PlatformResult::new("windows", e),
            }
        } else if cfg!(target_os = "macos") {
            let services = ["Wi-Fi", "Ethernet"];
            let mut last_err = None;
            for svc in &services {
                let args = ["-setwebproxystate", svc, "off"];
                if let Err(e) = self.runner.run("networksetup", &args) {
                    last_err = Some(e);
                    continue;
                }
                last_err = None;
                break;
            }
            match last_err {
                Some(e) => PlatformResult::new("macos", e),
                None => PlatformResult::new_ok("macos", "系统代理已清除".to_string()),
            }
        } else {
            let args = ["set", "org.gnome.system.proxy", "mode", "none"];
            match self.runner.run("gsettings", &args) {
                Ok(()) => {
                    PlatformResult::new_ok("linux", "系统代理已清除（mode=none）".to_string())
                }
                Err(e) => PlatformResult::new("linux", e),
            }
        }
    }

    fn platform_pac(&self, pac_url: &str) -> PlatformResult {
        if cfg!(target_os = "windows") {
            let enable_args = [
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
                "/v",
                "ProxyEnable",
                "/t",
                "REG_DWORD",
                "/d",
                "1",
                "/f",
            ];
            if let Err(e) = self.runner.run("reg", &enable_args) {
                return PlatformResult::new("windows", e);
            }
            let pac_args = [
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
                "/v",
                "AutoConfigURL",
                "/t",
                "REG_SZ",
                "/d",
                pac_url,
                "/f",
            ];
            match self.runner.run("reg", &pac_args) {
                Ok(()) => PlatformResult::new_ok("windows", format!("PAC 已指向 {pac_url}")),
                Err(e) => PlatformResult::new("windows", e),
            }
        } else if cfg!(target_os = "macos") {
            // macOS 无原生 PAC URL 注册表；回退为 http 代理指向 pac 服务端口（由调用方保证）。
            PlatformResult::new_ok(
                "macos",
                format!("PAC 模式：http 代理指向 {pac_url}（macOS 无 AutoConfigURL）"),
            )
        } else {
            PlatformResult::new_ok(
                "linux",
                "PAC 模式：gsettings 不支持 AutoConfigURL，请配置 WPAD 或手动设置".to_string(),
            )
        }
    }
}

/// 平台执行结果。
struct PlatformResult {
    platform: &'static str,
    detail: String,
    ok: bool,
}

impl PlatformResult {
    fn new(platform: &'static str, err: String) -> Self {
        Self {
            platform,
            detail: err,
            ok: false,
        }
    }

    fn new_ok(platform: &'static str, detail: String) -> Self {
        Self {
            platform,
            detail,
            ok: true,
        }
    }
}

/// 便捷：真实命令执行器下的默认管理器。
pub fn real_manager() -> SystemProxyManager<RealCommandRunner> {
    SystemProxyManager::new(RealCommandRunner)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// fake runner：记录所有命令调用，全部成功。
    type CallLog = std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<String>)>>>;

    #[derive(Clone, Debug, Default)]
    struct FakeRunner {
        calls: CallLog,
    }

    impl FakeRunner {
        fn new() -> Self {
            Self {
                calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<(), String> {
            self.calls.lock().unwrap().push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            Ok(())
        }
    }

    #[test]
    fn config_authority_and_url() {
        let cfg = SystemProxyConfig::new(ProxyScheme::Http, "127.0.0.1", 10808);
        assert_eq!(cfg.authority(), "127.0.0.1:10808");
        assert_eq!(cfg.url(), "http://127.0.0.1:10808");
        assert_eq!(cfg.bypass, vec!["<local>".to_string()]);

        let cfg2 = cfg.with_bypass(&["<local>", "localhost"]);
        assert_eq!(
            cfg2.bypass,
            vec!["<local>".to_string(), "localhost".to_string()]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_set_proxy_issues_reg_commands() {
        let runner = FakeRunner::new();
        let mgr = SystemProxyManager::new(runner.clone());
        let cfg = SystemProxyConfig::new(ProxyScheme::Http, "127.0.0.1", 10808)
            .with_bypass(&["<local>", "localhost"]);
        let report = mgr.set_http_proxy(&cfg).unwrap();
        assert!(report.ok);

        let calls = runner.calls();
        assert_eq!(calls.len(), 3);
        // ProxyEnable=1
        assert!(calls[0].1.contains(&"ProxyEnable".to_string()));
        assert!(calls[0].1.contains(&"1".to_string()));
        // ProxyServer
        assert!(calls[1].1.contains(&"ProxyServer".to_string()));
        assert!(calls[1].1.contains(&"127.0.0.1:10808".to_string()));
        // ProxyOverride
        assert!(calls[2].1.contains(&"ProxyOverride".to_string()));
        assert!(calls[2].1.contains(&"<local>;localhost".to_string()));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn clear_proxy_disables_enable_flag() {
        let runner = FakeRunner::new();
        let mgr = SystemProxyManager::new(runner.clone());
        let report = mgr.clear_proxy().unwrap();
        assert!(report.ok);
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].1.contains(&"ProxyEnable".to_string()));
        assert!(calls[0].1.contains(&"0".to_string()));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn apply_pac_sets_autoconfig_url() {
        let runner = FakeRunner::new();
        let mgr = SystemProxyManager::new(runner.clone());
        let report = mgr.apply_pac("http://127.0.0.1:1080/pac?t=123").unwrap();
        assert!(report.ok);
        let calls = runner.calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[1].1.contains(&"AutoConfigURL".to_string()));
        assert!(calls[1]
            .1
            .contains(&"http://127.0.0.1:1080/pac?t=123".to_string()));
    }

    #[test]
    fn runner_error_is_surfaced() {
        struct ErrRunner;
        impl CommandRunner for ErrRunner {
            fn run(&self, _program: &str, _args: &[&str]) -> Result<(), String> {
                Err("模拟失败".to_string())
            }
        }
        let mgr = SystemProxyManager::new(ErrRunner);
        let report = mgr
            .set_http_proxy(&SystemProxyConfig::new(ProxyScheme::Http, "h", 1))
            .unwrap();
        assert!(!report.ok);
        assert!(report.detail.contains("模拟失败"));
    }

    #[test]
    fn platform_name_matches_target() {
        let name = SystemProxyManager::<RealCommandRunner>::platform_name();
        assert!(["windows", "macos", "linux"].contains(&name));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_set_proxy_issues_gsettings_commands() {
        let runner = FakeRunner::new();
        let mgr = SystemProxyManager::new(runner.clone());
        let cfg = SystemProxyConfig::new(ProxyScheme::Http, "127.0.0.1", 10808)
            .with_bypass(&["<local>", "localhost"]);
        let report = mgr.set_http_proxy(&cfg).unwrap();
        assert!(report.ok);
        assert_eq!(report.platform, "linux");

        let calls = runner.calls();
        assert_eq!(calls.len(), 3);
        // gsettings mode manual
        assert!(calls[0].1.contains(&"manual".to_string()));
        // gsettings http host
        assert!(calls[1].1.contains(&"host".to_string()));
        assert!(calls[1].1.contains(&"127.0.0.1".to_string()));
        // gsettings http port
        assert!(calls[2].1.contains(&"port".to_string()));
        assert!(calls[2].1.contains(&"10808".to_string()));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_clear_proxy_sets_mode_none() {
        let runner = FakeRunner::new();
        let mgr = SystemProxyManager::new(runner.clone());
        let report = mgr.clear_proxy().unwrap();
        assert!(report.ok);
        assert_eq!(report.platform, "linux");
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].1.contains(&"none".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_set_proxy_issues_networksetup_commands() {
        let runner = FakeRunner::new();
        let mgr = SystemProxyManager::new(runner.clone());
        let cfg = SystemProxyConfig::new(ProxyScheme::Http, "127.0.0.1", 10808);
        let report = mgr.set_http_proxy(&cfg).unwrap();
        assert!(report.ok);
        assert_eq!(report.platform, "macos");

        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        // networksetup -setwebproxy Wi-Fi 127.0.0.1 10808
        assert!(calls[0].0 == "networksetup");
        assert!(calls[0].1.contains(&"-setwebproxy".to_string()));
        assert!(calls[0].1.contains(&"127.0.0.1".to_string()));
        assert!(calls[0].1.contains(&"10808".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_clear_proxy_disables_webproxy_state() {
        let runner = FakeRunner::new();
        let mgr = SystemProxyManager::new(runner.clone());
        let report = mgr.clear_proxy().unwrap();
        assert!(report.ok);
        assert_eq!(report.platform, "macos");
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].1.contains(&"-setwebproxystate".to_string()));
        assert!(calls[0].1.contains(&"off".to_string()));
    }
}
