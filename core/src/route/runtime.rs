use super::control_protocol::{decode_advertisement, encode_advertisement};
use super::{
    ROUTING_ADVERTISEMENT_MAGIC, RouteNeighbor, RoutingControlError, RoutingControlPlane,
    RoutingPeer, SignedRouteAdvertisement,
};
use crate::identity::fingerprint::Fingerprint;
use crate::link::LinkSession;
use crate::network::{AetherAddress, AetherFlowId, L3Route, NodeId, TrafficPriority};
use crate::runtime::{NativeAetherStack, NativeIngress, NativeStackError};
use crate::transport::AetherTransportMode;
use sha2::{Digest, Sha256};

pub const DEFAULT_ROUTE_ADVERTISEMENT_INTERVAL_SECONDS: u64 = 10;

#[derive(Debug)]
pub enum RoutingRuntimeError {
    Control(RoutingControlError),
    Codec(super::control_protocol::RouteControlCodecError),
    Native(NativeStackError),
    InvalidNeighborSet,
}

impl From<RoutingControlError> for RoutingRuntimeError {
    fn from(value: RoutingControlError) -> Self {
        Self::Control(value)
    }
}

impl From<super::control_protocol::RouteControlCodecError> for RoutingRuntimeError {
    fn from(value: super::control_protocol::RouteControlCodecError) -> Self {
        Self::Codec(value)
    }
}

