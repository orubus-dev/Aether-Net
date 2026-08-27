use aether_core::identity::device::generate_device;
use aether_core::link::{SessionNegotiation, SessionRole};
use aether_core::network::{
    AetherPacket, FlowId, MessageType, NodeId, PacketFlags, RouteToken, TrafficPriority,
};
use aether_core::runtime::{
    EncryptedDataPlane, ForwardCandidate, ForwardSelector, ForwardingEngine, ForwardingPolicy,
};

#[test]
fn three_node_aether_path_is_end_to_end_secure() {
    /*
        A -> B -> C

        A and C have a secure application path.
        B represents the routing/relay decision point.

        This test deliberately keeps application content out of B.
    */

    let (_, device_a) = generate_device(1).expect("node A device generation failed");

    let (_, device_b) = generate_device(1).expect("node B device generation failed");

    let (_, device_c) = generate_device(1).expect("node C device generation failed");

    let node_a = NodeId::from_device(&device_a);

    let node_b = NodeId::from_device(&device_b);

    let node_c = NodeId::from_device(&device_c);

    assert_ne!(node_a, node_b);

    assert_ne!(node_b, node_c);

    assert_ne!(node_a, node_c);

    /*
        Build an A <-> C authenticated session.
        B is the forwarding node and does not receive the
        application plaintext in this test.
    */

    let initiator = SessionNegotiation::create(SessionRole::Initiator)
        .expect("initiator session creation failed");

    let responder = SessionNegotiation::create(SessionRole::Responder)
        .expect("responder session creation failed");

    let initiator_public = initiator.local_ephemeral.public_key();

    let responder_public = responder.local_ephemeral.public_key();

    let transcript: Vec<&[u8]> = vec![
        node_a.as_bytes(),
        node_c.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
        b"M1-END-TO-END-V1",
    ];

    let mut session_a = initiator
        .finish(&responder_public, &transcript, node_a, node_c)
        .expect("A session derivation failed");

    let mut session_c = responder
        .finish(&initiator_public, &transcript, node_a, node_c)
        .expect("C session derivation failed");

    assert_eq!(session_a.session_id(), session_c.session_id());

    let flow = FlowId::generate().expect("flow generation failed");

    let route = RouteToken::generate().expect("route token generation failed");

    let plaintext = b"Aether end-to-end payload".to_vec();

    let encrypted = EncryptedDataPlane::encode_packet(
        &mut session_a,
        TrafficPriority::Messaging,
        flow,
        route,
        8,
        plaintext.clone(),
    )
    .expect("A could not encrypt packet");

    /*
        B must only make a routing decision.
        It does not decrypt the A->C application path.
    */

    let forwarding = ForwardingEngine::new(ForwardingPolicy::default());

    let packet = AetherPacket::new(
        MessageType::Data,
        TrafficPriority::Messaging,
        PacketFlags::RELAYABLE,
        8,
        flow,
        route,
        encrypted.clone(),
    )
    .expect("relay envelope construction failed");

    let decision = forwarding
        .prepare(&packet, *node_c.as_bytes())
        .expect("relay decision failed");

    assert_eq!(decision.next_hop, *node_c.as_bytes());

    assert_eq!(decision.remaining_hops, 7);

    /*
        C decrypts the actual AetherLink payload.
    */

    let received = EncryptedDataPlane::decode_packet(&mut session_c, &encrypted)
        .expect("C could not decrypt A's packet");

    assert_eq!(received.payload, plaintext);

    /*
        B cannot treat the application plaintext as the
        forwarding payload merely from the encrypted frame.
    */

    assert!(
        !encrypted
            .windows(b"Aether end-to-end payload".len())
            .any(|window| window == b"Aether end-to-end payload")
    );
}

#[test]
fn alternate_route_is_selected_after_failure() {
    let a = ForwardCandidate {
        node: NodeId::from_bytes([1u8; 32]),
        latency_ms: 20.0,
        jitter_ms: 2.0,
        loss: 0.001,
        bandwidth_bps: 100_000_000,
        reliability: 0.999,
    };

    let b = ForwardCandidate {
        node: NodeId::from_bytes([2u8; 32]),
        latency_ms: 35.0,
        jitter_ms: 4.0,
        loss: 0.003,
        bandwidth_bps: 80_000_000,
        reliability: 0.997,
    };

    let c = ForwardCandidate {
        node: NodeId::from_bytes([3u8; 32]),
        latency_ms: 500.0,
        jitter_ms: 100.0,
        loss: 0.20,
        bandwidth_bps: 5_000_000,
        reliability: 0.70,
    };

    let first = ForwardSelector::best(&[a, b, c]).expect("initial route missing");

    assert_eq!(first.node, a.node);

    /*
        A is now unavailable.
        The selector must use B rather than C.
    */

    let second = ForwardSelector::best(&[b, c]).expect("fallback route missing");

    assert_eq!(second.node, b.node);
}

#[test]
fn route_packet_hop_limit_is_bounded() {
    let packet = AetherPacket::new(
        MessageType::Data,
        TrafficPriority::Interactive,
        PacketFlags::RELAYABLE,
        2,
        FlowId::generate().unwrap(),
        RouteToken::generate().unwrap(),
        b"route".to_vec(),
    )
    .expect("packet creation failed");

    let mut packet = packet;

    packet
        .header
        .decrement_hop_limit()
        .expect("first hop failed");

    assert_eq!(packet.header.hop_limit, 1);

    assert!(packet.header.decrement_hop_limit().is_err());
}
