//! L4 live media sessions — realtime call media above the transport engine.
//!
//! A media session binds one call to one realtime L4 flow. Frames are small
//! self-describing datagrams (magic, call id, sequence, timestamp, codec),
//! carried inside `AetherTransportMode::Realtime` segments so stale frames
//! are dropped by the transport exactly like by the session. A bounded
//! jitter buffer orders arrival-side frames for playout and accounts for
//! loss. Call control rides a reliable L4 flow as JSON `CallSignal`s, so
//! the same wire format works loopback, over the native plane, and inside
//! conformance tests.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

pub const MEDIA_MAGIC: [u8; 4] = *b"AEM1";
pub const MEDIA_VERSION: u8 = 1;
pub const MEDIA_HEADER_LEN: usize = 25;
pub const MAX_MEDIA_PAYLOAD: usize = 1400;
pub const DEFAULT_JITTER_BUFFER_FRAMES: usize = 64;
pub const CALL_ID_LENGTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaError {
    TooShort,
    BadMagic,
    UnsupportedVersion,
    PayloadTooLarge,
    PayloadLengthMismatch,
    NotActive,
    BufferFull,
    InvalidTransition,
    CallNotFound,
    DuplicateCall,
    CapacityExceeded,
}

/// Codecs are negotiated once at invite time and never changed mid-call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaCodec {
    Opus,
    Pcm,
    Silence,
}

impl MediaCodec {
    pub fn tag(self) -> u8 {
        match self {
            MediaCodec::Opus => 1,
            MediaCodec::Pcm => 2,
            MediaCodec::Silence => 3,
        }
    }
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(MediaCodec::Opus),
            2 => Some(MediaCodec::Pcm),
            3 => Some(MediaCodec::Silence),
            _ => None,
        }
    }
}

/// Fixed wire header for a media datagram. 25 bytes, then the codec payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaFrameHeader {
    pub call_id: [u8; CALL_ID_LENGTH],
    pub sequence: u32,
    pub timestamp_ms: u32,
    pub codec: u8,
    pub payload_len: u16,
}

