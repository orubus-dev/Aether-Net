use crate::core::AetherCore;
use crate::discovery::FormationState;
use crate::errors::CoreError;
use crate::naming::{NameRecord, ResourceId};
use crate::naming_protocol::{NameQuery, NameResponse};
use crate::network::NodeId;
use crate::runtime::node::{NodeEvent, NodeEventLoop, NodeState};
use crate::runtime::{CoreConfig, NameRuntimeError, StorageRuntimeError};
use crate::storage::{ChunkDigest, ChunkId, ContentId, ObjectManifest, ProviderAdvertisement, RepairPlan, StorageControl};
use crate::transport::{UdpTransport, UdpTransportError};
use std::net::SocketAddr;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

pub struct AetherNodeRuntime {
    core: AetherCore,
    transport: Option<UdpTransport>,
    state: NodeState,
}

impl AetherNodeRuntime {
    pub async fn bind(config: CoreConfig, address: SocketAddr) -> Result<Self, NodeRuntimeError> {
        let transport = UdpTransport::bind(address).await.map_err(NodeRuntimeError::Transport)?;
        let mut core = AetherCore::new(config);
        core.initialize().map_err(NodeRuntimeError::Core)?;
        let device = core.device().cloned().ok_or(NodeRuntimeError::IdentityUnavailable)?;
        let signing_key = core.device_key().cloned().ok_or(NodeRuntimeError::IdentityUnavailable)?;
        let state = NodeState::new(Instant::now(), device, signing_key);
        Ok(Self { core, transport: Some(transport), state })
    }
    pub fn local_addr(&self) -> Result<SocketAddr, NodeRuntimeError> { self.transport.as_ref().ok_or(NodeRuntimeError::Stopped)?.local_addr().map_err(NodeRuntimeError::Transport) }
    pub fn core(&self) -> &AetherCore { &self.core }
    pub fn formation_state(&self) -> FormationState { self.state.formation_state() }
    pub fn formation_generation(&self) -> u64 { self.state.formation_generation() }
    pub fn discovered_peer_count(&self) -> usize { self.state.formation.peer_count() }
    pub fn publish_name(&mut self, name: &str, record: NameRecord) -> Result<bool, NameRuntimeError> { self.state.naming.publish_local(name, record, unix_timestamp()) }
    pub fn resolve_name(&mut self, name: &str) -> Result<ResourceId, NameRuntimeError> { self.state.naming.resolve(name, unix_timestamp()) }
    pub fn resolve_content_name(&mut self, name: &str) -> Result<ContentId, NameRuntimeError> { Ok(ContentId::from_raw(*self.resolve_name(name)?.as_bytes())) }
    pub fn begin_name_query(&mut self, name: &str) -> Result<NameQuery, NameRuntimeError> { self.state.naming.begin_query(name, unix_timestamp()) }
    pub fn consume_name_response(&mut self, response: &NameResponse) -> Result<Option<ResourceId>, NameRuntimeError> { self.state.naming.consume_response(response, unix_timestamp()) }
    pub fn local_name_count(&self) -> usize { self.state.naming.local_count() }
    pub fn publish_storage_bytes(&mut self, bytes: &[u8]) -> Result<(ObjectManifest, ProviderAdvertisement), StorageRuntimeError> { self.state.storage.publish_bytes(bytes, unix_timestamp()) }
    pub fn begin_storage_manifest_query(&mut self, content: ContentId, start: u32) -> StorageControl { self.state.storage.begin_manifest_query(content, start) }
    pub fn begin_storage_chunk_query(&mut self, content: ContentId, chunk: ChunkId, offset: u32, length: u16, digest: ChunkDigest) -> StorageControl { self.state.storage.begin_chunk_query(content, chunk, offset, length, digest) }
    pub fn storage_required_replicas(&self) -> usize { self.state.storage.required_replicas() }
    pub fn set_storage_required_replicas(&mut self, count: usize) { self.state.storage.set_required_replicas(count); }
    pub fn storage_replica_count(&self, content: ContentId) -> usize { self.state.storage.replica_count(content, unix_timestamp()) }
    pub fn storage_repair_plan(&self, content: ContentId) -> RepairPlan { self.state.storage.repair_plan(content, unix_timestamp()) }
    pub fn stored_object_count(&self) -> usize { self.state.storage.object_count() }
    pub fn stored_chunk_count(&self) -> usize { self.state.storage.chunk_count() }
    pub fn stored_bytes(&self) -> u64 { self.state.storage.stored_bytes() }
    pub fn local_node_id(&self) -> NodeId { NodeId::from_device(self.state.device()) }
    pub async fn run(self, advertised_peers: Vec<SocketAddr>) -> Result<(), NodeRuntimeError> { let (_shutdown_tx, shutdown_rx) = mpsc::channel(1); self.run_with_shutdown(advertised_peers, shutdown_rx).await }
    pub async fn run_with_shutdown(mut self, advertised_peers: Vec<SocketAddr>, mut shutdown: mpsc::Receiver<()>) -> Result<(), NodeRuntimeError> {
        self.core.start().map_err(NodeRuntimeError::Core)?;
        let transport = self.transport.take().ok_or(NodeRuntimeError::Stopped)?;
        let (events_tx, mut events_rx) = mpsc::channel(128);
        let (loop_shutdown_tx, loop_shutdown_rx) = mpsc::channel(1);
        let event_loop = NodeEventLoop::new(transport, self.state, events_tx, loop_shutdown_rx, advertised_peers);
        let loop_task = tokio::spawn(event_loop.run());
        loop {
            tokio::select! {
                event = events_rx.recv() => {
                    match event {
                        Some(NodeEvent::Started) => println!("Aether node event: started"),
                        Some(NodeEvent::ReceivedHello { hello, address }) => { println!("Aether HELLO received from {}", address); let _ = hello; }
                        Some(NodeEvent::PeerDiscovered { node_id, address }) => println!("Aether peer discovered: {} at {}", node_id, address),
                        Some(NodeEvent::PeerExpired { node_id }) => println!("Aether peer expired: {}", node_id),
                        Some(NodeEvent::FormationChanged { state, generation }) => println!("Aether network formation changed: {:?} (generation {})", state, generation),
                        Some(NodeEvent::Tick) => {},
                        Some(NodeEvent::Shutdown) | None => break,
                    }
                }
                _ = shutdown.recv() => break,
            }
        }
        let _ = loop_shutdown_tx.send(()).await;
        let result = loop_task.await.map_err(|_| NodeRuntimeError::Runtime)?.map_err(NodeRuntimeError::Loop);
        self.core.stop().map_err(NodeRuntimeError::Core)?;
        result.map(|_| ())
    }
}
fn unix_timestamp() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }

#[derive(Debug)]
pub enum NodeRuntimeError { Core(CoreError), Transport(UdpTransportError), IdentityUnavailable, Stopped, Runtime, Loop(crate::runtime::node::NodeLoopError) }
impl std::fmt::Display for NodeRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { match self { Self::Core(e) => write!(f,"Aether core error: {e}"), Self::Transport(e) => write!(f,"Aether transport error: {e}"), Self::IdentityUnavailable => f.write_str("Aether node identity unavailable"), Self::Stopped => f.write_str("Aether node is stopped"), Self::Runtime => f.write_str("Aether runtime task failed"), Self::Loop(e) => write!(f,"Aether node event loop error: {e}"), } }
}
impl std::error::Error for NodeRuntimeError {}
impl std::fmt::Display for crate::transport::UdpTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { match self { Self::Bind(e) => write!(f,"UDP bind failed: {e}"), Self::LocalAddress(e) => write!(f,"could not read local UDP address: {e}"), Self::Send(e) => write!(f,"UDP send failed: {e}"), Self::Receive(e) => write!(f,"UDP receive failed: {e}"), Self::DatagramTooLarge => f.write_str("UDP datagram exceeds maximum size"), } }
}
impl std::error::Error for crate::transport::UdpTransportError {}
