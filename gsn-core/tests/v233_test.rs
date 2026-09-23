//! V2.3.3 P2P 分布式网络层测试

use gsn_core::*;

// ========== Agent 协作测试 ==========

#[test]
fn test_collaboration_group_creation() {
    let mut mgr = CollaborationManager::new(
        "agent_001".to_string(),
        AgentTier::EndAgent,
    );

    let group_id = mgr.create_group("智能家居组".to_string(), SceneType::Home);
    assert!(group_id.starts_with("grp_"));

    mgr.join_group(group_id.clone(), "agent_002".to_string(), AgentTier::RouteAgent);
    mgr.join_group(group_id, "agent_003".to_string(), AgentTier::EndAgent);

    let groups = mgr.get_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].members.len(), 2);
}

#[test]
fn test_agent_tier_hierarchy() {
    assert!((AgentTier::McpAgent as u8) < (AgentTier::RouteAgent as u8));
    assert!((AgentTier::RouteAgent as u8) < (AgentTier::EndAgent as u8));
}

// ========== 分布式推理测试 ==========

#[test]
fn test_compute_scheduler() {
    let mut scheduler = ComputeScheduler::new();

    scheduler.register_resource(ComputeResource {
        node_id: "gpu_node_1".to_string(),
        gpu_model: "A100".to_string(),
        vram_gb: 80,
        cpus: 32,
        ram_gb: 256,
        bandwidth_mbps: 10000,
        available: true,
        kv_cache_shards: vec![],
    });

    scheduler.register_resource(ComputeResource {
        node_id: "gpu_node_2".to_string(),
        gpu_model: "RTX 4090".to_string(),
        vram_gb: 24,
        cpus: 16,
        ram_gb: 64,
        bandwidth_mbps: 1000,
        available: true,
        kv_cache_shards: vec![],
    });

    assert_eq!(scheduler.available_nodes(), 2);
    assert_eq!(scheduler.total_vram_gb(), 104);
}

#[test]
fn test_inference_task_assignment() {
    let mut scheduler = ComputeScheduler::new();

    scheduler.register_resource(ComputeResource {
        node_id: "fast_gpu".to_string(),
        gpu_model: "H100".to_string(),
        vram_gb: 80,
        cpus: 64,
        ram_gb: 512,
        bandwidth_mbps: 25000,
        available: true,
        kv_cache_shards: vec![],
    });

    let task = InferenceTask {
        task_id: "infer_001".to_string(),
        model_hash: "llama_70b".to_string(),
        prompt: "Hello".to_string(),
        max_tokens: 512,
        required_vram_gb: 40,
        timeout_secs: 60,
        status: InferenceTaskStatus::Pending,
    };

    scheduler.submit_task(task);
    let assigned = scheduler.assign_task("infer_001".to_string());
    assert!(assigned.is_some());
    assert_eq!(assigned.unwrap(), "fast_gpu");
}

// ========== 任务众包测试 ==========

#[test]
fn test_crowdsourcing_market() {
    let mut market = CrowdsourcingMarket::new();

    market.register_solver(Solver {
        solver_id: "solver_001".to_string(),
        name: "Alice".to_string(),
        reputation: 0.95,
        completed_tasks: 42,
        success_rate: 0.95,
        available: true,
        points: 1500,
    });

    market.publish_task(CrowdTask {
        task_id: "task_001".to_string(),
        publisher: "publisher_001".to_string(),
        title: "网页抓取".to_string(),
        description: "抓取新闻标题".to_string(),
        task_type: CrowdTaskType::WebScraping,
        reward_points: 100,
        deadline: 1234567890,
        status: CrowdTaskStatus::Open,
        solver: None,
        result: None,
    });

    assert_eq!(market.open_tasks().len(), 1);
    assert_eq!(market.total_solvers(), 1);
}

#[test]
fn test_task_complete_and_reward() {
    let mut market = CrowdsourcingMarket::new();

    market.register_solver(Solver {
        solver_id: "solver_001".to_string(),
        name: "Bob".to_string(),
        reputation: 0.8,
        completed_tasks: 10,
        success_rate: 0.8,
        available: true,
        points: 500,
    });

    market.publish_task(CrowdTask {
        task_id: "task_002".to_string(),
        publisher: "pub_001".to_string(),
        title: "翻译".to_string(),
        description: "翻译英文文章".to_string(),
        task_type: CrowdTaskType::Translation,
        reward_points: 200,
        deadline: 1234567890,
        status: CrowdTaskStatus::Open,
        solver: None,
        result: None,
    });

    let accepted = market.accept_task("task_002".to_string(), "solver_001".to_string());
    assert!(accepted);

    let completed = market.complete_task("task_002".to_string(), "translated text".to_string());
    assert!(completed);

    let solver = market.get_solver("solver_001").unwrap();
    assert_eq!(solver.points, 700);
    assert_eq!(solver.completed_tasks, 11);
}

// ========== 安全防御测试 ==========

#[test]
fn test_sybil_detection() {
    let mut engine = SecurityEngine::new();

    // 每次报告 score *= 0.8，6 次后 0.262 < 0.3 阈值触发封禁
    for _ in 0..6 {
        engine.report_behavior("malicious_node".to_string(), SecurityFlag::SybilSuspected);
    }

    assert!(engine.is_banned("malicious_node"));
    assert_eq!(engine.banned_count(), 1);
}

#[test]
fn test_benign_node_not_banned() {
    let mut engine = SecurityEngine::new();

    engine.report_behavior("good_node".to_string(), SecurityFlag::OfflineTooLong);
    engine.report_behavior("good_node".to_string(), SecurityFlag::OfflineTooLong);

    // 连续两次 offline 只扣 36% 分，仍高于 0.3 阈值
    assert!(!engine.is_banned("good_node"));
}

// ========== NAT 穿透测试 ==========

#[test]
fn test_nat_type_detection() {
    let mut manager = NatTraversalManager::new();
    let nat_type = manager.detect_nat_type();
    assert!(matches!(nat_type, NatType::PortRestrictedCone));
}

#[test]
fn test_connection_establishment() {
    let mut manager = NatTraversalManager::new();

    let state = manager.connect(
        "peer_001".to_string(),
        vec![IceCandidate {
            candidate_id: "cand_001".to_string(),
            addr: "192.168.1.100".to_string(),
            port: 8080,
            priority: 100,
            candidate_type: CandidateType::Host,
        }],
    );

    assert!(matches!(state, ConnectionState::Connected));
    assert_eq!(manager.connected_count(), 1);
}

#[test]
fn test_stun_server_config() {
    let manager = NatTraversalManager::new();
    assert!(manager.stun_server_count() >= 2);
    assert!(!manager.has_turn_relay());
}
