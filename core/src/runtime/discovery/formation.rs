use crate::discovery::{
    BootstrapPeer, DiscoveryError, DiscoveryGossipState, FormationState, MAX_ADVERTISEMENT_PEERS,
    MAX_ADVERTISEMENT_RESOURCES, MAX_ADVERTISEMENT_SERVICES, NetworkFormationEngine, PeerRecord,
    SignedDiscoveryAdvertisement,
};
use crate::identity::device::{DeviceId, DeviceSigningKey};
use crate::network::NodeId;
use crate::physical::PhysicalTransport;

use std::net::SocketAddr;

pub const DEFAULT_DISCOVERY_GC_INTERVAL_SECONDS: u64 = 5;
pub const DISCOVERY_ADVERTISEMENT_INTERVAL_SECONDS: u64 = 10;
const DISCOVERY_ADVERTISEMENT_TTL_SECONDS: u64 = 20;

pub struct DiscoveryFormation {
    engine: NetworkFormationEngine,
    gossip: DiscoveryGossipState,
    transport: PhysicalTransport,
    signing_key: DeviceSigningKey,
    last_gc: u64,
    sequence: u64,
}

impl DiscoveryFormation {
    pub fn new(transport: PhysicalTransport, signing_key: DeviceSigningKey) -> Self {
        Self {
            engine: NetworkFormationEngine::new(),
            gossip: DiscoveryGossipState::new(),
            transport,
            signing_key,
            last_gc: 0,
            sequence: 0,
        }
    }

    pub fn with_engine(
        transport: PhysicalTransport,
        signing_key: DeviceSigningKey,
        engine: NetworkFormationEngine,
    ) -> Self {
        Self {
            engine,
            gossip: DiscoveryGossipState::new(),
            transport,
            signing_key,
            last_gc: 0,
            sequence: 0,
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
    pub fn gossip_origin_count(&self) -> usize {
        self.gossip.origin_count()
    }
    pub fn bootstrap_peers(&self) -> &[BootstrapPeer] {
        self.engine.bootstrap_peers()
    }

    pub fn add_bootstrap(
        &mut self,
        node_id: NodeId,
        device_id: DeviceId,
        address: SocketAddr,
    ) -> Result<(), DiscoveryError> {
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
        self.engine.observe_peer(
            PeerRecord::new(node_id, device_id, self.transport, now),
            now,
        )
    }

    pub fn accept_advertisement(
        &mut self,
        advertisement: &SignedDiscoveryAdvertisement,
        now: u64,
    ) -> Result<bool, DiscoveryError> {
        let was_partitioned = self.state() == FormationState::Partitioned;
        let changed = self.gossip.accept(&mut self.engine, advertisement, now)?;
        if was_partitioned && changed {
            self.engine.recover_partition(now);
        }
        Ok(changed)
    }

    pub fn create_advertisement(
        &mut self,
        node_id: NodeId,
        now: u64,
    ) -> Result<SignedDiscoveryAdvertisement, DiscoveryError> {
        let sequence = self.sequence.saturating_add(1);
        let mut peers = Vec::with_capacity(MAX_ADVERTISEMENT_PEERS);
        peers.push(PeerRecord::new(
            node_id,
            self.signing_key.device_id(),
            self.transport,
            now,
        ));
        peers.extend(
            self.engine
                .peers(now)
                .filter(|peer| peer.node_id != node_id)
                .take(MAX_ADVERTISEMENT_PEERS.saturating_sub(1))
                .copied(),
        );
        let services: Vec<_> = self
            .engine
            .services(now)
            .take(MAX_ADVERTISEMENT_SERVICES)
            .cloned()
            .collect();
        let resources: Vec<_> = self
            .engine
            .resources(now)
            .take(MAX_ADVERTISEMENT_RESOURCES)
            .cloned()
            .collect();
        let advertisement = SignedDiscoveryAdvertisement::new(
            &self.signing_key,
            node_id,
            self.signing_key.device_id(),
            sequence,
            now,
            now.saturating_add(DISCOVERY_ADVERTISEMENT_TTL_SECONDS),
            &peers,
            &services,
            &resources,
        )
        .map_err(|_| DiscoveryError::InvalidAdvertisement)?;
        self.sequence = sequence;
        Ok(advertisement)
    }

    pub fn maintain(&mut self, now: u64) -> usize {
        if now.saturating_sub(self.last_gc) < DEFAULT_DISCOVERY_GC_INTERVAL_SECONDS {
            return 0;
        }
        self.last_gc = now;
        let was_connected = self.state() == FormationState::Connected;
        let removed = self.engine.expire(now);
        if was_connected && self.engine.peer_count() == 0 {
            self.engine.mark_partitioned();
        }
        removed
    }

    pub fn force_expire(&mut self, now: u64) -> usize {
        self.last_gc = now;
        let was_connected = self.state() == FormationState::Connected;
        let removed = self.engine.expire(now);
        if was_connected && self.engine.peer_count() == 0 {
            self.engine.mark_partitioned();
        }
        removed
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
            let flow = value.flowinfo().to_be_bytes();
            hint[19..23].copy_from_slice(&flow);
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
    fn key() -> DeviceSigningKey {
        DeviceSigningKey::generate().unwrap()
    }

    #[test]
    fn bootstrap_is_registered_and_forms_network() {
        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet, key());
        discovery
            .add_bootstrap(node(1), device(1), "127.0.0.1:4000".parse().unwrap())
            .unwrap();
        assert_eq!(discovery.state(), FormationState::Forming);
        assert_eq!(discovery.bootstrap_peers().len(), 1);
    }

    #[test]
    fn observed_peer_transitions_to_connected() {
        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet, key());
        assert!(discovery.observe_peer(node(1), device(1), 10).unwrap());
        assert_eq!(discovery.state(), FormationState::Connected);
    }

    #[test]
    fn advertisement_is_self_describing() {
        let signing_key = key();
        let device = crate::identity::device::DeviceState::from_signing_key(
            &signing_key,
            0,
            Default::default(),
        );
        let node = NodeId::from_device(&device);
        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet, signing_key);
        let advertisement = discovery.create_advertisement(node, 100).unwrap();
        assert_eq!(advertisement.origin, node);
        assert_eq!(advertisement.device_id, device.device_id);
        assert_eq!(advertisement.sequence, 1);
        assert_eq!(advertisement.peer_records().count(), 1);
    }

