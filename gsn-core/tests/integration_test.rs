use gsn_core::*;

#[test]
fn test_full_node_workflow() {
    // 1. 创建节点
    let mut node = InMemoryNode::new("peer-1".to_string(), "/ip4/0.0.0.0/tcp/4001".to_string());
    
    // 2. 创建 AgentCard
    let card = AgentCard::new(
        "did:aip:agent-1".to_string(),
        "Text Generator".to_string(),
    ).with_capability("text-generation".to_string());
    
    // 3. 写入进程内替身（非真实 DHT，不联网）
    node.publish_card(card);
    
    // 4. 本地按能力检索（非真实网络发现）
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
    let dht = InMemoryKademlia::new(16);
    
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
    let mut gossip = InMemoryGossip::new();
    
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
    assert_eq!(shards.len(), 6);

    // 校验片与数据片一致
    assert!(coder.verify_parity(&shards));

    // 场景 1：丢失 2 个校验片，仅靠 4 个数据片恢复
    let only_data: Vec<_> = shards.iter().take(4).cloned().collect();
    assert_eq!(coder.decode(&only_data, data.len()).unwrap(), data);

    // 场景 2：丢失 2 个数据片，靠 2 数据 + 2 校验片重建（真正的 RS 恢复）
    let mut partial: Vec<_> = shards.iter().take(2).cloned().collect();
    partial.extend(shards.iter().skip(4).cloned().collect::<Vec<_>>());
    assert_eq!(coder.decode(&partial, data.len()).unwrap(), data);

    // 场景 3：丢失 1 数据片 + 1 校验片，仍可恢复
    let mut partial: Vec<_> = shards.iter().take(3).cloned().collect();
    partial.push(shards[5].clone());
    assert_eq!(coder.decode(&partial, data.len()).unwrap(), data);

    // 场景 4：丢失 3 片（> parity_shards=2），无法恢复，必须报错
    let three_lost: Vec<_> = shards.iter().take(3).cloned().collect();
    assert!(coder.decode(&three_lost, data.len()).is_err());
}
