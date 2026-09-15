use crate::identity::{sign, verify_bytes};
use crate::network::{AetherAddress, AetherL3Engine, NodeId, TrafficPriority};
use crate::route::{DistributedRoutingEngine, RouteAdvertisement, RouteMetrics, RouteNeighbor, RoutingError};
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::collections::HashMap;

pub const ROUTING_WIRE_VERSION: u8 = 1;
pub const ROUTING_SIGNATURE_LENGTH: usize = 64;
pub const ROUTING_PUBLIC_KEY_LENGTH: usize = 32;
pub const DEFAULT_MAX_PEERS: usize = 65_536;
pub const DEFAULT_MAX_DYNAMIC_ROUTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingControlError {
    PeerTableFull,
    PeerUnknown,
    OriginKeyMismatch,
    InvalidSignature,
    InvalidWire,
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

#[derive(Debug, Clone)]
pub struct SignedRouteAdvertisement {
    pub advertisement: RouteAdvertisement,
    pub public_key: [u8; ROUTING_PUBLIC_KEY_LENGTH],
    pub signature: [u8; ROUTING_SIGNATURE_LENGTH],
}

impl SignedRouteAdvertisement {
    pub fn sign(advertisement: RouteAdvertisement, signing_key: &SigningKey) -> Self {
        let public_key = signing_key.verifying_key().to_bytes();
        let signature = sign(signing_key, &canonical_advertisement_bytes(&advertisement)).to_bytes();
        Self { advertisement, public_key, signature }
    }

    pub fn verify(&self) -> bool {
        let Ok(public_key) = VerifyingKey::from_bytes(&self.public_key) else { return false; };
        verify_bytes(&public_key, &canonical_advertisement_bytes(&self.advertisement), &self.signature)
    }

    pub fn encode(&self, max_neighbors: usize) -> Result<Vec<u8>, RoutingControlError> {
        if self.advertisement.neighbors.len() > max_neighbors || self.advertisement.neighbors.len() > u16::MAX as usize {
            return Err(RoutingControlError::AdvertisementTooLarge);
        }
        let mut out = canonical_advertisement_bytes(&self.advertisement);
        out.extend_from_slice(&self.public_key);
        out.extend_from_slice(&self.signature);
        Ok(out)
    }

    pub fn decode(bytes: &[u8], max_neighbors: usize) -> Result<Self, RoutingControlError> {
        const FIXED: usize = 1 + 32 + 8 + 8 + 8 + 2 + 32 + 64;
        if bytes.len() < FIXED { return Err(RoutingControlError::InvalidWire); }
        if bytes[0] != ROUTING_WIRE_VERSION { return Err(RoutingControlError::InvalidWire); }

        let mut cursor = 1usize;
        let origin = read_node_id(bytes, &mut cursor)?;
        let sequence = read_u64(bytes, &mut cursor)?;
        let issued_at = read_u64(bytes, &mut cursor)?;
        let expires_at = read_u64(bytes, &mut cursor)?;
        let count = read_u16(bytes, &mut cursor)? as usize;
        if count > max_neighbors { return Err(RoutingControlError::AdvertisementTooLarge); }

        let neighbor_bytes = count.checked_mul(32 + 8 + 8 + 4 + 8 + 4 + 8 + 4 + 2).ok_or(RoutingControlError::InvalidWire)?;
        let expected = 1usize
            .checked_add(32 + 8 + 8 + 8 + 2)
            .and_then(|v| v.checked_add(neighbor_bytes))
            .and_then(|v| v.checked_add(ROUTING_PUBLIC_KEY_LENGTH + ROUTING_SIGNATURE_LENGTH))
            .ok_or(RoutingControlError::InvalidWire)?;
        if bytes.len() != expected { return Err(RoutingControlError::InvalidWire); }

        let mut neighbors = Vec::with_capacity(count);
        for _ in 0..count {
            let node_id = read_node_id(bytes, &mut cursor)?;
            let metrics = RouteMetrics {
                latency_us: read_u64(bytes, &mut cursor)?,
                jitter_us: read_u64(bytes, &mut cursor)?,
                packet_loss_ppm: read_u32(bytes, &mut cursor)?,
                bandwidth_bps: read_u64(bytes, &mut cursor)?,
                reliability_ppm: read_u32(bytes, &mut cursor)?,
                monetary_cost_micros: read_u64(bytes, &mut cursor)?,
                energy_cost: read_u32(bytes, &mut cursor)?,
                hop_count: read_u16(bytes, &mut cursor)?,
            };
            neighbors.push(RouteNeighbor { node_id, metrics });
        }

        let public_key = read_array_32(bytes, &mut cursor)?;
        let signature = read_array_64(bytes, &mut cursor)?;
        Ok(Self {
            advertisement: RouteAdvertisement::with_expiry(origin, sequence, issued_at, expires_at, neighbors),
            public_key,
            signature,
        })
    }
}

pub struct RoutingControlPlane {
    local_node: NodeId,
    local_address: AetherAddress,
    max_peers: usize,
    max_dynamic_routes: usize,
    peers: HashMap<NodeId, RoutingPeer>,
    distributed: DistributedRoutingEngine,
    l3: AetherL3Engine,
    dynamic_destinations: Vec<AetherAddress>,
    next_sequence: u64,
}

impl RoutingControlPlane {
    pub fn new(local_node: NodeId, local_address: AetherAddress) -> Self {
        Self::with_limits(local_node, local_address, DEFAULT_MAX_PEERS, DEFAULT_MAX_DYNAMIC_ROUTES)
    }

    pub fn with_limits(local_node: NodeId, local_address: AetherAddress, max_peers: usize, max_dynamic_routes: usize) -> Self {
        Self {
            local_node,
            local_address,
            max_peers: max_peers.max(1),
            max_dynamic_routes: max_dynamic_routes.max(1),
            peers: HashMap::new(),
            distributed: DistributedRoutingEngine::new(local_node),
            l3: AetherL3Engine::with_capacity(local_address, max_dynamic_routes.max(1)),
            dynamic_destinations: Vec::new(),
            next_sequence: 1,
        }
    }

    pub fn local_node(&self) -> NodeId { self.local_node }
    pub fn local_address(&self) -> AetherAddress { self.local_address }
    pub fn l3(&self) -> &AetherL3Engine { &self.l3 }
    pub fn l3_mut(&mut self) -> &mut AetherL3Engine { &mut self.l3 }
    pub fn peer_count(&self) -> usize { self.peers.len() }
    pub fn route_count(&self) -> usize { self.l3.route_count() }

    pub fn register_peer(
        &mut self,
        node_id: NodeId,
        address: AetherAddress,
        public_key: [u8; ROUTING_PUBLIC_KEY_LENGTH],
    ) -> Result<(), RoutingControlError> {
        if let Some(peer) = self.peers.get_mut(&node_id) {
            peer.address = address;
            peer.public_key = public_key;
            return Ok(());
        }
        if self.peers.len() >= self.max_peers { return Err(RoutingControlError::PeerTableFull); }
        self.peers.insert(node_id, RoutingPeer { node_id, address, public_key });
        Ok(())
    }

    pub fn remove_peer(&mut self, node_id: NodeId) -> bool {
        let removed = self.peers.remove(&node_id).is_some();
        if removed {
            if let Some(address) = self.dynamic_destinations.iter().copied().find(|address| {
                self.peers.values().any(|peer| peer.address == *address)
            }) {
                self.l3.remove_route(address);
            }
        }
        removed
    }

    pub fn local_advertisement(
        &mut self,
        now: u64,
        neighbors: Vec<RouteNeighbor>,
        signing_key: &SigningKey,
    ) -> Result<SignedRouteAdvertisement, RoutingControlError> {
        if neighbors.len() > 256 { return Err(RoutingControlError::AdvertisementTooLarge); }
        for neighbor in &neighbors {
            if !self.peers.contains_key(&neighbor.node_id) { return Err(RoutingControlError::PeerUnknown); }
        }
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1).max(sequence.saturating_add(1));
        Ok(SignedRouteAdvertisement::sign(RouteAdvertisement::new(self.local_node, sequence, now, neighbors), signing_key))
    }

    pub fn accept_advertisement(
        &mut self,
        signed: SignedRouteAdvertisement,
        now: u64,
    ) -> Result<bool, RoutingControlError> {
        let peer = self.peers.get(&signed.advertisement.origin).ok_or(RoutingControlError::PeerUnknown)?;
        if peer.public_key != signed.public_key { return Err(RoutingControlError::OriginKeyMismatch); }
        if !signed.verify() { return Err(RoutingControlError::InvalidSignature); }
        Ok(self.distributed.install_advertisement(signed.advertisement, now)?)
    }

    pub fn recompute(&mut self, priority: TrafficPriority, now: u64) -> Result<usize, RoutingControlError> {
        let peers: Vec<RoutingPeer> = self.peers.values().copied().collect();
        let destinations: Vec<AetherAddress> = peers.iter().map(|peer| peer.address).collect();
        for destination in self.dynamic_destinations.drain(..) {
            self.l3.remove_route(destination);
        }
        self.distributed.expire(now);

        let mut installed = 0usize;
        for peer in peers {
            if peer.address == self.local_address { continue; }
            let Ok(path) = self.distributed.best_route(peer.node_id, priority, now) else { continue; };
            let Some(next_hop_node) = path.nodes.get(1).copied() else { continue; };
            let Some(next_hop_peer) = self.peers.get(&next_hop_node).copied() else { continue; };
            if installed >= self.max_dynamic_routes { break; }
            let metric = route_metric(path.metrics, priority);
            self.l3.install_route(crate::network::L3Route::new(peer.address, next_hop_peer.address, metric, now, Some(now.saturating_add(crate::route::DEFAULT_ROUTE_TTL_SECONDS))))
                .map_err(|_| RoutingControlError::L3TableFull)?;
            self.dynamic_destinations.push(peer.address);
            installed += 1;
        }
        debug_assert_eq!(destinations.len(), self.peers.len());
        Ok(installed)
    }

    pub fn tick(&mut self, priority: TrafficPriority, now: u64) -> Result<usize, RoutingControlError> {
        self.distributed.expire(now);
        self.l3.expire_routes(now);
        self.recompute(priority, now)
    }
}

