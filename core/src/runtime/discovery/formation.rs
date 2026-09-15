use crate::discovery::{
    BootstrapPeer, DiscoveryError, FormationState, NetworkFormationEngine, PeerRecord,
};
use crate::identity::device::DeviceId;
use crate::network::NodeId;
use crate::physical::PhysicalTransport;

use std::net::SocketAddr;

pub const DEFAULT_DISCOVERY_GC_INTERVAL_SECONDS: u64 = 5;

/// Runtime-owned Phase 6 coordinator. It turns authenticated/validated discovery
/// observations into bounded network-formation state and provides a single
/// integration point for native underlays and legacy bootstrap carriers.
pub struct DiscoveryFormation {
    engine: NetworkFormationEngine,
    transport: PhysicalTransport,
    last_gc: u64,
}

impl DiscoveryFormation {
    pub fn new(transport: PhysicalTransport) -> Self {
        Self {
            engine: NetworkFormationEngine::new(),
            transport,
            last_gc: 0,
        }
    }

    pub fn with_engine(transport: PhysicalTransport, engine: NetworkFormationEngine) -> Self {
        Self {
            engine,
            transport,
            last_gc: 0,
        }
    }

    pub fn state(&self) -> FormationState {
        self.engine.state()
    }

    pub fn generation(&self) -> u64 {
        self.engine.generation()
    }

    pub fn peer_count(&self) -> usize {
        self.engine.peer_count()
    }

    pub fn service_count(&self) -> usize {
        self.engine.service_count()
    }

    pub fn resource_count(&self) -> usize {
        self.engine.resource_count()
    }

    pub fn bootstrap_peers(&self) -> &[BootstrapPeer] {
        self.engine.bootstrap_peers()
    }

    pub fn add_bootstrap(&mut self, node_id: NodeId, device_id: DeviceId, address: SocketAddr) -> Result<(), DiscoveryError> {
        self.engine.add_bootstrap(BootstrapPeer {
            node_id,
            device_id,
            transport: self.transport,
            endpoint_hint: endpoint_hint(address),
        })
    }

    pub fn observe_peer(
        &mut self,
        node_id: NodeId,
        device_id: DeviceId,
        now: u64,
    ) -> Result<bool, DiscoveryError> {
        self.engine.observe_peer(PeerRecord::new(
            node_id,
            device_id,
            self.transport,
            now,
        ), now)
    }

    pub fn maintain(&mut self, now: u64) -> usize {
        if now.saturating_sub(self.last_gc) < DEFAULT_DISCOVERY_GC_INTERVAL_SECONDS {
            return 0;
        }
        self.last_gc = now;
        self.engine.expire(now)
    }

    pub fn force_expire(&mut self, now: u64) -> usize {
        self.last_gc = now;
        self.engine.expire(now)
    }

    pub fn mark_partitioned(&mut self) {
        self.engine.mark_partitioned();
    }

    pub fn recover_partition(&mut self, now: u64) {
        self.engine.recover_partition(now);
        self.last_gc = now;
    }

    pub fn engine(&self) -> &NetworkFormationEngine {
        &self.engine
    }

    fn endpoint_hint_for(address: SocketAddr) -> [u8; 32] {
        endpoint_hint(address)
    }
}

fn endpoint_hint(address: SocketAddr) -> [u8; 32] {
    let mut hint = [0u8; 32];
    match address {
        SocketAddr::V4(value) => {
            hint[0] = 4;
            hint[1..5].copy_from_slice(&value.ip().octets());
            hint[5..7].copy_from_slice(&value.port().to_be_bytes());
        }
        SocketAddr::V6(value) => {
            hint[0] = 6;
            hint[1..17].copy_from_slice(&value.ip().octets());
            hint[17..19].copy_from_slice(&value.port().to_be_bytes());
            hint[19] = value.flowinfo() as u8;
            hint[20] = (value.flowinfo() >> 8) as u8;
            hint[21] = (value.flowinfo() >> 16) as u8;
            hint[22] = (value.flowinfo() >> 24) as u8;
            let scope = value.scope_id().to_be_bytes();
            hint[23..27].copy_from_slice(&scope);
        }
    }
    hint
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(seed: u8) -> NodeId {
        NodeId::from_bytes([seed; 32])
    }

    fn device(seed: u8) -> DeviceId {
        DeviceId::from_bytes([seed; 32])
    }

    #[test]
    fn bootstrap_is_registered_and_forms_network() {
        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet);
        let address = "127.0.0.1:4000".parse().unwrap();
        discovery.add_bootstrap(node(1), device(1), address).unwrap();
        assert_eq!(discovery.state(), FormationState::Forming);
        assert_eq!(discovery.bootstrap_peers().len(), 1);
    }

    #[test]
    fn observed_peer_transitions_to_connected() {
        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet);
        assert!(discovery.observe_peer(node(1), device(1), 10).unwrap());
        assert_eq!(discovery.state(), FormationState::Connected);
        assert_eq!(discovery.peer_count(), 1);
    }

    #[test]
    fn maintenance_is_rate_limited_and_collects_expired_state() {
        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet);
        discovery.observe_peer(node(1), device(1), 0).unwrap();
        assert_eq!(discovery.maintain(1), 0);
        assert_eq!(discovery.force_expire(200), 1);
        assert_eq!(discovery.peer_count(), 0);
    }

    #[test]
    fn endpoint_hints_are_deterministic() {
        let address = "192.0.2.44:4242".parse().unwrap();
        assert_eq!(endpoint_hint(address), endpoint_hint(address));
    }

    #[test]
    fn helper_method_uses_same_endpoint_encoding() {
        let address = "[2001:db8::1]:4242".parse().unwrap();
        let discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet);
        assert_eq!(
            DiscoveryFormation::endpoint_hint_for(address),
            endpoint_hint(address)
        );
        assert_eq!(discovery.state(), FormationState::Isolated);
    }
}
