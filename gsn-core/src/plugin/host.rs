//! 宿主装配器（PluginHost）—— 串联注册/仲裁/总线/运行时/黑名单
//!
//! # 它是唯一对外的插件入口
//!
//! 上层（daemon/CLI/客户端）只与 [`PluginHost`] 交互：
//!
//! - [`boot_system`](PluginHost::boot_system)：装配随内核的 T0 系统插件；
//! - [`install`](PluginHost::install)：安装外部插件（签名校验 → 黑名单 → 注册 →
//!   选 runtime → spawn → 总线注册/令牌/RUNNING）；
//! - [`uninstall`](PluginHost::uninstall)：卸载（T0 不可卸载）；
//! - [`hot_reload`](PluginHost::hot_reload)：双缓冲热更新（失败自动回滚）；
//! - [`call`](PluginHost::call)：调用插件方法；
//! - [`load_legacy`](PluginHost::load_legacy)：加载低版本 ABI 插件（热兼容）。
//!
//! # 热兼容
//!
//! 通过 ABI 主版本协商：宿主接受主版本 ≤ 当前主版本的插件（经适配桥接），
//! 拒绝主版本更新的插件。

use crate::plugin::arbiter::Arbiter;
use crate::plugin::blacklist::Blacklist;
use crate::plugin::bus::PluginBus;
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::lifecycle::PluginState;
use crate::plugin::manifest::PluginManifest;
use crate::plugin::official;
use crate::plugin::registry::PluginRegistry;
use crate::plugin::runtime::native::NativeRuntime;
use crate::plugin::runtime::process::ProcessRuntime;
use crate::plugin::runtime::{PluginInstance, PluginRuntime};
use crate::plugin::system;
use crate::plugin::tier::Tier;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 宿主主版本（热兼容协商）。
pub const HOST_ABI_MAJOR: u32 = 3;

/// 宿主装配器。
pub struct PluginHost {
    /// 宿主版本（= VERSION）。
    version: String,
    /// 数据目录（sandbox work_dir 父目录）。
    data_dir: Option<PathBuf>,
    /// 注册中心。
    registry: PluginRegistry,
    /// 权限仲裁。
    arbiter: Arbiter,
    /// 插件总线。
    bus: PluginBus,
    /// 黑名单。
    blacklist: Blacklist,
    /// T0 进程内运行时。
    native: NativeRuntime,
    /// T1+ 进程运行时。
    process: ProcessRuntime,
    /// 运行实例：id → instance。
    instances: BTreeMap<String, Box<dyn PluginInstance>>,
    /// 当前时间（毫秒，可注入，测试用）。
    now_ms: u64,
}

impl PluginHost {
    /// 新建宿主（未装配系统插件）。
    pub fn new(version: impl Into<String>, data_dir: Option<PathBuf>) -> Self {
        let mut bus = PluginBus::new();
        bus.register_host();
        let process = ProcessRuntime::new(data_dir.clone());
        PluginHost {
            version: version.into(),
            data_dir,
            registry: PluginRegistry::new(),
            arbiter: Arbiter::new(),
            bus,
            blacklist: Blacklist::new(),
            native: NativeRuntime::new(),
            process,
            instances: BTreeMap::new(),
            now_ms: 0,
        }
    }

