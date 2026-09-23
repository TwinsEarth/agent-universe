use gsn_core::*;

#[test]
fn test_did_creation() {
    let keypair = Keypair::generate();
    let did = Did::from_public_key(keypair.public_key());
    assert!(did.as_str().starts_with("did:aip:"));
}

#[test]
fn test_signer() {
    let keypair = Keypair::generate();
    let signer = Ed25519Signer::new(&keypair);
    let message = b"hello world";
    let signature = signer.sign(message);
    assert!(signer.verify(message, &signature));
}

#[test]
fn test_agent_card() {
    let card = AgentCard::new("did:aip:test".to_string(), "Test Agent".to_string())
        .with_capability("text-generation".to_string());
    assert_eq!(card.name, "Test Agent");
    assert!(card.capabilities.contains(&"text-generation".to_string()));
}

#[test]
fn test_task_state_machine() {
    let mut task = Task::new(
        "did:aip:requester".to_string(),
        "text-generation".to_string(),
        serde_json::json!({"prompt": "hello"}),
    );
    assert_eq!(task.status, TaskStatus::Pending);
    
    task.assign("did:aip:executor".to_string());
    assert_eq!(task.status, TaskStatus::Assigned);
    
    task.start();
    assert_eq!(task.status, TaskStatus::Running);
    
    task.complete(serde_json::json!({"result": "world"}));
    assert_eq!(task.status, TaskStatus::Completed);
    
    task.verify();
    assert_eq!(task.status, TaskStatus::Verified);
    
    task.settle();
    assert_eq!(task.status, TaskStatus::Settled);
}

#[test]
fn test_dht_sharding() {
    let dht = KademliaClient::new(16);
    assert_eq!(dht.shard_count, 16);
    
    let shard1 = dht.shard_of("test-key-1");
    let shard2 = dht.shard_of("test-key-1");
    assert_eq!(shard1, shard2);
    
    assert!(shard1 < 16);
}

#[test]
fn test_dht_put_get() {
    let mut dht = KademliaClient::new(4);
    dht.put("key1".to_string(), b"value1".to_vec());
    assert_eq!(dht.get("key1"), Some(&b"value1".to_vec()));
    assert_eq!(dht.len(), 1);
}

#[test]
fn test_gossip_sub() {
    let mut gossip = GossipSub::new();
    gossip.subscribe("agents".to_string());
    gossip.publish("agents", b"hello".to_vec());
    assert_eq!(gossip.get_messages("agents").len(), 1);
}

#[test]
fn test_node_publish_and_discover() {
    let mut node = GsnNode::new("peer123".to_string(), "/ip4/0.0.0.0/tcp/4001".to_string());
    
    let card = AgentCard::new("did:aip:agent1".to_string(), "Text Generator".to_string())
        .with_capability("text-generation".to_string());
    
    node.publish_card(card);
    
    let discovered = node.discover_by_capability("text-generation");
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].name, "Text Generator");
}

#[test]
fn test_pocv_verifier() {
    let verifier = PoCVVerifier::new();
    let data = b"test data";
    let hash = verifier.compute_hash(data);
    assert!(verifier.verify_hash(data, &hash));
}

#[test]
fn test_local_storage() {
    let mut storage = LocalStorage::new();
    storage.put("key1".to_string(), b"value1".to_vec());
    assert_eq!(storage.get("key1"), Some(&b"value1".to_vec()));
    assert_eq!(storage.len(), 1);
}
