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
use crate::plugin::bus::{MessageKind, PluginBus, PmbMessage, Priority, Target};
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

/// 当前墙钟毫秒（Unix epoch）。SystemTime 早于 epoch 在实践中不可能，若真出现则返回 0——
/// 此时总线时钟等于 `CLOCK_NOT_SET`，TTL 检查按未注入处理（安全降级，绝不 panic）。
fn real_wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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

    /// 构造消息 issued_at 时使用的宿主时钟：测试经 [`set_now`] 显式注入则沿用（确定性），
    /// 生产 `now_ms==0` 时退到真实墙钟，避免把自签消息的 issued_at 钉死在 1970 年。
    fn clock_now_ms(&self) -> u64 {
        if self.now_ms != 0 {
            self.now_ms
        } else {
            real_wall_ms()
        }
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

    /// 启动时从环境变量 `GSN_BLACKLIST_FILE` 播种黑名单（v3.5.2，AU-07/AU-24）。
    ///
    /// 文件每行一个 plugin-id；未设置该变量或文件不存在 = 空黑名单（保持现状）。
    /// 文件存在但读取失败时返回类型化错误，由调用方（节点启动）决定是否拒绝启动。
    /// 不联网、不引入任何硬编码封禁。
    pub fn seed_blacklist_from_env(&mut self) -> PluginResult<()> {
        let Ok(path) = std::env::var("GSN_BLACKLIST_FILE") else {
            return Ok(());
        };
        if path.trim().is_empty() {
            return Ok(());
        }
        let loaded = Blacklist::load_operator_file(std::path::Path::new(&path))
            .map_err(|e| PluginError::Runtime(format!("GSN_BLACKLIST_FILE: {e}")))?;
        for e in loaded.entries().iter().cloned() {
            self.blacklist.add(e);
        }
        Ok(())
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
    ///
    /// `handles` 接线 daemon 真实句柄（存储/网络）；未接线的能力对应方法不注册。
    pub fn boot_system(&mut self, handles: &system::SystemHandles) -> PluginResult<Vec<String>> {
        system::register_handlers(&mut self.native, handles);
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

        // 0.5（v3.5.2，AU-08）外部 install() 不得占用系统命名空间
        //     `com.twinsearth.sys.*`。该命名空间在 arbiter 中走 Tier::System 分支、
        //     「签名由宿主构建链路保证」而**不验签**；若外部自证系统名，即可绕过一切
        //     签名/发布者信任，直接获得进程内全能力（提权）。系统插件只允许由
        //     [`PluginHost::boot_system`]（宿主构建链路）装配。
        if Tier::from_name(&manifest.plugin.name) == Tier::System {
            return Err(PluginError::Manifest(format!(
                "PLUGIN_SYS_NAMESPACE_FORBIDDEN: 外部安装不得使用系统命名空间 {}",
                manifest.plugin.name
            )));
        }

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

        // 5.（v3.5.2，AU-23）选 runtime 并 spawn：**先 spawn 成功再落注册/总线**，
        //    spawn 失败时不写 registry、不留孤儿注册项（旧实现先 registry.register
        //    再 spawn，spawn 失败会留下与实例 desync 的残留注册）。
        let instance = self.spawn_runtime(&manifest, tier)?;

        // 6. spawn 成功：注册 + 总线注册、令牌、置 RUNNING。
        let id = manifest.plugin.name.clone();
        self.registry.register(manifest.clone())?;
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
        //（v3.5.2，AU-07）start() 重新过黑名单闸门：插件可能在停止期间被操作员/运行时
        // 加入黑名单，不得仅因「之前装过」就重新拉起进入数据面。
        if self
            .blacklist
            .is_blacklisted(id, &manifest.plugin.module_sha256)
        {
            return Err(PluginError::Blacklisted(format!(
                "插件 {id} 在黑名单中，拒绝启动"
            )));
        }
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
        //（v3.5.2，AU-24）卸载必须从总线移除整条 RouteEntry（Sender/状态/令牌），
        // 旧实现只置 Stopped 而保留路由，形成总线上的孤儿路由。
        self.bus.remove_route(id);
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
    ///
    /// B2：调用后若插件主动产生了消息（outbox），由宿主代表插件逐条投递 PMB
    /// （宿主仍是唯一投递点，保持七道检查；插件本身不持有 PMB 投递能力）。
    pub fn call(&mut self, id: &str, method: &str, payload: &[u8]) -> PluginResult<Vec<u8>> {
        // 先在实例借用内完成方法调用并取走 outbox。
        let (bytes, outbox) = {
            let inst = self
                .instances
                .get_mut(id)
                .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
            let bytes = inst.call(method, payload)?;
            let outbox = inst.drain_outbox()?;
            (bytes, outbox)
        };
        // 再代表插件逐条投递。
        for m in outbox {
            match m.kind.as_str() {
                "send" => {
                    if let Some(target) = m.target.as_deref() {
                        self.send_to(id, target, &m.capability, m.payload)?;
                    }
                }
                "publish" => self.publish(id, &m.capability, m.payload)?,
                other => return Err(PluginError::Runtime(format!("outbox 未知 kind: {other}"))),
            }
        }
        Ok(bytes)
    }

    /// 测试专用：替换已注册实例（保留 registry/route/能力令牌），用于注入
    /// 带预设 outbox 的 stub 实例。
    #[cfg(test)]
    fn replace_instance(&mut self, id: &str, inst: Box<dyn PluginInstance>) {
        self.instances.insert(id.to_string(), inst);
    }

    /// 代表插件 `source` 构造、签名并投递一条消息（插件间通信的唯一入口）。
    ///
    /// 消息由宿主用 source 的会话密钥签名（见 [`PluginBus::sign_message`]），
    /// 再经 [`PluginBus::dispatch`] 完成全部七道检查（状态/令牌/能力/速率/
    /// 签名/nonce）。未注册或不在 RUNNING 的 source 会被拒绝。
    fn dispatch_for_plugin(
        &mut self,
        source: &str,
        target: Target,
        capability: &str,
        payload: serde_json::Value,
        kind: MessageKind,
    ) -> PluginResult<()> {
        // v3.5.2（AU-21 生产接线）：issued_at 取宿主时钟；生产 now_ms==0 时退到真实墙钟，
        // 避免自签消息被打成 1970 年。测试可经 set_now() 固定 issued_at。
        let issued_at = self.clock_now_ms() / 1000;
        let mut msg = PmbMessage {
            id: uuid::Uuid::new_v4().to_string(),
            corr_id: None,
            source: source.to_string(),
            target,
            capability: capability.to_string(),
            kind,
            topic: None,
            payload,
            issued_at,
            ttl_ms: 5000,
            priority: Priority::Normal,
            nonce: String::new(),
            signature: String::new(),
        };
        self.bus.sign_message(&mut msg)?;
        // v3.5.2（AU-21 生产接线）：这是插件消息离开宿主的唯一发送点（send_to/publish
        // 及 outbox 代投全部汇聚于此）。旧实现 set_server_clock_ms 仅被测试调用，生产总线
        // server_now_ms 恒为 CLOCK_NOT_SET，TTL 新鲜度检查形同关闭。此处每次发送前把**真实
        // 墙钟**写入总线，使 now∈[issued_at, issued_at+ttl_ms] 断言在生产真正生效；重放/陈旧
        // 消息据此被拒。测试用 set_now 固定旧 issued_at、总线仍取真墙钟即可复现过期。
        self.bus.set_server_clock_ms(real_wall_ms());
        self.bus.dispatch(&msg)
    }

    /// 代表插件 `source` 向单个目标插件发送一条已认证消息。
    pub fn send_to(
        &mut self,
        source: &str,
        target: &str,
        capability: &str,
        payload: serde_json::Value,
    ) -> PluginResult<()> {
        self.dispatch_for_plugin(
            source,
            Target::Plugin(target.to_string()),
            capability,
            payload,
            MessageKind::Request,
        )
    }

    /// 代表插件 `source` 广播一条已认证消息给所有 RUNNING 插件。
    pub fn publish(
        &mut self,
        source: &str,
        capability: &str,
        payload: serde_json::Value,
    ) -> PluginResult<()> {
        self.dispatch_for_plugin(
            source,
            Target::Broadcast,
            capability,
            payload,
            MessageKind::Event,
        )
    }

    /// 注册一个纯收件箱路由并置 RUNNING，返回其接收端。
    ///
    /// 用于把进程插件/外部网络传输桥接到总线：收件箱**没有能力令牌、不能发送**，
    /// 只能接收投递给它的消息。id 已存在时返回错误。
    pub fn open_inbox(&mut self, id: &str) -> PluginResult<std::sync::mpsc::Receiver<PmbMessage>> {
        if self.registry.contains(id) {
            return Err(PluginError::Runtime(format!("收件箱 {id} 已存在")));
        }
        let rx = self.bus.register(id);
        self.bus.set_state(id, PluginState::Running)?;
        Ok(rx)
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
    use crate::plugin::runtime::OutboxMessage;

    /// B2 测试：产生预设 outbox 的 stub 实例（不真正跑子进程）。
    struct StubOutboxInstance {
        outbox: Vec<OutboxMessage>,
    }

    impl PluginInstance for StubOutboxInstance {
        fn call(&mut self, _method: &str, _payload: &[u8]) -> PluginResult<Vec<u8>> {
            Ok(b"{}".to_vec())
        }
        fn stop(&mut self) -> PluginResult<()> {
            Ok(())
        }
        fn is_alive(&mut self) -> bool {
            true
        }
        fn drain_outbox(&mut self) -> PluginResult<Vec<OutboxMessage>> {
            Ok(std::mem::take(&mut self.outbox))
        }
    }

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
        let started = host.boot_system(&system::SystemHandles::default()).unwrap();
        assert_eq!(started.len(), 4);
        let routes = host.route_table();
        for id in system::system_ids() {
            assert_eq!(routes.get(id), Some(&PluginState::Running));
        }
    }

    #[test]
    fn system_identity_callable_via_host() {
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
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
        host.boot_system(&system::SystemHandles::default()).unwrap();
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
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(manifest).unwrap();
        host.uninstall(id).unwrap();
        // v3.5.2（AU-24）：卸载后总线路由应被整体移除（不再是残留的 Stopped 路由）。
        assert!(
            !host.route_table().contains_key(id),
            "卸载后 RouteEntry 应被移除，实际: {:?}",
            host.route_table().get(id)
        );
        assert!(!host.plugin_count_is_registered(id));
    }

    #[test]
    fn system_plugin_cannot_uninstall() {
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        assert!(host.uninstall(system::SYS_IDENTITY).is_err());
    }

    #[test]
    fn hot_reload_rolls_back_on_bad_new_version() {
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        let manifest = signed_official(id, &dev);
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
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
        host.boot_system(&system::SystemHandles::default()).unwrap();
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
        host.boot_system(&system::SystemHandles::default()).unwrap();
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
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        assert!(host.load_legacy(m).is_ok());
        assert_eq!(host.route_table().get(id), Some(&PluginState::Running));
    }

    #[test]
    fn send_to_delivers_authenticated_message() {
        let dev = Keypair::generate();
        let source = official::OFF_MARKET_MATCH;
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(signed_official(source, &dev)).unwrap();
        // 注册一个可消费的目标收件箱。
        let rx = host.open_inbox("bridge-peer-a").unwrap();
        host.send_to(
            source,
            "bridge-peer-a",
            "plugin:message:send",
            serde_json::json!({"hello": "world"}),
        )
        .unwrap();
        let got = rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .unwrap();
        assert_eq!(got.source, source);
        assert_eq!(got.payload["hello"], "world");
        assert!(!got.signature.is_empty());
        assert!(!got.nonce.is_empty());
    }

    #[test]
    fn publish_delivers_authenticated_broadcast() {
        let dev = Keypair::generate();
        let source = official::OFF_MARKET_MATCH;
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(signed_official(source, &dev)).unwrap();
        let rx1 = host.open_inbox("bridge-peer-b1").unwrap();
        let rx2 = host.open_inbox("bridge-peer-b2").unwrap();
        host.publish(
            source,
            "plugin:message:send",
            serde_json::json!({"event": "ping"}),
        )
        .unwrap();
        for rx in [rx1, rx2] {
            let got = rx
                .recv_timeout(std::time::Duration::from_millis(200))
                .unwrap();
            assert_eq!(got.payload["event"], "ping");
        }
    }

    // ── v3.5.2（AU-21 生产接线）：经真实 PluginHost 证明 TTL 新鲜度在生产发送路径被强制 ──
    // 不手动 set_server_clock——时钟由宿主在 dispatch_for_plugin 内用真实墙钟注入。
    #[test]
    fn au21_production_path_enforces_ttl_freshness() {
        let dev = Keypair::generate();
        let source = official::OFF_MARKET_MATCH;
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(signed_official(source, &dev)).unwrap();
        let rx = host.open_inbox("bridge-peer-au21").unwrap();

        // 新鲜消息：issued_at 取真墙钟，总线时钟也是真墙钟 → 在 ttl 窗口内，应被接受投递。
        host.send_to(
            source,
            "bridge-peer-au21",
            "plugin:message:send",
            serde_json::json!({"seq": 1}),
        )
        .expect("窗口内新鲜消息应被接受");
        let got = rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .unwrap();
        assert_eq!(got.payload["seq"], 1);

        // 过期消息：把宿主时钟钉到 epoch+1s（issued_at=1s=1000ms，ttl=5000ms→有效至6000ms），
        // 但总线仍按真实墙钟判定（早已远超 6000ms）→ 必须 PMB_MESSAGE_EXPIRED 拒绝。
        host.set_now(1_000);
        let err = host
            .send_to(
                source,
                "bridge-peer-au21",
                "plugin:message:send",
                serde_json::json!({"seq": 2}),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("PMB_MESSAGE_EXPIRED"),
            "陈旧 issued_at 的消息应被生产 TTL 闸门拒绝，实际: {err}"
        );
    }

    // ── B2（v3.5.0）：host.call 把插件 outbox 经 PMB 投递 ─────────────
    #[test]
    fn call_delivers_plugin_send_outbox_over_pmb() {
        let dev = Keypair::generate();
        let source = official::OFF_MARKET_MATCH;
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        // 真实 install 建立 registry/route/能力令牌（RUNNING）。
        host.install(signed_official(source, &dev)).unwrap();
        // 目标收件箱。
        let rx = host.open_inbox("bridge-peer-a").unwrap();
        // 替换为带预设 outbox 的 stub（模拟插件在 entry 内调 host.send_to）。
        host.replace_instance(
            source,
            Box::new(StubOutboxInstance {
                outbox: vec![OutboxMessage {
                    kind: "send".to_string(),
                    target: Some("bridge-peer-a".to_string()),
                    capability: "plugin:message:send".to_string(),
                    payload: serde_json::json!({"n": 9}),
                }],
            }),
        );
        let out = host.call(source, "notify", br#"{"n":9}"#).unwrap();
        assert_eq!(out, b"{}");
        // 目标收到由宿主代表插件投递的已认证消息。
        let got = rx
            .recv_timeout(std::time::Duration::from_millis(300))
            .unwrap();
        assert_eq!(got.source, source);
        match &got.target {
            Target::Plugin(t) => assert_eq!(t, "bridge-peer-a"),
            other => panic!("unexpected target {other:?}"),
        }
        assert_eq!(got.payload["n"], 9);
        assert!(!got.signature.is_empty());
        assert!(!got.nonce.is_empty());
    }

    #[test]
    fn call_delivers_plugin_publish_outbox_over_pmb() {
        let dev = Keypair::generate();
        let source = official::OFF_MARKET_MATCH;
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(signed_official(source, &dev)).unwrap();
        let rx1 = host.open_inbox("sub-a").unwrap();
        let rx2 = host.open_inbox("sub-b").unwrap();
        host.replace_instance(
            source,
            Box::new(StubOutboxInstance {
                outbox: vec![OutboxMessage {
                    kind: "publish".to_string(),
                    target: None,
                    capability: "plugin:message:send".to_string(),
                    payload: serde_json::json!({"done": true}),
                }],
            }),
        );
        host.call(source, "notify", b"{}").unwrap();
        for rx in [rx1, rx2] {
            let got = rx
                .recv_timeout(std::time::Duration::from_millis(300))
                .unwrap();
            assert_eq!(got.source, source);
            assert_eq!(got.payload["done"], true);
        }
    }

    #[test]
    fn send_to_rejects_unknown_source() {
        let mut host = PluginHost::new("3.0.0", None);
        // source 未注册 → 签名阶段即失败。
        let r = host.send_to("nobody", "x", "plugin:message:send", serde_json::json!({}));
        assert!(r.is_err());
    }

    #[test]
    fn inbox_without_token_cannot_send() {
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        // 收件箱没有能力令牌 → 代表它发送会被第 2/3 道令牌检查拒绝。
        let _rx = host.open_inbox("mute-inbox").unwrap();
        let r = host.send_to(
            "mute-inbox",
            "any",
            "plugin:message:send",
            serde_json::json!({}),
        );
        assert!(r.is_err());
    }

    // ── v3.5.2（AU-08）：外部 install 拒绝系统命名空间 ───────────────
    #[test]
    fn install_rejects_external_system_namespace() {
        let mut host = PluginHost::new("3.0.0", None);
        // 即使开发者自签 com.twinsearth.sys.* 名称，也必须在签名校验前被拒——
        // 该命名空间在 arbiter 走 System 分支不验签，外部自证即可提权。
        let mut m = official::official_manifest("com.twinsearth.sys.evil", "3.0.0");
        let dev = Keypair::generate();
        m.sign_with(&dev).unwrap();
        let err = host.install(m).unwrap_err();
        assert!(
            err.to_string().contains("PLUGIN_SYS_NAMESPACE_FORBIDDEN"),
            "外部自证系统名必须被拒，got {err}"
        );
    }

    // ── v3.5.2（AU-07）：黑名单可播种 + start() 复检 ────────────────
    #[test]
    fn blacklist_seeded_from_operator_file_blocks_install() {
        let dir = std::env::temp_dir().join(format!("au-bl-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let blf = dir.join("blacklist.txt");
        // 注释行 + 一个被拉黑 id。
        std::fs::write(&blf, "# seed file\ncom.twinsearth.official.evil\n").unwrap();
        // 环境变量仅在本测试内设入并立即清除，避免污染全局。
        std::env::set_var("GSN_BLACKLIST_FILE", &blf);
        let mut host = PluginHost::new("3.0.0", None);
        let r = host.seed_blacklist_from_env();
        std::env::remove_var("GSN_BLACKLIST_FILE");
        r.unwrap();
        assert!(host
            .blacklist()
            .is_blacklisted("com.twinsearth.official.evil", ""));
        // 播种后该 id 安装被拒。
        let mut m = official::official_manifest("com.twinsearth.official.evil", "3.0.0");
        let dev = Keypair::generate();
        m.sign_with(&dev).unwrap();
        m.counter_sign_with(&dev).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        assert!(host.install(m).is_err());
    }

    /// v3.5.2（AU-07 生产接线）：操作员显式设置 GSN_BLACKLIST_FILE 但文件不可读/损坏时，
    /// seed_blacklist_from_env 必须返回类型化错误——run_daemon 据此 fail-closed 拒绝启动，
    /// 而不是静默按空黑名单继续。
    #[test]
    fn blacklist_seed_read_failure_is_error_not_silent_empty() {
        // 指向一个目录（load_operator_file 读它会失败），模拟"显式配置却读不出来"。
        let dir = std::env::temp_dir().join(format!("au-bldir-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("GSN_BLACKLIST_FILE", &dir);
        let mut host = PluginHost::new("3.0.0", None);
        let r = host.seed_blacklist_from_env();
        std::env::remove_var("GSN_BLACKLIST_FILE");
        assert!(
            r.is_err(),
            "显式设置的黑名单文件读失败必须报错（run_daemon 据此 fail-closed），实际: {r:?}"
        );
        // 未设置变量时才是 no-op（现状）。
        let mut host2 = PluginHost::new("3.0.0", None);
        assert!(host2.seed_blacklist_from_env().is_ok());
    }

    #[test]
    fn start_rechecks_blacklist_after_stop() {
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        let manifest = signed_official(id, &dev);
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        host.install(manifest).unwrap();
        host.stop(id).unwrap();
        // 停止期间被操作员加入黑名单。
        host.blacklist_mut()
            .add(crate::plugin::blacklist::BlacklistEntry {
                plugin_name: id.to_string(),
                module_sha256: None,
                reason: crate::plugin::blacklist::BlacklistReason::RuntimeAbuse,
                blacklisted_at: 0,
                evidence: "stopped-then-blacklisted".to_string(),
                appeal: None,
            });
        // start() 必须重新过黑名单闸门，不得仅因「之前装过」就拉起。
        assert!(host.start(id).is_err());
    }

    // ── v3.5.2（AU-23）：spawn 失败不留孤儿注册 ────────────────────
    #[test]
    fn spawn_failure_leaves_no_orphan_registration() {
        let dev = Keypair::generate();
        let id = official::OFF_MARKET_MATCH;
        // 构造一个签名合法、但资源 cpu_ms=0 的清单 → ProcessSandbox::create 的
        // validate 拒绝 → spawn 失败。旧实现先 register 再 spawn，spawn 失败会留
        // 下与实例 desync 的注册项；新实现先 spawn 后 register。
        let mut m = official::official_manifest(id, "3.0.0");
        m.limits.cpu_ms = 0;
        m.sign_with(&dev).unwrap();
        m.counter_sign_with(&dev).unwrap();
        let mut host = PluginHost::new("3.0.0", None);
        host.boot_system(&system::SystemHandles::default()).unwrap();
        host.add_official_root(&hex::encode(dev.public_key()));
        let r = host.install(m);
        assert!(r.is_err(), "spawn 应因非法资源失败，got {r:?}");
        // 失败不得留下孤儿注册或总线路由。
        assert!(
            !host.plugin_count_is_registered(id),
            "spawn 失败不得留注册项"
        );
        assert!(
            !host.route_table().contains_key(id),
            "spawn 失败不得留总线路由"
        );
    }

    // 辅助断言。
    impl PluginHost {
        fn plugin_count_is_registered(&self, id: &str) -> bool {
            self.registry.contains(id)
        }
    }
}