fn canonical_advertisement_bytes(ad: &RouteAdvertisement) -> Vec<u8> {
    let count = ad.neighbors.len().min(u16::MAX as usize);
    let mut out = Vec::with_capacity(1 + 32 + 24 + 2 + count.saturating_mul(82));
    out.push(ROUTING_WIRE_VERSION);
    out.extend_from_slice(ad.origin.as_bytes());
    out.extend_from_slice(&ad.sequence.to_le_bytes());
    out.extend_from_slice(&ad.issued_at.to_le_bytes());
    out.extend_from_slice(&ad.expires_at.to_le_bytes());
    out.extend_from_slice(&(count as u16).to_le_bytes());
    for neighbor in ad.neighbors.iter().take(count) {
        out.extend_from_slice(neighbor.node_id.as_bytes());
        out.extend_from_slice(&neighbor.metrics.latency_us.to_le_bytes());
        out.extend_from_slice(&neighbor.metrics.jitter_us.to_le_bytes());
        out.extend_from_slice(&neighbor.metrics.packet_loss_ppm.to_le_bytes());
        out.extend_from_slice(&neighbor.metrics.bandwidth_bps.to_le_bytes());
        out.extend_from_slice(&neighbor.metrics.reliability_ppm.to_le_bytes());
        out.extend_from_slice(&neighbor.metrics.monetary_cost_micros.to_le_bytes());
        out.extend_from_slice(&neighbor.metrics.energy_cost.to_le_bytes());
        out.extend_from_slice(&neighbor.metrics.hop_count.to_le_bytes());
    }
    out
}

