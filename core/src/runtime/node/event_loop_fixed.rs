use crate::discovery::{FormationState, ResourceRecord, SignedDiscoveryAdvertisement};
use crate::naming_protocol::{DelegationAnnouncement, NAMING_MAGIC, NameControl, NameResponse};
use crate::network::{NodeCapabilities, NodeId, NodeRole};
use crate::runtime::discovery::{DISCOVERY_ADVERTISEMENT_INTERVAL_SECONDS, DiscoveryControl, DiscoveryHello, HelloAck};
use crate::runtime::node::{NodeEvent, NodeRuntimeState, NodeState};
use crate::runtime::peers::RuntimePeer;
use crate::runtime::storage::StorageRuntimeError;
use crate::storage::protocol::{StorageControl, STORAGE_MAGIC};
use crate::transport::{MAX_DATAGRAM_SIZE, UdpTransport, UdpTransportError};
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::time;

pub const PEER_TIMEOUT: Duration = Duration::from_secs(30);
pub const PEER_STALE_TIMEOUT: Duration = Duration::from_secs(15);
pub const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(5);
pub const HELLO_INTERVAL: Duration = Duration::from_secs(10);
pub const DISCOVERY_GOSSIP_FANOUT: usize = 32;
pub const NAME_GOSSIP_FANOUT: usize = 32;
pub const STORAGE_GOSSIP_FANOUT: usize = 32;
pub const STORAGE_ADVERTISEMENT_INTERVAL: Duration = Duration::from_secs(30);
pub const STORAGE_RESOURCE_KIND: u8 = 8;

pub struct NodeEventLoop {
    transport: UdpTransport,
    state: NodeState,
    events: mpsc::Sender<NodeEvent>,
    shutdown: mpsc::Receiver<()>,
    advertised_peers: Vec<SocketAddr>,
    formation_snapshot: (FormationState, u64),
}
impl NodeEventLoop {
    pub fn new(transport: UdpTransport, state: NodeState, events: mpsc::Sender<NodeEvent>, shutdown: mpsc::Receiver<()>, advertised_peers: Vec<SocketAddr>) -> Self {
        let formation_snapshot = (state.formation_state(), state.formation_generation());
        Self { transport, state, events, shutdown, advertised_peers, formation_snapshot }
    }
    pub fn peers(&self) -> &crate::runtime::peers::PeerTable { &self.state.peer_table }
    pub fn formation_state(&self) -> FormationState { self.state.formation_state() }

    pub async fn run(mut self) -> Result<NodeState, NodeLoopError> {
        self.state.lifecycle = NodeRuntimeState::Running;
        if !self.advertised_peers.is_empty() && self.state.formation_state() == FormationState::Isolated {
            for peer in self.advertised_peers.clone() {
                let _ = self.state.formation.add_bootstrap(crate::discovery::BootstrapPeer {
                    node_id: NodeId::from_bytes([0; 32]),
                    device_id: crate::identity::device::DeviceId::from_bytes([0; 32]),
                    transport: crate::physical::PhysicalTransport::Ethernet,
                    endpoint_hint: peer.to_string().as_bytes().iter().copied().chain(std::iter::repeat(0)).take(32).collect::<Vec<_>>().try_into().unwrap_or([0;32]),
                });
            }
        }
        self.events.send(NodeEvent::Started).await.map_err(|_| NodeLoopError::EventChannelClosed)?;
        self.emit_formation_if_changed().await?;
        self.broadcast_hello().await?;
        self.broadcast_advertisement().await?;
        self.broadcast_storage_advertisements().await?;
        let mut maintenance = time::interval(MAINTENANCE_INTERVAL);
        let mut hello_timer = time::interval(HELLO_INTERVAL);
        let mut ad_timer = time::interval(Duration::from_secs(DISCOVERY_ADVERTISEMENT_INTERVAL_SECONDS));
        let mut storage_timer = time::interval(STORAGE_ADVERTISEMENT_INTERVAL);
        let mut buf = vec![0u8; MAX_DATAGRAM_SIZE];
        loop {
            tokio::select! {
                shutdown=self.shutdown.recv()=>{self.state.lifecycle=NodeRuntimeState::Stopping;let _=self.events.send(NodeEvent::Shutdown).await;self.state.lifecycle=NodeRuntimeState::Stopped;return Ok(self.state);},
                _=hello_timer.tick()=>self.broadcast_hello().await?,
                _=ad_timer.tick()=>self.broadcast_advertisement().await?,
                _=storage_timer.tick()=>self.broadcast_storage_advertisements().await?,
                _=maintenance.tick()=>self.maintain().await?,
                result=self.transport.receive_into(&mut buf)=>{let (length,source)=result.map_err(NodeLoopError::Transport)?;self.handle_datagram(&buf[..length],source).await?;}
            }
        }
    }

