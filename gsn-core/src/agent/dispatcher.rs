//! 调配 Agent（Dispatcher，v5.0.0 骨架）
//!
//! 按角色 / 能力 / 记忆调度，持有 [`TaskRouter`] 与 [`LoadBalancer`]。
//! v5.0.0 提供角色定义、节点画像、**角色路由**（FLEET-ROLES 路由表：按角色
//! 而非便利）与消息入口；角色/能力画像在 v5.0.6 完善，记忆驱动调度在 v5.0.7。

use std::collections::HashMap;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeProfile {
    pub node_id: String,
    pub role: FleetRole,
    pub capabilities: Vec<String>,
    pub latency_ms: u64,
    pub capacity: u64,
}

impl NodeProfile {
    pub fn new(node_id: impl Into<String>, role: FleetRole) -> Self {
        Self {
            node_id: node_id.into(),
            role,
            capabilities: Vec::new(),
            latency_ms: 100,
            capacity: 16,
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
}
