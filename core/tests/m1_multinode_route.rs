use aether_core::network::NodeId;
use aether_core::runtime::{ForwardCandidate, ForwardSelector};

#[test]
fn route_selector_prefers_reliable_low_latency_path() {
    let slow = ForwardCandidate {
        node: NodeId::from_bytes([1u8; 32]),
        latency_ms: 180.0,
        jitter_ms: 20.0,
        loss: 0.04,
        bandwidth_bps: 20_000_000,
        reliability: 0.92,
    };

    let fast = ForwardCandidate {
        node: NodeId::from_bytes([2u8; 32]),
        latency_ms: 30.0,
        jitter_ms: 2.0,
        loss: 0.001,
        bandwidth_bps: 100_000_000,
        reliability: 0.999,
    };

    let selected = ForwardSelector::best(&[slow, fast]).expect("route should exist");

    assert_eq!(selected.node, fast.node);
}
