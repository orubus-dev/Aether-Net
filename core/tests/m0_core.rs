use aether_core::identity::{
    AetherId, AuthorizationGrant, Capability, DeviceCapabilities, DeviceSigningKey, DeviceState,
};
use aether_core::link::{SessionNegotiation, SessionRole};
use aether_core::network::TrafficPriority;
use aether_core::network::{NodeCapabilities, NodeDescriptor, NodeId, NodeRegistry, NodeRole};
use aether_core::physical::{PhysicalCapabilities, PhysicalTransport};
use aether_core::route::{RouteEngine, RouteMetrics, RoutePath};
use aether_core::runtime::{CoreConfig, CoreMetrics, HealthReport};
use aether_core::services::{ServiceKind, ServiceTrafficPolicy};
use std::sync::atomic::Ordering;

#[test]
fn identity_and_device_are_independent() {
    let identity = AetherId::generate().expect("identity generation failed");

    let device = DeviceSigningKey::generate().expect("device generation failed");

    let device_state = DeviceState::from_signing_key(&device, 1, DeviceCapabilities::default());

    assert_ne!(
        identity.public_key_bytes(),
        device.verifying_key().to_bytes()
    );

    assert!(device_state.is_active());
}

#[test]
fn authorization_is_cryptographically_bound() {
    let identity = AetherId::generate().expect("identity generation failed");

    let device = DeviceSigningKey::generate().expect("device generation failed");

    let grant = AuthorizationGrant::issue(
        identity.signing_key(),
        device.device_id(),
        Capability::Relay,
        100,
        Some(200),
        [7u8; 32],
    );

    assert!(grant.verify(&identity.public_key(), 150,));

    assert!(!grant.verify(&identity.public_key(), 200,));
}

#[test]
fn node_registry_accepts_real_descriptor() {
    let device = DeviceSigningKey::generate().expect("device generation failed");

    let state = DeviceState::from_signing_key(&device, 1, DeviceCapabilities::default());

    let descriptor = NodeDescriptor::from_device(
        &state,
        NodeCapabilities::SEND
            .insert(NodeCapabilities::RECEIVE)
            .insert(NodeCapabilities::RELAY),
        NodeRole::Device,
        1,
    );

    let mut registry = NodeRegistry::new();

    assert!(!registry.register(descriptor.clone()));

    assert_eq!(registry.len(), 1);

    assert!(registry.get(&descriptor.node_id).is_some());
}

#[test]
fn link_establishes_and_encrypts() {
    let device_a = DeviceSigningKey::generate().expect("device A failed");

    let device_b = DeviceSigningKey::generate().expect("device B failed");

    let state_a = DeviceState::from_signing_key(&device_a, 1, DeviceCapabilities::default());

    let state_b = DeviceState::from_signing_key(&device_b, 1, DeviceCapabilities::default());

    let node_a = NodeId::from_device(&state_a);

    let node_b = NodeId::from_device(&state_b);

    let initiator = SessionNegotiation::create(SessionRole::Initiator).expect("initiator failed");

    let responder = SessionNegotiation::create(SessionRole::Responder).expect("responder failed");

    let initiator_public = initiator.local_ephemeral.public_key();

    let responder_public = responder.local_ephemeral.public_key();

    let transcript: Vec<&[u8]> = vec![
        b"AETHER-M0-END-TO-END",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let mut session_a = initiator
        .finish(&responder_public, &transcript, node_a, node_b)
        .expect("session A failed");

    let mut session_b = responder
        .finish(&initiator_public, &transcript, node_a, node_b)
        .expect("session B failed");

    let frame = session_a
        .encrypt(
            aether_core::network::MessageType::Data,
            TrafficPriority::Messaging,
            aether_core::network::PacketFlags::RELAYABLE,
            b"Aether M0",
        )
        .expect("encryption failed");

    let plaintext = session_b.decrypt(&frame).expect("decryption failed");

    assert_eq!(plaintext.payload, b"Aether M0");
}

#[test]
fn route_engine_ranks_paths() {
    let device_a = DeviceSigningKey::generate().expect("device A failed");

    let device_b = DeviceSigningKey::generate().expect("device B failed");

    let state_a = DeviceState::from_signing_key(&device_a, 1, DeviceCapabilities::default());

    let state_b = DeviceState::from_signing_key(&device_b, 1, DeviceCapabilities::default());

    let a = NodeId::from_device(&state_a);

    let b = NodeId::from_device(&state_b);

    let path = RoutePath::new(
        vec![a, b],
        RouteMetrics {
            latency_us: 1_000,
            jitter_us: 50,
            packet_loss_ppm: 100,
            bandwidth_bps: 1_000_000_000,
            reliability_ppm: 999_900,
            monetary_cost_micros: 1,
            energy_cost: 1,
            hop_count: 1,
        },
    );

    let engine = RouteEngine::new();

    let best = engine
        .best(vec![path.clone()], TrafficPriority::Interactive)
        .expect("route should exist");

    assert_eq!(best.nodes, path.nodes);
}

#[test]
fn physical_layer_reports_capabilities() {
    let capabilities = PhysicalCapabilities {
        ethernet: true,
        wifi: true,
        wifi_direct: true,
        wifi_aware: true,
        bluetooth_le: true,
        uwb: false,
        lora: false,
        cellular: true,
        satellite: false,
    };

    assert!(capabilities.supports(PhysicalTransport::Wifi));

    assert!(capabilities.supports(PhysicalTransport::WifiDirect));

    assert!(!capabilities.supports(PhysicalTransport::Uwb));
}

#[test]
fn service_priority_is_system_classified() {
    assert_eq!(
        ServiceTrafficPolicy::classify(ServiceKind::Gaming,),
        TrafficPriority::Interactive,
    );

    assert_eq!(
        ServiceTrafficPolicy::classify(ServiceKind::Storage,),
        TrafficPriority::Background,
    );
}

#[test]
fn runtime_health_and_metrics_work() {
    let config = CoreConfig::default();

    assert_eq!(config.default_hop_limit, 16);

    let health = HealthReport::healthy();

    assert!(health.is_healthy());

    let metrics = CoreMetrics::default();

    metrics.packets_sent.fetch_add(1, Ordering::Relaxed);

    let snapshot = metrics.snapshot();

    assert_eq!(snapshot.packets_sent, 1);
}