impl From<NativeStackError> for RoutingRuntimeError {
    fn from(value: NativeStackError) -> Self {
        Self::Native(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingOutboundFrame {
    pub destination: NodeId,
    pub address: AetherAddress,
    pub frame: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingTickResult {
    pub route_count: usize,
    pub advertisement_sent: bool,
    pub accepted_advertisements: usize,
    pub forwarded_advertisements: usize,
}

/// Live distributed-routing coordinator above the authenticated native L2/L3/L4 stack.
///
/// Local signed link-state advertisements are periodically emitted over a dedicated
/// native L4 datagram flow. Newly accepted remote advertisements are flooded to the
/// remaining authenticated peers; the monotonic routing engine suppresses duplicates.
pub struct RoutingRuntime {
    control: RoutingControlPlane,
    native: NativeAetherStack,
    signing_key: ed25519_dalek::SigningKey,
    local_neighbors: Vec<RouteNeighbor>,
    priority: TrafficPriority,
    advertisement_interval_seconds: u64,
    last_advertisement_seconds: Option<u64>,
}

impl RoutingRuntime {
    pub fn new(
        local_node: NodeId,
        local_address: AetherAddress,
        signing_key: ed25519_dalek::SigningKey,
    ) -> Result<Self, RoutingRuntimeError> {
        let mut control = RoutingControlPlane::new(local_node, local_address);
        control.bind_local_signing_key(&signing_key)?;
        Ok(Self {
            control,
            native: NativeAetherStack::new(local_address),
            signing_key,
            local_neighbors: Vec::new(),
            priority: TrafficPriority::Interactive,
            advertisement_interval_seconds: DEFAULT_ROUTE_ADVERTISEMENT_INTERVAL_SECONDS,
            last_advertisement_seconds: None,
        })
    }

    pub fn from_signing_key(
        signing_key: ed25519_dalek::SigningKey,
        local_address: AetherAddress,
    ) -> Result<Self, RoutingRuntimeError> {
        let local_node = node_id_from_signing_key(&signing_key);
        Self::new(local_node, local_address, signing_key)
    }

    pub fn control(&self) -> &RoutingControlPlane {
        &self.control
    }

    pub fn control_mut(&mut self) -> &mut RoutingControlPlane {
        &mut self.control
    }

    pub fn native(&self) -> &NativeAetherStack {
        &self.native
    }

    pub fn native_mut(&mut self) -> &mut NativeAetherStack {
        &mut self.native
    }

    pub fn local_node(&self) -> NodeId {
        self.control.local_node()
    }

    pub fn local_address(&self) -> AetherAddress {
        self.control.local_address()
    }

    pub fn set_priority(&mut self, priority: TrafficPriority) {
        self.priority = priority;
    }

    pub fn set_advertisement_interval(&mut self, seconds: u64) {
        self.advertisement_interval_seconds = seconds.max(1);
    }

    pub fn set_local_neighbors(
        &mut self,
        neighbors: Vec<RouteNeighbor>,
    ) -> Result<(), RoutingRuntimeError> {
        if neighbors.len() > super::DEFAULT_MAX_NEIGHBORS {
            return Err(RoutingRuntimeError::InvalidNeighborSet);
        }
        self.local_neighbors = neighbors;
        Ok(())
    }

    pub fn register_peer(
        &mut self,
        peer: RoutingPeer,
        session: LinkSession,
    ) -> Result<(), RoutingRuntimeError> {
        self.control.register_peer(peer)?;
        if let Err(error) = self.native.attach_peer(peer.node_id, peer.address, session) {
            self.control.remove_peer(peer.node_id);
            return Err(error.into());
        }
        self.native
            .l3_mut()
            .install_route(L3Route::new(peer.address, peer.address, 1, 0, None))
            .map_err(|error| RoutingRuntimeError::Native(NativeStackError::Route(error)))?;
        Ok(())
    }

    pub fn remove_peer(&mut self, node_id: NodeId) -> bool {
        let removed = self.control.remove_peer(node_id);
        if removed {
            self.local_neighbors
                .retain(|neighbor| neighbor.node_id != node_id);
        }
        removed
    }

    pub fn advertise_now(
        &mut self,
        now_seconds: u64,
        now_ms: u64,
    ) -> Result<Vec<RoutingOutboundFrame>, RoutingRuntimeError> {
        let advertisement = self.control.local_advertisement(
            now_seconds,
            self.local_neighbors.clone(),
            &self.signing_key,
        )?;
        self.control.install_local_neighbors(
            advertisement.advertisement.sequence,
            now_seconds,
            self.local_neighbors.clone(),
        )?;
        let payload = encode_advertisement(&advertisement)?;
        let mut output = Vec::with_capacity(self.control.peer_count());
        let peers: Vec<RoutingPeer> = self.control.peers().collect();
        for peer in peers {
            let flow = route_flow_id(self.local_address(), peer.address);
            self.native
                .open_flow(peer.address, flow, AetherTransportMode::Datagram)?;
            let frame =
                self.native
                    .send_l4(peer.address, flow, payload.clone(), now_ms, now_seconds)?;
            output.push(RoutingOutboundFrame {
                destination: peer.node_id,
                address: peer.address,
                frame,
            });
        }
        self.last_advertisement_seconds = Some(now_seconds);
        Ok(output)
    }

    pub fn tick(
        &mut self,
        now_seconds: u64,
        now_ms: u64,
    ) -> Result<(RoutingTickResult, Vec<RoutingOutboundFrame>), RoutingRuntimeError> {
        let route_count = self.control.tick(self.priority, now_seconds)?;
        let due = self
            .last_advertisement_seconds
            .map(|last| now_seconds.saturating_sub(last) >= self.advertisement_interval_seconds)
            .unwrap_or(true);
        if due {
            let frames = self.advertise_now(now_seconds, now_ms)?;
            return Ok((
                RoutingTickResult {
                    route_count,
                    advertisement_sent: true,
                    accepted_advertisements: 0,
                    forwarded_advertisements: 0,
                },
                frames,
            ));
        }
        Ok((
            RoutingTickResult {
                route_count,
                advertisement_sent: false,
                accepted_advertisements: 0,
                forwarded_advertisements: 0,
            },
            Vec::new(),
        ))
    }

    pub fn ingest_frame(
        &mut self,
        from: NodeId,
        frame: &[u8],
        now_seconds: u64,
        now_ms: u64,
    ) -> Result<(NativeIngress, Vec<RoutingOutboundFrame>, usize), RoutingRuntimeError> {
        let ingress = self.native.ingest_frame(from, frame, now_seconds, now_ms)?;
        let NativeIngress::Local { delivered, .. } = &ingress else {
            return Ok((ingress, Vec::new(), 0));
        };

        let mut accepted = 0usize;
        let mut flood_payloads: Vec<SignedRouteAdvertisement> = Vec::new();
        for payload in delivered {
            if payload.len() < 4 || payload[0..4] != ROUTING_ADVERTISEMENT_MAGIC {
                continue;
            }
            let advertisement = decode_advertisement(payload)?;
            if self
                .control
                .accept_advertisement(advertisement.clone(), now_seconds)?
            {
                accepted += 1;
                flood_payloads.push(advertisement);
            }
        }

        let mut output = Vec::new();
        if accepted > 0 {
            self.control.tick(self.priority, now_seconds)?;
            let peers: Vec<RoutingPeer> = self.control.peers().collect();
            for advertisement in flood_payloads {
                let payload = encode_advertisement(&advertisement)?;
                for peer in &peers {
                    if peer.node_id == from {
                        continue;
                    }
                    let flow = route_flow_id(self.local_address(), peer.address);
                    self.native
                        .open_flow(peer.address, flow, AetherTransportMode::Datagram)?;
                    let frame = self.native.send_l4(
                        peer.address,
                        flow,
                        payload.clone(),
                        now_ms,
                        now_seconds,
                    )?;
                    output.push(RoutingOutboundFrame {
                        destination: peer.node_id,
                        address: peer.address,
                        frame,
                    });
                }
            }
        }
        Ok((ingress, output, accepted))
    }
}

pub fn node_id_from_signing_key(signing_key: &ed25519_dalek::SigningKey) -> NodeId {
    let fingerprint = Fingerprint::from_domain(
        b"aether/node-id/v1",
        &signing_key.verifying_key().to_bytes(),
    );
    NodeId::from_bytes(*fingerprint.as_bytes())
}

fn route_flow_id(source: AetherAddress, destination: AetherAddress) -> AetherFlowId {
    let mut hasher = Sha256::new();
    hasher.update(b"aether:l4:route-control:v1:");
    hasher.update(source.as_bytes());
    hasher.update(destination.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    AetherFlowId::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::{SessionNegotiation, SessionRole};
    use crate::route::RouteMetrics;

    fn key(seed: u8) -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
    }

    fn address(seed: u8) -> AetherAddress {
        AetherAddress::from_bytes([seed; 32])
    }

    fn metrics() -> RouteMetrics {
        RouteMetrics {
            latency_us: 10,
            jitter_us: 1,
            packet_loss_ppm: 100,
            bandwidth_bps: 10_000_000,
            reliability_ppm: 999_000,
            monetary_cost_micros: 1,
            energy_cost: 1,
            hop_count: 1,
        }
    }

    fn paired_sessions(a: NodeId, b: NodeId) -> (LinkSession, LinkSession) {
        let initiator = SessionNegotiation::create(SessionRole::Initiator).unwrap();
        let responder = SessionNegotiation::create(SessionRole::Responder).unwrap();
        let a_pub = initiator.local_ephemeral.public_key();
        let b_pub = responder.local_ephemeral.public_key();
        let transcript: [&[u8]; 2] = [a.as_bytes(), b.as_bytes()];
        (
            initiator.finish(&b_pub, &transcript, a, b).unwrap(),
            responder.finish(&a_pub, &transcript, a, b).unwrap(),
        )
    }

    #[test]
    fn live_signed_advertisement_crosses_native_l4() {
        let a_key = key(1);
        let b_key = key(2);
        let a = node_id_from_signing_key(&a_key);
        let b = node_id_from_signing_key(&b_key);
        let (ab_a, ab_b) = paired_sessions(a, b);
        let mut runtime_a = RoutingRuntime::new(a, address(1), a_key).unwrap();
        let mut runtime_b = RoutingRuntime::new(b, address(2), b_key).unwrap();
        let peer_b = RoutingPeer {
            node_id: b,
            address: address(2),
            public_key: key(2).verifying_key().to_bytes(),
        };
        let peer_a = RoutingPeer {
            node_id: a,
            address: address(1),
            public_key: key(1).verifying_key().to_bytes(),
        };
        runtime_a.register_peer(peer_b, ab_a).unwrap();
        runtime_b.register_peer(peer_a, ab_b).unwrap();
        runtime_a
            .set_local_neighbors(vec![RouteNeighbor {
                node_id: b,
                metrics: metrics(),
            }])
            .unwrap();
        let frames = runtime_a.advertise_now(10, 10_000).unwrap();
        assert_eq!(frames.len(), 1);
        let (_ingress, flooded, accepted) = runtime_b
            .ingest_frame(a, &frames[0].frame, 10, 10_000)
            .unwrap();
        assert_eq!(accepted, 1);
        assert!(flooded.is_empty());
        assert!(
            runtime_b
                .native()
                .l3()
                .lookup_route(address(1), 10)
                .is_some()
        );
    }
}