    async fn broadcast_hello(&self) -> Result<(), NodeLoopError> {
        let frame = DiscoveryControl::Hello(self.create_hello()).encode();
        for peer in &self.advertised_peers { self.transport.send_to(&frame, *peer).await.map_err(NodeLoopError::Transport)?; }
        Ok(())
    }
    async fn broadcast_advertisement(&mut self) -> Result<(), NodeLoopError> {
        let node_id = NodeId::from_device(self.state.device());
        let frame = DiscoveryControl::Advertisement(self.state.formation.create_advertisement(node_id, unix_timestamp()).map_err(NodeLoopError::Discovery)?).encode();
        for peer in &self.advertised_peers { self.transport.send_to(&frame, *peer).await.map_err(NodeLoopError::Transport)?; }
        let targets: Vec<_> = self.state.runtime_peers().map(|(_, p)| p.address).filter(|a| *a != SocketAddr::from(([0,0,0,0],0))).take(DISCOVERY_GOSSIP_FANOUT).collect();
        for target in targets { self.transport.send_to(&frame, target).await.map_err(NodeLoopError::Transport)?; }
        Ok(())
    }
    async fn broadcast_storage_advertisements(&mut self) -> Result<(), NodeLoopError> {
        let now = unix_timestamp();
        let advertisements = self.state.storage.local_advertisements(now);
        let mut targets: Vec<SocketAddr> = self.advertised_peers.clone();
        targets.extend(self.state.runtime_peers().map(|(_, p)| p.address).filter(|a| *a != SocketAddr::from(([0,0,0,0],0))));
        targets.sort_unstable();
        targets.dedup();
        for advertisement in advertisements {
            let resource = ResourceRecord { resource_id: *advertisement.content.as_bytes(), provider: advertisement.provider, kind: STORAGE_RESOURCE_KIND, version: advertisement.version, observed_at: advertisement.observed_at, expires_at: advertisement.expires_at };
            let _ = self.state.formation.publish_resource(resource, now);
            let frame = StorageControl::ProviderAnnouncement(advertisement).encode().map_err(NodeLoopError::StorageEncode)?;
            for target in &targets { self.transport.send_to(&frame, *target).await.map_err(NodeLoopError::Transport)?; }
        }
        Ok(())
    }
    fn create_hello(&self) -> DiscoveryHello {
        let device = self.state.device();
        let mut capabilities = NodeCapabilities::SEND.insert(NodeCapabilities::RECEIVE);
        if device.capabilities.relay { capabilities = capabilities.insert(NodeCapabilities::RELAY); }
        if device.capabilities.gateway { capabilities = capabilities.insert(NodeCapabilities::GATEWAY); }
        DiscoveryHello { protocol: crate::network::ProtocolVersion::CURRENT, node_id: NodeId::from_device(device).into_bytes(), device_id: *device.device_id.as_bytes(), capabilities, role: if device.capabilities.gateway { NodeRole::Gateway } else if device.capabilities.relay { NodeRole::Relay } else { NodeRole::Device }, timestamp: unix_timestamp() }
    }
    async fn handle_datagram(&mut self, bytes: &[u8], source: SocketAddr) -> Result<(), NodeLoopError> {
        if bytes.starts_with(&NAMING_MAGIC) { return self.handle_name_message(bytes, source).await; }
        if bytes.starts_with(&STORAGE_MAGIC) { return self.handle_storage_message(bytes, source).await; }
        let control = DiscoveryControl::decode(bytes).map_err(NodeLoopError::InvalidDiscovery)?;
        match control {
            DiscoveryControl::Hello(h) => self.handle_hello(h, source).await?,
            DiscoveryControl::HelloAck(a) => self.handle_hello_ack(a, source).await?,
            DiscoveryControl::Advertisement(a) => self.handle_advertisement(a, source).await?,
        }
        Ok(())
    }
    async fn handle_hello(&mut self, hello: DiscoveryHello, source: SocketAddr) -> Result<(), NodeLoopError> {
        let now = Instant::now();
        let node_id = self.state.peer_table.upsert(&hello, source, now);
        self.state.remember_peer(RuntimePeer::new(node_id, source, now));
        self.state.mark_peer_hello(node_id, now);
        let ack = HelloAck { node_id: NodeId::from_device(self.state.device()), timestamp: unix_timestamp() };
        self.transport.send_to(&DiscoveryControl::HelloAck(ack).encode(), source).await.map_err(NodeLoopError::Transport)?;
        self.events.send(NodeEvent::ReceivedHello { hello, address: source }).await.map_err(|_| NodeLoopError::EventChannelClosed)?;
        self.events.send(NodeEvent::PeerDiscovered { node_id, address: source }).await.map_err(|_| NodeLoopError::EventChannelClosed)?;
        self.send_advertisement_to(source).await?;
        Ok(())
    }
    async fn handle_advertisement(&mut self, advertisement: SignedDiscoveryAdvertisement, source: SocketAddr) -> Result<(), NodeLoopError> {
        if !self.state.formation.accept_advertisement(&advertisement, unix_timestamp()).map_err(NodeLoopError::Discovery)? { return Ok(()); }
        self.emit_formation_if_changed().await?;
        let frame = DiscoveryControl::Advertisement(advertisement).encode();
        let targets: Vec<_> = self.state.runtime_peers().map(|(_, p)| p.address).filter(|a| *a != source && *a != SocketAddr::from(([0,0,0,0],0))).take(DISCOVERY_GOSSIP_FANOUT).collect();
        for target in targets { self.transport.send_to(&frame, target).await.map_err(NodeLoopError::Transport)?; }
        Ok(())
    }
    async fn send_advertisement_to(&mut self, target: SocketAddr) -> Result<(), NodeLoopError> {
        let node_id = NodeId::from_device(self.state.device());
        let frame = DiscoveryControl::Advertisement(self.state.formation.create_advertisement(node_id, unix_timestamp()).map_err(NodeLoopError::Discovery)?).encode();
        self.transport.send_to(&frame, target).await.map_err(NodeLoopError::Transport)?;
        Ok(())
    }
    async fn handle_hello_ack(&mut self, ack: HelloAck, source: SocketAddr) -> Result<(), NodeLoopError> { self.state.mark_peer_ack(ack.node_id, source, Instant::now()); Ok(()) }

