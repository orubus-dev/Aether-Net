use crate::network::AetherFlowId;

use super::native::{
    AetherL4Segment, AetherTransportMode, L4_FLAG_ACK, L4_FLAG_FIN, L4_FLAG_RESET, L4_FLAG_SYN,
    L4Error, MAX_L4_PAYLOAD,
};

pub const DEFAULT_MAX_IN_FLIGHT: usize = 64;
pub const DEFAULT_MAX_RECV_BUFFER: usize = 64;
pub const DEFAULT_INITIAL_CONGESTION_WINDOW: usize = 16 * 1024;
pub const DEFAULT_MAX_CONGESTION_WINDOW: usize = 4 * 1024 * 1024;
pub const DEFAULT_RETRANSMIT_TIMEOUT_MS: u64 = 500;
pub const DEFAULT_MIN_RETRANSMIT_TIMEOUT_MS: u64 = 100;
pub const DEFAULT_MAX_RETRANSMIT_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L4SessionState {
    Closed,
    SynSent,
    Established,
    FinSent,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L4SendError {
    Closed,
    NotEstablished,
    InvalidPayload,
    CongestionWindowFull,
    InFlightLimit,
    SequenceExhausted,
    Segment(L4Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L4ReceiveError {
    WrongFlow,
    Reset,
    BufferFull,
    ConflictingSequence,
    Segment(L4Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CongestionState {
    pub congestion_window: usize,
    pub slow_start_threshold: usize,
    pub in_flight: usize,
    pub retransmit_timeout_ms: u64,
    pub peer_receive_window: usize,
}

#[derive(Debug, Clone)]
struct OutstandingSegment {
    segment: AetherL4Segment,
    last_sent_ms: u64,
    payload_len: usize,
}

#[derive(Debug, Clone)]
struct BufferedReceive {
    sequence: u64,
    payload: Vec<u8>,
}

/// Native Aether L4 session state machine.
/// Reliable modes provide ordered delivery, cumulative acknowledgements,
/// bounded receive buffering, retransmission and AIMD congestion control.
/// Datagram and realtime modes avoid reliable retransmission by design.
pub struct AetherL4Session {
    flow_id: AetherFlowId,
    mode: AetherTransportMode,
    state: L4SessionState,
    next_send_sequence: u64,
    next_receive_sequence: u64,
    last_realtime_sequence: Option<u64>,
    peer_receive_window: usize,
    outstanding: Vec<OutstandingSegment>,
    receive_buffer: Vec<BufferedReceive>,
    max_in_flight: usize,
    max_recv_buffer: usize,
    congestion_window: usize,
    slow_start_threshold: usize,
    max_congestion_window: usize,
    retransmit_timeout_ms: u64,
    mss: usize,
}

impl AetherL4Session {
    pub fn new(flow_id: AetherFlowId, mode: AetherTransportMode) -> Self {
        Self::with_limits(
            flow_id,
            mode,
            DEFAULT_MAX_IN_FLIGHT,
            DEFAULT_MAX_RECV_BUFFER,
            DEFAULT_INITIAL_CONGESTION_WINDOW,
            DEFAULT_MAX_CONGESTION_WINDOW,
            DEFAULT_RETRANSMIT_TIMEOUT_MS,
        )
    }

    pub fn with_limits(
        flow_id: AetherFlowId,
        mode: AetherTransportMode,
        max_in_flight: usize,
        max_recv_buffer: usize,
        initial_congestion_window: usize,
        max_congestion_window: usize,
        retransmit_timeout_ms: u64,
    ) -> Self {
        let max_cwnd = max_congestion_window.max(1);
        Self {
            flow_id,
            mode,
            state: L4SessionState::Established,
            next_send_sequence: 0,
            next_receive_sequence: 0,
            last_realtime_sequence: None,
            peer_receive_window: u32::MAX as usize,
            outstanding: Vec::with_capacity(max_in_flight.max(1)),
            receive_buffer: Vec::with_capacity(max_recv_buffer.max(1)),
            max_in_flight: max_in_flight.max(1),
            max_recv_buffer: max_recv_buffer.max(1),
            congestion_window: initial_congestion_window.max(1).min(max_cwnd),
            slow_start_threshold: (max_cwnd / 2).max(1),
            max_congestion_window: max_cwnd,
            retransmit_timeout_ms: retransmit_timeout_ms.clamp(
                DEFAULT_MIN_RETRANSMIT_TIMEOUT_MS,
                DEFAULT_MAX_RETRANSMIT_TIMEOUT_MS,
            ),
            mss: 1200,
        }
    }

    pub fn flow_id(&self) -> AetherFlowId {
        self.flow_id
    }

    pub fn mode(&self) -> AetherTransportMode {
        self.mode
    }

    pub fn state(&self) -> L4SessionState {
        self.state
    }

    pub fn in_flight(&self) -> usize {
        self.outstanding.iter().map(|item| item.payload_len).sum()
    }

    pub fn buffered_receive(&self) -> usize {
        self.receive_buffer.len()
    }

    pub fn congestion(&self) -> CongestionState {
        CongestionState {
            congestion_window: self.congestion_window,
            slow_start_threshold: self.slow_start_threshold,
            in_flight: self.in_flight(),
            retransmit_timeout_ms: self.retransmit_timeout_ms,
            peer_receive_window: self.peer_receive_window,
        }
    }

    /// Start a native transport handshake on a session created in Closed state.
    pub fn connect(&mut self) -> Result<AetherL4Segment, L4SendError> {
        if self.state != L4SessionState::Closed {
            return Err(L4SendError::Closed);
        }
        self.state = L4SessionState::SynSent;
        AetherL4Segment::new(
            self.flow_id,
            self.mode,
            self.next_send_sequence,
            0,
            self.advertised_receive_window(),
            L4_FLAG_SYN,
            Vec::new(),
        )
        .map_err(L4SendError::Segment)
    }

    pub fn accept_connect(
        &mut self,
        segment: &AetherL4Segment,
    ) -> Result<AetherL4Segment, L4ReceiveError> {
        self.validate_flow(segment)?;
        if segment.flags & L4_FLAG_SYN == 0 {
            return Err(L4ReceiveError::Segment(L4Error::InvalidFlags));
        }
        self.state = L4SessionState::Established;
        self.peer_receive_window = segment.receive_window as usize;
        AetherL4Segment::new(
            self.flow_id,
            self.mode,
            0,
            segment.sequence.saturating_add(1),
            self.advertised_receive_window(),
            L4_FLAG_ACK,
            Vec::new(),
        )
        .map_err(L4ReceiveError::Segment)
    }

    pub fn confirm_connect(&mut self, segment: &AetherL4Segment) -> Result<(), L4ReceiveError> {
        self.validate_flow(segment)?;
        if self.state != L4SessionState::SynSent || segment.flags & L4_FLAG_ACK == 0 {
            return Err(L4ReceiveError::Segment(L4Error::InvalidFlags));
        }
        self.peer_receive_window = segment.receive_window as usize;
        self.state = L4SessionState::Established;
        Ok(())
    }

    pub fn send(&mut self, payload: Vec<u8>, now_ms: u64) -> Result<AetherL4Segment, L4SendError> {
        if self.state != L4SessionState::Established {
            return Err(L4SendError::NotEstablished);
        }
        if payload.len() > MAX_L4_PAYLOAD {
            return Err(L4SendError::InvalidPayload);
        }

        if matches!(
            self.mode,
            AetherTransportMode::Stream | AetherTransportMode::ReliableStream
        ) {
            if self.outstanding.len() >= self.max_in_flight {
                return Err(L4SendError::InFlightLimit);
            }
            let send_limit = self.congestion_window.min(self.peer_receive_window);
            if self.in_flight().saturating_add(payload.len()) > send_limit {
                return Err(L4SendError::CongestionWindowFull);
            }
        }

        let sequence = self.next_send_sequence;
        self.next_send_sequence = self
            .next_send_sequence
            .checked_add(1)
            .ok_or(L4SendError::SequenceExhausted)?;

        let flags = if matches!(
            self.mode,
            AetherTransportMode::Stream | AetherTransportMode::ReliableStream
        ) {
            L4_FLAG_ACK
        } else {
            0
        };

        let segment = AetherL4Segment::new(
            self.flow_id,
            self.mode,
            sequence,
            self.next_receive_sequence,
            self.advertised_receive_window(),
            flags,
            payload,
        )
        .map_err(L4SendError::Segment)?;

        if matches!(
            self.mode,
            AetherTransportMode::Stream | AetherTransportMode::ReliableStream
        ) {
            self.outstanding.push(OutstandingSegment {
                payload_len: segment.payload.len(),
                segment: segment.clone(),
                last_sent_ms: now_ms,
            });
        }

        Ok(segment)
    }

    pub fn close(&mut self, now_ms: u64) -> Result<AetherL4Segment, L4SendError> {
        if self.state != L4SessionState::Established {
            return Err(L4SendError::NotEstablished);
        }
        self.state = L4SessionState::FinSent;
        self.make_control_segment(L4_FLAG_FIN | L4_FLAG_ACK, now_ms)
    }

    pub fn reset(&mut self) -> Result<AetherL4Segment, L4SendError> {
        if self.state == L4SessionState::Reset {
            return Err(L4SendError::Closed);
        }
        self.state = L4SessionState::Reset;
        AetherL4Segment::new(
            self.flow_id,
            self.mode,
            self.next_send_sequence,
            self.next_receive_sequence,
            self.advertised_receive_window(),
            L4_FLAG_RESET,
            Vec::new(),
        )
        .map_err(L4SendError::Segment)
    }

    pub fn receive(
        &mut self,
        segment: AetherL4Segment,
    ) -> Result<(Vec<Vec<u8>>, Option<AetherL4Segment>), L4ReceiveError> {
        self.validate_flow(&segment)?;
        self.peer_receive_window = segment.receive_window as usize;

        if segment.flags & L4_FLAG_RESET != 0 {
            self.state = L4SessionState::Reset;
            return Err(L4ReceiveError::Reset);
        }

        if segment.flags & L4_FLAG_ACK != 0 {
            self.acknowledge(segment.acknowledgement);
        }

        if segment.flags & L4_FLAG_SYN != 0 {
            return Err(L4ReceiveError::Segment(L4Error::InvalidFlags));
        }

        let mut delivered = Vec::new();
        let mut acknowledgement = None;

        if !segment.payload.is_empty() || segment.flags & L4_FLAG_FIN != 0 {
            match self.mode {
                AetherTransportMode::Datagram => {
                    delivered.push(segment.payload);
                }
                AetherTransportMode::Realtime => {
                    if self
                        .last_realtime_sequence
                        .map(|last| segment.sequence > last)
                        .unwrap_or(true)
                    {
                        self.last_realtime_sequence = Some(segment.sequence);
                        delivered.push(segment.payload);
                    }
                }
                AetherTransportMode::Stream | AetherTransportMode::ReliableStream => {
                    if segment.sequence == self.next_receive_sequence {
                        if !segment.payload.is_empty() {
                            delivered.push(segment.payload);
                        }
                        self.next_receive_sequence = self.next_receive_sequence.saturating_add(1);
                        self.drain_receive_buffer(&mut delivered);
                    } else if segment.sequence > self.next_receive_sequence {
                        if self.receive_buffer.len() >= self.max_recv_buffer {
                            return Err(L4ReceiveError::BufferFull);
                        }
                        if let Some(existing) = self
                            .receive_buffer
                            .iter()
                            .find(|item| item.sequence == segment.sequence)
                        {
                            if existing.payload != segment.payload {
                                return Err(L4ReceiveError::ConflictingSequence);
                            }
                        } else {
                            self.receive_buffer.push(BufferedReceive {
                                sequence: segment.sequence,
                                payload: segment.payload,
                            });
                        }
                    }

                    acknowledgement = Some(self.make_ack()?);
                }
            }
        }

        if segment.flags & L4_FLAG_FIN != 0 {
            self.state = L4SessionState::Closed;
            acknowledgement = Some(self.make_ack()?);
        }

        Ok((delivered, acknowledgement))
    }

    /// Cumulative acknowledgement: every reliable segment with sequence <
    /// `next_sequence` has been received by the peer.
    pub fn acknowledge(&mut self, next_sequence: u64) {
        if self.outstanding.is_empty() {
            return;
        }

        let mut acked_bytes = 0usize;
        self.outstanding.retain(|item| {
            if item.segment.sequence < next_sequence {
                acked_bytes = acked_bytes.saturating_add(item.payload_len);
                false
            } else {
                true
            }
        });

        if acked_bytes > 0 {
            self.on_ack(acked_bytes);
        }
    }

    pub fn due_retransmissions(&mut self, now_ms: u64) -> Vec<AetherL4Segment> {
        if !matches!(
            self.mode,
            AetherTransportMode::Stream | AetherTransportMode::ReliableStream
        ) {
            return Vec::new();
        }

        let mut due = Vec::new();
        for item in &mut self.outstanding {
            if now_ms.saturating_sub(item.last_sent_ms) >= self.retransmit_timeout_ms {
                item.last_sent_ms = now_ms;
                due.push(item.segment.clone());
            }
        }

        if !due.is_empty() {
            self.on_loss();
        }
        due
    }

    pub fn tick(&mut self, now_ms: u64) -> usize {
        self.due_retransmissions(now_ms).len()
    }

    fn validate_flow(&self, segment: &AetherL4Segment) -> Result<(), L4ReceiveError> {
        if segment.flow_id != self.flow_id {
            Err(L4ReceiveError::WrongFlow)
        } else {
            Ok(())
        }
    }

    fn make_ack(&self) -> Result<AetherL4Segment, L4ReceiveError> {
        AetherL4Segment::new(
            self.flow_id,
            self.mode,
            0,
            self.next_receive_sequence,
            self.advertised_receive_window(),
            L4_FLAG_ACK,
            Vec::new(),
        )
        .map_err(L4ReceiveError::Segment)
    }

    fn make_control_segment(
        &self,
        flags: u8,
        _now_ms: u64,
    ) -> Result<AetherL4Segment, L4SendError> {
        AetherL4Segment::new(
            self.flow_id,
            self.mode,
            self.next_send_sequence,
            self.next_receive_sequence,
            self.advertised_receive_window(),
            flags,
            Vec::new(),
        )
        .map_err(L4SendError::Segment)
    }

    fn drain_receive_buffer(&mut self, delivered: &mut Vec<Vec<u8>>) {
        loop {
            let position = self
                .receive_buffer
                .iter()
                .position(|item| item.sequence == self.next_receive_sequence);
            let Some(position) = position else { break };
            let buffered = self.receive_buffer.swap_remove(position);
            if !buffered.payload.is_empty() {
                delivered.push(buffered.payload);
            }
            self.next_receive_sequence = self.next_receive_sequence.saturating_add(1);
        }
    }

    fn advertised_receive_window(&self) -> u32 {
        let available = self
            .max_recv_buffer
            .saturating_sub(self.receive_buffer.len())
            .saturating_mul(self.mss)
            .min(u32::MAX as usize);
        available as u32
    }

    fn on_ack(&mut self, acked_bytes: usize) {
        if self.congestion_window < self.slow_start_threshold {
            self.congestion_window = self
                .congestion_window
                .saturating_add(acked_bytes.max(1))
                .min(self.max_congestion_window);
        } else {
            let additive = ((self.mss * self.mss) / self.congestion_window.max(1)).max(1);
            self.congestion_window = self
                .congestion_window
                .saturating_add(additive.min(acked_bytes.max(1)))
                .min(self.max_congestion_window);
        }
        self.retransmit_timeout_ms = self
            .retransmit_timeout_ms
            .saturating_sub((self.retransmit_timeout_ms / 16).max(1))
            .max(DEFAULT_MIN_RETRANSMIT_TIMEOUT_MS);
    }

    fn on_loss(&mut self) {
        self.slow_start_threshold = (self.congestion_window / 2).max(self.mss);
        self.congestion_window = self.slow_start_threshold.min(self.max_congestion_window);
        self.retransmit_timeout_ms = self
            .retransmit_timeout_ms
            .saturating_mul(2)
            .min(DEFAULT_MAX_RETRANSMIT_TIMEOUT_MS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow() -> AetherFlowId {
        AetherFlowId::from_bytes([9; 16])
    }

    #[test]
    fn wire_segment_round_trip_carries_ack_and_window() {
        let segment = AetherL4Segment::new(
            flow(),
            AetherTransportMode::ReliableStream,
            8,
            7,
            8192,
            L4_FLAG_ACK,
            b"hello".to_vec(),
        )
        .unwrap();
        assert_eq!(
            AetherL4Segment::decode(&segment.encode().unwrap()).unwrap(),
            segment
        );
    }

    #[test]
    fn reliable_stream_reorders_and_drains() {
        let mut receiver = AetherL4Session::new(flow(), AetherTransportMode::ReliableStream);
        let second = AetherL4Segment::new(
            flow(),
            AetherTransportMode::ReliableStream,
            1,
            0,
            8192,
            0,
            b"b".to_vec(),
        )
        .unwrap();
        let first = AetherL4Segment::new(
            flow(),
            AetherTransportMode::ReliableStream,
            0,
            0,
            8192,
            0,
            b"a".to_vec(),
        )
        .unwrap();

        assert!(receiver.receive(second).unwrap().0.is_empty());
        let (delivered, ack) = receiver.receive(first).unwrap();
        assert_eq!(delivered, vec![b"a".to_vec(), b"b".to_vec()]);
        assert_eq!(ack.unwrap().acknowledgement, 2);
    }

    #[test]
    fn cumulative_ack_removes_outstanding_data() {
        let mut sender = AetherL4Session::new(flow(), AetherTransportMode::ReliableStream);
        sender.send(b"a".to_vec(), 0).unwrap();
        sender.send(b"b".to_vec(), 0).unwrap();
        assert_eq!(sender.in_flight(), 2);
        sender.acknowledge(2);
        assert_eq!(sender.in_flight(), 0);
    }

    #[test]
    fn retransmission_reduces_congestion_window() {
        let mut sender = AetherL4Session::new(flow(), AetherTransportMode::ReliableStream);
        sender.send(vec![1; 1000], 0).unwrap();
        let before = sender.congestion().congestion_window;
        let due = sender.due_retransmissions(DEFAULT_RETRANSMIT_TIMEOUT_MS);
        assert_eq!(due.len(), 1);
        assert!(sender.congestion().congestion_window <= before);
    }

    #[test]
    fn realtime_drops_stale_packets() {
        let mut receiver = AetherL4Session::new(flow(), AetherTransportMode::Realtime);
        let newer = AetherL4Segment::new(
            flow(),
            AetherTransportMode::Realtime,
            10,
            0,
            0,
            0,
            b"new".to_vec(),
        )
        .unwrap();
        let older = AetherL4Segment::new(
            flow(),
            AetherTransportMode::Realtime,
            9,
            0,
            0,
            0,
            b"old".to_vec(),
        )
        .unwrap();
        assert_eq!(receiver.receive(newer).unwrap().0, vec![b"new".to_vec()]);
        assert!(receiver.receive(older).unwrap().0.is_empty());
    }

    #[test]
    fn wrong_flow_is_rejected() {
        let mut receiver = AetherL4Session::new(flow(), AetherTransportMode::Datagram);
        let other = AetherL4Segment::new(
            AetherFlowId::from_bytes([8; 16]),
            AetherTransportMode::Datagram,
            0,
            0,
            0,
            0,
            b"bad".to_vec(),
        )
        .unwrap();
        assert_eq!(receiver.receive(other), Err(L4ReceiveError::WrongFlow));
    }
}
