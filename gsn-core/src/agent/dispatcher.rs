//! 调配 Agent（Dispatcher，v5.0.0 骨架）
//!
//! 按角色 / 能力 / 记忆调度，持有 [`TaskRouter`] 与 [`LoadBalancer`]。
//! v5.0.0 提供角色定义、节点画像、**角色路由**（FLEET-ROLES 路由表：按角色
//! 而非便利）与消息入口；v5.0.6 提供**节点画像**：从记忆条目聚合节点历史
//! （耗时 / 成功率 / 观察能力），更新 [`NodeProfile`]；记忆驱动调度在 v5.0.7。

use std::collections::HashMap;

use crate::memory::MemoryEntry;
use crate::scheduler::{BalanceStrategy, LoadBalancer, TaskRouter};

/// 舰队机器角色（对齐 FLEET-ROLES）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetRole {
    /// Linux 中继主机（在线/中继/转发/归档）。
    Relay,
    /// 真机主力（MacMini / 其他 Windows）。
    Workhorse,
    /// 受保护主机（如 ToFuture Windows）。
    Protected,
    /// 未指定角色（调度时不按角色过滤）。
    Unknown,
}

/// 节点画像。
///
/// `role / capabilities / latency_ms / capacity` 为**静态配置**（注册时权威）；
/// `success_rate / avg_ms / p95_ms / history_n / observed_capabilities` 为
/// **记忆聚合**（v5.0.6，经 [`NodeProfile::aggregate_from_entries`] /
/// [`DispatcherAgent::refresh_from_memory`] 更新），反映节点历史表现。
#[derive(Debug, Clone, PartialEq)]
pub struct NodeProfile {
    pub node_id: String,
    pub role: FleetRole,
    pub capabilities: Vec<String>,
    pub latency_ms: u64,
    pub capacity: u64,
    /// 历史成功率（0.0–1.0）；仅统计有 `exit_code` 的条目，无样本时为 `None`。
    pub success_rate: Option<f64>,
    /// 历史平均耗时（毫秒）；仅统计有 `ms` 的条目，无样本时为 `None`。
    pub avg_ms: Option<u64>,
    /// 历史 P95 耗时（毫秒）；仅统计有 `ms` 的条目，无样本时为 `None`。
    pub p95_ms: Option<u64>,
    /// 参与聚合的记忆条目数（node_id 匹配条数，含无 exit_code / ms 的条目）。
    pub history_n: u64,
    /// 从记忆 `gate` 观察到的能力（去重、字典序），不替代静态 capabilities。
    pub observed_capabilities: Vec<String>,
}

impl NodeProfile {
    pub fn new(node_id: impl Into<String>, role: FleetRole) -> Self {
        Self {
            node_id: node_id.into(),
            role,
            capabilities: Vec::new(),
            latency_ms: 100,
            capacity: 16,
            success_rate: None,
            avg_ms: None,
            p95_ms: None,
            history_n: 0,
            observed_capabilities: Vec::new(),
        }
    }

    pub fn capability(mut self, c: impl Into<String>) -> Self {
        self.capabilities.push(c.into());
        self
    }
    pub fn latency_ms(mut self, ms: u64) -> Self {
        self.latency_ms = ms;
        self
    }
    pub fn capacity(mut self, c: u64) -> Self {
        self.capacity = c;
        self
    }

