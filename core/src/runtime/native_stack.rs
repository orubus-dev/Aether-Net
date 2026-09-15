use crate::link::{FrameError, LinkSession};
use crate::network::{AetherAddress, AetherL3Packet, L3Delivery, L3EngineError, L3Flags};
use crate::transport::{AetherL4Segment, AetherL4Session, AetherTransportMode, L4ReceiveError, L4SendError};
use crate::network::AetherFlowId;
use std::collections::HashMap;

pub const DEFAULT_MAX_PEER_SESSIONS: usize = 16_384;
pub const DEFAULT_MAX_L4_SESSIONS: usize = 65_536;

struct PeerBinding {
    node: crate::network::NodeId,
    address: AetherAddress,
    session: LinkSession,
}

struct FlowBinding {
    peer: AetherAddress,
    session: AetherL4Session,
}

#[derive(Debug)]
pub enum NativeStackError {
    PeerCapacityExceeded,
    PeerNotFound,
    DuplicatePeer,
    Route(L3EngineError),
    Frame(FrameError),
    L3(crate::network::L3Error),
    L4Receive(L4ReceiveError),
    L4Send(L4SendError),
    L4(crate::transport::L4Error),
    FlowCapacityExceeded,
    FlowNotFound,
    LocalDeliveryRequiresL4,
}

pub enum NativeIngress {
    Forward {
        next_hop: AetherAddress,
        frame: Vec<u8>,
    },
    Local {
        packet: AetherL3Packet,
        delivered: Vec<Vec<u8>>,
        acknowledgement: Option<Vec<u8>>,
    },
}

/// End-to-end native datapath above authenticated Aether links.
///
/// A frame is decrypted at one L2 peer session, classified/forwarded by L3,
/// and, for locally addressed packets, demultiplexed into an L4 flow session.
/// Forwarded packets are re-encrypted into the next authenticated peer's
/// session. TCP/IP is not involved in this path.
pub struct NativeAetherStack {
    local_address: AetherAddress,
    l3: crate::network::AetherL3Engine,
    peers: Vec<PeerBinding>,
    flows: HashMap<AetherFlowId, FlowBinding>,
    max_peers: usize,
    max_flows: usize,
}

impl NativeAetherStack {
    pub fn new(local_address: AetherAddress) -> Self {
        Self::with_limits(local_address, DEFAULT_MAX_PEER_SESSIONS, DEFAULT_MAX_L4_SESSIONS)
    }

    pub fn with_limits(local_address: AetherAddress, max_peers: usize, max_flows: usize) -> Self {
        Self {
            local_address,
            l3: crate::network::AetherL3Engine::new(local_address),
            peers: Vec::new(),
            flows: HashMap::new(),
            max_peers: max_peers.max(1),
            max_flows: max_flows.max(1),
        }
    }

    pub fn local_address(&self) -> AetherAddress { self.local_address }
    pub fn l3(&self) -> &crate::network::AetherL3Engine { &self.l3 }
    pub fn l3_mut(&mut self) -> &mut crate::network::AetherL3Engine { &mut self.l3 }
    pub fn peer_count(&self) -> usize { self.peers.len() }
    pub fn flow_count(&self) -> usize { self.flows.len() }

    pub fn attach_peer(&mut self, node: crate::network::NodeId, address: AetherAddress, session: LinkSession) -> Result<(), NativeStackError> {
        if self.peers.iter().any(|peer| peer.node == node || peer.address == address) {
            return Err(NativeStackError::DuplicatePeer);
        }
        if self.peers.len() >= self.max_peers {
            return Err(NativeStackError::PeerCapacityExceeded);
        }
        self.peers.push(PeerBinding { node, address, session });
        Ok(())
    }

    pub fn open_flow(&mut self, peer: AetherAddress, flow: AetherFlowId, mode: AetherTransportMode) -> Result<(), NativeStackError> {
        if self.flows.contains_key(&flow) { return Ok(()); }
        if !self.peers.iter().any(|item| item.address == peer) {
            return Err(NativeStackError::PeerNotFound);
        }
        if self.flows.len() >= self.max_flows {
            return Err(NativeStackError::FlowCapacityExceeded);
        }
        self.flows.insert(flow, FlowBinding { peer, session: AetherL4Session::new(flow, mode) });
        Ok(())
    }