    #[test]
    fn advertisement_sequences_increment() {
        let signing_key = key();
        let device = crate::identity::device::DeviceState::from_signing_key(
            &signing_key,
            0,
            Default::default(),
        );
        let node = NodeId::from_device(&device);
        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet, signing_key);
        assert_eq!(
            discovery.create_advertisement(node, 100).unwrap().sequence,
            1
        );
        assert_eq!(
            discovery.create_advertisement(node, 101).unwrap().sequence,
            2
        );
    }

    #[test]
    fn partition_is_detected_and_recovered_by_valid_gossip() {
        let local_key = key();
        let local_device = crate::identity::device::DeviceState::from_signing_key(
            &local_key,
            0,
            Default::default(),
        );
        let local_node = NodeId::from_device(&local_device);
        let remote_key = key();
        let remote_device = crate::identity::device::DeviceState::from_signing_key(
            &remote_key,
            0,
            Default::default(),
        );
        let remote_node = NodeId::from_device(&remote_device);

        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet, local_key);
        let initial = SignedDiscoveryAdvertisement::new(
            &remote_key,
            remote_node,
            remote_device.device_id,
            1,
            100,
            220,
            &[PeerRecord::new(remote_node, remote_device.device_id, PhysicalTransport::Ethernet, 100)],
            &[],
            &[],
        )
        .unwrap();
        discovery.accept_advertisement(&initial, 110).unwrap();
        assert_eq!(discovery.state(), FormationState::Connected);
        let generation = discovery.generation();
        discovery.force_expire(300);
        assert_eq!(discovery.state(), FormationState::Partitioned);
        assert!(discovery.generation() > generation);

        let recovery = SignedDiscoveryAdvertisement::new(
            &remote_key,
            remote_node,
            remote_device.device_id,
            2,
            301,
            360,
            &[PeerRecord::new(remote_node, remote_device.device_id, PhysicalTransport::Ethernet, 301)],
            &[],
            &[],
        )
        .unwrap();
        assert!(discovery.accept_advertisement(&recovery, 310).unwrap());
        assert_eq!(discovery.state(), FormationState::Connected);
    }

    #[test]
    fn maintenance_is_rate_limited_and_collects_expired_state() {
        let mut discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet, key());
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
        let discovery = DiscoveryFormation::new(PhysicalTransport::Ethernet, key());
        assert_eq!(
            DiscoveryFormation::endpoint_hint_for(address),
            endpoint_hint(address)
        );
        assert_eq!(discovery.state(), FormationState::Isolated);
    }
}
