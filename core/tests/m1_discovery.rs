use aether_core::network::{NodeCapabilities, NodeId, NodeRole, ProtocolVersion};
use aether_core::runtime::discovery::DiscoveryHello;
use aether_core::runtime::peers::PeerTable;
use std::time::{Duration, Instant};

#[test]
fn discovery_hello_binary_round_trip() {
    let node_id = [1u8; 32];

    let device_id = [2u8; 32];

    let hello = DiscoveryHello {
        protocol: ProtocolVersion::CURRENT,

        node_id,

        device_id,

        capabilities: NodeCapabilities::SEND
            .insert(NodeCapabilities::RECEIVE)
            .insert(NodeCapabilities::RELAY),

        role: NodeRole::Device,

        timestamp: 1_000,
    };

    let encoded = hello.encode();

    let decoded = DiscoveryHello::decode(&encoded).expect("discovery decode failed");

    assert_eq!(decoded, hello);
}

#[test]
fn discovery_rejects_invalid_magic() {
    let node_id = [1u8; 32];

    let device_id = [2u8; 32];

    let hello = DiscoveryHello {
        protocol: ProtocolVersion::CURRENT,

        node_id,
        device_id,

        capabilities: NodeCapabilities::SEND,

        role: NodeRole::Device,

        timestamp: 1_000,
    };

    let mut encoded = hello.encode();

    encoded[0] ^= 0xff;

    assert!(DiscoveryHello::decode(&encoded).is_err());
}

#[test]
fn peer_table_tracks_and_expires_peer() {
    let hello = DiscoveryHello {
        protocol: ProtocolVersion::CURRENT,

        node_id: [7u8; 32],

        device_id: [8u8; 32],

        capabilities: NodeCapabilities::RELAY,

        role: NodeRole::Relay,

        timestamp: 1_000,
    };

    let now = Instant::now();

    let address = "127.0.0.1:7001".parse().unwrap();

    let mut peers = PeerTable::new();

    let node_id = peers.upsert(&hello, address, now);

    assert_eq!(node_id, NodeId::from_bytes([7u8; 32]));

    assert_eq!(peers.len(), 1);

    assert!(peers.get(&node_id).is_some());

    let expired = peers.expire(now + Duration::from_secs(31), Duration::from_secs(30));

    assert_eq!(expired.len(), 1);

    assert!(peers.is_empty());
}