    pub fn send_l3(&mut self, mut packet: AetherL3Packet, now: u64) -> Result<Vec<u8>, NativeStackError> {
        let next_hop = match self.l3.prepare(&mut packet, now).map_err(NativeStackError::Route)? {
            L3Delivery::Local => return Err(NativeStackError::LocalDeliveryRequiresL4),
            L3Delivery::Forward(decision) => decision.next_hop,
        };
        let peer = self.peers.iter_mut().find(|peer| peer.address == next_hop).ok_or(NativeStackError::PeerNotFound)?;
        let payload = packet.encode().map_err(NativeStackError::L3)?;
        peer.session.encrypt(crate::network::MessageType::Data, packet.priority, crate::network::PacketFlags::RELAYABLE, &payload).map_err(NativeStackError::Frame)
    }

    pub fn send_l4(&mut self, destination: AetherAddress, flow: AetherFlowId, payload: Vec<u8>, now_ms: u64, now_seconds: u64) -> Result<Vec<u8>, NativeStackError> {
        let (segment, mode) = {
            let binding = self.flows.get_mut(&flow).ok_or(NativeStackError::FlowNotFound)?;
            if binding.peer == destination { return Err(NativeStackError::PeerNotFound); }
            let mode = binding.session.mode();
            let segment = binding.session.send(payload, now_ms).map_err(NativeStackError::L4Send)?;
            (segment, mode)
        };
        let mut packet = AetherL3Packet::new(self.local_address, destination, match mode {
            AetherTransportMode::Realtime => crate::network::TrafficPriority::Voice,
            _ => crate::network::TrafficPriority::Interactive,
        }, L3Flags::RELAYABLE, 16, segment.encode().map_err(NativeStackError::L4)?).map_err(NativeStackError::L3)?;
        packet.flow_id = flow;
        packet.sequence = segment.sequence;
        self.send_l3(packet, now_seconds)
    }

