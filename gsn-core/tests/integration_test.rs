use gsn_core::*;

#[test]
fn test_full_node_workflow() {
    // 1. 创建节点
    let mut node = GsnNode::new("peer-1".to_string(), "/ip4/0.0.0.0/tcp/4001".to_string());
    
    // 2. 创建 AgentCard
    let card = AgentCard::new(
        "did:aip:agent-1".to_string(),
        "Text Generator".to_string(),
    ).with_capability("text-generation".to_string());
    
    // 3. 发布到 DHT
    node.publish_card(card);
    
    // 4. 发现 Agent
    let discovered = node.discover_by_capability("text-generation");
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].name, "Text Generator");
}

#[test]
fn test_task_lifecycle() {
    let mut task = Task::new(
        "did:aip:requester".to_string(),
        "code-generation".to_string(),
        serde_json::json!({"language": "rust"}),
    );
    
    // 完整任务生命周期
    task.assign("did:aip:executor".to_string());
    task.start();
    task.complete(serde_json::json!({"result": "fn main() {}"}));
    task.verify();
    task.settle();
    
    assert_eq!(task.status, TaskStatus::Settled);
}

#[test]
fn test_dht_shard_distribution() {
    let dht = KademliaClient::new(16);
    
    // 测试多个 key 的分片分布
    let mut shards = std::collections::HashSet::new();
    for i in 0..100 {
        let key = format!("agent-{}", i);
        shards.insert(dht.shard_of(&key));
    }
    
    // 应该分散在多个分片中
    assert!(shards.len() > 10);
}

#[test]
fn test_crdt_merge_convergence() {
    let mut vv1 = VersionVector::new();
    vv1.increment("node1");
    vv1.increment("node1");
    vv1.increment("node2");
    
    let mut vv2 = VersionVector::new();
    vv2.increment("node1");
    vv2.increment("node2");
    vv2.increment("node2");
    vv2.increment("node3");
    
    // 双向合并应该收敛
    vv1.merge(&vv2);
    vv2.merge(&vv1);
    
    assert_eq!(vv1.get("node1"), vv2.get("node1"));
    assert_eq!(vv1.get("node2"), vv2.get("node2"));
    assert_eq!(vv1.get("node3"), vv2.get("node3"));
}

#[test]
fn test_gossip_pubsub() {
    let mut gossip = GossipSub::new();
    
    gossip.subscribe("agents".to_string());
    gossip.subscribe("tasks".to_string());
    
    gossip.publish("agents", b"agent1 online".to_vec());
    gossip.publish("tasks", b"new task: hello".to_vec());
    
    assert_eq!(gossip.get_messages("agents").len(), 1);
    assert_eq!(gossip.get_messages("tasks").len(), 1);
}

#[test]
fn test_erasure_recovery() {
    let coder = ErasureCoder::new(4, 2);
    let data = b"important data that needs redundancy";
    let shards = coder.encode(data);
    
    // 模拟丢失 2 个分片，仍然可以恢复
    let partial_shards = shards[0..4].to_vec();
    let recovered = coder.decode(&partial_shards, data.len()).unwrap();
    assert_eq!(recovered.len(), data.len());
}
