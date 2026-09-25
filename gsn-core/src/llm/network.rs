//! 国内外模型网络分组与连通性管理（v2.5.2）
//!
//! 把模型适配分成两个独立网络组：
//! - Overseas（国外）：OpenAI / Gemini / Anthropic / DeepSeek 海外端点
//! - Domestic（国内）：豆包 / Kimi / Qwen / 智谱 / MiniMax / 混元 / 小米
//!
//! 每个组独立运行、独立健康检查、独立故障转移。
//! 无论在国内网络、国外网络还是跳转网络，都能正确路由。

use std::collections::HashMap;
use super::openai::OpenAiModel;
use super::gemini::GeminiModel;
use super::anthropic::AnthropicModel;
use super::doubao::DoubaoModel;
use super::domestic::{DomesticModel, DomesticProvider};

/// 网络区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkRegion {
    /// 国内网络（直连，无需代理）
    Domestic,
    /// 国外网络（可能需要代理）
    Overseas,
}

impl NetworkRegion {
    pub fn label(&self) -> &'static str {
        match self {
            NetworkRegion::Domestic => "国内网络",
            NetworkRegion::Overseas => "海外网络",
        }
    }
}

/// 单个模型端点的网络配置。
#[derive(Debug, Clone)]
pub struct LlmEndpoint {
    pub family: &'static str,
    pub base_url: &'static str,
    pub region: NetworkRegion,
    /// 是否需要代理才能访问（国外端点在国内网络下通常需要）。
    pub requires_proxy_from_china: bool,
    /// 国内备用端点（CDN/镜像/代理地址），None 表示无备用。
    pub china_proxy_url: Option<&'static str>,
}

impl LlmEndpoint {
    pub fn reachable_from(&self, region: NetworkRegion) -> bool {
        match (self.region, region) {
            // 国外端点在国内：需要代理或备用端点
            (NetworkRegion::Overseas, NetworkRegion::Domestic) => self.china_proxy_url.is_some(),
            // 国内端点在国外：理论上可直连（国际带宽），但可能慢
            (NetworkRegion::Domestic, NetworkRegion::Overseas) => true,
            // 同区域直连
            _ => true,
        }
    }

    pub fn effective_url(&self, current_region: NetworkRegion) -> &str {
        match (self.region, current_region) {
            (NetworkRegion::Overseas, NetworkRegion::Domestic) => {
                self.china_proxy_url.unwrap_or(self.base_url)
            }
            _ => self.base_url,
        }
    }
}

/// 端点健康状态。
#[derive(Debug, Clone, PartialEq)]
pub enum EndpointHealth {
    /// 正常连通
    Reachable,
    /// 需要代理但未配置
    BlockedNoProxy,
    /// 已切换到备用端点
    Failover,
    /// 未知（未探测）
    Unknown,
}

/// 国内模型端点注册表。
pub struct DomesticRegistry {
    endpoints: HashMap<&'static str, LlmEndpoint>,
    health: HashMap<&'static str, EndpointHealth>,
}

/// 国外模型端点注册表。
pub struct OverseasRegistry {
    endpoints: HashMap<&'static str, LlmEndpoint>,
    health: HashMap<&'static str, EndpointHealth>,
}

impl DomesticRegistry {
    pub fn new() -> Self {
        let mut endpoints = HashMap::new();
        // 豆包
        endpoints.insert("doubao", LlmEndpoint {
            family: "doubao",
            base_url: "https://ark.cn-beijing.volces.com/api/v3",
            region: NetworkRegion::Domestic,
            requires_proxy_from_china: false,
            china_proxy_url: None,
        });
        // Kimi
        endpoints.insert("kimi", LlmEndpoint {
            family: "kimi",
            base_url: "https://api.moonshot.cn/v1",
            region: NetworkRegion::Domestic,
            requires_proxy_from_china: false,
            china_proxy_url: None,
        });
        // Qwen
        endpoints.insert("qwen", LlmEndpoint {
            family: "qwen",
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
            region: NetworkRegion::Domestic,
            requires_proxy_from_china: false,
            china_proxy_url: None,
        });
        // Zhipu
        endpoints.insert("zhipu", LlmEndpoint {
            family: "zhipu",
            base_url: "https://open.bigmodel.cn/api/paas/v4",
            region: NetworkRegion::Domestic,
            requires_proxy_from_china: false,
            china_proxy_url: None,
        });
        // MiniMax
        endpoints.insert("minimax", LlmEndpoint {
            family: "minimax",
            base_url: "https://api.minimax.chat/v1",
            region: NetworkRegion::Domestic,
            requires_proxy_from_china: false,
            china_proxy_url: None,
        });
        // Hunyuan
        endpoints.insert("hunyuan", LlmEndpoint {
            family: "hunyuan",
            base_url: "https://api.hunyuan.cloud.tencent.com/v1",
            region: NetworkRegion::Domestic,
            requires_proxy_from_china: false,
            china_proxy_url: None,
        });
        // Xiaomi
        endpoints.insert("xiaomi", LlmEndpoint {
            family: "xiaomi",
            base_url: "https://api.xiaomi.com/v1",
            region: NetworkRegion::Domestic,
            requires_proxy_from_china: false,
            china_proxy_url: None,
        });

        let mut health = HashMap::new();
        for k in endpoints.keys() {
            health.insert(*k, EndpointHealth::Unknown);
        }
        Self { endpoints, health }
    }