    pub fn ingest_frame(&mut self, from: crate::network::NodeId, frame: &[u8], now_seconds: u64, now_ms: u64) -> Result<NativeIngress, NativeStackError> {
        let binding = self.peers.iter_mut().find(|peer| peer.node == from).ok_or(NativeStackError::PeerNotFound)?;
        let decrypted = binding.session.decrypt(frame).map_err(NativeStackError::Frame)?;
        if decrypted.message_type != crate::network::MessageType::Data {
            return Err(NativeStackError::Frame(FrameError::UnknownMessageType));
        }
        let mut packet = AetherL3Packet::decode(&decrypted.payload).map_err(NativeStackError::L3)?;
        match self.l3.prepare(&mut packet, now_seconds).map_err(NativeStackError::Route)? {
            L3Delivery::Forward(decision) => {
                let next = self.peers.iter_mut().find(|peer| peer.address == decision.next_hop).ok_or(NativeStackError::PeerNotFound)?;
                let encoded = packet.encode().map_err(NativeStackError::L3)?;
                let frame = next.session.encrypt(crate::network::MessageType::Data, packet.priority, crate::network::PacketFlags::RELAYABLE, &encoded).map_err(NativeStackError::Frame)?;
                Ok(NativeIngress::Forward { next_hop: decision.next_hop, frame })
            }
            L3Delivery::Local => {
                let segment = AetherL4Segment::decode(&packet.payload).map_err(NativeStackError::L4)?;
                let binding = if self.flows.contains_key(&segment.flow_id) {
                    self.flows.get_mut(&segment.flow_id).expect("flow exists")
                } else {
                    if self.flows.len() >= self.max_flows { return Err(NativeStackError::FlowCapacityExceeded); }
                    self.flows.insert(segment.flow_id, FlowBinding { peer: packet.source, session: AetherL4Session::new(segment.flow_id, segment.mode) });
                    self.flows.get_mut(&segment.flow_id).expect("flow inserted")
                };
                let (delivered, ack) = binding.session.receive(segment).map_err(NativeStackError::L4Receive)?;
                let acknowledgement = if let Some(ack_segment) = ack {
                    let mut ack_packet = AetherL3Packet::new(self.local_address, packet.source, packet.priority, L3Flags::RELAYABLE, 16, ack_segment.encode().map_err(NativeStackError::L4)?).map_err(NativeStackError::L3)?;
                    ack_packet.flow_id = packet.flow_id;
                    ack_packet.sequence = ack_segment.sequence;
                    Some(self.send_l3(ack_packet, now_seconds).map_err(|error| error)?)
                } else { let _ = now_ms; None };
                Ok(NativeIngress::Local { packet, delivered, acknowledgement })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::{SessionNegotiation, SessionRole};
    use crate::network::{AetherAddress, AetherFlowId, NodeId, TrafficPriority};

    fn addr(seed: u8) -> AetherAddress { AetherAddress::from_bytes([seed; 32]) }
    fn node(seed: u8) -> NodeId { NodeId::from_bytes([seed; 32]) }

    fn paired_sessions(a: NodeId, b: NodeId) -> (LinkSession, LinkSession) {
        let initiator = SessionNegotiation::create(SessionRole::Initiator).unwrap();
        let responder = SessionNegotiation::create(SessionRole::Responder).unwrap();
        let a_pub = initiator.local_ephemeral.public_key();
        let b_pub = responder.local_ephemeral.public_key();
        let transcript: [&[u8]; 2] = [a.as_bytes(), b.as_bytes()];
        let left = initiator.finish(&b_pub, &transcript, a, b).unwrap();
        let right = responder.finish(&a_pub, &transcript, a, b).unwrap();
        (left, right)
    }

    #[test]
    fn authenticated_l3_packet_traverses_two_hops() {
        let (ab_a, ab_b) = paired_sessions(node(1), node(2));
        let (bc_b, bc_c) = paired_sessions(node(2), node(3));
        let mut a = NativeAetherStack::new(addr(1));
        let mut b = NativeAetherStack::new(addr(2));
        let mut c = NativeAetherStack::new(addr(3));
        a.attach_peer(node(2), addr(2), ab_a).unwrap();
        b.attach_peer(node(1), addr(1), ab_b).unwrap();
        b.attach_peer(node(3), addr(3), bc_b).unwrap();
        c.attach_peer(node(2), addr(2), bc_c).unwrap();
        b.l3_mut().install_route(crate::network::L3Route::new(addr(3), addr(3), 1, 1, None)).unwrap();
        a.l3_mut().install_route(crate::network::L3Route::new(addr(3), addr(2), 1, 1, None)).unwrap();
        let packet = AetherL3Packet::new(addr(1), addr(3), TrafficPriority::Interactive, L3Flags::RELAYABLE, 4, b"hello".to_vec()).unwrap();
        let first = a.send_l3(packet, 1).unwrap();
        let forwarded = match b.ingest_frame(node(1), &first, 1, 1).unwrap() { NativeIngress::Forward { frame, .. } => frame, _ => panic!("expected forwarding") };
        match c.ingest_frame(node(2), &forwarded, 1, 1).unwrap() { NativeIngress::Local { packet, .. } => assert_eq!(packet.payload, b"hello"), _ => panic!("expected local delivery") }
    }

    #[test]
    fn l4_flow_is_multiplexed_inside_native_l3() {
        let (ab_a, ab_b) = paired_sessions(node(1), node(2));
        let mut a = NativeAetherStack::new(addr(1));
        let mut b = NativeAetherStack::new(addr(2));
        a.attach_peer(node(2), addr(2), ab_a).unwrap();
        b.attach_peer(node(1), addr(1), ab_b).unwrap();
        a.l3_mut().install_route(crate::network::L3Route::new(addr(2), addr(2), 1, 1, None)).unwrap();
        let flow = AetherFlowId::from_bytes([7; 16]);
        a.open_flow(addr(2), flow, AetherTransportMode::ReliableStream).unwrap();
        let frame = a.send_l4(addr(2), flow, b"payload".to_vec(), 1, 1).unwrap();
        match b.ingest_frame(node(1), &frame, 1, 1).unwrap() { NativeIngress::Local { delivered, .. } => assert_eq!(delivered, vec![b"payload".to_vec()]), _ => panic!("expected local delivery") }
    }
}