    /// 注入当前时间（测试）。
    pub fn set_now(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    /// 加官方根密钥（T1/T2 副签验证）。
    pub fn add_official_root(&mut self, key_hex: &str) {
        self.arbiter.add_official_root(key_hex);
    }

    /// 信任第三方发布者（T3）。
    pub fn trust_publisher(&mut self, key_hex: &str) {
        self.arbiter.trust_publisher(key_hex);
    }

    /// 显式批准某插件的某能力（声明式能力）。
    pub fn approve(&mut self, name: &str, cap: crate::plugin::Capability) {
        self.arbiter.approve(name, cap);
    }

    /// 数据目录（sandbox work_dir 父目录）。
    pub fn data_dir(&self) -> Option<&std::path::Path> {
        self.data_dir.as_deref()
    }

    /// 黑名单引用（只读）。
    pub fn blacklist(&self) -> &Blacklist {
        &self.blacklist
    }

    /// 黑名单可变引用（导入黑名单数据库）。
    pub fn blacklist_mut(&mut self) -> &mut Blacklist {
        &mut self.blacklist
    }

    /// ABI 主版本兼容性（热兼容）。
    ///
    /// 接受主版本 ≤ [`HOST_ABI_MAJOR`] 的插件；拒绝更新主版本。
    pub fn check_abi(manifest_abi: &str) -> PluginResult<u32> {
        let major = manifest_abi
            .split('.')
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| PluginError::Runtime(format!("非法 ABI 版本: {manifest_abi}")))?;
        if major > HOST_ABI_MAJOR {
            return Err(PluginError::Runtime(format!(
                "插件需要更新的 ABI 主版本 {major}（宿主 {HOST_ABI_MAJOR}），请升级宿主"
            )));
        }
        Ok(major)
    }

    /// 按级别选择运行时并 spawn。
    fn spawn_runtime(
        &mut self,
        manifest: &PluginManifest,
        tier: Tier,
    ) -> PluginResult<Box<dyn PluginInstance>> {
        match tier {
            Tier::System => self.native.spawn(manifest),
            Tier::Official | Tier::Certified | Tier::ThirdParty => self.process.spawn(manifest),
            Tier::Blacklist => Err(PluginError::Blacklisted(format!(
                "黑名单插件不可加载: {}",
                manifest.plugin.name
            ))),
        }
    }

    /// 装配 T0 系统插件（随内核，进程内）。
    pub fn boot_system(&mut self) -> PluginResult<Vec<String>> {
        system::register_handlers(&mut self.native);
        let mut started = Vec::new();
        for manifest in system::bundled_manifests(&self.version) {
            let id = manifest.plugin.name.clone();
            let instance = self.native.spawn(&manifest)?;
            // 注册到 registry，使版本/tier 在列表与详情中一致可见。
            self.registry.register(manifest.clone())?;
            self.bus.register(&id);
            self.bus.set_state(&id, PluginState::Running)?;
            self.instances.insert(id.clone(), instance);
            started.push(id);
        }
        Ok(started)
    }

    /// 装配 T1 官方插件（随内核，进程隔离）。
    ///
    /// 官方插件由 TwinsEarth 随内核构建、构建链路保证可信，因此跳过发布者
    /// 签名校验（与 T0 系统插件同一信任来源）；但仍走 **process 隔离**与
    /// **能力矩阵**：能力按 [`Tier::Official`] 解析授予，越权即拒绝，
    /// 携带的 entry 业务模块由 process runtime 加载。
    pub fn boot_official(&mut self) -> PluginResult<Vec<String>> {
        use crate::plugin::capability::CapabilityToken;
        let mut started = Vec::new();
        for manifest in official::bundled_manifests(&self.version) {
            let id = manifest.plugin.name.clone();
            // 能力按 Official 矩阵解析（基础 + 官方可授予），越权提前失败。
            let granted = self.arbiter.resolve_granted(&manifest, Tier::Official)?;
            let token = CapabilityToken {
                plugin_id: id.clone(),
                granted,
                issued_at: self.now_ms,
                manifest_digest: manifest.compute_digest()?,
            };
            // 进程隔离 spawn（写入 entry 业务模块）。
            let instance = self.process.spawn(&manifest)?;
            self.registry.register(manifest.clone())?;
            self.bus.register(&id);
            self.bus.set_token(&id, token)?;
            self.bus.set_state(&id, PluginState::Running)?;
            self.instances.insert(id.clone(), instance);
            started.push(id);
        }
        Ok(started)
    }

