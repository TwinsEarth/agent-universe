use gsn_core::*;

#[test]
fn test_root_seed_config_default() {
    let config = RootSeedConfig::default();
    assert_eq!(config.mode, SeedMode::Root);
    assert!(config.enable_dht_server);
}

#[test]
fn test_root_seed_should_downgrade() {
    let config = RootSeedConfig::default();
    assert!(config.should_downgrade(1500, 800));
    assert!(!config.should_downgrade(500, 200));
}

#[test]
fn test_root_seed_downgrade() {
    let mut config = RootSeedConfig::default();
    assert_eq!(config.mode, SeedMode::Root);
    config.downgrade();
    assert_eq!(config.mode, SeedMode::Bootstrap);
}

#[test]
fn test_node_modes() {
    assert!(NodeMode::Archive.requires_dht_server());
    assert!(NodeMode::Full.requires_dht_server());
    assert!(!NodeMode::Light.requires_dht_server());
    assert!(!NodeMode::Browser.requires_dht_server());
}

#[test]
fn test_mode_controller() {
    let mut controller = ModeController::new(NodeMode::Full);
    assert_eq!(controller.current(), NodeMode::Full);
    controller.switch_to(NodeMode::Light).unwrap();
    assert_eq!(controller.current(), NodeMode::Light);
}

#[test]
fn test_version_vector() {
    let mut vv1 = VersionVector::new();
    vv1.increment("node1");
    vv1.increment("node1");
    vv1.increment("node2");
    
    assert_eq!(vv1.get("node1"), 2);
    assert_eq!(vv1.get("node2"), 1);
}

#[test]
fn test_version_vector_merge() {
    let mut vv1 = VersionVector::new();
    vv1.increment("node1");
    vv1.increment("node1");
    
    let mut vv2 = VersionVector::new();
    vv2.increment("node1");
    vv2.increment("node2");
    
    vv1.merge(&vv2);
    
    assert_eq!(vv1.get("node1"), 2);
    assert_eq!(vv1.get("node2"), 1);
}

#[test]
fn test_erasure_coder() {
    let coder = ErasureCoder::new(4, 2);
    assert_eq!(coder.total_shards(), 6);
    
    let data = b"hello world, this is a test data for erasure coding";
    let shards = coder.encode(data);
    assert_eq!(shards.len(), 6);
    
    let decoded = coder.decode(&shards, data.len()).unwrap();
    assert_eq!(decoded.len(), data.len());
}
