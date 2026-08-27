use aether_core::network::{
    AetherPacket, FlowId, MessageType, NodeId, PacketFlags, RouteToken, TrafficPriority,
};
use aether_core::runtime::{ForwardCache, ForwardCacheKey, ForwardingEngine, ForwardingPolicy};
use std::time::{Duration, Instant};

#[test]
fn forwarding_decrements_hop_limit() {
    let packet = AetherPacket::new(
        MessageType::Data,
        TrafficPriority::Messaging,
        PacketFlags::RELAYABLE,
        8,
        FlowId::generate().unwrap(),
        RouteToken::generate().unwrap(),
        b"payload".to_vec(),
    )
    .expect("packet creation failed");

    let engine = ForwardingEngine::new(ForwardingPolicy::default());

    let mut packet = packet;

    engine.decrement(&mut packet).expect("hop decrement failed");

    assert_eq!(packet.header.hop_limit, 7);
}

#[test]
fn forwarding_rejects_expired_packet() {
    let packet = AetherPacket::new(
        MessageType::Data,
        TrafficPriority::Messaging,
        PacketFlags::RELAYABLE,
        1,
        FlowId::generate().unwrap(),
        RouteToken::generate().unwrap(),
        b"payload".to_vec(),
    )
    .expect("packet creation failed");

    let engine = ForwardingEngine::new(ForwardingPolicy::default());

    let result = engine.prepare(&packet, [2u8; 32]);

    assert!(result.is_err());
}

#[test]
fn forwarding_cache_remembers_route() {
    let now = Instant::now();

    let key = ForwardCacheKey {
        packet: aether_core::network::PacketId::generate().unwrap(),

        flow: FlowId::generate().unwrap(),
    };

    let next = NodeId::from_bytes([9u8; 32]);

    let mut cache = ForwardCache::new(Duration::from_secs(30));

    cache.insert(key, *next.as_bytes(), now);

    let entry = cache.lookup(&key, now).expect("route should be cached");

    assert_eq!(entry.next_hop, *next.as_bytes());

    cache.expire(now + Duration::from_secs(31));

    assert!(cache.is_empty());
}