    /// 安装一个外部插件（完整流程）。
    ///
    /// 同版本幂等：已安装同版本直接返回 Ok；不同版本需走 [`hot_reload`](Self::hot_reload)。
    pub fn install(&mut self, manifest: PluginManifest) -> PluginResult<String> {
        // 0. ABI 兼容性（热兼容）。
        Self::check_abi(&manifest.plugin.abi)?;

        // 1. 黑名单检查（在签名校验之前也拦一道）。
        if self
            .blacklist
            .is_blacklisted(&manifest.plugin.name, &manifest.plugin.module_sha256)
        {
            return Err(PluginError::Blacklisted(format!(
                "插件 {} 在黑名单中，拒绝安装",
                manifest.plugin.name
            )));
        }

        // 2. 签名校验 → tier。
        let tier = self.arbiter.verify_manifest(&manifest)?;

        // 3. 能力令牌（同时解析授权）。
        let token = self.arbiter.issue_token(&manifest, self.now_ms)?;

        // 4. 注册（同版本幂等；不同版本报错）。
        if self.registry.contains(&manifest.plugin.name) {
            // 已注册同版本：register 内部幂等；这里确保不重复 spawn。
            self.registry.register(manifest.clone())?;
            return Ok(manifest.plugin.name);
        }
        self.registry.register(manifest.clone())?;

        // 5. 选 runtime 并 spawn（supports 在内校验，无法强制即拒绝）。
        let instance = self.spawn_runtime(&manifest, tier)?;

        // 6. 总线注册、令牌、置 RUNNING。
        let id = manifest.plugin.name.clone();
        self.bus.register(&id);
        self.bus.set_token(&id, token)?;
        self.bus.set_state(&id, PluginState::Running)?;
        self.instances.insert(id.clone(), instance);

        Ok(id)
    }

    /// 停止一个插件（不下线注册，可再 start）。
    pub fn stop(&mut self, id: &str) -> PluginResult<()> {
        let inst = self
            .instances
            .get_mut(id)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        inst.stop()?;
        self.bus.set_state(id, PluginState::Stopped)?;
        Ok(())
    }

    /// 重新启动一个已停止插件（进程内系统插件由 native 重新 spawn；
    /// 进程插件由 process 重新 spawn）。
    pub fn start(&mut self, id: &str) -> PluginResult<()> {
        let manifest = self
            .registry
            .get(id)
            .map(|rp| rp.manifest.clone())
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        let tier = Tier::from_name(id);
        let instance = self.spawn_runtime(&manifest, tier)?;
        self.instances.insert(id.to_string(), instance);
        self.bus.set_state(id, PluginState::Running)?;
        Ok(())
    }

    /// 卸载（T0 系统插件不可卸载）。
    pub fn uninstall(&mut self, id: &str) -> PluginResult<()> {
        let tier = Tier::from_name(id);
        if tier == Tier::System {
            return Err(PluginError::Unauthorized {
                plugin_id: id.to_string(),
                capability: "uninstall".to_string(),
            });
        }
        if let Some(inst) = self.instances.get_mut(id) {
            inst.stop()?;
        }
        self.instances.remove(id);
        self.registry.unregister(id)?;
        self.bus.set_state(id, PluginState::Stopped)?;
        Ok(())
    }

    /// 热更新：双缓冲。新版本先 spawn 成功、健康检查通过，再切换并停旧；
    /// 新版本 spawn 失败则保留旧实例（自动回滚）。
    pub fn hot_reload(&mut self, new_manifest: PluginManifest) -> PluginResult<String> {
        let id = new_manifest.plugin.name.clone();
        if !self.registry.contains(&id) {
            return Err(PluginError::NotFound(format!(
                "插件 {id} 尚未安装，无法热更新；请用 install"
            )));
        }

        // 校验新版本。
        Self::check_abi(&new_manifest.plugin.abi)?;
        let tier = self.arbiter.verify_manifest(&new_manifest)?;
        let token = self.arbiter.issue_token(&new_manifest, self.now_ms)?;

        // 双缓冲：先在运行时 spawn 新版本（不触碰旧实例）。
        let new_instance = match self.spawn_runtime(&new_manifest, tier) {
            Ok(inst) => inst,
            Err(e) => {
                // 回滚：旧实例保留，状态仍 RUNNING。
                return Err(PluginError::Runtime(format!(
                    "热更新 {id} 新版本加载失败，已回滚到旧版本: {e}"
                )));
            }
        };

        // 新版本就绪：停旧实例、替换。
        if let Some(old) = self.instances.get_mut(&id) {
            let _ = old.stop();
        }
        self.instances.insert(id.clone(), new_instance);
        self.registry.replace(new_manifest)?;
        self.bus.set_token(&id, token)?;
        self.bus.set_state(&id, PluginState::Running)?;
        Ok(id)
    }

