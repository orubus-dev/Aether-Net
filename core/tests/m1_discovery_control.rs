use aether_core::network::{NodeCapabilities, NodeId, NodeRole, ProtocolVersion};

use aether_core::runtime::discovery::{DiscoveryControl, DiscoveryHello, HelloAck};

#[test]
fn hello_control_round_trip() {
    let hello = DiscoveryHello {
        protocol: ProtocolVersion::CURRENT,

        node_id: [1u8; 32],

        device_id: [2u8; 32],

        capabilities: NodeCapabilities::SEND
            .insert(NodeCapabilities::RECEIVE)
            .insert(NodeCapabilities::RELAY),

        role: NodeRole::Relay,

        timestamp: 123,
    };

    let frame = DiscoveryControl::Hello(hello).encode();

    let decoded = DiscoveryControl::decode(&frame).expect("hello should decode");

    assert_eq!(decoded, DiscoveryControl::Hello(hello));
}

#[test]
fn hello_ack_round_trip() {
    let ack = HelloAck {
        node_id: NodeId::from_bytes([3u8; 32]),

        timestamp: 456,
    };

    let frame = DiscoveryControl::HelloAck(ack).encode();

    let decoded = DiscoveryControl::decode(&frame).expect("ack should decode");

    assert_eq!(decoded, DiscoveryControl::HelloAck(ack));
}
