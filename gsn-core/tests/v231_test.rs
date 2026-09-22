//! V2.3.1 群体智能核心测试

use gsn_core::*;

// ========== swarm 群体智能测试 ==========

#[test]
fn test_swarm_join_and_leave() {
    let mut swarm = Swarm::new();
    let did = Did::from_public_key(b"test-key-1");
    let card = AgentCard::new(did.to_string(), "Test Agent".to_string());
    let node = AgentNode::new(did.clone(), card).with_stake(1000);
    
    swarm.join(node);
    assert_eq!(swarm.node_count(), 1);
    assert_eq!(swarm.online_count(), 1);
    
    swarm.leave(&did);
    assert_eq!(swarm.node_count(), 0);
}

#[test]
fn test_swarm_select_by_capability() {
    let mut swarm = Swarm::new();
    
    for i in 0..5 {
        let did = Did::from_public_key(format!("key-{}", i).as_bytes());
        let card = AgentCard::new(did.to_string(), format!("Agent {}", i).to_string())
            .with_capability("text-generation".to_string());
        let node = AgentNode::new(did, card).with_stake(1000 * (i + 1));
        swarm.join(node);
    }
    
    let selected = swarm.select_by_capability("text-generation", 3);
    assert_eq!(selected.len(), 3);
    // 质押最高的应该排第一
    assert!(selected[0].stake >= selected[1].stake);
}

#[test]
fn test_agent_node_score() {
    let did = Did::from_public_key(b"test");
    let card = AgentCard::new(did.to_string(), "Test".to_string());
    let mut node = AgentNode::new(did, card).with_stake(1000);
    
    assert!(node.score() > 0.0);
    
    node.online = false;
    assert_eq!(node.score(), 0.0);
}

#[test]
fn test_emergence_detector() {
    let mut detector = EmergenceDetector::new(0.2, 5);
    
    // 记录历史数据
    for i in 0..10 {
        detector.record(i, 100.0 + i as f64 * 5.0, 1000.0 - i as f64 * 50.0);
    }
    
    let signals = detector.detect();
    assert!(!signals.is_empty());
}

#[test]
fn test_lightweight_consensus() {
    let mut consensus = LightweightConsensus::new(10000, 0.5);
    
    consensus.propose("prop-1".to_string(), "proposer".to_string(), "Test proposal".to_string(), 100);
    
    consensus.vote("prop-1", "voter1".to_string(), true, 4000);
    consensus.vote("prop-1", "voter2".to_string(), true, 3000);
    consensus.vote("prop-1", "voter3".to_string(), false, 2000);
    
    let result = consensus.tally("prop-1").unwrap();
    assert!(result.consensus_reached);
    assert!(result.accepted);
}

// ========== economy 信誉经济测试 ==========

#[test]
fn test_reputation_system() {
    let mut rep = ReputationSystem::new(86400);
    
    rep.register("agent-1".to_string());
    assert_eq!(rep.get_score("agent-1"), 5000);
    
    rep.record_success("agent-1", 1000);
    assert!(rep.get_score("agent-1") > 5000);
    
    rep.record_failure("agent-1", 100);
    assert!(rep.get_score("agent-1") < 5100);
}

#[test]
fn test_contribution_proof() {
    let proof = ContributionProof::new(
        "agent-1".to_string(),
        ContributionType::TaskCompletion,
        "task-1".to_string(),
        100,
    );
    
    assert!(proof.verify_hash());
    assert!(proof.effective_value() > 100);
}

#[test]
fn test_task_pricing() {
    let pricing = TaskPricing::new(1000)
        .with_difficulty(DifficultyLevel::Hard)
        .with_urgency(UrgencyLevel::High);
    
    let price = pricing.price();
    assert!(price > 1000);
    assert!(price > 4000); // Hard(4.0) * High(1.5) * 1000 = 6000
}

// ========== scheduler 任务调度测试 ==========

