use aether_core::identity::device::generate_device;
use aether_core::mesh::{
    LinkMetrics, MeshPowerPolicy, MeshTopology, Neighbor, RelayPolicy, RelayTable,
};
use aether_core::network::{NodeCapabilities, NodeId, NodeRegistry, NodeRole};
use aether_core::physical::{PhysicalCapabilities, PhysicalTransport};
use aether_core::route::{RouteEngine, RouteMetrics, RoutePath};
use aether_core::services::{
    RuntimeServiceRegistry, ServiceDescriptor, ServiceId, ServiceKind, ServiceTrafficPolicy,
};
use std::time::Instant;

#[test]
fn physical_capabilities_report_supported_transports() {
    let capabilities = PhysicalCapabilities {
        ethernet: true,
        wifi: true,
        wifi_direct: true,
        wifi_aware: false,
        bluetooth_le: true,
        uwb: false,
        lora: false,
        cellular: true,
        satellite: false,
    };

    assert!(capabilities.supports(PhysicalTransport::Ethernet));

    assert!(capabilities.supports(PhysicalTransport::Wifi));

    assert!(capabilities.supports(PhysicalTransport::WifiDirect));

    assert!(!capabilities.supports(PhysicalTransport::Uwb));
}

#[test]
fn battery_policy_stops_normal_relay_at_threshold() {
    let policy = MeshPowerPolicy::default();

    assert!(policy.relay_allowed(50, false, false));

    assert!(!policy.relay_allowed(10, false, false));

    assert!(policy.relay_allowed(10, true, false));

    assert!(policy.relay_allowed(5, false, true));
}

#[test]
fn topology_tracks_bidirectional_neighbors() {
    let (_, device_a) = generate_device(1).expect("device generation failed");

    let (_, device_b) = generate_device(1).expect("device generation failed");

    let node_a = NodeId::from_device(&device_a);

    let node_b = NodeId::from_device(&device_b);

    let mut topology = MeshTopology::new();

    topology.connect(node_a, node_b);

    assert_eq!(topology.degree(&node_a), 1);

    assert_eq!(topology.degree(&node_b), 1);

    topology.disconnect(&node_a, &node_b);

    assert_eq!(topology.degree(&node_a), 0);
}

#[test]
fn relay_table_prefers_better_neighbor() {
    let (_, device_a) = generate_device(1).expect("device generation failed");

    let (_, device_b) = generate_device(1).expect("device generation failed");

    let node_a = NodeId::from_device(&device_a);

    let node_b = NodeId::from_device(&device_b);

    let base = Instant::now();

    let weak = Neighbor {
        node_id: node_a,
        transport: PhysicalTransport::Wifi,
        metrics: LinkMetrics {
            latency_us: 100_000,
            bandwidth_bps: 1_000_000,
            packet_loss_ppm: 100_000,
            jitter_us: 20_000,
            reliability_ppm: 800_000,
        },
        battery_percent: Some(50),
        charging: false,
        thermal_ok: true,
        last_seen: base,
    };

    let strong = Neighbor {
        node_id: node_b,
        transport: PhysicalTransport::Ethernet,
        metrics: LinkMetrics {
            latency_us: 1_000,
            bandwidth_bps: 1_000_000_000,
            packet_loss_ppm: 100,
            jitter_us: 50,
            reliability_ppm: 999_900,
        },
        battery_percent: None,
        charging: true,
        thermal_ok: true,
        last_seen: base,
    };

    let mut table = RelayTable::new();

    table.update(weak);
    table.update(strong);

    let best = table
        .best(RelayPolicy::default())
        .expect("eligible relay should exist");

    assert_eq!(best.node_id, node_b);
}

#[test]
fn network_registry_tracks_nodes() {
    let (_, device) = generate_device(1).expect("device generation failed");

    let descriptor = aether_core::network::NodeDescriptor::from_device(
        &device,
        NodeCapabilities::SEND
            .insert(NodeCapabilities::RECEIVE)
            .insert(NodeCapabilities::RELAY),
        NodeRole::Device,
        1,
    );

    let node_id = descriptor.node_id;

    let mut registry = NodeRegistry::new();

    assert!(!registry.register(descriptor.clone()));

    assert_eq!(registry.len(), 1);

    assert!(registry.get(&node_id).is_some());

    assert!(registry.register(descriptor));
}

#[test]
fn route_engine_prefers_better_route() {
    let (_, a) = generate_device(1).expect("device generation failed");

    let (_, b) = generate_device(1).expect("device generation failed");

    let (_, c) = generate_device(1).expect("device generation failed");

    let node_a = NodeId::from_device(&a);

    let node_b = NodeId::from_device(&b);

    let node_c = NodeId::from_device(&c);

    let poor = RoutePath::new(
        vec![node_a, node_b, node_c],
        RouteMetrics {
            latency_us: 100_000,
            jitter_us: 30_000,
            packet_loss_ppm: 50_000,
            bandwidth_bps: 5_000_000,
            reliability_ppm: 900_000,
            monetary_cost_micros: 1_000_000,
            energy_cost: 20,
            hop_count: 2,
        },
    );

    let good = RoutePath::new(
        vec![node_a, node_c],
        RouteMetrics {
            latency_us: 2_000,
            jitter_us: 100,
            packet_loss_ppm: 100,
            bandwidth_bps: 1_000_000_000,
            reliability_ppm: 999_900,
            monetary_cost_micros: 10,
            energy_cost: 1,
            hop_count: 1,
        },
    );

    let engine = RouteEngine::new();

    let best = engine
        .best(
            vec![poor, good.clone()],
            aether_core::network::TrafficPriority::Interactive,
        )
        .expect("route should exist");

    assert_eq!(best.nodes, good.nodes);
}

#[test]
fn service_policy_assigns_internal_priority() {
    assert_eq!(
        ServiceTrafficPolicy::classify(ServiceKind::Gaming),
        aether_core::network::TrafficPriority::Interactive
    );

    assert_eq!(
        ServiceTrafficPolicy::classify(ServiceKind::Communication),
        aether_core::network::TrafficPriority::Messaging
    );

    assert_eq!(
        ServiceTrafficPolicy::classify(ServiceKind::Storage),
        aether_core::network::TrafficPriority::Background
    );
}

#[test]
fn runtime_service_registry_manages_services() {
    let service = ServiceDescriptor {
        id: ServiceId([1u8; 16]),
        name: "test-service".to_owned(),
        version: "1.0.0".to_owned(),
        kind: ServiceKind::ThirdParty,
        minimum_priority: aether_core::network::TrafficPriority::Browsing,
    };

    let mut registry = RuntimeServiceRegistry::new();

    assert!(!registry.register(service.clone()));

    assert!(registry.get(&service.id).is_some());

    assert!(registry.unregister(&service.id).is_some());

    assert!(registry.get(&service.id).is_none());
}
