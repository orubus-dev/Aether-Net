use aether_core::identity::device::generate_device;
use aether_core::network::{
    AetherPacket, FlowId, MessageType, NodeCapabilities, NodeDescriptor, NodeRole, PacketFlags,
    PacketHeader, ProtocolVersion, RouteToken, TrafficPriority,
};

#[test]
fn protocol_version_is_compatible_with_same_major() {
    let current = ProtocolVersion::CURRENT;
    let compatible = ProtocolVersion::new(current.major, current.minor + 1);

    assert!(current.is_compatible_with(compatible));
}

#[test]
fn different_major_versions_are_incompatible() {
    let current = ProtocolVersion::CURRENT;
    let incompatible = ProtocolVersion::new(current.major.wrapping_add(1), 0);

    assert!(!current.is_compatible_with(incompatible));
}

#[test]
fn node_capabilities_are_composable() {
    let capabilities = NodeCapabilities::NONE
        .insert(NodeCapabilities::SEND)
        .insert(NodeCapabilities::RECEIVE)
        .insert(NodeCapabilities::RELAY);

    assert!(capabilities.contains(NodeCapabilities::SEND));
    assert!(capabilities.contains(NodeCapabilities::RECEIVE));
    assert!(capabilities.contains(NodeCapabilities::RELAY));
    assert!(!capabilities.contains(NodeCapabilities::GATEWAY));

    let removed = capabilities.remove(NodeCapabilities::RELAY);

    assert!(!removed.contains(NodeCapabilities::RELAY));
}

#[test]
fn node_descriptor_is_derived_from_device_crypto() {
    let (_, device) = generate_device(1_000_000).expect("device generation must succeed");

    let capabilities = NodeCapabilities::NONE
        .insert(NodeCapabilities::SEND)
        .insert(NodeCapabilities::RECEIVE)
        .insert(NodeCapabilities::RELAY);

    let descriptor =
        NodeDescriptor::from_device(&device, capabilities, NodeRole::Device, 1_000_000);

    assert_eq!(descriptor.protocol, ProtocolVersion::CURRENT);

    assert!(descriptor.can_relay());
    assert!(!descriptor.can_gateway());
}

#[test]
fn packet_round_trip_preserves_protocol_data() {
    let flow = FlowId::generate().expect("flow generation must succeed");

    let route = RouteToken::generate().expect("route generation must succeed");

    let packet = AetherPacket::new(
        MessageType::Data,
        TrafficPriority::Messaging,
        PacketFlags::ENCRYPTED.insert(PacketFlags::RELAYABLE),
        8,
        flow,
        route,
        b"aether-network-data".to_vec(),
    )
    .expect("packet creation must succeed");

    let encoded = packet.encode().expect("packet encoding must succeed");

    let decoded = AetherPacket::decode(&encoded).expect("packet decoding must succeed");

    assert_eq!(decoded.header.message_type, MessageType::Data);

    assert_eq!(decoded.header.priority, TrafficPriority::Messaging);

    assert!(decoded.header.flags.contains(PacketFlags::ENCRYPTED));

    assert_eq!(decoded.payload, b"aether-network-data");
}

#[test]
fn packet_detects_tampering_with_magic() {
    let flow = FlowId::generate().expect("flow generation must succeed");

    let route = RouteToken::generate().expect("route generation must succeed");

    let packet = AetherPacket::new(
        MessageType::Data,
        TrafficPriority::Browsing,
        PacketFlags::ENCRYPTED,
        4,
        flow,
        route,
        b"data".to_vec(),
    )
    .expect("packet creation must succeed");

    let mut encoded = packet.encode().expect("packet encoding must succeed");

    encoded[0] ^= 0xff;

    assert!(AetherPacket::decode(&encoded).is_err());
}

#[test]
fn packet_rejects_zero_hop_limit() {
    let result = PacketHeader::new(
        MessageType::Data,
        TrafficPriority::Browsing,
        PacketFlags::ENCRYPTED,
        0,
        FlowId::generate().unwrap(),
        RouteToken::generate().unwrap(),
        0,
    );

    assert!(result.is_err());
}

#[test]
fn packet_hop_limit_can_be_decremented() {
    let mut header = PacketHeader::new(
        MessageType::Data,
        TrafficPriority::Browsing,
        PacketFlags::RELAYABLE,
        3,
        FlowId::generate().unwrap(),
        RouteToken::generate().unwrap(),
        0,
    )
    .expect("header creation must succeed");

    header
        .decrement_hop_limit()
        .expect("hop decrement should succeed");

    assert_eq!(header.hop_limit, 2);
}

#[test]
fn priority_classes_are_internal_protocol_values() {
    assert_eq!(TrafficPriority::Emergency as u8, 0);

    assert_eq!(TrafficPriority::Voice as u8, 1);

    assert_eq!(TrafficPriority::Interactive as u8, 2);

    assert_eq!(TrafficPriority::Messaging as u8, 3);

    assert_eq!(TrafficPriority::Browsing as u8, 4);

    assert_eq!(TrafficPriority::Background as u8, 5);

    assert_eq!(TrafficPriority::Prefetch as u8, 6);
}