    /// v5.0.6：从记忆条目聚合本节点历史画像。
    ///
    /// 只统计 `node_id` 与本节点一致的条目；静态字段（role / capabilities /
    /// latency_ms / capacity）保持不变。成功率只把**有 exit_code 的条目**计入
    /// 分母（`0` 计成功）；耗时统计只把**有 ms 的条目**计入；`gate` 去重、
    /// 字典序后进入 `observed_capabilities`。无匹配样本时统计字段保持 `None`/0。
    pub fn aggregate_from_entries(mut self, entries: &[MemoryEntry]) -> Self {
        let mut matched = 0u64;
        let mut ok = 0u64;
        let mut coded = 0u64;
        let mut ms_samples: Vec<u64> = Vec::new();
        let mut gates: Vec<String> = Vec::new();
        for e in entries {
            let is_self = e.node_id.as_deref() == Some(self.node_id.as_str());
            if !is_self {
                continue;
            }
            matched += 1;
            if let Some(code) = e.exit_code {
                coded += 1;
                if code == 0 {
                    ok += 1;
                }
            }
            if let Some(m) = e.ms {
                ms_samples.push(m);
            }
            if let Some(g) = e.gate.as_deref() {
                if !gates.iter().any(|x| x == g) {
                    gates.push(g.to_string());
                }
            }
        }
        self.history_n = matched;
        if coded > 0 {
            self.success_rate = Some(ok as f64 / coded as f64);
        }
        if !ms_samples.is_empty() {
            let sum: u64 = ms_samples.iter().sum();
            self.avg_ms = Some(sum / ms_samples.len() as u64);
            ms_samples.sort_unstable();
            let idx = ((ms_samples.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
            self.p95_ms = Some(ms_samples[idx]);
        }
        gates.sort();
        self.observed_capabilities = gates;
        self
    }
}

/// 调度请求。
pub struct DispatchRequest {
    pub task_id: String,
    pub required_role: FleetRole,
    pub required_capability: Option<String>,
    pub candidates: Vec<String>,
}

impl DispatchRequest {
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            required_role: FleetRole::Unknown,
            required_capability: None,
            candidates: Vec::new(),
        }
    }
    pub fn required_role(mut self, r: FleetRole) -> Self {
        self.required_role = r;
        self
    }
    pub fn required_capability(mut self, c: impl Into<String>) -> Self {
        self.required_capability = Some(c.into());
        self
    }
    pub fn candidates(mut self, c: Vec<String>) -> Self {
        self.candidates = c;
        self
    }
}

/// 调度决策。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchDecision {
    pub task_id: String,
    pub target: Option<String>,
    pub reason: String,
}

/// v5.0.7：记忆驱动调度的排序键。
///
/// 升序比较、键最小者胜出：`Memory` 恒优于 `Static`（有记忆画像的节点优先
/// 于只有静态信息的节点）；`Memory` 内先比成功率档（`10 - tier`，档越高键越
/// 小），再比 avg_ms / p95_ms；`Static` 保持既有 `(load, latency)` 语义。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RankKey {
    Memory(u8, u64, u64),
    Static(u64, u64),
}

/// 记忆驱动调度所需的最小历史样本数；样本不足回退静态 `(load, latency)`。
const MIN_MEMORY_SAMPLES: u64 = 3;

/// 调配 Agent。
pub struct DispatcherAgent {
    id: String,
    router: TaskRouter,
    balancer: LoadBalancer,
    profiles: HashMap<String, NodeProfile>,
    memory_aware: bool,
}

