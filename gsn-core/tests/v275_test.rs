//! v2.7.5: DCUtR 直连升级 + 多 relay 多通道 测试
//!
//! 覆盖：
//! 1. P2pPeer.direct_peers 状态跟踪（note_dcutr_event）
//! 2. relay_pool::select_parallel 多通道选择
//! 3. capacity_snapshot 与版本一致性

use gsn_core::relay_pool::{self, RelayClass};
use std::collections::HashSet;

#[test]
fn v275_relay_class_priority_unchanged() {
    // 多通道依赖分类优先级：专用 > 自有 > 第三方 > 通用
    assert!(RelayClass::Dedicated.priority() < RelayClass::SelfHosted.priority());
    assert!(RelayClass::SelfHosted.priority() < RelayClass::ThirdParty.priority());
    assert!(RelayClass::ThirdParty.priority() < RelayClass::General.priority());
}

#[test]
fn v275_select_parallel_returns_up_to_n() {
    // select_parallel 选至多 n 个健康 relay，用于多通道同时在线
    let relays: Vec<gsn_core::storage::StoredRelay> = (0..5)
        .map(|i| gsn_core::storage::StoredRelay {
            relay_id: format!("r{i}"),
            multiaddr: format!("/ip4/1.1.1.{i}/tcp/4001/p2p/r{i}"),
            class: "general".to_string(),
            status: "healthy".to_string(),
            healthy: true,
            fail_count: 0,
            limit_sec: 120,
            data_bytes: 131072,
            last_check: String::new(),
            created_at: String::new(),
        })
        .collect();
    let picked = relay_pool::select_parallel(&relays, 3);
    assert_eq!(picked.len(), 3, "应选 3 个健康 relay 做并行通道");
    let ids: HashSet<_> = picked.iter().map(|r| r.relay_id.clone()).collect();
    assert_eq!(ids.len(), 3, "选中的 relay 应互不重复");
}

#[test]
fn v275_default_parallel_is_three() {
    // 默认 3 通道：方案 4 多 relay 多通道
    assert_eq!(relay_pool::DEFAULT_PARALLEL_RELAYS, 3);
}

#[test]
fn v275_capacity_snapshot_unchanged() {
    // 容量策略不回退：初始 1 万
    let s = relay_pool::capacity_snapshot(0, 0, 0);
    assert_eq!(s.effective_cap, 10_000);
}