impl MediaFrameHeader {
    pub fn encode(&self) -> [u8; MEDIA_HEADER_LEN] {
        let mut out = [0u8; MEDIA_HEADER_LEN];
        out[0..4].copy_from_slice(&MEDIA_MAGIC);
        out[4..12].copy_from_slice(&self.call_id);
        out[12..16].copy_from_slice(&self.sequence.to_be_bytes());
        out[16..20].copy_from_slice(&self.timestamp_ms.to_be_bytes());
        out[20] = self.codec;
        out[21] = MEDIA_VERSION;
        out[22] = 0; // flags, reserved
        out[23..25].copy_from_slice(&self.payload_len.to_be_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, MediaError> {
        if bytes.len() < MEDIA_HEADER_LEN {
            return Err(MediaError::TooShort);
        }
        if bytes[0..4] != MEDIA_MAGIC {
            return Err(MediaError::BadMagic);
        }
        if bytes[21] != MEDIA_VERSION {
            return Err(MediaError::UnsupportedVersion);
        }
        let mut call_id = [0u8; CALL_ID_LENGTH];
        call_id.copy_from_slice(&bytes[4..12]);
        Ok(Self {
            call_id,
            sequence: u32::from_be_bytes(bytes[12..16].try_into().expect("4 bytes")),
            timestamp_ms: u32::from_be_bytes(bytes[16..20].try_into().expect("4 bytes")),
            codec: bytes[20],
            payload_len: u16::from_be_bytes(bytes[23..25].try_into().expect("2 bytes")),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFrame {
    pub sequence: u32,
    pub timestamp_ms: u32,
    pub codec: u8,
    pub payload: Vec<u8>,
}

/// Encode a frame (header + payload) into one datagram for an L4 send.
pub fn encode_frame(
    call_id: [u8; CALL_ID_LENGTH],
    sequence: u32,
    timestamp_ms: u32,
    codec: MediaCodec,
    payload: &[u8],
) -> Result<Vec<u8>, MediaError> {
    if payload.len() > MAX_MEDIA_PAYLOAD {
        return Err(MediaError::PayloadTooLarge);
    }
    let header = MediaFrameHeader {
        call_id,
        sequence,
        timestamp_ms,
        codec: codec.tag(),
        payload_len: payload.len() as u16,
    }
    .encode();
    let mut out = Vec::with_capacity(MEDIA_HEADER_LEN + payload.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(payload);
    Ok(out)
}

pub fn decode_datagram(bytes: &[u8]) -> Result<(MediaFrameHeader, MediaFrame), MediaError> {
    let header = MediaFrameHeader::decode(bytes)?;
    if bytes.len() < MEDIA_HEADER_LEN + header.payload_len as usize {
        return Err(MediaError::PayloadLengthMismatch);
    }
    let payload = bytes[MEDIA_HEADER_LEN..MEDIA_HEADER_LEN + header.payload_len as usize].to_vec();
    Ok((
        header,
        MediaFrame {
            sequence: header.sequence,
            timestamp_ms: header.timestamp_ms,
            codec: header.codec,
            payload,
        },
    ))
}

/// Bounded reorder buffer. Frames arriving out of order are held until their
/// predecessors play or the gap is declared lost. Strictly in-order playout.
#[derive(Debug)]
pub struct JitterBuffer {
    capacity: usize,
    queue: VecDeque<MediaFrame>,
    next_expected: Option<u32>,
    received: u64,
    lost: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitterPush {
    Queued,
    Late,
    OverflowDroppedOldest,
}

impl JitterBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(2),
            queue: VecDeque::new(),
            next_expected: None,
            received: 0,
            lost: 0,
        }
    }

    pub fn push(&mut self, frame: MediaFrame) -> Result<JitterPush, MediaError> {
        // Stale frames (older than what already played) never re-enter.
        if let Some(expected) = self.next_expected {
            if frame.sequence.wrapping_sub(expected) > u32::MAX / 2 {
                self.received += 1;
                return Ok(JitterPush::Late);
            }
        }
        if self.queue.iter().any(|f| f.sequence == frame.sequence) {
            self.received += 1;
            return Ok(JitterPush::Late);
        }
        self.received += 1;

        let mut result = JitterPush::Queued;
        if self.queue.len() >= self.capacity {
            // Overflow drops the oldest held frame; playout stays ordered.
            self.queue.pop_front();
            result = JitterPush::OverflowDroppedOldest;
        }
        // Insert before the first queued frame that is ahead of the new one
        // (wrapping-safe: "ahead" means within the next half of sequence space).
        let position = self
            .queue
            .iter()
            .position(|f| f.sequence.wrapping_sub(frame.sequence) < u32::MAX / 2)
            .unwrap_or(self.queue.len());
        self.queue.insert(position, frame);
        Ok(result)
    }

    /// Pop the next playable frame in strict order, declaring gaps lost.
    pub fn pop(&mut self) -> Option<MediaFrame> {
        let frame = self.queue.pop_front()?;
        match self.next_expected {
            None => {}
            Some(expected) => {
                if frame.sequence != expected {
                    self.lost = self
                        .lost
                        .saturating_add(frame.sequence.wrapping_sub(expected) as u64);
                }
            }
        }
        self.next_expected = Some(frame.sequence.wrapping_add(1));
        Some(frame)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    pub fn received(&self) -> u64 {
        self.received
    }
    pub fn lost(&self) -> u64 {
        self.lost
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MediaStats {
    pub received: u64,
    pub lost: u64,
    pub jitter_ms: u32,
    pub frames_sent: u64,
}

/// One direction of a live call sees this: sender sequencing plus an
/// arrival-side jitter buffer with an RFC-3550-style jitter estimate.
pub struct MediaSession {
    call_id: [u8; CALL_ID_LENGTH],
    codec: MediaCodec,
    send_sequence: u32,
    frames_sent: u64,
    jitter: JitterBuffer,
    last_arrival_ms: Option<u64>,
    last_timestamp_ms: Option<u32>,
    jitter_estimate: f64,
}

impl MediaSession {
    pub fn new(call_id: [u8; CALL_ID_LENGTH], codec: MediaCodec) -> Self {
        Self::with_jitter(call_id, codec, DEFAULT_JITTER_BUFFER_FRAMES)
    }

    pub fn with_jitter(call_id: [u8; CALL_ID_LENGTH], codec: MediaCodec, capacity: usize) -> Self {
        Self {
            call_id,
            codec,
            send_sequence: 0,
            frames_sent: 0,
            jitter: JitterBuffer::new(capacity),
            last_arrival_ms: None,
            last_timestamp_ms: None,
            jitter_estimate: 0.0,
        }
    }

    pub fn call_id(&self) -> [u8; CALL_ID_LENGTH] {
        self.call_id
    }
    pub fn codec(&self) -> MediaCodec {
        self.codec
    }
    pub fn buffered(&self) -> usize {
        self.jitter.len()
    }

    /// Encode one outgoing frame with sender sequencing.
    pub fn send_frame(&mut self, payload: &[u8], timestamp_ms: u32) -> Result<Vec<u8>, MediaError> {
        let sequence = self.send_sequence;
        self.send_sequence = self.send_sequence.wrapping_add(1);
        self.frames_sent += 1;
        encode_frame(self.call_id, sequence, timestamp_ms, self.codec, payload)
    }

    /// Ingest one received datagram into the jitter buffer and update the
    /// interarrival jitter estimate (mean absolute deviation of arrival
    /// spacing against sender timestamp spacing, smoothed by 1/16).
    pub fn receive_datagram(&mut self, bytes: &[u8], arrival_ms: u64) -> Result<(), MediaError> {
        let (header, frame) = decode_datagram(bytes)?;
        if header.call_id != self.call_id {
            return Err(MediaError::CallNotFound);
        }
        if let Some(last_arrival) = self.last_arrival_ms {
            let last_ts = self.last_timestamp_ms.unwrap_or(frame.timestamp_ms);
            let transit = (arrival_ms.saturating_sub(last_arrival)) as f64
                - (frame.timestamp_ms.saturating_sub(last_ts)) as f64;
            let transit = transit.abs();
            self.jitter_estimate += (transit - self.jitter_estimate) / 16.0;
        }
        self.last_arrival_ms = Some(arrival_ms);
        self.last_timestamp_ms = Some(frame.timestamp_ms);
        self.jitter.push(frame)?;
        Ok(())
    }

    /// Next in-order playable frame, if one is available.
    pub fn pop_playout(&mut self) -> Option<MediaFrame> {
        self.jitter.pop()
    }

    pub fn stats(&self) -> MediaStats {
        MediaStats {
            received: self.jitter.received(),
            lost: self.jitter.lost(),
            jitter_ms: self.jitter_estimate as u32,
            frames_sent: self.frames_sent,
        }
    }
}

/// Multiplexes concurrent calls on one socket: demux by call id.
pub struct MediaMux {
    sessions: HashMap<[u8; CALL_ID_LENGTH], MediaSession>,
    max_sessions: usize,
}

impl MediaMux {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            max_sessions: 8,
        }
    }

    pub fn open(&mut self, session: MediaSession) -> Result<(), MediaError> {
        if self.sessions.len() >= self.max_sessions {
            return Err(MediaError::CapacityExceeded);
        }
        if self
            .sessions
            .insert(session.call_id(), session)
            .is_some()
        {
            return Err(MediaError::DuplicateCall);
        }
        Ok(())
    }

    pub fn close(&mut self, call_id: &[u8; CALL_ID_LENGTH]) -> Option<MediaSession> {
        self.sessions.remove(call_id)
    }

    pub fn get_mut(
        &mut self,
        call_id: &[u8; CALL_ID_LENGTH],
    ) -> Option<&mut MediaSession> {
        self.sessions.get_mut(call_id)
    }

    pub fn call_ids(&self) -> Vec<[u8; CALL_ID_LENGTH]> {
        self.sessions.keys().copied().collect()
    }

    /// Route one datagram to its session. Returns the demuxed call id.
    pub fn demux(&mut self, bytes: &[u8], arrival_ms: u64) -> Result<[u8; CALL_ID_LENGTH], MediaError> {
        let header = MediaFrameHeader::decode(bytes)?;
        let session = self
            .sessions
            .get_mut(&header.call_id)
            .ok_or(MediaError::CallNotFound)?;
        session.receive_datagram(bytes, arrival_ms)?;
        Ok(header.call_id)
    }
}

impl Default for MediaMux {
    fn default() -> Self {
        Self::new()
    }
}

/// Call control envelopes. Carried as JSON on a reliable L4 control flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum CallSignal {
    Invite {
        call_id: String,
        codec: String,
        display_name: Option<String>,
    },
    Accept {
        call_id: String,
    },
    Reject {
        call_id: String,
    },
    Busy {
        call_id: String,
    },
    End {
        call_id: String,
    },
    Ping {
        call_id: String,
    },
    Pong {
        call_id: String,
    },
}

pub fn call_id_hex(call_id: &[u8; CALL_ID_LENGTH]) -> String {
    call_id.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn call_id_from_hex(hex: &str) -> Option<[u8; CALL_ID_LENGTH]> {
    if hex.len() != CALL_ID_LENGTH * 2 {
        return None;
    }
    let mut out = [0u8; CALL_ID_LENGTH];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        out[index] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

impl CallSignal {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("CallSignal is serializable")
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call() -> [u8; CALL_ID_LENGTH] {
        [0xab; CALL_ID_LENGTH]
    }

    #[test]
    fn header_round_trip_is_stable() {
        let bytes = encode_frame(call(), 7, 1_234, MediaCodec::Opus, b"pcm-bytes").unwrap();
        let (header, frame) = decode_datagram(&bytes).unwrap();
        assert_eq!(header.call_id, call());
        assert_eq!(header.sequence, 7);
        assert_eq!(header.timestamp_ms, 1_234);
        assert_eq!(header.codec, MediaCodec::Opus.tag());
        assert_eq!(frame.payload, b"pcm-bytes");
    }

    #[test]
    fn short_datagrams_and_wrong_magic_are_rejected() {
        assert_eq!(decode_datagram(&[0u8; 5]), Err(MediaError::TooShort));
        let mut bytes = encode_frame(call(), 0, 0, MediaCodec::Pcm, b"x").unwrap();
        bytes[0] = b'X';
        assert_eq!(decode_datagram(&bytes), Err(MediaError::BadMagic));
    }

    #[test]
    fn jitter_buffer_restores_order_and_counts_loss() {
        let mut buffer = JitterBuffer::new(16);
        for sequence in [3u32, 0, 2] {
            buffer
                .push(MediaFrame {
                    sequence,
                    timestamp_ms: sequence * 20,
                    codec: 2,
                    payload: vec![sequence as u8],
                })
                .unwrap();
        }
        let first = buffer.pop().unwrap();
        assert_eq!(first.sequence, 0);
        // Sequence 1 never arrived: popping 2 declares 1 lost, then 3 declares nothing.
        assert_eq!(buffer.pop().unwrap().sequence, 2);
        assert_eq!(buffer.lost(), 1);
        assert_eq!(buffer.pop().unwrap().sequence, 3);
        assert_eq!(buffer.lost(), 1);
        assert!(buffer.pop().is_none());
    }

    #[test]
    fn late_frames_never_replay() {
        let mut buffer = JitterBuffer::new(16);
        let frame = |sequence: u32| MediaFrame {
            sequence,
            timestamp_ms: sequence * 20,
            codec: 2,
            payload: vec![0u8; 3],
        };
        buffer.push(frame(5)).unwrap();
        buffer.pop().unwrap();
        assert_eq!(buffer.push(frame(4)).unwrap(), JitterPush::Late);
        assert_eq!(buffer.push(frame(5)).unwrap(), JitterPush::Late);
    }

    #[test]
    fn overflow_drops_oldest_but_keeps_playout_ordered() {
        let mut buffer = JitterBuffer::new(2);
        let frame = |sequence: u32| MediaFrame {
            sequence,
            timestamp_ms: sequence * 20,
            codec: 2,
            payload: vec![0u8; 3],
        };
        buffer.push(frame(0)).unwrap();
        buffer.push(frame(1)).unwrap();
        assert_eq!(buffer.push(frame(2)).unwrap(), JitterPush::OverflowDroppedOldest);
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.pop().unwrap().sequence, 1);
        assert_eq!(buffer.pop().unwrap().sequence, 2);
    }

    #[test]
    fn media_session_sequences_and_estimates_jitter() {
        let mut session = MediaSession::new(call(), MediaCodec::Opus);
        let first = session.send_frame(b"a", 100).unwrap();
        let second = session.send_frame(b"b", 120).unwrap();
        assert_eq!(session.stats().frames_sent, 2);

        session.receive_datagram(&second, 1_120).unwrap();
        session.receive_datagram(&first, 1_100).unwrap();
        assert_eq!(session.buffered(), 2);
        assert_eq!(session.pop_playout().unwrap().payload, b"a");
        assert_eq!(session.pop_playout().unwrap().payload, b"b");
        let stats = session.stats();
        assert_eq!(stats.received, 2);
        // Perfectly spaced arrivals keep the jitter estimate near zero.
        assert!(stats.jitter_ms <= 1, "jitter was {}", stats.jitter_ms);
    }

    #[test]
    fn mux_routes_by_call_id() {
        let mut mux = MediaMux::new();
        mux.open(MediaSession::new(call(), MediaCodec::Opus)).unwrap();
        let other = [0xcd; CALL_ID_LENGTH];
        mux.open(MediaSession::new(other, MediaCodec::Pcm)).unwrap();
        assert_eq!(mux.call_ids().len(), 2);

        let datagram = encode_frame(other, 0, 0, MediaCodec::Pcm, b"z").unwrap();
        assert_eq!(mux.demux(&datagram, 10).unwrap(), other);
        assert_eq!(mux.get_mut(&other).unwrap().buffered(), 1);

        let stray = encode_frame([0xff; CALL_ID_LENGTH], 0, 0, MediaCodec::Pcm, b"z").unwrap();
        assert_eq!(mux.demux(&stray, 10), Err(MediaError::CallNotFound));

        assert!(mux.close(&other).is_some());
        assert_eq!(mux.demux(&datagram, 10), Err(MediaError::CallNotFound));
    }

    #[test]
    fn call_signals_round_trip_through_json() {
        let invite = CallSignal::Invite {
            call_id: call_id_hex(&call()),
            codec: "opus".into(),
            display_name: Some("Orubus".into()),
        };
        let decoded = CallSignal::decode(&invite.encode()).unwrap();
        assert_eq!(decoded, invite);
        assert_eq!(
            call_id_from_hex(&call_id_hex(&call())).unwrap(),
            call()
        );
        assert!(call_id_from_hex("zz").is_none());
    }
}