impl DispatcherAgent {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            router: TaskRouter::new(64),
            balancer: LoadBalancer::new(BalanceStrategy::LeastConnections),
            profiles: HashMap::new(),
            memory_aware: false,
        }
    }

    /// v5.0.7：开启记忆驱动调度（默认关闭，保持既有负载/延迟语义）。
    pub fn memory_aware(mut self, on: bool) -> Self {
        self.memory_aware = on;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn profile(&self, node_id: &str) -> Option<&NodeProfile> {
        self.profiles.get(node_id)
    }

    pub fn node_count(&self) -> usize {
        self.profiles.len()
    }

    /// 注册节点画像：登记本地 profiles，并委托 router / balancer。
    pub fn register_node(&mut self, p: NodeProfile) {
        self.router.register_node(p.node_id.clone(), p.latency_ms);
        self.balancer.add_node(p.node_id.clone(), p.capacity);
        self.profiles.insert(p.node_id.clone(), p);
    }

    /// v5.0.6：从记忆批量刷新已注册节点的历史画像。
    ///
    /// 仅更新已注册节点（注册由 [`register_node`] 显式管理，记忆中的陌生
    /// node_id 不自动建节点）；静态字段与 router/balancer 不受影响。
    pub fn refresh_from_memory(&mut self, entries: &[MemoryEntry]) {
        for p in self.profiles.values_mut() {
            let node_id = p.node_id.clone();
            let refreshed = NodeProfile {
                node_id,
                ..p.clone()
            }
            .aggregate_from_entries(entries);
            *p = refreshed;
        }
    }

    /// 角色路由：返回候选中符合角色（及能力）要求的节点。
    fn eligible(&self, req: &DispatchRequest) -> Vec<String> {
        let mut out = Vec::new();
        for c in &req.candidates {
            if let Some(p) = self.profiles.get(c) {
                if req.required_role != FleetRole::Unknown && p.role != req.required_role {
                    continue;
                }
                if let Some(need) = &req.required_capability {
                    if !p.capabilities.contains(need) {
                        continue;
                    }
                }
                out.push(c.clone());
            }
        }
        out
    }

    /// 调度：先按角色/能力过滤（按角色不按便利），再在合格节点中按负载、延迟选优；
    /// 开启记忆驱动（v5.0.7）后，历史样本充足的节点按成功率/耗时优先。
    pub fn dispatch(&mut self, req: DispatchRequest) -> DispatchDecision {
        let eligible = self.eligible(&req);
        if eligible.is_empty() {
            return DispatchDecision {
                task_id: req.task_id,
                target: None,
                reason: "没有符合角色/能力要求的候选节点".to_string(),
            };
        }
        let memory = self.memory_aware && eligible.iter().any(|c| self.memory_score(c).is_some());
        let mut best: Option<(String, RankKey)> = None;
        for c in &eligible {
            let key = if memory {
                match self.memory_score(c) {
                    Some(k) => RankKey::Memory(k.0, k.1, k.2),
                    None => self.static_key(c),
                }
            } else {
                self.static_key(c)
            };
            let take = match &best {
                None => true,
                Some((_, bk)) => key < *bk,
            };
            if take {
                best = Some((c.clone(), key));
            }
        }
        match best {
            Some((target, key)) => {
                let mode = if matches!(key, RankKey::Memory(..)) {
                    "记忆驱动（历史成功率/耗时）"
                } else {
                    "负载/延迟"
                };
                DispatchDecision {
                    task_id: req.task_id,
                    target: Some(target),
                    reason: format!("在 {} 个合格节点中按{mode}选择", eligible.len()),
                }
            }
            // eligible 非空时 best 必然为 Some；此分支仅为类型化兜底，避免 panic。
            None => DispatchDecision {
                task_id: req.task_id,
                target: None,
                reason: "调度内部错误：合格集非空但未选出节点".to_string(),
            },
        }
    }

    /// 静态排序键：`(load, latency_ms)`。
    fn static_key(&self, c: &str) -> RankKey {
        let load = self.router.node_load(c);
        let lat = self
            .profiles
            .get(c)
            .map(|p| p.latency_ms)
            .unwrap_or(u64::MAX);
        RankKey::Static(load, lat)
    }

    /// v5.0.7：记忆排序键 `(10 - 成功率档, avg_ms, p95_ms)`。
    ///
    /// 样本数 ≥ `MIN_MEMORY_SAMPLES` 且成功率为 `Some` 时才参与记忆排序；
    /// 否则返回 `None`（回退静态键）。成功率高者键小（优），同档比耗时。
    fn memory_score(&self, c: &str) -> Option<(u8, u64, u64)> {
        let p = self.profiles.get(c)?;
        if p.history_n < MIN_MEMORY_SAMPLES {
            return None;
        }
        let rate = p.success_rate?;
        let tier = ((rate * 10.0).floor() as u8).min(10);
        let avg = p.avg_ms.unwrap_or(u64::MAX);
        let p95 = p.p95_ms.unwrap_or(u64::MAX);
        Some((10 - tier, avg, p95))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profiles() -> Vec<NodeProfile> {
        vec![
            NodeProfile::new("vps", FleetRole::Relay).latency_ms(5),
            NodeProfile::new("mac", FleetRole::Workhorse)
                .capability("build")
                .latency_ms(20),
            NodeProfile::new("win", FleetRole::Workhorse)
                .capability("build")
                .latency_ms(40),
        ]
    }

    #[test]
    fn registers_profiles() {
        let mut d = DispatcherAgent::new("dispatcher");
        for p in profiles() {
            d.register_node(p);
        }
        assert_eq!(d.node_count(), 3);
        assert_eq!(d.profile("vps").unwrap().role, FleetRole::Relay);
    }

    #[test]
    fn routes_by_role() {
        let mut d = DispatcherAgent::new("dispatcher");
        for p in profiles() {
            d.register_node(p);
        }
        // 真机执行类任务只派 Workhorse，不会派给 Relay（即便 Relay 延迟最低）。
        let decision = d.dispatch(
            DispatchRequest::new("task-1")
                .required_role(FleetRole::Workhorse)
                .required_capability("build")
                .candidates(vec!["vps".into(), "mac".into(), "win".into()]),
        );
        assert_eq!(decision.target, Some("mac".to_string()));
    }

    #[test]
    fn capability_filter_excludes_unqualified() {
        let mut d = DispatcherAgent::new("dispatcher");
        for p in profiles() {
            d.register_node(p);
        }
        // Relay 没有 build 能力；仅要求 build 时只剩 mac/win。
        let decision = d.dispatch(
            DispatchRequest::new("task-2")
                .required_capability("build")
                .candidates(vec!["vps".into(), "mac".into(), "win".into()]),
        );
        assert_eq!(decision.target, Some("mac".to_string()));
    }

    #[test]
    fn no_eligible_candidate() {
        let mut d = DispatcherAgent::new("dispatcher");
        d.register_node(NodeProfile::new("vps", FleetRole::Relay));
        let decision = d.dispatch(
            DispatchRequest::new("task-3")
                .required_role(FleetRole::Workhorse)
                .candidates(vec!["vps".into()]),
        );
        assert_eq!(decision.target, None);
        assert!(decision.reason.contains("没有符合"));
    }

    #[test]
    fn unknown_role_does_not_filter() {
        let mut d = DispatcherAgent::new("dispatcher");
        for p in profiles() {
            d.register_node(p);
        }
        let decision =
            d.dispatch(DispatchRequest::new("task-4").candidates(vec!["vps".into(), "mac".into()]));
        // 不按角色过滤，Relay 负载同为 0、延迟最低，被选中。
        assert_eq!(decision.target, Some("vps".to_string()));
    }

    // ── v5.0.6 节点画像聚合 ──────────────────────────────────────

    fn mem(node: &str, exit_code: Option<i32>, ms: Option<u64>, gate: Option<&str>) -> MemoryEntry {
        let mut e = MemoryEntry::new("t").node_id(node);
        if let Some(c) = exit_code {
            e = e.exit_code(c);
        }
        if let Some(m) = ms {
            e = e.ms(m);
        }
        if let Some(g) = gate {
            e = e.gate(g);
        }
        e
    }

    #[test]
    fn aggregate_computes_success_rate_avg_p95() {
        let entries = vec![
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(30), Some("build")),
            mem("mac", Some(1), Some(50), Some("test")),
            // 无 exit_code/ms 的条目计入样本数，但不进成功率/耗时统计。
            mem("mac", None, None, None),
            // 其他节点不参与本节点聚合。
            mem("win", Some(1), Some(999), Some("build")),
        ];
        let p = NodeProfile::new("mac", FleetRole::Workhorse).aggregate_from_entries(&entries);
        assert_eq!(p.history_n, 4);
        // 成功率：3 条有 exit_code，2 条成功。
        assert_eq!(p.success_rate, Some(2.0 / 3.0));
        // 耗时：10/30/50 → avg 30，p95(0.95*3=2.85→ceil 3→idx 2) = 50。
        assert_eq!(p.avg_ms, Some(30));
        assert_eq!(p.p95_ms, Some(50));
        // 观察能力去重、字典序。
        assert_eq!(p.observed_capabilities, vec!["build", "test"]);
    }

    #[test]
    fn aggregate_no_matching_entries_keeps_defaults() {
        let entries = vec![mem("win", Some(0), Some(5), Some("build"))];
        let p = NodeProfile::new("mac", FleetRole::Workhorse).aggregate_from_entries(&entries);
        assert_eq!(p.history_n, 0);
        assert_eq!(p.success_rate, None);
        assert_eq!(p.avg_ms, None);
        assert_eq!(p.p95_ms, None);
        assert!(p.observed_capabilities.is_empty());
        // 静态字段不受影响。
        assert_eq!(p.role, FleetRole::Workhorse);
        assert_eq!(p.capacity, 16);
        assert_eq!(p.latency_ms, 100);
    }

    #[test]
    fn aggregate_p95_single_sample_and_no_exit_code() {
        let one = vec![mem("mac", Some(0), Some(7), None)];
        let p = NodeProfile::new("mac", FleetRole::Workhorse).aggregate_from_entries(&one);
        assert_eq!(p.p95_ms, Some(7));
        assert_eq!(p.avg_ms, Some(7));
        assert_eq!(p.success_rate, Some(1.0));

        // 只有无 exit_code 的条目：成功率保持 None，但 history_n 计 1。
        let no_code = vec![mem("mac", None, Some(7), None)];
        let p = NodeProfile::new("mac", FleetRole::Workhorse).aggregate_from_entries(&no_code);
        assert_eq!(p.history_n, 1);
        assert_eq!(p.success_rate, None);
        assert_eq!(p.avg_ms, Some(7));
    }

    #[test]
    fn refresh_from_memory_updates_profiles_keeps_static_fields() {
        let mut d = DispatcherAgent::new("dispatcher");
        d.register_node(NodeProfile::new("mac", FleetRole::Workhorse).capability("build"));
        d.register_node(NodeProfile::new("win", FleetRole::Workhorse).capability("build"));
        let entries = vec![
            mem("mac", Some(0), Some(20), Some("build")),
            mem("mac", Some(1), Some(40), Some("test")),
            mem("win", Some(0), Some(5), Some("build")),
        ];
        d.refresh_from_memory(&entries);

        let mac = d.profile("mac").unwrap();
        assert_eq!(mac.history_n, 2);
        assert_eq!(mac.success_rate, Some(0.5));
        assert_eq!(mac.avg_ms, Some(30));
        assert_eq!(mac.p95_ms, Some(40));
        // 静态配置保留。
        assert_eq!(mac.role, FleetRole::Workhorse);
        assert_eq!(mac.capabilities, vec!["build"]);
        assert_eq!(mac.latency_ms, 100);

        let win = d.profile("win").unwrap();
        assert_eq!(win.history_n, 1);
        assert_eq!(win.success_rate, Some(1.0));
    }

    #[test]
    fn refresh_from_memory_ignores_unregistered_nodes() {
        let mut d = DispatcherAgent::new("dispatcher");
        d.register_node(NodeProfile::new("mac", FleetRole::Workhorse));
        let entries = vec![
            mem("mac", Some(0), Some(1), None),
            // 陌生 node_id：不应自动建节点。
            mem("stranger", Some(1), Some(9), None),
        ];
        d.refresh_from_memory(&entries);
        assert_eq!(d.node_count(), 1);
        assert!(d.profile("stranger").is_none());
    }

    // ── v5.0.7 记忆驱动调度 ──────────────────────────────────────

    fn workhorse(node: &str, latency: u64) -> NodeProfile {
        NodeProfile::new(node, FleetRole::Workhorse)
            .capability("build")
            .latency_ms(latency)
    }

    #[test]
    fn memory_scheduling_prefers_high_success_rate() {
        // mac 历史 9 成功 1 失败（90%），win 4 成功 6 失败（40%）；延迟相同。
        let mut d = DispatcherAgent::new("dispatcher").memory_aware(true);
        d.register_node(workhorse("mac", 20));
        d.register_node(workhorse("win", 20));
        let entries = vec![
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(0), Some(10), Some("build")),
            mem("mac", Some(1), Some(10), Some("test")),
            mem("win", Some(0), Some(10), Some("build")),
            mem("win", Some(0), Some(10), Some("build")),
            mem("win", Some(0), Some(10), Some("build")),
            mem("win", Some(0), Some(10), Some("build")),
            mem("win", Some(1), Some(10), Some("test")),
            mem("win", Some(1), Some(10), Some("test")),
            mem("win", Some(1), Some(10), Some("test")),
            mem("win", Some(1), Some(10), Some("test")),
            mem("win", Some(1), Some(10), Some("test")),
            mem("win", Some(1), Some(10), Some("test")),
        ];
        d.refresh_from_memory(&entries);
        let decision = d.dispatch(
            DispatchRequest::new("t")
                .required_capability("build")
                .candidates(vec!["mac".into(), "win".into()]),
        );
        assert_eq!(decision.target, Some("mac".to_string()));
        assert!(decision.reason.contains("记忆驱动"));
    }

    #[test]
    fn memory_scheduling_binds_ties_by_avg_ms() {
        // 同为 100% 成功率档：avg 10ms 的节点优先于 avg 60ms。
        let mut d = DispatcherAgent::new("dispatcher").memory_aware(true);
        d.register_node(workhorse("mac", 20));
        d.register_node(workhorse("win", 20));
        d.refresh_from_memory(&[
            mem("mac", Some(0), Some(10), None),
            mem("mac", Some(0), Some(10), None),
            mem("mac", Some(0), Some(10), None),
            mem("win", Some(0), Some(60), None),
            mem("win", Some(0), Some(60), None),
            mem("win", Some(0), Some(60), None),
        ]);
        let decision =
            d.dispatch(DispatchRequest::new("t").candidates(vec!["mac".into(), "win".into()]));
        assert_eq!(decision.target, Some("mac".to_string()));
    }

    #[test]
    fn memory_aware_off_keeps_static_behavior() {
        // 关闭记忆驱动：即使 win 历史成功率低，仍按静态负载/延迟选择。
        let mut d = DispatcherAgent::new("dispatcher"); // 默认关闭
        d.register_node(workhorse("mac", 50));
        d.register_node(workhorse("win", 10));
        d.refresh_from_memory(&[
            mem("win", Some(0), Some(5), None),
            mem("win", Some(0), Some(5), None),
            mem("win", Some(0), Some(5), None),
        ]);
        let decision =
            d.dispatch(DispatchRequest::new("t").candidates(vec!["mac".into(), "win".into()]));
        // 静态：win 延迟 10 < mac 50，仍选 win；reason 无“记忆驱动”。
        assert_eq!(decision.target, Some("win".to_string()));
        assert!(!decision.reason.contains("记忆驱动"));
    }

    #[test]
    fn insufficient_samples_falls_back_to_static() {
        // mac 仅 2 条样本（< MIN_MEMORY_SAMPLES=3）：回退静态键，仍按延迟选 mac。
        let mut d = DispatcherAgent::new("dispatcher").memory_aware(true);
        d.register_node(workhorse("mac", 5));
        d.register_node(workhorse("win", 50));
        d.refresh_from_memory(&[
            mem("mac", Some(0), Some(5), None),
            mem("mac", Some(1), Some(5), None),
        ]);
        let decision =
            d.dispatch(DispatchRequest::new("t").candidates(vec!["mac".into(), "win".into()]));
        assert_eq!(decision.target, Some("mac".to_string()));
        assert!(!decision.reason.contains("记忆驱动"));
    }

    #[test]
    fn refresh_then_dispatch_uses_updated_memory() {
        // 先注册（无记忆），调度走静态；注入失败记忆后开启记忆驱动，改为选成功率更高的节点。
        let mut d = DispatcherAgent::new("dispatcher").memory_aware(true);
        d.register_node(workhorse("mac", 5));
        d.register_node(workhorse("win", 10));
        d.refresh_from_memory(&[
            mem("mac", Some(1), Some(5), None),
            mem("mac", Some(1), Some(5), None),
            mem("mac", Some(1), Some(5), None),
            mem("win", Some(0), Some(10), None),
            mem("win", Some(0), Some(10), None),
            mem("win", Some(0), Some(10), None),
        ]);
        let decision =
            d.dispatch(DispatchRequest::new("t").candidates(vec!["mac".into(), "win".into()]));
        // mac 成功率 0%（3 失败）→ 键劣于 win（100%），即使 mac 延迟更低。
        assert_eq!(decision.target, Some("win".to_string()));
        assert!(decision.reason.contains("记忆驱动"));
    }

    #[test]
    fn rank_key_memory_precedes_static() {
        // RankKey 有序性：Memory 恒优于 Static。
        assert!(RankKey::Memory(0, 0, 0) < RankKey::Static(0, 0));
        // Memory 内档高者（10 - tier 小）优。
        assert!(RankKey::Memory(1, 0, 0) < RankKey::Memory(5, 0, 0));
        // 同档比 avg。
        assert!(RankKey::Memory(0, 10, 50) < RankKey::Memory(0, 60, 50));
        // Static 保持 (load, latency)。
        assert!(RankKey::Static(0, 40) < RankKey::Static(1, 5));
    }
}