    pub fn endpoint(&self, family: &str) -> Option<&LlmEndpoint> {
        self.endpoints.get(family)
    }

    pub fn mark_reachable(&mut self, family: &'static str) {
        self.health.insert(family, EndpointHealth::Reachable);
    }

    pub fn healthy_count(&self) -> usize {
        self.health.values().filter(|h| **h == EndpointHealth::Reachable).count()
    }

    pub fn all_families(&self) -> Vec<&&'static str> {
        self.endpoints.keys().collect()
    }
}

impl OverseasRegistry {
    pub fn new() -> Self {
        let mut endpoints = HashMap::new();
        // OpenAI
        endpoints.insert("openai", LlmEndpoint {
            family: "openai",
            base_url: "https://api.openai.com/v1",
            region: NetworkRegion::Overseas,
            requires_proxy_from_china: true,
            china_proxy_url: Some("https://api.openai.com/v1-proxy"), // 国内镜像/代理占位
        });
        // Gemini
        endpoints.insert("gemini", LlmEndpoint {
            family: "gemini",
            base_url: "https://generativelanguage.googleapis.com/v1beta",
            region: NetworkRegion::Overseas,
            requires_proxy_from_china: true,
            china_proxy_url: Some("https://generativelanguage.googleapis.com/v1beta-proxy"),
        });
        // Anthropic
        endpoints.insert("anthropic", LlmEndpoint {
            family: "anthropic",
            base_url: "https://api.anthropic.com/v1",
            region: NetworkRegion::Overseas,
            requires_proxy_from_china: true,
            china_proxy_url: Some("https://api.anthropic.com/v1-proxy"),
        });
        // DeepSeek（国内可直连，归到海外组做对照）
        endpoints.insert("deepseek", LlmEndpoint {
            family: "deepseek",
            base_url: "https://api.deepseek.com/v1",
            region: NetworkRegion::Domestic, // DeepSeek 是国内模型
            requires_proxy_from_china: false,
            china_proxy_url: None,
        });

        let mut health = HashMap::new();
        for k in endpoints.keys() {
            health.insert(*k, EndpointHealth::Unknown);
        }
        Self { endpoints, health }
    }

    pub fn endpoint(&self, family: &str) -> Option<&LlmEndpoint> {
        self.endpoints.get(family)
    }

    pub fn mark_reachable(&mut self, family: &'static str) {
        self.health.insert(family, EndpointHealth::Reachable);
    }

    pub fn healthy_count(&self) -> usize {
        self.health.values().filter(|h| **h == EndpointHealth::Reachable).count()
    }

    /// 从国内网络访问时，哪些国外端点需要切换到代理 URL。
    pub fn needs_proxy_from_china(&self) -> Vec<&str> {
        self.endpoints
            .values()
            .filter(|e| e.region == NetworkRegion::Overseas && e.requires_proxy_from_china)
            .map(|e| e.family)
            .collect()
    }

    pub fn all_families(&self) -> Vec<&&'static str> {
        self.endpoints.keys().collect()
    }
}

/// 网络路由决策。
pub struct NetworkRouter {
    pub current_region: NetworkRegion,
    domestic: DomesticRegistry,
    overseas: OverseasRegistry,
}

impl NetworkRouter {
    pub fn new(current_region: NetworkRegion) -> Self {
        Self {
            current_region,
            domestic: DomesticRegistry::new(),
            overseas: OverseasRegistry::new(),
        }
    }

    /// 根据当前网络环境，返回某 family 应该用哪个 URL。
    pub fn resolve_url(&self, family: &str) -> Option<&str> {
        if let Some(ep) = self.domestic.endpoint(family) {
            return Some(ep.effective_url(self.current_region));
        }
        if let Some(ep) = self.overseas.endpoint(family) {
            return Some(ep.effective_url(self.current_region));
        }
        None
    }

    /// 某 family 在当前网络下是否可直连。
    pub fn is_reachable(&self, family: &str) -> bool {
        if let Some(ep) = self.domestic.endpoint(family) {
            return ep.reachable_from(self.current_region);
        }
        if let Some(ep) = self.overseas.endpoint(family) {
            return ep.reachable_from(self.current_region);
        }
        false
    }

