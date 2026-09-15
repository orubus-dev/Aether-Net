use crate::identity::{sign, verify_bytes};
use crate::network::{AetherAddress, AetherL3Engine, L3Route, NodeId, TrafficPriority};
use crate::route::{
    DEFAULT_ROUTE_TTL_SECONDS, DistributedRoutingEngine, RouteAdvertisement, RouteMetrics,
    RouteNeighbor, RoutingError, ROUTING_PUBLIC_KEY_LENGTH,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::collections::HashMap;

pub const DEFAULT_MAX_PEERS: usize = 65_536;
pub const DEFAULT_MAX_DYNAMIC_ROUTES: usize = 16_384;
pub const DEFAULT_MAX_NEIGHBORS: usize = 256;
pub const ROUTING_SIGNATURE_LENGTH: usize = 64;
pub const ROUTING_ADVERTISEMENT_MAGIC: [u8; 8] = *b"AETHRTE1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingControlError {
    PeerTableFull,
    PeerUnknown,
    OriginKeyMismatch,
    InvalidPublicKey,
    InvalidSignature,
    InvalidAdvertisement,
    AdvertisementTooLarge,
    Routing(RoutingError),
    L3TableFull,
}

impl From<RoutingError> for RoutingControlError {
    fn from(value: RoutingError) -> Self {
        Self::Routing(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingPeer {
    pub node_id: NodeId,
    pub address: AetherAddress,
    pub public_key: [u8; ROUTING_PUBLIC_KEY_LENGTH],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedRouteAdvertisement {
    pub advertisement: RouteAdvertisement,
    pub public_key: [u8; ROUTING_PUBLIC_KEY_LENGTH],
    pub signature: [u8; ROUTING_SIGNATURE_LENGTH],
}

impl SignedRouteAdvertisement {
    pub fn sign(advertisement: RouteAdvertisement, signing_key: &SigningKey) -> Self {
        let public_key = signing_key.verifying_key().to_bytes();
        let signature = sign(signing_key, &canonical_advertisement(&advertisement)).to_bytes();
        Self {
            advertisement,
            public_key,
            signature,
        }
    }

    pub fn verify(&self) -> bool {
        let Ok(key) = VerifyingKey::from_bytes(&self.public_key) else {
            return false;
        };
        verify_bytes(
            &key,
            &canonical_advertisement(&self.advertisement),
            &self.signature,
        )
    }
}

pub struct RoutingControlPlane {
    local_node: NodeId,
    local_address: AetherAddress,
    max_neighbors: usize,
    max_dynamic_routes: usize,
    max_peers: usize,
    peers: HashMap<NodeId, RoutingPeer>,
    distributed: DistributedRoutingEngine,
    l3: AetherL3Engine,
    dynamic_destinations: Vec<AetherAddress>,
    next_sequence: u64,
}

impl RoutingControlPlane {
    pub fn new(local_node: NodeId, local_address: AetherAddress) -> Self {
        Self::with_limits(
            local_node,
            local_address,
            DEFAULT_MAX_PEERS,
            DEFAULT_MAX_DYNAMIC_ROUTES,
            DEFAULT_MAX_NEIGHBORS,
        )
    }

    pub fn with_limits(
        local_node: NodeId,
        local_address: AetherAddress,
        max_peers: usize,
        max_dynamic_routes: usize,
        max_neighbors: usize,
    ) -> Self {
        Self {
            local_node,
            local_address,
            max_neighbors: max_neighbors.max(1),
            max_dynamic_routes: max_dynamic_routes.max(1),
            max_peers: max_peers.max(1),
            peers: HashMap::with_capacity(max_peers.min(1024)),
            distributed: DistributedRoutingEngine::new(local_node),
            l3: AetherL3Engine::with_capacity(local_address, max_dynamic_routes.max(1)),
            dynamic_destinations: Vec::with_capacity(max_dynamic_routes.min(1024)),
            next_sequence: 1,
        }
    }

    pub fn local_node(&self) -> NodeId {
        self.local_node
    }

    pub fn local_address(&self) -> AetherAddress {
        self.local_address
    }

    pub fn l3(&self) -> &AetherL3Engine {
        &self.l3
    }

    pub fn l3_mut(&mut self) -> &mut AetherL3Engine {
        &mut self.l3
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn route_count(&self) -> usize {
        self.l3.route_count()
    }

    pub fn peer(&self, node_id: &NodeId) -> Option<RoutingPeer> {
        self.peers.get(node_id).copied()
    }

    pub fn peers(&self) -> impl Iterator<Item = RoutingPeer> + '_ {
        self.peers.values().copied()
    }

    pub fn register_peer(&mut self, peer: RoutingPeer) -> Result<(), RoutingControlError> {
        if peer.node_id == self.local_node
            || peer.address == self.local_address
            || peer.address.is_zero()
        {
            return Err(RoutingControlError::PeerUnknown);
        }
        VerifyingKey::from_bytes(&peer.public_key)
            .map_err(|_| RoutingControlError::InvalidPublicKey)?;
        if !self.peers.contains_key(&peer.node_id) && self.peers.len() >= self.max_peers {
            return Err(RoutingControlError::PeerTableFull);
        }
        self.peers.insert(peer.node_id, peer);
        Ok(())
    }

    pub fn remove_peer(&mut self, node_id: NodeId) -> bool {
        let removed = self.peers.remove(&node_id).is_some();
        if removed {
            self.distributed.remove_origin(node_id);
            if let Some(peer) = self.peers.get(&node_id).copied() {
                self.l3.remove_route(peer.address);
            }
            self.recompute_dynamic_destinations();
        }
        removed
    }

    pub fn local_advertisement(
        &mut self,
        now: u64,
        neighbors: Vec<RouteNeighbor>,
        signing_key: &SigningKey,
    ) -> Result<SignedRouteAdvertisement, RoutingControlError> {
        if neighbors.len() > self.max_neighbors {
            return Err(RoutingControlError::AdvertisementTooLarge);
        }
        if neighbors.iter().any(|neighbor| {
            neighbor.node_id == self.local_node
                || !self.peers.contains_key(&neighbor.node_id)
                || neighbor.metrics.bandwidth_bps == 0
                || neighbor.metrics.packet_loss_ppm > 1_000_000
                || neighbor.metrics.reliability_ppm > 1_000_000
                || neighbor.metrics.hop_count == 0
        }) {
            return Err(RoutingControlError::PeerUnknown);
        }
        if signing_key.verifying_key().to_bytes() != self.local_public_key_hint() {
            return Err(RoutingControlError::InvalidPublicKey);
        }
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        Ok(SignedRouteAdvertisement::sign(
            RouteAdvertisement::new(self.local_node, sequence, now, neighbors),
            signing_key,
        ))
    }

    pub fn accept_advertisement(
        &mut self,
        signed: SignedRouteAdvertisement,
        now: u64,
    ) -> Result<bool, RoutingControlError> {
        let peer = self
            .peers
            .get(&signed.advertisement.origin)
            .ok_or(RoutingControlError::PeerUnknown)?;
        if peer.public_key != signed.public_key {
            return Err(RoutingControlError::OriginKeyMismatch);
        }
        if signed.advertisement.origin == self.local_node
            || !signed.advertisement.is_valid_at(now)
            || signed.advertisement.expires_at <= signed.advertisement.issued_at
            || signed.advertisement.neighbors.len() > self.max_neighbors
        {
            return Err(RoutingControlError::InvalidAdvertisement);
        }
        if !signed.verify() {
            return Err(RoutingControlError::InvalidSignature);
        }
        Ok(self
            .distributed
            .install_advertisement(signed.advertisement, now)?)
    }

    pub fn install_local_neighbors(
        &mut self,
        sequence: u64,
        now: u64,
        neighbors: Vec<RouteNeighbor>,
    ) -> Result<bool, RoutingControlError> {
        if neighbors.len() > self.max_neighbors {
            return Err(RoutingControlError::AdvertisementTooLarge);
        }
        if neighbors.iter().any(|neighbor| {
            neighbor.node_id == self.local_node
                || !self.peers.contains_key(&neighbor.node_id)
        }) {
            return Err(RoutingControlError::PeerUnknown);
        }
        Ok(self.distributed.install_advertisement(
            RouteAdvertisement::with_expiry(
                self.local_node,
                sequence,
                now,
                now.saturating_add(DEFAULT_ROUTE_TTL_SECONDS),
                neighbors,
            ),
            now,
        )?)
    }

    pub fn recompute(
        &mut self,
        priority: TrafficPriority,
        now: u64,
    ) -> Result<usize, RoutingControlError> {
        self.distributed.expire(now);
        self.recompute_dynamic_destinations();
        let peers: Vec<RoutingPeer> = self.peers.values().copied().collect();
        let mut installed = 0usize;
        for destination_peer in peers {
            if installed >= self.max_dynamic_routes {
                break;
            }
            let Ok(path) = self
                .distributed
                .best_route(destination_peer.node_id, priority, now)
            else {
                continue;
            };
            let Some(next_hop_node) = path.nodes.get(1).copied() else {
                continue;
            };
            let Some(next_hop_peer) = self.peers.get(&next_hop_node).copied() else {
                continue;
            };
            let metric = l3_metric(path.metrics, priority);
            self.l3
                .install_route(L3Route::new(
                    destination_peer.address,
                    next_hop_peer.address,
                    metric,
                    now,
                    Some(now.saturating_add(DEFAULT_ROUTE_TTL_SECONDS)),
                ))
                .map_err(|_| RoutingControlError::L3TableFull)?;
            self.dynamic_destinations.push(destination_peer.address);
            installed += 1;
        }
        Ok(installed)
    }

    pub fn tick(
        &mut self,
        priority: TrafficPriority,
        now: u64,
    ) -> Result<usize, RoutingControlError> {
        self.distributed.expire(now);
        self.l3.expire_routes(now);
        self.recompute(priority, now)
    }

    fn recompute_dynamic_destinations(&mut self) {
        for destination in self.dynamic_destinations.drain(..) {
            self.l3.remove_route(destination);
        }
    }

    fn local_public_key_hint(&self) -> [u8; ROUTING_PUBLIC_KEY_LENGTH] {
        [0u8; ROUTING_PUBLIC_KEY_LENGTH]
    }
}

pub fn canonical_advertisement(ad: &RouteAdvertisement) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(
        ROUTING_ADVERTISEMENT_MAGIC.len() + 32 + 8 + 8 + 8 + 2 + ad.neighbors.len().saturating_mul(88),
    );
    bytes.extend_from_slice(&ROUTING_ADVERTISEMENT_MAGIC);
    bytes.extend_from_slice(ad.origin.as_bytes());
    bytes.extend_from_slice(&ad.sequence.to_le_bytes());
    bytes.extend_from_slice(&ad.issued_at.to_le_bytes());
    bytes.extend_from_slice(&ad.expires_at.to_le_bytes());
    bytes.extend_from_slice(&(ad.neighbors.len() as u16).to_be_bytes());
    for neighbor in &ad.neighbors {
        bytes.extend_from_slice(neighbor.node_id.as_bytes());
        bytes.extend_from_slice(&neighbor.metrics.latency_us.to_be_bytes());
        bytes.extend_from_slice(&neighbor.metrics.jitter_us.to_be_bytes());
        bytes.extend_from_slice(&neighbor.metrics.packet_loss_ppm.to_be_bytes());
        bytes.extend_from_slice(&neighbor.metrics.bandwidth_bps.to_be_bytes());
        bytes.extend_from_slice(&neighbor.metrics.reliability_ppm.to_be_bytes());
        bytes.extend_from_slice(&neighbor.metrics.monetary_cost_micros.to_be_bytes());
        bytes.extend_from_slice(&neighbor.metrics.energy_cost.to_be_bytes());
        bytes.extend_from_slice(&neighbor.metrics.hop_count.to_be_bytes());
    }
    bytes
}

fn l3_metric(metrics: RouteMetrics, priority: TrafficPriority) -> u32 {
    let score = metrics.score(priority).clamp(0.0, 1.0);
    ((1.0 - score) * (u32::MAX.saturating_sub(1) as f64))
        .round()
        .clamp(1.0, u32::MAX as f64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn node(seed: u8) -> NodeId {
        NodeId::from_bytes([seed; 32])
    }

    fn address(seed: u8) -> AetherAddress {
        AetherAddress::from_bytes([seed; 32])
    }

    fn metrics(latency: u64) -> RouteMetrics {
        RouteMetrics {
            latency_us: latency,
            jitter_us: 1,
            packet_loss_ppm: 1000,
            bandwidth_bps: 10_000_000,
            reliability_ppm: 999_000,
            monetary_cost_micros: 1,
            energy_cost: 1,
            hop_count: 1,
        }
    }

    fn peer(seed: u8, key: &SigningKey) -> RoutingPeer {
        RoutingPeer {
            node_id: node(seed),
            address: address(seed),
            public_key: key.verifying_key().to_bytes(),
        }
    }

    #[test]
    fn signed_advertisement_is_authenticatable() {
        let signing_key = key(7);
        let signed = SignedRouteAdvertisement::sign(
            RouteAdvertisement::with_expiry(
                node(7),
                1,
                0,
                100,
                vec![RouteNeighbor {
                    node_id: node(8),
                    metrics: metrics(10),
                }],
            ),
            &signing_key,
        );
        assert!(signed.verify());
    }

    #[test]
    fn forged_advertisement_is_rejected() {
        let trusted = key(2);
        let attacker = key(3);
        let mut plane = RoutingControlPlane::with_limits(node(1), address(1), 8, 8, 8);
        plane.register_peer(peer(2, &trusted)).unwrap();
        let forged = SignedRouteAdvertisement::sign(
            RouteAdvertisement::with_expiry(node(2), 1, 0, 100, vec![]),
            &attacker,
        );
        assert_eq!(
            plane.accept_advertisement(forged, 10).unwrap_err(),
            RoutingControlError::OriginKeyMismatch
        );
    }

    #[test]
    fn peer_capacity_is_enforced() {
        let mut plane = RoutingControlPlane::with_limits(node(1), address(1), 1, 8, 8);
        let k2 = key(2);
        let k3 = key(3);
        plane.register_peer(peer(2, &k2)).unwrap();
        assert_eq!(
            plane.register_peer(peer(3, &k3)).unwrap_err(),
            RoutingControlError::PeerTableFull
        );
    }

    #[test]
    fn authenticated_topology_syncs_to_l3_and_repairs_after_expiry() {
        let b_key = key(2);
        let c_key = key(3);
        let mut plane = RoutingControlPlane::with_limits(node(1), address(1), 8, 8, 8);
        plane.register_peer(peer(2, &b_key)).unwrap();
        plane.register_peer(peer(3, &c_key)).unwrap();
        plane
            .install_local_neighbors(
                1,
                0,
                vec![RouteNeighbor {
                    node_id: node(2),
                    metrics: metrics(10),
                }],
            )
            .unwrap();
        let b_ad = SignedRouteAdvertisement::sign(
            RouteAdvertisement::with_expiry(
                node(2),
                1,
                0,
                100,
                vec![RouteNeighbor {
                    node_id: node(3),
                    metrics: metrics(10),
                }],
            ),
            &b_key,
        );
        plane.accept_advertisement(b_ad, 10).unwrap();
        assert_eq!(plane.recompute(TrafficPriority::Voice, 10).unwrap(), 2);
        assert_eq!(
            plane.l3().lookup_route(address(3), 10).unwrap().next_hop,
            address(2)
        );
        assert_eq!(plane.tick(TrafficPriority::Voice, 101).unwrap(), 0);
        assert!(plane.l3().lookup_route(address(3), 101).is_none());
    }
}
