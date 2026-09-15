use crate::network::{NodeCapabilities, NodeId, NodeRole};
use crate::runtime::discovery::{
    DiscoveryControl, DiscoveryHello, HelloAck, DISCOVERY_ADVERTISEMENT_INTERVAL_SECONDS,
};
use crate::runtime::node::{NodeEvent, NodeRuntimeState, NodeState};
use crate::runtime::peers::RuntimePeer;
use crate::transport::{MAX_DATAGRAM_SIZE, UdpTransport, UdpTransportError};

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::time;

pub const PEER_TIMEOUT: Duration = Duration::from_secs(30);
pub const PEER_STALE_TIMEOUT: Duration = Duration::from_secs(15);
pub const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(5);
pub const HELLO_INTERVAL: Duration = Duration::from_secs(10);
pub const DISCOVERY_GOSSIP_FANOUT: usize = 32;

pub struct NodeEventLoop {
    transport: UdpTransport,
    state: NodeState,
    events: mpsc::Sender<NodeEvent>,
    shutdown: mpsc::Receiver<()>,
    advertised_peers: Vec<SocketAddr>,
    formation_snapshot: (crate::discovery::FormationState, u64),
}

impl NodeEventLoop {
    pub fn new(
        transport: UdpTransport,
        state: NodeState,
        events: mpsc::Sender<NodeEvent>,
        shutdown: mpsc::Receiver<()>,
        advertised_peers: Vec<SocketAddr>,
    ) -> Self {
        let formation_snapshot = (state.formation_state(), state.formation_generation());
        Self {
            transport,
            state,
            events,
            shutdown,
            advertised_peers,
            formation_snapshot,
        }
    }

    pub fn peers(&self) -> &crate::runtime::peers::PeerTable {
        &self.state.peer_table
    }

    pub fn formation_state(&self) -> crate::discovery::FormationState {
        self.state.formation_state()
    }

    pub async fn run(mut self) -> Result<NodeState, NodeLoopError> {
        self.state.lifecycle = NodeRuntimeState::Running;

        if !self.advertised_peers.is_empty()
            && self.state.formation_state() == crate::discovery::FormationState::Isolated
        {
            for peer in self.advertised_peers.clone() {
                self.state
                    .formation
                    .add_bootstrap(
                        NodeId::from_bytes([0; 32]),
                        crate::identity::device::DeviceId::from_bytes([0; 32]),
                        peer,
                    )
                    .ok();
            }
        }

        self.events
            .send(NodeEvent::Started)
            .await
            .map_err(|_| NodeLoopError::EventChannelClosed)?;

        self.emit_formation_if_changed().await?;
        self.broadcast_hello().await?;
        self.broadcast_advertisement().await?;

        let mut maintenance = time::interval(MAINTENANCE_INTERVAL);
        let mut hello_timer = time::interval(HELLO_INTERVAL);
        let mut advertisement_timer = time::interval(Duration::from_secs(
            DISCOVERY_ADVERTISEMENT_INTERVAL_SECONDS,
        ));
        let mut receive_buffer = vec![0u8; MAX_DATAGRAM_SIZE];

        loop {
            tokio::select! {
                shutdown = self.shutdown.recv() => {
                    if shutdown.is_some() {
                        self.state.lifecycle = NodeRuntimeState::Stopping;
                    }
                    let _ = self.events.send(NodeEvent::Shutdown).await;
                    self.state.lifecycle = NodeRuntimeState::Stopped;
                    return Ok(self.state);
                }

                _ = hello_timer.tick() => {
                    self.broadcast_hello().await?;
                }

                _ = advertisement_timer.tick() => {
                    self.broadcast_advertisement().await?;
                }

                _ = maintenance.tick() => {
                    self.maintain().await?;
                }

                result = self.transport.receive_into(&mut receive_buffer) => {
                    let (length, source) = result.map_err(NodeLoopError::Transport)?;
                    let bytes = &receive_buffer[..length];
                    self.handle_datagram(bytes, source).await?;
                }
            }
        }
    }

