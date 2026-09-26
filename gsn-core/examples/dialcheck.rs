// v2.5.4 独立诊断：最小 libp2p 节点 dial 公共节点，打印全部 swarm 事件
// 运行: cargo run --example dialcheck
use futures::StreamExt;
use libp2p::{
    identify, ping, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux,
    Multiaddr, SwarmBuilder,
};
use std::time::Duration;

#[derive(NetworkBehaviour)]
struct DiagBehaviour {
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let target: Multiaddr =
        "/ip4/15.235.144.210/tcp/4001/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt"
            .parse()?;

    let mut swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let identify = identify::Behaviour::new(identify::Config::new(
                "/diag/1".to_string(),
                key.public(),
            ));
            let ping = ping::Behaviour::new(
                ping::Config::new().with_interval(Duration::from_secs(15)),
            );
            DiagBehaviour { identify, ping }
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    println!("Local peer id: {}", swarm.local_peer_id());
    println!("Dialing: {}", target);
    match swarm.dial(target.clone()) {
        Ok(_) => println!("dial() returned Ok"),
        Err(e) => println!("dial() returned Err: {e:?}"),
    }

    let mut established = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(25);
    loop {
        tokio::select! {
            ev = swarm.next() => {
                let ev = ev.expect("swarm stream");
                println!("EVENT: {ev:?}");
                if let SwarmEvent::ConnectionEstablished { peer_id, .. } = ev {
                    println!(">>> CONNECTED to {peer_id}");
                    established = true;
                }
                if let SwarmEvent::OutgoingConnectionError { peer_id, error, .. } = ev {
                    println!(">>> OUTGOING ERROR peer={peer_id:?} error={error:?}");
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    println!(
        "RESULT: connected_peers={:?} established_flag={established}",
        swarm.connected_peers().collect::<Vec<_>>()
    );
    Ok(())
}
