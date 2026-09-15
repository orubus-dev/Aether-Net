use super::gossip::SignedDiscoveryAdvertisement;
use super::{DiscoveryError, NetworkFormationEngine, PeerRecord};
use crate::network::NodeId;

pub const DEFAULT_MAX_ADVERTISEMENT_ORIGINS: usize = 16_384;

pub struct DiscoveryGossipState {
    seen: Vec<(NodeId, u64)>,
    max_origins: usize,
}
impl DiscoveryGossipState {
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_MAX_ADVERTISEMENT_ORIGINS)
    }
    pub fn with_limit(max_origins: usize) -> Self {
        Self {
            seen: Vec::new(),
            max_origins: max_origins.max(1),
        }
    }
    pub fn origin_count(&self) -> usize {
        self.seen.len()
    }
    pub fn sequence(&self, origin: NodeId) -> Option<u64> {
        self.seen
            .iter()
            .find_map(|(known_origin, sequence)| (*known_origin == origin).then_some(*sequence))
    }
    pub fn accept(
        &mut self,
        engine: &mut NetworkFormationEngine,
        advertisement: &SignedDiscoveryAdvertisement,
        now: u64,
    ) -> Result<bool, DiscoveryError> {
        advertisement
            .verify(now)
            .map_err(|_| DiscoveryError::InvalidAdvertisement)?;
        if let Some((_, known_sequence)) = self
            .seen
            .iter()
            .find(|(origin, _)| *origin == advertisement.origin)
        {
            if advertisement.sequence <= *known_sequence {
                return Ok(false);
            }
        } else if self.seen.len() >= self.max_origins {
            return Err(DiscoveryError::CapacityExceeded);
        }
        for peer in advertisement.peer_records() {
            if peer.node_id == NodeId::from_bytes([0; 32]) || !peer.is_valid_at(now) {
                return Err(DiscoveryError::InvalidAdvertisement);
            }
        }
        for service in advertisement.service_records() {
            if !service.is_valid_at(now) {
                return Err(DiscoveryError::InvalidAdvertisement);
            }
        }
        for resource in advertisement.resource_records() {
            if !resource.is_valid_at(now) {
                return Err(DiscoveryError::InvalidAdvertisement);
            }
        }
        let mut changed = false;
        for peer in advertisement.peer_records() {
            changed |= engine.observe_peer(*peer, now)?;
        }
        for service in advertisement.service_records() {
            changed |= engine.publish_service(service.clone(), now)?;
        }
        for resource in advertisement.resource_records() {
            changed |= engine.publish_resource(resource.clone(), now)?;
        }
        if let Some((_, known_sequence)) = self
            .seen
            .iter_mut()
            .find(|(origin, _)| *origin == advertisement.origin)
        {
            *known_sequence = advertisement.sequence;
        } else {
            self.seen
                .push((advertisement.origin, advertisement.sequence));
        }
        Ok(changed)
    }
}
impl Default for DiscoveryGossipState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::gossip::SignedDiscoveryAdvertisement;
    use crate::identity::device::{DeviceId, DeviceSigningKey};
    use crate::physical::PhysicalTransport;
    fn identity(key: &DeviceSigningKey) -> (NodeId, DeviceId) {
        let verifying = key.verifying_key();
        let device = DeviceId::from_public_key(&verifying);
        let node = NodeId::from_device(&crate::identity::device::DeviceState::from_signing_key(
            key,
            0,
            Default::default(),
        ));
        (node, device)
    }
    fn advertisement(
        key: &DeviceSigningKey,
        node: NodeId,
        device: DeviceId,
        sequence: u64,
    ) -> SignedDiscoveryAdvertisement {
        SignedDiscoveryAdvertisement::new(
            key,
            node,
            device,
            sequence,
            100,
            180,
            &[PeerRecord::new(
                NodeId::from_bytes([7; 32]),
                DeviceId::from_bytes([8; 32]),
                PhysicalTransport::Wifi,
                100,
            )],
            &[],
            &[],
        )
        .unwrap()
    }
    #[test]
    fn sequence_is_monotonic_per_origin() {
        let key = DeviceSigningKey::generate().unwrap();
        let (node, device) = identity(&key);
        let old = advertisement(&key, node, device, 1);
        let new = advertisement(&key, node, device, 2);
        let mut state = DiscoveryGossipState::new();
        let mut engine = NetworkFormationEngine::new();
        assert!(state.accept(&mut engine, &old, 110).unwrap());
        assert!(state.accept(&mut engine, &new, 110).unwrap());
        assert!(!state.accept(&mut engine, &new, 110).unwrap());
        assert_eq!(state.sequence(node), Some(2));
    }
    #[test]
    fn invalid_member_does_not_advance_sequence() {
        let key = DeviceSigningKey::generate().unwrap();
        let (node, device) = identity(&key);
        let mut invalid = advertisement(&key, node, device, 1);
        invalid.peers[0].expires_at = 101;
        let mut state = DiscoveryGossipState::new();
        let mut engine = NetworkFormationEngine::new();
        assert_eq!(
            state.accept(&mut engine, &invalid, 110),
            Err(DiscoveryError::InvalidAdvertisement)
        );
        assert_eq!(state.sequence(node), None);
    }
}
