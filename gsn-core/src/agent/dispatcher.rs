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

/// 调配 Agent。
pub struct DispatcherAgent {
    id: String,
    router: TaskRouter,
    balancer: LoadBalancer,
    profiles: HashMap<String, NodeProfile>,
}

impl DispatcherAgent {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            router: TaskRouter::new(64),
            balancer: LoadBalancer::new(BalanceStrategy::LeastConnections),
            profiles: HashMap::new(),
        }
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

    /// 调度：先按角色/能力过滤（按角色不按便利），再在合格节点中按负载、延迟选优。
    pub fn dispatch(&mut self, req: DispatchRequest) -> DispatchDecision {
        let eligible = self.eligible(&req);
        if eligible.is_empty() {
            return DispatchDecision {
                task_id: req.task_id,
                target: None,
                reason: "没有符合角色/能力要求的候选节点".to_string(),
            };
        }
        let mut best: Option<(String, u64, u64)> = None;
        for c in &eligible {
            let load = self.router.node_load(c);
            let lat = self
                .profiles
                .get(c)
                .map(|p| p.latency_ms)
                .unwrap_or(u64::MAX);
            let take = match &best {
                None => true,
                Some((_, bl, btm)) => (load, lat) < (*bl, *btm),
            };
            if take {
                best = Some((c.clone(), load, lat));
            }
        }
        match best {
            Some((target, ..)) => DispatchDecision {
                task_id: req.task_id,
                target: Some(target),
                reason: format!("在 {} 个合格节点中按负载/延迟选择", eligible.len()),
            },
            // eligible 非空时 best 必然为 Some；此分支仅为类型化兜底，避免 panic。
            None => DispatchDecision {
                task_id: req.task_id,
                target: None,
                reason: "调度内部错误：合格集非空但未选出节点".to_string(),
            },
        }
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
}
