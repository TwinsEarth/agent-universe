// gsn-core/tests/regression_net.rs
// 网络层历史 Bug 回归（Rust 侧）。
//
// 覆盖：
//  - gsn-core/src/net/peer.rs 的 subscribe() 曾把参数名误写成 IdentTopic::new(t)
//    （实际参数名是 topic），导致 cargo build E0425。这里通过真实构造节点、
//    subscribe 再 publish，确保该路径编译通过且不 panic。
//  - GossipSub 在没有 mesh peer 时 publish 返回 InsufficientPeers 属正常现象，
//    不应当成 bug；其他错误才让测试失败。

use gsn_core::net::P2pPeer;
use libp2p::identity::Keypair;

#[tokio::test]
async fn reg_subscribe_then_publish_does_not_panic() {
    // 每次用随机临时身份，不污染 ~/.gsn/identity.key
    let key = Keypair::generate_ed25519();

    let mut peer = P2pPeer::with_identity(key)
        .await
        .expect("with_identity 应能构造节点");

    // subscribe 曾因 new(t) 自引用编译失败；走到这里即说明已修复
    peer.subscribe("gsn.test.regression")
        .expect("subscribe 应成功");

    match peer.publish("gsn.test.regression", b"regression".to_vec()) {
        Ok(_) => {}
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                msg.contains("InsufficientPeers"),
                "publish 无 mesh peer 时只允许 InsufficientPeers，实际错误: {msg}"
            );
        }
    }
}