    async fn handle_name_message(&mut self, bytes: &[u8], source: SocketAddr) -> Result<(), NodeLoopError> {
        let control = NameControl::decode(bytes).map_err(NodeLoopError::Naming)?;
        match control {
            NameControl::Query(q) => {
                let mut response = NameResponse { request_id: q.request_id, name: None, record: None };
                if let Some((name, record)) = self.state.naming.local_records().find(|(_, r)| r.name_hash == q.name_hash) { response.name = Some(name.to_owned()); response.record = Some(*record); }
                self.transport.send_to(&NameControl::Response(response).encode().map_err(NodeLoopError::NamingEncode)?, source).await.map_err(NodeLoopError::Transport)?;
            }
            NameControl::Response(r) => { let _ = self.state.naming.consume_response(&r, unix_timestamp()); }
            NameControl::Announcement(a) => { self.state.naming.remember(&a.name, a.record, unix_timestamp()).map_err(NodeLoopError::NameRuntime)?; let frame = NameControl::Announcement(a).encode().map_err(NodeLoopError::NamingEncode)?; self.gossip_name(frame, source).await?; }
            NameControl::Delegation(DelegationAnnouncement { delegation }) => { self.state.naming.resolution_mut().accept_signed_delegation(&delegation, unix_timestamp()).map_err(NodeLoopError::Resolution)?; let frame = NameControl::Delegation(DelegationAnnouncement { delegation }).encode().map_err(NodeLoopError::NamingEncode)?; self.gossip_name(frame, source).await?; }
        }
        Ok(())
    }
    async fn gossip_name(&self, frame: Vec<u8>, source: SocketAddr) -> Result<(), NodeLoopError> {
        let targets: Vec<_> = self.state.runtime_peers().map(|(_, p)| p.address).filter(|a| *a != source && *a != SocketAddr::from(([0,0,0,0],0))).take(NAME_GOSSIP_FANOUT).collect();
        for target in targets { self.transport.send_to(&frame, target).await.map_err(NodeLoopError::Transport)?; }
        Ok(())
    }