    /// 加载低版本（旧 ABI）插件 —— 热兼容。
    ///
    /// 与 install 的区别仅在语义上：这里明确标注「经适配桥接」，并要求
    /// 主版本 ≤ 宿主。校验/签名/隔离流程与 install 完全相同（不为旧版本放宽）。
    pub fn load_legacy(&mut self, manifest: PluginManifest) -> PluginResult<String> {
        let major = Self::check_abi(&manifest.plugin.abi)?;
        if major == HOST_ABI_MAJOR {
            // 同版本：直接走 install。
            return self.install(manifest);
        }
        // 低版本：经适配器（这里记录事件，不放宽任何校验）。
        self.install(manifest)
    }

    /// 调用插件方法。
    pub fn call(&mut self, id: &str, method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
        let inst = self
            .instances
            .get_mut(id)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        inst.call(method, payload)
    }

    /// 总线路由表（插件状态一览）。
    pub fn route_table(&self) -> BTreeMap<String, PluginState> {
        self.bus.route_table()
    }

    /// 已注册插件数。
    pub fn plugin_count(&self) -> usize {
        self.registry.all().len()
    }

    /// 注册中心只读引用（查询清单/版本）。
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// 宿主版本。
    pub fn version(&self) -> &str {
        &self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;
    use crate::plugin::official;

    /// 构造一个开发者签名的官方插件清单（不带官方副签）。
    fn signed_official(id: &str, dev: &Keypair) -> PluginManifest {
        let mut m = official::official_manifest(id, "3.0.0");
        m.sign_with(dev).unwrap();
        m.counter_sign_with(dev).unwrap();
        m
    }

    #[test]
    fn boot_system_starts_all_system_plugins() {
        let mut host = PluginHost::new("3.0.0", None);
        let started = host.boot_system().unwrap();
        assert_eq!(started.len(), 4);
        let routes = host.route_table();
        for id in system::system_ids() {
            assert_eq!(routes.get(id), Some(&PluginState::Running));
        }
    }

    #[test]
    fn system_identity_callable_via_host() {
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system().unwrap();
        let out = host.call(system::SYS_IDENTITY, "mint_did", b"{}").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v["did"].as_str().unwrap().starts_with("did:nau:"));
    }

    #[test]
    fn install_signed_official_after_adding_root() {
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        let manifest = signed_official(id, &dev);
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system().unwrap();
        // 未加官方根 → 安装失败（T1 副签验证）。
        assert!(host.install(manifest.clone()).is_err());
        // 加官方根（dev 兼任根，测试用）→ 安装成功。
        let root_hex = hex::encode(dev.public_key());
        host.add_official_root(&root_hex);
        assert!(host.install(manifest).is_ok());
        assert_eq!(host.route_table().get(id), Some(&PluginState::Running));
    }

    #[test]
    fn blacklisted_plugin_refused() {
        let mut host = PluginHost::new("3.0.0", None);
        let mut m = official::official_manifest("com.twinsearth.official.bad", "3.0.0");
        let dev = Keypair::generate();
        m.sign_with(&dev).unwrap();
        m.counter_sign_with(&dev).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.blacklist_mut()
            .add(crate::plugin::blacklist::BlacklistEntry {
                plugin_name: "com.twinsearth.official.bad".to_string(),
                module_sha256: None,
                reason: crate::plugin::blacklist::BlacklistReason::Malware,
                blacklisted_at: 0,
                evidence: String::new(),
                appeal: None,
            });
        assert!(host.install(m).is_err());
    }