#[test]
fn test_task_router() {
    let mut router = TaskRouter::new(5);
    
    router.register_node("agent-1".to_string(), 100);
    router.register_node("agent-2".to_string(), 200);
    
    let task = Task::new("requester".to_string(), "text-gen".to_string(), serde_json::json!({}));
    let candidates = vec!["agent-1".to_string(), "agent-2".to_string()];
    
    let route = router.assign_task(&task, &candidates);
    assert!(route.is_some());
    // 延迟低的应该被选中
    assert_eq!(route.unwrap().agent_did, "agent-1");
}

#[test]
fn test_load_balancer() {
    let mut lb = LoadBalancer::new(BalanceStrategy::LeastConnections);
    
    lb.add_node("node-1".to_string(), 10);
    lb.add_node("node-2".to_string(), 10);
    
    lb.record_task_start("node-1");
    
    let selected = lb.select_node();
    assert!(selected.is_some());
    // node-2 负载更低，应该被选中
    assert_eq!(selected.unwrap(), "node-2");
}

// ========== topology 网络拓扑测试 ==========

#[test]
fn test_neighbor_manager() {
    let mut nm = NeighborManager::new(20);
    
    nm.add_neighbor("node-1".to_string(), "/ip4/1.2.3.4/tcp/4001".to_string(), 50);
    nm.add_neighbor("node-2".to_string(), "/ip4/5.6.7.8/tcp/4001".to_string(), 100);
    
    assert_eq!(nm.neighbor_count(), 2);
    
    let nearest = nm.nearest_neighbors(1);
    assert_eq!(nearest.len(), 1);
    assert_eq!(nearest[0].did, "node-1");
}

#[test]
fn test_topology_graph() {
    let mut graph = TopologyGraph::new();
    
    graph.add_node("A".to_string());
    graph.add_node("B".to_string());
    graph.add_node("C".to_string());
    
    graph.add_edge("A".to_string(), "B".to_string());
    graph.add_edge("B".to_string(), "C".to_string());
    
    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 2);
    assert!(graph.avg_degree() > 0.0);
    
    let path = graph.shortest_path("A", "C");
    assert_eq!(path, Some(2));
}

// ========== proof 贡献证明测试 ==========

#[test]
fn test_proof_of_contribution() {
    let mut poc = ProofOfContribution::new(2);
    
    let hash = poc.submit_contribution(
        "agent-1".to_string(),
        "task-1".to_string(),
        "task_completion".to_string(),
        100,
    );
    
    assert!(!poc.is_valid(&hash));
    
    poc.verify_contribution(&hash, "verifier-1".to_string());
    assert!(!poc.is_valid(&hash));
    
    poc.verify_contribution(&hash, "verifier-2".to_string());
    assert!(poc.is_valid(&hash));
    
    assert_eq!(poc.total_contributions("agent-1"), 100);
}

// ========== 修复验证测试 ==========

#[test]
fn test_erasure_coder_fixed() {
    let coder = ErasureCoder::new(4, 2);
    let data = b"Hello, Agent Universe! This is a test of erasure coding.";
    
    let shards = coder.encode(data);
    assert_eq!(shards.len(), 6); // 4 data + 2 parity
    
    // 只用数据分片解码
    let decoded = coder.decode(&shards, data.len()).unwrap();
    assert_eq!(decoded, data);
    
    // 验证校验分片
    assert!(coder.verify_parity(&shards));
}

#[test]
fn test_pocv_verifier_fixed() {
    let verifier = PoCVVerifier::new();
    
    let input = b"test input";
    let output = b"test output";
    
    let proof = verifier.generate_proof(input, output, 100, "prover".to_string());
    
    assert!(verifier.verify_proof(&proof, input, output));
    assert!(!verifier.verify_proof(&proof, b"wrong input", output));
}

#[test]
fn test_crdt_merge() {
    let mut vv1 = VersionVector::new();
    let mut vv2 = VersionVector::new();
    
    vv1.increment("node-a");
    vv1.increment("node-a");
    vv2.increment("node-b");
    
    vv1.merge(&vv2);
    
    assert_eq!(vv1.get("node-a"), 2);
    assert_eq!(vv1.get("node-b"), 1);
}