    async fn broadcast_hello(&self) -> Result<(), NodeLoopError> {
        let hello = self.create_hello();
        let frame = DiscoveryControl::Hello(hello).encode();
        for peer in &self.advertised_peers {
            self.transport
                .send_to(&frame, *peer)
                .await
                .map_err(NodeLoopError::Transport)?;
        }
        Ok(())
    }

    async fn broadcast_advertisement(&mut self) -> Result<(), NodeLoopError> {
        let node_id = NodeId::from_device(self.state.device());
        let advertisement = self
            .state
            .formation
            .create_advertisement(node_id, unix_timestamp())
            .map_err(NodeLoopError::Discovery)?;
        let frame = DiscoveryControl::Advertisement(advertisement).encode();

        for peer in &self.advertised_peers {
            self.transport
                .send_to(&frame, *peer)
                .await
                .map_err(NodeLoopError::Transport)?;
        }

        let targets: Vec<_> = self
            .state
            .runtime_peers()
            .filter_map(|(_, peer)| (peer.address != SocketAddr::from(([0, 0, 0, 0], 0))).then_some(peer.address))
            .take(DISCOVERY_GOSSIP_FANOUT)
            .collect();
        for target in targets {
            self.transport
                .send_to(&frame, target)
                .await
                .map_err(NodeLoopError::Transport)?;
        }
        Ok(())
    }

    fn create_hello(&self) -> DiscoveryHello {
        let device = self.state.device();
        let node_id = NodeId::from_device(device);
        let mut capabilities = NodeCapabilities::NONE;
        capabilities = capabilities.insert(NodeCapabilities::SEND);
        capabilities = capabilities.insert(NodeCapabilities::RECEIVE);
        if device.capabilities.relay {
            capabilities = capabilities.insert(NodeCapabilities::RELAY);
        }
        if device.capabilities.gateway {
            capabilities = capabilities.insert(NodeCapabilities::GATEWAY);
        }
        DiscoveryHello {
            protocol: crate::network::ProtocolVersion::CURRENT,
            node_id: node_id.into_bytes(),
            device_id: *device.device_id.as_bytes(),
            capabilities,
            role: if device.capabilities.gateway {
                NodeRole::Gateway
            } else if device.capabilities.relay {
                NodeRole::Relay
            } else {
                NodeRole::Device
            },
            timestamp: unix_timestamp(),
        }
    }

    async fn handle_datagram(
        &mut self,
        bytes: &[u8],
        source: SocketAddr,
    ) -> Result<(), NodeLoopError> {
        let control = DiscoveryControl::decode(bytes).map_err(NodeLoopError::InvalidDiscovery)?;
        match control {
            DiscoveryControl::Hello(hello) => self.handle_hello(hello, source).await?,
            DiscoveryControl::HelloAck(ack) => self.handle_hello_ack(ack, source).await?,
            DiscoveryControl::Advertisement(advertisement) => {
                self.handle_advertisement(advertisement, source).await?
            }
        }
        Ok(())
    }

    async fn handle_hello(
        &mut self,
        hello: DiscoveryHello,
        source: SocketAddr,
    ) -> Result<(), NodeLoopError> {
        let now = Instant::now();
        let node_id = self.state.peer_table.upsert(&hello, source, now);
        self.state
            .remember_peer(RuntimePeer::new(node_id, source, now));
        self.state.mark_peer_hello(node_id, now);
        let ack = HelloAck {
            node_id: NodeId::from_device(self.state.device()),
            timestamp: unix_timestamp(),
        };
        self.transport
            .send_to(&DiscoveryControl::HelloAck(ack).encode(), source)
            .await
            .map_err(NodeLoopError::Transport)?;
        self.events
            .send(NodeEvent::ReceivedHello {
                hello,
                address: source,
            })
            .await
            .map_err(|_| NodeLoopError::EventChannelClosed)?;
        self.events
            .send(NodeEvent::PeerDiscovered {
                node_id,
                address: source,
            })
            .await
            .map_err(|_| NodeLoopError::EventChannelClosed)?;
        self.send_advertisement_to(source).await?;
        Ok(())
    }

