use crate::discovery::FormationState;
use crate::identity::device::{DeviceSigningKey, DeviceState};
use crate::network::NodeId;
use crate::runtime::discovery::DiscoveryFormation;
use crate::runtime::naming::NameRuntime;
use crate::runtime::peers::{PeerTable, RuntimePeer, RuntimePeerState};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRuntimeState { Starting, Running, Stopping, Stopped }

pub struct NodeState {
    pub lifecycle: NodeRuntimeState,
    pub started_at: Instant,
    pub peer_table: PeerTable,
    pub formation: DiscoveryFormation,
    pub naming: NameRuntime,
    device: DeviceState,
    runtime_peers: HashMap<NodeId, RuntimePeer>,
}

impl NodeState {
    pub fn new(now: Instant, device: DeviceState, signing_key: DeviceSigningKey) -> Self {
        Self {
            lifecycle: NodeRuntimeState::Starting,
            started_at: now,
            peer_table: PeerTable::new(),
            formation: DiscoveryFormation::new(crate::physical::PhysicalTransport::Ethernet, signing_key),
            naming: NameRuntime::new(),
            device,
            runtime_peers: HashMap::new(),
        }
    }
    pub fn device(&self) -> &DeviceState { &self.device }
    pub fn formation_state(&self) -> FormationState { self.formation.state() }
    pub fn formation_generation(&self) -> u64 { self.formation.generation() }
    pub fn remember_peer(&mut self, peer: RuntimePeer) { self.runtime_peers.insert(peer.node_id, peer); }
    pub fn mark_peer_hello(&mut self, node_id: NodeId, now: Instant) { if let Some(peer)=self.runtime_peers.get_mut(&node_id){peer.seen_hello(now);} }
    pub fn mark_peer_ack(&mut self, node_id: NodeId, address: SocketAddr, now: Instant) {
        if let Some(peer)=self.runtime_peers.get_mut(&node_id){peer.address=address;peer.seen_ack(now);return;}
        self.runtime_peers.insert(node_id, RuntimePeer{node_id,address,state:RuntimePeerState::Alive,first_seen:now,last_seen:now,hello_count:0,ack_count:1});
    }
    pub fn mark_peer_expired(&mut self,node_id:NodeId){if let Some(peer)=self.runtime_peers.get_mut(&node_id){peer.mark_expired();}}
    pub fn mark_stale_peers(&mut self,now:Instant,timeout:Duration){for peer in self.runtime_peers.values_mut(){if now.duration_since(peer.last_seen)>=timeout&&peer.state!=RuntimePeerState::Expired{peer.mark_stale();}}}
    pub fn runtime_peer(&self,node_id:&NodeId)->Option<&RuntimePeer>{self.runtime_peers.get(node_id)}
    pub fn runtime_peers(&self)->impl Iterator<Item=(&NodeId,&RuntimePeer)>{self.runtime_peers.iter()}
}