    async fn handle_storage_message(&mut self, bytes: &[u8], source: SocketAddr) -> Result<(), NodeLoopError> {
        let control = StorageControl::decode(bytes).map_err(NodeLoopError::Storage)?;
        let Some(peer) = self.node_id_for_source(source) else { return Ok(()); };
        let is_provider = matches!(&control, StorageControl::ProviderAnnouncement(_));
        let responses = match self.state.storage.handle(control.clone(), peer, monotonic_ms(), unix_timestamp()) {
            Ok(responses) => responses,
            Err(StorageRuntimeError::ProviderRejected | StorageRuntimeError::InvalidPeer) => return Ok(()),
            Err(error) => return Err(NodeLoopError::StorageRuntime(error)),
        };
        for response in responses {
            let frame = response.encode().map_err(NodeLoopError::StorageEncode)?;
            self.transport.send_to(&frame, source).await.map_err(NodeLoopError::Transport)?;
        }
        if is_provider {
            if let StorageControl::ProviderAnnouncement(advertisement) = control {
                let now = unix_timestamp();
                let resource = ResourceRecord { resource_id: *advertisement.content.as_bytes(), provider: advertisement.provider, kind: STORAGE_RESOURCE_KIND, version: advertisement.version, observed_at: advertisement.observed_at, expires_at: advertisement.expires_at };
                let _ = self.state.formation.publish_resource(resource, now);
                let frame = StorageControl::ProviderAnnouncement(advertisement).encode().map_err(NodeLoopError::StorageEncode)?;
                self.gossip_storage(frame, source).await?;
            }
        }
        Ok(())
    }
    fn node_id_for_source(&self, source: SocketAddr) -> Option<NodeId> { self.state.runtime_peers().find(|(_, peer)| peer.address == source).map(|(node, _)| *node) }
    async fn gossip_storage(&self, frame: Vec<u8>, source: SocketAddr) -> Result<(), NodeLoopError> {
        let targets: Vec<_> = self.state.runtime_peers().map(|(_, p)| p.address).filter(|a| *a != source && *a != SocketAddr::from(([0,0,0,0],0))).take(STORAGE_GOSSIP_FANOUT).collect();
        for target in targets { self.transport.send_to(&frame, target).await.map_err(NodeLoopError::Transport)?; }
        Ok(())
    }
    async fn maintain(&mut self) -> Result<(), NodeLoopError> {
        let now = Instant::now();
        for node_id in self.state.peer_table.expire(now, PEER_TIMEOUT) { self.state.mark_peer_expired(node_id); let _ = self.state.formation.forget_peer(node_id); self.events.send(NodeEvent::PeerExpired { node_id }).await.map_err(|_| NodeLoopError::EventChannelClosed)?; }
        let wall = unix_timestamp();
        let _ = self.state.formation.maintain(wall);
        let _ = self.state.naming.expire(wall);
        self.state.storage.gc(monotonic_ms(), wall);
        self.state.mark_stale_peers(now, PEER_STALE_TIMEOUT);
        self.events.send(NodeEvent::Tick).await.map_err(|_| NodeLoopError::EventChannelClosed)?;
        self.emit_formation_if_changed().await
    }
    async fn emit_formation_if_changed(&mut self) -> Result<(), NodeLoopError> {
        let snapshot = (self.state.formation_state(), self.state.formation_generation());
        if snapshot == self.formation_snapshot { return Ok(()); }
        self.formation_snapshot = snapshot;
        self.events.send(NodeEvent::FormationChanged { state: snapshot.0, generation: snapshot.1 }).await.map_err(|_| NodeLoopError::EventChannelClosed)
    }
}
fn unix_timestamp() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn monotonic_ms() -> u64 { static START: OnceLock<Instant> = OnceLock::new(); START.get_or_init(Instant::now).elapsed().as_millis() as u64 }

#[derive(Debug)]
pub enum NodeLoopError {
    Transport(UdpTransportError), InvalidDiscovery(crate::runtime::discovery::ControlError), Discovery(crate::discovery::DiscoveryError), Naming(crate::naming_protocol::NamingCodecError), NamingEncode(crate::naming_protocol::NamingCodecError), NameRuntime(crate::runtime::naming::NameRuntimeError), Resolution(crate::name_resolution::ResolutionError), Storage(crate::storage::protocol::StorageCodecError), StorageEncode(crate::storage::protocol::StorageCodecError), StorageRuntime(StorageRuntimeError), EventChannelClosed,
}
impl std::fmt::Display for NodeLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { match self { Self::Transport(e)=>write!(f,"transport error: {e}"), Self::InvalidDiscovery(e)=>write!(f,"invalid discovery message: {e:?}"), Self::Discovery(e)=>write!(f,"discovery formation error: {e:?}"), Self::Naming(e)=>write!(f,"invalid naming message: {e:?}"), Self::NamingEncode(e)=>write!(f,"naming encode error: {e:?}"), Self::NameRuntime(e)=>write!(f,"name runtime error: {e:?}"), Self::Resolution(e)=>write!(f,"name resolution error: {e:?}"), Self::Storage(e)=>write!(f,"invalid storage message: {e:?}"), Self::StorageEncode(e)=>write!(f,"storage encode error: {e:?}"), Self::StorageRuntime(e)=>write!(f,"storage runtime error: {e:?}"), Self::EventChannelClosed=>f.write_str("node event channel closed") } }
}
impl std::error::Error for NodeLoopError {}