    async fn handle_advertisement(
        &mut self,
        advertisement: crate::discovery::SignedDiscoveryAdvertisement,
        source: SocketAddr,
    ) -> Result<(), NodeLoopError> {
        let accepted = self
            .state
            .formation
            .accept_advertisement(&advertisement, unix_timestamp())
            .map_err(NodeLoopError::Discovery)?;
        if !accepted {
            return Ok(());
        }
        self.emit_formation_if_changed().await?;
        let frame = DiscoveryControl::Advertisement(advertisement).encode();
        let targets: Vec<_> = self
            .state
            .runtime_peers()
            .filter_map(|(_, peer)| {
                (peer.address != source).then_some(peer.address)
            })
            .take(DISCOVERY_GOSSIP_FANOUT)
            .collect();
        for target in targets {
            self.transport
                .send_to(&frame, target)
                .await
                .map_err(NodeLoopError::Transport)?;
        }
        Ok(())
    }

    async fn send_advertisement_to(&mut self, target: SocketAddr) -> Result<(), NodeLoopError> {
        let node_id = NodeId::from_device(self.state.device());
        let advertisement = self
            .state
            .formation
            .create_advertisement(node_id, unix_timestamp())
            .map_err(NodeLoopError::Discovery)?;
        self.transport
            .send_to(&DiscoveryControl::Advertisement(advertisement).encode(), target)
            .await
            .map_err(NodeLoopError::Transport)
    }

    async fn handle_hello_ack(
        &mut self,
        ack: HelloAck,
        source: SocketAddr,
    ) -> Result<(), NodeLoopError> {
        self.state
            .mark_peer_ack(ack.node_id, source, Instant::now());
        Ok(())
    }

    async fn maintain(&mut self) -> Result<(), NodeLoopError> {
        let now = Instant::now();
        let expired = self.state.peer_table.expire(now, PEER_TIMEOUT);
        for node_id in expired {
            self.state.mark_peer_expired(node_id);
            let _ = self.state.formation.forget_peer(node_id);
            self.events
                .send(NodeEvent::PeerExpired { node_id })
                .await
                .map_err(|_| NodeLoopError::EventChannelClosed)?;
        }
        if self.state.formation.maintain(unix_timestamp()) != 0 {
            self.emit_formation_if_changed().await?;
        }
        self.state.mark_stale_peers(now, PEER_STALE_TIMEOUT);
        self.events
            .send(NodeEvent::Tick)
            .await
            .map_err(|_| NodeLoopError::EventChannelClosed)?;
        self.emit_formation_if_changed().await?;
        Ok(())
    }

    async fn emit_formation_if_changed(&mut self) -> Result<(), NodeLoopError> {
        let snapshot = (
            self.state.formation_state(),
            self.state.formation_generation(),
        );
        if snapshot == self.formation_snapshot {
            return Ok(());
        }
        self.formation_snapshot = snapshot;
        self.events
            .send(NodeEvent::FormationChanged {
                state: snapshot.0,
                generation: snapshot.1,
            })
            .await
            .map_err(|_| NodeLoopError::EventChannelClosed)
    }
}

fn unix_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug)]
pub enum NodeLoopError {
    Transport(UdpTransportError),
    InvalidDiscovery(crate::runtime::discovery::ControlError),
    Discovery(crate::discovery::DiscoveryError),
    EventChannelClosed,
}

impl std::fmt::Display for NodeLoopError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "transport error: {error}"),
            Self::InvalidDiscovery(error) => {
                write!(formatter, "invalid discovery message: {error:?}")
            }
            Self::Discovery(error) => write!(formatter, "discovery formation error: {error:?}"),
            Self::EventChannelClosed => formatter.write_str("node event channel closed"),
        }
    }
}

impl std::error::Error for NodeLoopError {}