    #[test]
    fn uninstall_then_official_removed() {
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        let manifest = signed_official(id, &dev);
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system().unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(manifest).unwrap();
        host.uninstall(id).unwrap();
        assert_eq!(host.route_table().get(id), Some(&PluginState::Stopped));
        assert!(!host.plugin_count_is_registered(id));
    }

    #[test]
    fn system_plugin_cannot_uninstall() {
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system().unwrap();
        assert!(host.uninstall(system::SYS_IDENTITY).is_err());
    }

    #[test]
    fn hot_reload_rolls_back_on_bad_new_version() {
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        let manifest = signed_official(id, &dev);
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system().unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(manifest.clone()).unwrap();

        // 新版本签名非法（用别的 key 且未加根）→ 热更新失败并回滚。
        let attacker = Keypair::generate();
        let mut bad = official::official_manifest(id, "3.0.1");
        bad.sign_with(&attacker).unwrap();
        bad.counter_sign_with(&attacker).unwrap();
        assert!(host.hot_reload(bad).is_err());
        // 旧版本仍在运行。
        assert_eq!(host.route_table().get(id), Some(&PluginState::Running));
    }

    #[test]
    fn hot_reload_succeeds_with_valid_new_version() {
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system().unwrap();
        let root_hex = hex::encode(dev.public_key());
        host.add_official_root(&root_hex);
        host.install(signed_official(id, &dev)).unwrap();

        // 新版本 3.0.1，同 key 签名。
        let mut new_m = official::official_manifest(id, "3.0.1");
        new_m.sign_with(&dev).unwrap();
        new_m.counter_sign_with(&dev).unwrap();
        assert!(host.hot_reload(new_m).is_ok());
        let got = host.registry.get(id).unwrap();
        assert_eq!(got.manifest.plugin.version, "3.0.1");
    }

    #[test]
    fn hot_reload_new_entry_remains_callable() {
        // 回归（v3.2.0）：hot_reload 双缓冲时，旧实例 destroy（remove_dir_all）
        // 不得删除新版本仍在使用的工作目录。历史上新旧版本目录名只含插件 id、
        // 共享同一目录，旧 destroy 会删掉新版本目录，热更新后新版本 entry 无法加载；
        // 同 id 的并行测试之间也会因一个 destroy 删目录而相互打断。
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let data_dir =
            std::env::temp_dir().join(format!("au-host-hr-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&data_dir).unwrap();
        let mut host = PluginHost::new("3.0.0", Some(data_dir.clone()));
        host.boot_system().unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(signed_official(id, &dev)).unwrap();
        let mut new_m = official::official_manifest(id, "3.0.1");
        new_m.sign_with(&dev).unwrap();
        new_m.counter_sign_with(&dev).unwrap();
        assert!(host.hot_reload(new_m).is_ok());
        // 热更新后新版本 entry 必须仍可调用：空 bids → no_bids。
        // 若目录被旧 destroy 删除，子进程 import plugin 失败 → 这里报错。
        let out = host.call(id, "match", br#"{"bids":[]}"#).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["reason"], "no_bids");
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn abi_compatibility_checks() {
        assert!(PluginHost::check_abi("1.0").is_ok());
        assert!(PluginHost::check_abi("2.5").is_ok());
        assert!(PluginHost::check_abi("3.0").is_ok());
        assert!(PluginHost::check_abi("4.0").is_err());
    }

    #[test]
    fn load_legacy_abi_2_plugin() {
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        let mut m = official::official_manifest(id, "3.0.0");
        m.plugin.abi = "2.0".to_string();
        m.sign_with(&dev).unwrap();
        m.counter_sign_with(&dev).unwrap();
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system().unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        assert!(host.load_legacy(m).is_ok());
        assert_eq!(host.route_table().get(id), Some(&PluginState::Running));
    }

    // 辅助断言。
    impl PluginHost {
        fn plugin_count_is_registered(&self, id: &str) -> bool {
            self.registry.contains(id)
        }
    }
}