    /// 健康检查：模拟探测所有端点。
    pub fn probe_all(&mut self) {
        let region = self.current_region;
        let domestic_keys: Vec<&'static str> = self.domestic.endpoints.keys().copied().collect();
        let domestic_reachable: Vec<bool> = domestic_keys
            .iter()
            .map(|k| self.domestic.endpoint(k).unwrap().reachable_from(region))
            .collect();
        for (k, reachable) in domestic_keys.into_iter().zip(domestic_reachable) {
            if reachable {
                self.domestic.mark_reachable(k);
            }
        }
        let overseas_keys: Vec<&'static str> = self.overseas.endpoints.keys().copied().collect();
        let overseas_reachable: Vec<bool> = overseas_keys
            .iter()
            .map(|k| self.overseas.endpoint(k).unwrap().reachable_from(region))
            .collect();
        for (k, reachable) in overseas_keys.into_iter().zip(overseas_reachable) {
            if reachable {
                self.overseas.mark_reachable(k);
            }
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "当前网络: {} | 国内端点: {}/{} 健康 | 国外端点: {}/{} 健康",
            self.current_region.label(),
            self.domestic.healthy_count(),
            self.domestic.endpoints.len(),
            self.overseas.healthy_count(),
            self.overseas.endpoints.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domestic_endpoints_reachable_from_china() {
        let router = NetworkRouter::new(NetworkRegion::Domestic);
        assert!(router.is_reachable("doubao"));
        assert!(router.is_reachable("kimi"));
        assert!(router.is_reachable("qwen"));
        assert!(router.is_reachable("zhipu"));
    }

    #[test]
    fn overseas_endpoints_have_proxy_from_china() {
        let router = NetworkRouter::new(NetworkRegion::Domestic);
        // openai/gemini/anthropic 在国内有代理 URL，通过代理可达
        assert!(router.is_reachable("openai")); // 有 china_proxy_url
        assert!(router.is_reachable("gemini"));
        assert!(router.is_reachable("anthropic"));
        // deepseek 国内直连
        assert!(router.is_reachable("deepseek"));
    }

    #[test]
    fn overseas_endpoints_direct_from_abroad() {
        let router = NetworkRouter::new(NetworkRegion::Overseas);
        assert!(router.is_reachable("openai"));
        assert!(router.is_reachable("gemini"));
        assert!(router.is_reachable("anthropic"));
        // 国内端点在国外也能访问
        assert!(router.is_reachable("doubao"));
        assert!(router.is_reachable("kimi"));
    }

    #[test]
    fn resolve_url_switches_to_proxy_in_china() {
        let router = NetworkRouter::new(NetworkRegion::Domestic);
        let openai_url = router.resolve_url("openai").unwrap();
        assert!(openai_url.contains("proxy")); // 切换到代理 URL
    }

    #[test]
    fn resolve_url_direct_abroad() {
        let router = NetworkRouter::new(NetworkRegion::Overseas);
        let openai_url = router.resolve_url("openai").unwrap();
        assert_eq!(openai_url, "https://api.openai.com/v1");
    }

    #[test]
    fn probe_all_marks_reachable() {
        let mut router = NetworkRouter::new(NetworkRegion::Domestic);
        router.probe_all();
        // 国内网络下，国内端点全健康，国外端点通过代理 URL 也可达
        assert_eq!(router.domestic.healthy_count(), 7); // 7 个国内模型
        assert_eq!(router.overseas.healthy_count(), 4); // openai/gemini/anthropic 有代理 + deepseek 直连
    }

    #[test]
    fn summary_includes_region() {
        let router = NetworkRouter::new(NetworkRegion::Domestic);
        let s = router.summary();
        assert!(s.contains("国内网络"));
    }
}

// ===== v2.5.2: 自动降级协商 =====

use crate::collaboration::hetero_llm::{Deliberation, LlmModel, Proposal};

/// 降级事件记录。
#[derive(Debug, Clone)]
pub struct DowngradeEvent {
    pub from_region: NetworkRegion,
    pub to_region: NetworkRegion,
    pub foreign_models_dropped: usize,
    pub domestic_models_active: usize,
}

/// 分区感知协商：根据网络状态自动选择模型子集。
///
/// - Global 模式：国内外模型同时协商
/// - DomesticOnly 模式：外网不可达时自动降级，仅国内模型投票
pub struct RegionAwareDeliberation {
    router: NetworkRouter,
    domestic_models: Vec<LlmModel>,
    foreign_models: Vec<LlmModel>,
    downgrade_events: Vec<DowngradeEvent>,
    /// 是否处于降级模式（外网不可达）。
    degraded: bool,
}

impl RegionAwareDeliberation {
    pub fn new(region: NetworkRegion) -> Self {
        Self {
            router: NetworkRouter::new(region),
            domestic_models: vec![],
            foreign_models: vec![],
            downgrade_events: vec![],
            degraded: false,
        }
    }

    pub fn router(&self) -> &NetworkRouter {
        &self.router
    }

    pub fn register_domestic(&mut self, model: LlmModel) {
        self.domestic_models.push(model);
    }

    pub fn register_foreign(&mut self, model: LlmModel) {
        self.foreign_models.push(model);
    }

    /// 外网不可达时调用：自动降级为仅国内模型投票。
    pub fn downgrade_to_domestic(&mut self) {
        if self.degraded {
            return;
        }
        let foreign_count = self.foreign_models.len();
        let domestic_count = self.domestic_models.len();
        self.degraded = true;
        self.downgrade_events.push(DowngradeEvent {
            from_region: self.router.current_region,
            to_region: NetworkRegion::Domestic,
            foreign_models_dropped: foreign_count,
            domestic_models_active: domestic_count,
        });
    }

    /// 外网恢复时调用。
    pub fn restore(&mut self) {
        self.degraded = false;
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn downgrade_events(&self) -> &[DowngradeEvent] {
        &self.downgrade_events
    }

    /// 当前活跃模型数。
    pub fn active_model_count(&self) -> usize {
        if self.degraded {
            self.domestic_models.len()
        } else {
            self.domestic_models.len() + self.foreign_models.len()
        }
    }

    /// 构建当前网络状态下的 Deliberation。
    pub fn build_deliberation(&self) -> Deliberation {
        let mut d = Deliberation::new();
        for m in &self.domestic_models {
            d.add_model(m.clone());
        }
        if !self.degraded {
            for m in &self.foreign_models {
                d.add_model(m.clone());
            }
        }
        d
    }
}

#[cfg(test)]
mod downgrade_tests {
    use super::*;

    fn m(id: &str, family: &str, weight: f64) -> LlmModel {
        LlmModel { id: id.into(), family: family.into(), weight }
    }

    #[test]
    fn normal_mode_includes_both() {
        let mut r = RegionAwareDeliberation::new(NetworkRegion::Overseas);
        r.register_domestic(m("qwen-1", "qwen", 0.8));
        r.register_foreign(m("gpt-1", "openai", 0.9));
        assert_eq!(r.active_model_count(), 2);
        assert!(!r.is_degraded());
    }

    #[test]
    fn downgrade_drops_foreign() {
        let mut r = RegionAwareDeliberation::new(NetworkRegion::Overseas);
        r.register_domestic(m("qwen-1", "qwen", 0.8));
        r.register_domestic(m("kimi-1", "kimi", 0.82));
        r.register_foreign(m("gpt-1", "openai", 0.9));
        r.register_foreign(m("claude-1", "anthropic", 0.85));

        r.downgrade_to_domestic();
        assert!(r.is_degraded());
        assert_eq!(r.active_model_count(), 2);
        assert_eq!(r.downgrade_events().len(), 1);
        assert_eq!(r.downgrade_events()[0].foreign_models_dropped, 2);
    }

    #[test]
    fn restore_brings_back_foreign() {
        let mut r = RegionAwareDeliberation::new(NetworkRegion::Overseas);
        r.register_domestic(m("qwen-1", "qwen", 0.8));
        r.register_foreign(m("gpt-1", "openai", 0.9));
        r.downgrade_to_domestic();
        assert_eq!(r.active_model_count(), 1);
        r.restore();
        assert!(!r.is_degraded());
        assert_eq!(r.active_model_count(), 2);
    }

    #[test]
    fn deliberate_in_degraded_mode() {
        let mut r = RegionAwareDeliberation::new(NetworkRegion::Domestic);
        r.register_domestic(m("qwen-1", "qwen", 0.8));
        r.register_domestic(m("kimi-1", "kimi", 0.82));
        r.register_domestic(m("glm-1", "zhipu", 0.78));
        r.register_foreign(m("gpt-1", "openai", 0.9));

        r.downgrade_to_domestic();
        let mut d = r.build_deliberation();
        d.propose(Proposal { model_id: "qwen-1".into(), answer: "A".into(), confidence: 0.9 }).unwrap();
        d.propose(Proposal { model_id: "kimi-1".into(), answer: "A".into(), confidence: 0.85 }).unwrap();
        d.propose(Proposal { model_id: "glm-1".into(), answer: "B".into(), confidence: 0.7 }).unwrap();

        let (winner, _) = d.vote().unwrap();
        assert_eq!(winner, "A");
        assert_eq!(d.proposal_count(), 3);
    }
}