fn route_metric(metrics: RouteMetrics, priority: TrafficPriority) -> u32 {
    let score = metrics.score(priority).clamp(0.0, 1.0);
    let inverse = (1.0 - score) * (u32::MAX.saturating_sub(1) as f64);
    inverse.round().clamp(1.0, u32::MAX as f64) as u32
}

fn read_node_id(bytes: &[u8], cursor: &mut usize) -> Result<NodeId, RoutingControlError> {
    Ok(NodeId::from_bytes(read_array_32(bytes, cursor)?))
}

fn read_array_32(bytes: &[u8], cursor: &mut usize) -> Result<[u8; 32], RoutingControlError> {
    let end = cursor.checked_add(32).ok_or(RoutingControlError::InvalidWire)?;
    if end > bytes.len() { return Err(RoutingControlError::InvalidWire); }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(out)
}

fn read_array_64(bytes: &[u8], cursor: &mut usize) -> Result<[u8; 64], RoutingControlError> {
    let end = cursor.checked_add(64).ok_or(RoutingControlError::InvalidWire)?;
    if end > bytes.len() { return Err(RoutingControlError::InvalidWire); }
    let mut out = [0u8; 64];
    out.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(out)
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, RoutingControlError> {
    let end = cursor.checked_add(2).ok_or(RoutingControlError::InvalidWire)?;
    if end > bytes.len() { return Err(RoutingControlError::InvalidWire); }
    let value = u16::from_le_bytes([bytes[*cursor], bytes[*cursor + 1]]);
    *cursor = end;
    Ok(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, RoutingControlError> {
    let end = cursor.checked_add(4).ok_or(RoutingControlError::InvalidWire)?;
    if end > bytes.len() { return Err(RoutingControlError::InvalidWire); }
    let mut array = [0u8; 4];
    array.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(u32::from_le_bytes(array))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, RoutingControlError> {
    let end = cursor.checked_add(8).ok_or(RoutingControlError::InvalidWire)?;
    if end > bytes.len() { return Err(RoutingControlError::InvalidWire); }
    let mut array = [0u8; 8];
    array.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(u64::from_le_bytes(array))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::AETHER_ADDRESS_LENGTH;
    use rand_core::OsRng;

    fn node(seed: u8) -> NodeId { NodeId::from_bytes([seed; 32]) }
    fn address(seed: u8) -> AetherAddress { AetherAddress::from_bytes([seed; AETHER_ADDRESS_LENGTH]) }
    fn metrics() -> RouteMetrics {
        RouteMetrics { latency_us: 10, jitter_us: 2, packet_loss_ppm: 1000, bandwidth_bps: 10_000_000, reliability_ppm: 999_000, monetary_cost_micros: 1, energy_cost: 1, hop_count: 1 }
    }

    #[test]
    fn signed_advertisement_round_trips_and_verifies() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let ad = RouteAdvertisement::with_expiry(node(1), 7, 10, 100, vec![RouteNeighbor { node_id: node(2), metrics: metrics() }]);
        let signed = SignedRouteAdvertisement::sign(ad.clone(), &signing_key);
        let bytes = signed.encode(8).unwrap();
        let decoded = SignedRouteAdvertisement::decode(&bytes, 8).unwrap();
        assert_eq!(decoded.advertisement.sequence, 7);
        assert!(decoded.verify());
    }

    #[test]
    fn forged_key_is_rejected_by_peer_binding() {
        let trusted = SigningKey::generate(&mut OsRng);
        let attacker = SigningKey::generate(&mut OsRng);
        let mut plane = RoutingControlPlane::with_limits(node(1), address(1), 8, 8);
        plane.register_peer(node(2), address(2), trusted.verifying_key().to_bytes()).unwrap();
        let signed = SignedRouteAdvertisement::sign(RouteAdvertisement::with_expiry(node(2), 1, 0, 100, vec![]), &attacker);
        assert_eq!(plane.accept_advertisement(signed, 10).unwrap_err(), RoutingControlError::OriginKeyMismatch);
    }

    #[test]
    fn authenticated_route_syncs_into_l3() {
        let local = SigningKey::generate(&mut OsRng);
        let peer_b = SigningKey::generate(&mut OsRng);
        let peer_c = SigningKey::generate(&mut OsRng);
        let mut plane = RoutingControlPlane::with_limits(node(1), address(1), 8, 8);
        plane.register_peer(node(2), address(2), peer_b.verifying_key().to_bytes()).unwrap();
        plane.register_peer(node(3), address(3), peer_c.verifying_key().to_bytes()).unwrap();
        let ad_b = SignedRouteAdvertisement::sign(RouteAdvertisement::with_expiry(node(2), 1, 0, 100, vec![RouteNeighbor { node_id: node(3), metrics: metrics() }]), &peer_b);
        let ad_local = SignedRouteAdvertisement::sign(RouteAdvertisement::with_expiry(node(1), 1, 0, 100, vec![RouteNeighbor { node_id: node(2), metrics: metrics() }]), &local);
        plane.accept_advertisement(ad_b, 10).unwrap();
        plane.register_peer(node(1), address(1), local.verifying_key().to_bytes()).unwrap();
        plane.accept_advertisement(ad_local, 10).unwrap();
        assert_eq!(plane.recompute(TrafficPriority::Interactive, 10).unwrap(), 2);
        let route = plane.l3().lookup_route(address(3), 10).unwrap();
        assert_eq!(route.next_hop, address(2));
    }
}
