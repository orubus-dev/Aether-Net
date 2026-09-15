use crate::identity::device::DeviceId;
use crate::network::NodeId;
use crate::physical::PhysicalTransport;

pub const DEFAULT_MAX_PEERS: usize = 16_384;
pub const DEFAULT_MAX_SERVICES: usize = 16_384;
pub const DEFAULT_MAX_RESOURCES: usize = 65_536;
pub const DEFAULT_MAX_BOOTSTRAP: usize = 64;
pub const DEFAULT_RECORD_TTL_SECONDS: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryKind { Peer, Service, Resource }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryError { CapacityExceeded, InvalidRecord, Expired, Duplicate }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapPeer {
    pub node_id: NodeId,
    pub device_id: DeviceId,
    pub transport: PhysicalTransport,
    pub endpoint_hint: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerRecord {
    pub node_id: NodeId,
    pub device_id: DeviceId,
    pub transport: PhysicalTransport,
    pub observed_at: u64,
    pub expires_at: u64,
}

impl PeerRecord {
    pub fn new(node_id: NodeId, device_id: DeviceId, transport: PhysicalTransport, now: u64) -> Self {
        Self { node_id, device_id, transport, observed_at: now, expires_at: now.saturating_add(DEFAULT_RECORD_TTL_SECONDS) }
    }
    pub fn is_valid_at(&self, now: u64) -> bool { now < self.expires_at && self.expires_at >= self.observed_at }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRecord {
    pub service_id: [u8; 16],
    pub owner: NodeId,
    pub kind: u8,
    pub version: u16,
    pub observed_at: u64,
    pub expires_at: u64,
}
impl ServiceRecord { pub fn is_valid_at(&self, now: u64) -> bool { now < self.expires_at && self.expires_at >= self.observed_at } }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRecord {
    pub resource_id: [u8; 32],
    pub provider: NodeId,
    pub kind: u8,
    pub version: u64,
    pub observed_at: u64,
    pub expires_at: u64,
}
impl ResourceRecord { pub fn is_valid_at(&self, now: u64) -> bool { now < self.expires_at && self.expires_at >= self.observed_at } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationState { Isolated, Forming, Connected, Partitioned }

pub struct NetworkFormationEngine {
    state: FormationState,
    max_peers: usize,
    max_services: usize,
    max_resources: usize,
    max_bootstrap: usize,
    bootstrap: Vec<BootstrapPeer>,
    peers: Vec<PeerRecord>,
    services: Vec<ServiceRecord>,
    resources: Vec<ResourceRecord>,
    generation: u64,
}

impl NetworkFormationEngine {
    pub fn new() -> Self { Self::with_limits(DEFAULT_MAX_PEERS, DEFAULT_MAX_SERVICES, DEFAULT_MAX_RESOURCES, DEFAULT_MAX_BOOTSTRAP) }
    pub fn with_limits(max_peers: usize, max_services: usize, max_resources: usize, max_bootstrap: usize) -> Self {
        Self { state: FormationState::Isolated, max_peers: max_peers.max(1), max_services: max_services.max(1), max_resources: max_resources.max(1), max_bootstrap: max_bootstrap.max(1), bootstrap: Vec::new(), peers: Vec::new(), services: Vec::new(), resources: Vec::new(), generation: 0 }
    }
    pub fn state(&self) -> FormationState { self.state }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn bootstrap_peers(&self) -> &[BootstrapPeer] { &self.bootstrap }
    pub fn peer_count(&self) -> usize { self.peers.len() }
    pub fn service_count(&self) -> usize { self.services.len() }
    pub fn resource_count(&self) -> usize { self.resources.len() }

    pub fn add_bootstrap(&mut self, peer: BootstrapPeer) -> Result<(), DiscoveryError> {
        if self.bootstrap.iter().any(|existing| existing.node_id == peer.node_id) { return Err(DiscoveryError::Duplicate); }
        if self.bootstrap.len() >= self.max_bootstrap { return Err(DiscoveryError::CapacityExceeded); }
        self.bootstrap.push(peer);
        if self.state == FormationState::Isolated { self.state = FormationState::Forming; }
        Ok(())
    }

    pub fn observe_peer(&mut self, record: PeerRecord, now: u64) -> Result<bool, DiscoveryError> {
        if record.node_id == NodeId::from_bytes([0; 32]) || !record.is_valid_at(now) { return Err(DiscoveryError::InvalidRecord); }
        if let Some(existing) = self.peers.iter_mut().find(|peer| peer.node_id == record.node_id) {
            if record.expires_at >= existing.expires_at || record.observed_at >= existing.observed_at {
                *existing = record;
                self.state = FormationState::Connected;
                return Ok(true);
            }
            return Ok(false);
        }
        if self.peers.len() >= self.max_peers { return Err(DiscoveryError::CapacityExceeded); }
        self.peers.push(record);
        self.state = FormationState::Connected;
        Ok(true)
    }

    pub fn publish_service(&mut self, record: ServiceRecord, now: u64) -> Result<bool, DiscoveryError> {
        validate_interval(record.observed_at, record.expires_at, now)?;
        if let Some(existing) = self.services.iter_mut().find(|service| service.service_id == record.service_id && service.owner == record.owner) {
            if record.version >= existing.version { *existing = record; return Ok(true); }
            return Ok(false);
        }
        if self.services.len() >= self.max_services { return Err(DiscoveryError::CapacityExceeded); }
        self.services.push(record); Ok(true)
    }

    pub fn publish_resource(&mut self, record: ResourceRecord, now: u64) -> Result<bool, DiscoveryError> {
        validate_interval(record.observed_at, record.expires_at, now)?;
        if let Some(existing) = self.resources.iter_mut().find(|resource| resource.resource_id == record.resource_id) {
            if record.version >= existing.version { *existing = record; return Ok(true); }
            return Ok(false);
        }
        if self.resources.len() >= self.max_resources { return Err(DiscoveryError::CapacityExceeded); }
        self.resources.push(record); Ok(true)
    }

    pub fn peers(&self, now: u64) -> impl Iterator<Item = &PeerRecord> { self.peers.iter().filter(move |r| r.is_valid_at(now)) }
    pub fn services(&self, now: u64) -> impl Iterator<Item = &ServiceRecord> { self.services.iter().filter(move |r| r.is_valid_at(now)) }
    pub fn resources(&self, now: u64) -> impl Iterator<Item = &ResourceRecord> { self.resources.iter().filter(move |r| r.is_valid_at(now)) }

    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.peers.len() + self.services.len() + self.resources.len();
        self.peers.retain(|r| r.is_valid_at(now));
        self.services.retain(|r| r.is_valid_at(now));
        self.resources.retain(|r| r.is_valid_at(now));
        let after = self.peers.len() + self.services.len() + self.resources.len();
        self.recompute_state(); before.saturating_sub(after)
    }

    pub fn mark_partitioned(&mut self) { self.generation = self.generation.saturating_add(1); self.state = FormationState::Partitioned; }

    pub fn recover_partition(&mut self, now: u64) {
        self.generation = self.generation.saturating_add(1);
        self.peers.retain(|r| r.is_valid_at(now));
        self.services.retain(|r| r.is_valid_at(now));
        self.resources.retain(|r| r.is_valid_at(now));
        if !self.peers.is_empty() { self.state = FormationState::Connected; }
        else if self.bootstrap.is_empty() { self.state = FormationState::Isolated; }
        else { self.state = FormationState::Forming; }
    }

    fn recompute_state(&mut self) {
        self.state = if !self.peers.is_empty() { FormationState::Connected }
        else if self.state == FormationState::Partitioned { FormationState::Partitioned }
        else if self.bootstrap.is_empty() { FormationState::Isolated }
        else { FormationState::Forming };
    }
}
impl Default for NetworkFormationEngine { fn default() -> Self { Self::new() } }
fn validate_interval(observed_at: u64, expires_at: u64, now: u64) -> Result<(), DiscoveryError> { if expires_at <= observed_at || now >= expires_at { Err(DiscoveryError::Expired) } else { Ok(()) } }

#[cfg(test)]
mod tests {
    use super::*;
    fn node(seed: u8) -> NodeId { NodeId::from_bytes([seed; 32]) }
    fn device(seed: u8) -> DeviceId { DeviceId::from_bytes([seed; 32]) }
    #[test] fn bootstrap_enters_forming() { let mut e=NetworkFormationEngine::with_limits(8,8,8,2); e.add_bootstrap(BootstrapPeer{node_id:node(1),device_id:device(1),transport:PhysicalTransport::Wifi,endpoint_hint:[1;32]}).unwrap(); assert_eq!(e.state(),FormationState::Forming); }
    #[test] fn observing_peer_connects() { let mut e=NetworkFormationEngine::new(); assert!(e.observe_peer(PeerRecord::new(node(1),device(1),PhysicalTransport::Ethernet,10),20).unwrap()); assert_eq!(e.state(),FormationState::Connected); }
    #[test] fn partition_recovery_drops_expired_peers() { let mut e=NetworkFormationEngine::new(); e.observe_peer(PeerRecord::new(node(1),device(1),PhysicalTransport::Wifi,0),1).unwrap(); e.mark_partitioned(); e.recover_partition(200); assert_eq!(e.state(),FormationState::Isolated); }
    #[test] fn resource_versions_are_monotonic() { let mut e=NetworkFormationEngine::new(); let old=ResourceRecord{resource_id:[4;32],provider:node(1),kind:1,version:2,observed_at:0,expires_at:100}; let new=ResourceRecord{version:3,..old.clone()}; assert!(e.publish_resource(old,10).unwrap()); assert!(e.publish_resource(new,10).unwrap()); assert_eq!(e.resource_count(),1); }
    #[test] fn peer_capacity_is_bounded() { let mut e=NetworkFormationEngine::with_limits(1,1,1,1); e.observe_peer(PeerRecord::new(node(1),device(1),PhysicalTransport::Wifi,0),1).unwrap(); assert_eq!(e.observe_peer(PeerRecord::new(node(2),device(2),PhysicalTransport::Wifi,0),1).unwrap_err(),DiscoveryError::CapacityExceeded); }
}
