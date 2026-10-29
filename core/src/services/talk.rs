//! L5 talk service — presence directory, message spool, call registry.
//!
//! The service layer the app sees: who is on the network (directory),
//! messages for peers not currently reachable (store-and-forward spool),
//! and the lifecycle of calls (one live line, busy semantics) with the
//! `CallSignal` exchange that binds a call to an L4 media session.
//! Pure logic, deterministic, no I/O: the runtime drives it from events.

use crate::transport::media::{CALL_ID_LENGTH, CallSignal, MediaCodec, MediaError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

pub const PRESENCE_TTL_SECONDS: u64 = 90;
pub const DEFAULT_MAX_PEERS: usize = 4_096;
pub const DEFAULT_MAX_SPOOLED_PER_PEER: usize = 64;
pub const DISPLAY_NAME_MAX: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TalkError {
    UnknownPeer,
    PeerCapacityExceeded,
    SpoolFull,
    CallNotFound,
    LineBusy,
    InvalidTransition,
    UnknownCodec,
    Media(MediaError),
    EmptyIdentity,
}

impl From<MediaError> for TalkError {
    fn from(value: MediaError) -> Self {
        TalkError::Media(value)
    }
}

/// One known peer in the directory. Numbers are the 10-digit AetherNet
/// numbers; `fingerprint` is the hex Ed25519 public key fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenceEntry {
    pub number: String,
    pub fingerprint: String,
    pub display_name: String,
    pub last_seen_unix: u64,
}

impl PresenceEntry {
    pub fn is_online(&self, now_unix: u64) -> bool {
        now_unix.saturating_sub(self.last_seen_unix) <= PRESENCE_TTL_SECONDS
    }
}

/// Who is on the network, as observed by beacons, heartbeats or registry
/// lookups. Sweeping is explicit and deterministic — no background jobs.
#[derive(Debug, Default)]
pub struct Directory {
    peers: HashMap<String, PresenceEntry>,
    max_peers: usize,
}

impl Directory {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            max_peers: DEFAULT_MAX_PEERS,
        }
    }

    pub fn with_limits(max_peers: usize) -> Self {
        Self {
            peers: HashMap::new(),
            max_peers: max_peers.max(1),
        }
    }

    /// Record a sighting. Returns true when the peer is new to the directory.
    pub fn mark_seen(
        &mut self,
        number: impl Into<String>,
        fingerprint: impl Into<String>,
        display_name: impl Into<String>,
        now_unix: u64,
    ) -> Result<bool, TalkError> {
        let number = number.into();
        let fingerprint = fingerprint.into();
        let display_name = display_name.into();
        if number.is_empty() || fingerprint.is_empty() {
            return Err(TalkError::EmptyIdentity);
        }
        let display_name = if display_name.len() > DISPLAY_NAME_MAX {
            display_name[..DISPLAY_NAME_MAX].to_string()
        } else {
            display_name
        };
        if !self.peers.contains_key(&number) && self.peers.len() >= self.max_peers {
            // Evict the stalest peer to admit the newcomer.
            let stalest = self
                .peers
                .values()
                .min_by_key(|entry| entry.last_seen_unix)
                .map(|entry| entry.number.clone());
            match stalest {
                Some(stalest) => {
                    self.peers.remove(&stalest);
                }
                None => return Err(TalkError::PeerCapacityExceeded),
            }
        }
        let fresh = !self.peers.contains_key(&number);
        self.peers.insert(
            number.clone(),
            PresenceEntry {
                number,
                fingerprint: fingerprint.into(),
                display_name,
                last_seen_unix: now_unix,
            },
        );
        Ok(fresh)
    }

    /// Remove peers silent past the TTL. Returns how many were dropped.
    pub fn sweep(&mut self, now_unix: u64) -> usize {
        let before = self.peers.len();
        self.peers.retain(|_, entry| entry.is_online(now_unix));
        before - self.peers.len()
    }

    pub fn online(&self, now_unix: u64) -> Vec<&PresenceEntry> {
        let mut entries: Vec<&PresenceEntry> = self
            .peers
            .values()
            .filter(|entry| entry.is_online(now_unix))
            .collect();
        entries.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        entries
    }

    pub fn lookup(&self, number: &str) -> Option<&PresenceEntry> {
        self.peers.get(number)
    }

    pub fn count(&self) -> usize {
        self.peers.len()
    }
}

/// A message parked for a peer that is not currently reachable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpooledMessage {
    pub id: u64,
    pub from: String,
    pub kind: String,
    pub bytes: usize,
    pub payload: Vec<u8>,
    pub queued_unix: u64,
}

/// Store-and-forward: messages park until the peer is sighted again, then
/// drain in order. The gap in coverage delays a message; it never drops it.
#[derive(Debug, Default)]
pub struct MessageSpool {
    per_peer: HashMap<String, VecDeque<SpooledMessage>>,
    next_id: u64,
    max_per_peer: usize,
}

impl MessageSpool {
    pub fn new() -> Self {
        Self {
            per_peer: HashMap::new(),
            next_id: 1,
            max_per_peer: DEFAULT_MAX_SPOOLED_PER_PEER,
        }
    }

    pub fn with_limits(max_per_peer: usize) -> Self {
        Self {
            per_peer: HashMap::new(),
            next_id: 1,
            max_per_peer: max_per_peer.max(1),
        }
    }

    pub fn park(
        &mut self,
        to: impl Into<String>,
        from: impl Into<String>,
        kind: impl Into<String>,
        payload: Vec<u8>,
        now_unix: u64,
    ) -> Result<u64, TalkError> {
        let to = to.into();
        let queue = self.per_peer.entry(to).or_default();
        if queue.len() >= self.max_per_peer {
            return Err(TalkError::SpoolFull);
        }
        let id = self.next_id;
        self.next_id += 1;
        queue.push_back(SpooledMessage {
            id,
            from: from.into(),
            kind: kind.into(),
            bytes: payload.len(),
            payload,
            queued_unix: now_unix,
        });
        Ok(id)
    }

    /// Deliver everything parked for a peer, oldest first, clearing the box.
    pub fn drain(&mut self, to: &str) -> Vec<SpooledMessage> {
        self.per_peer
            .remove(to)
            .map(|queue| queue.into_iter().collect())
            .unwrap_or_default()
    }

    pub fn pending(&self, to: &str) -> usize {
        self.per_peer.get(to).map_or(0, |queue| queue.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallRole {
    Caller,
    Callee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallState {
    Ringing,
    Active,
    Ended,
}

#[derive(Debug, Clone)]
pub struct CallEntry {
    pub call_id: [u8; CALL_ID_LENGTH],
    pub peer_number: String,
    pub role: CallRole,
    pub state: CallState,
    pub codec: MediaCodec,
    pub started_unix: u64,
    pub ended_unix: Option<u64>,
}

/// One live line. A new outgoing call while any call is active is refused
/// with `LineBusy`; an incoming invite during an active call is answered
/// with a `Busy` signal, exactly like the wire expects.
#[derive(Debug, Default)]
pub struct CallRegistry {
    calls: HashMap<[u8; CALL_ID_LENGTH], CallEntry>,
}

impl CallRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, call_id: &[u8; CALL_ID_LENGTH]) -> Option<&CallEntry> {
        self.calls.get(call_id)
    }

    pub fn active_call(&self) -> Option<&CallEntry> {
        self.calls
            .values()
            .find(|call| call.state == CallState::Active)
    }

    pub fn ringing_call(&self) -> Option<&CallEntry> {
        self.calls
            .values()
            .find(|call| call.state == CallState::Ringing)
    }

    pub fn begin_outgoing(
        &mut self,
        call_id: [u8; CALL_ID_LENGTH],
        peer_number: impl Into<String>,
        codec: MediaCodec,
        now_unix: u64,
    ) -> Result<CallSignal, TalkError> {
        if self.active_call().is_some() || self.ringing_call().is_some() {
            return Err(TalkError::LineBusy);
        }
        self.calls.insert(
            call_id,
            CallEntry {
                call_id,
                peer_number: peer_number.into(),
                role: CallRole::Caller,
                state: CallState::Ringing,
                codec,
                started_unix: now_unix,
                ended_unix: None,
            },
        );
        Ok(CallSignal::Invite {
            call_id: crate::transport::media::call_id_hex(&call_id),
            codec: codec_tag_name(codec),
            display_name: None,
        })
    }

    /// Handle a remote invite. Returns the signal to send back: `Accept`
    /// when the line is free, `Busy` otherwise.
    pub fn on_incoming_invite(
        &mut self,
        call_id: [u8; CALL_ID_LENGTH],
        peer_number: impl Into<String>,
        codec_tag: &str,
        now_unix: u64,
    ) -> Result<CallSignal, TalkError> {
        if self.active_call().is_some() {
            return Ok(CallSignal::Busy {
                call_id: crate::transport::media::call_id_hex(&call_id),
            });
        }
        let codec = codec_from_tag(codec_tag).ok_or(TalkError::UnknownCodec)?;
        self.calls.insert(
            call_id,
            CallEntry {
                call_id,
                peer_number: peer_number.into(),
                role: CallRole::Callee,
                state: CallState::Ringing,
                codec,
                started_unix: now_unix,
                ended_unix: None,
            },
        );
        Ok(CallSignal::Accept {
            call_id: crate::transport::media::call_id_hex(&call_id),
        })
    }

    pub fn accept(
        &mut self,
        call_id: &[u8; CALL_ID_LENGTH],
        now_unix: u64,
    ) -> Result<CallSignal, TalkError> {
        let call = self.calls.get_mut(call_id).ok_or(TalkError::CallNotFound)?;
        if call.state != CallState::Ringing || call.role != CallRole::Callee {
            return Err(TalkError::InvalidTransition);
        }
        call.state = CallState::Active;
        call.started_unix = now_unix;
        Ok(CallSignal::Accept {
            call_id: crate::transport::media::call_id_hex(call_id),
        })
    }

    pub fn mark_accepted(
        &mut self,
        call_id: &[u8; CALL_ID_LENGTH],
        now_unix: u64,
    ) -> Result<(), TalkError> {
        let call = self.calls.get_mut(call_id).ok_or(TalkError::CallNotFound)?;
        if call.state != CallState::Ringing || call.role != CallRole::Caller {
            return Err(TalkError::InvalidTransition);
        }
        call.state = CallState::Active;
        call.started_unix = now_unix;
        Ok(())
    }

    pub fn end(
        &mut self,
        call_id: &[u8; CALL_ID_LENGTH],
        now_unix: u64,
    ) -> Result<CallSignal, TalkError> {
        let call = self.calls.get_mut(call_id).ok_or(TalkError::CallNotFound)?;
        if call.state == CallState::Ended {
            return Err(TalkError::InvalidTransition);
        }
        call.state = CallState::Ended;
        call.ended_unix = Some(now_unix);
        Ok(CallSignal::End {
            call_id: crate::transport::media::call_id_hex(call_id),
        })
    }

    pub fn mark_ended(
        &mut self,
        call_id: &[u8; CALL_ID_LENGTH],
        now_unix: u64,
    ) -> Result<(), TalkError> {
        let call = self.calls.get_mut(call_id).ok_or(TalkError::CallNotFound)?;
        if call.state == CallState::Ended {
            return Err(TalkError::InvalidTransition);
        }
        call.state = CallState::Ended;
        call.ended_unix = Some(now_unix);
        Ok(())
    }

    /// Forget calls ended before `now_unix - keep_seconds`.
    pub fn prune(&mut self, now_unix: u64, keep_seconds: u64) -> usize {
        let before = self.calls.len();
        self.calls.retain(|_, call| {
            call.state != CallState::Ended
                || call
                    .ended_unix
                    .map(|ended| now_unix.saturating_sub(ended) <= keep_seconds)
                    .unwrap_or(true)
        });
        before - self.calls.len()
    }

    pub fn count(&self) -> usize {
        self.calls.len()
    }
}

fn codec_tag_name(codec: MediaCodec) -> String {
    match codec {
        MediaCodec::Opus => "opus".into(),
        MediaCodec::Pcm => "pcm".into(),
        MediaCodec::Silence => "silence".into(),
    }
}

fn codec_from_tag(tag: &str) -> Option<MediaCodec> {
    match tag {
        "opus" => Some(MediaCodec::Opus),
        "pcm" => Some(MediaCodec::Pcm),
        "silence" => Some(MediaCodec::Silence),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call() -> [u8; CALL_ID_LENGTH] {
        [0x11; CALL_ID_LENGTH]
    }

    #[test]
    fn directory_marks_sweeps_and_times_out_presence() {
        let mut directory = Directory::new();
        assert!(
            directory
                .mark_seen("5001234567", "aa:bb", "Orubus", 1_000)
                .unwrap()
        );
        assert!(
            !directory
                .mark_seen("5001234567", "aa:bb", "Orubus", 1_010)
                .unwrap()
        );
        assert_eq!(directory.online(1_010).len(), 1);
        assert_eq!(directory.online(1_010 + PRESENCE_TTL_SECONDS).len(), 1);
        assert_eq!(directory.online(1_010 + PRESENCE_TTL_SECONDS + 1).len(), 0);
        directory.sweep(1_010 + PRESENCE_TTL_SECONDS + 1);
        assert_eq!(directory.count(), 0);
    }

    #[test]
    fn directory_truncates_long_display_names() {
        let mut directory = Directory::new();
        directory
            .mark_seen("5001234567", "aa", "x".repeat(200), 0)
            .unwrap();
        assert_eq!(
            directory.lookup("5001234567").unwrap().display_name.len(),
            DISPLAY_NAME_MAX
        );
    }

    #[test]
    fn spool_parks_and_drains_in_order() {
        let mut spool = MessageSpool::with_limits(2);
        spool
            .park("5001234567", "5000000001", "message", b"one".to_vec(), 10)
            .unwrap();
        spool
            .park("5001234567", "5000000001", "message", b"two".to_vec(), 11)
            .unwrap();
        assert_eq!(spool.pending("5001234567"), 2);
        assert_eq!(
            spool.park("5001234567", "5000000001", "message", b"three".to_vec(), 12),
            Err(TalkError::SpoolFull)
        );
        let drained = spool.drain("5001234567");
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].payload, b"one");
        assert_eq!(drained[1].payload, b"two");
        assert_eq!(spool.pending("5001234567"), 0);
        assert!(spool.drain("5001234567").is_empty());
    }

    #[test]
    fn outgoing_call_rings_actives_and_ends() {
        let mut registry = CallRegistry::new();
        let invite = registry
            .begin_outgoing(call(), "5001234567", MediaCodec::Opus, 100)
            .unwrap();
        assert!(matches!(invite, CallSignal::Invite { .. }));
        assert!(registry.active_call().is_none());
        registry.mark_accepted(&call(), 110).unwrap();
        assert_eq!(registry.active_call().unwrap().state, CallState::Active);

        let second = [0x22; CALL_ID_LENGTH];
        assert_eq!(
            registry.begin_outgoing(second, "5007654321", MediaCodec::Opus, 120),
            Err(TalkError::LineBusy)
        );

        let end = registry.end(&call(), 200).unwrap();
        assert!(matches!(end, CallSignal::End { .. }));
        assert_eq!(registry.get(&call()).unwrap().state, CallState::Ended);
        assert!(registry.active_call().is_none());
        registry.prune(200 + 3600, 600);
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn incoming_invite_when_line_free_accepts_when_busy_sends_busy() {
        let mut registry = CallRegistry::new();
        let incoming = [0x33; CALL_ID_LENGTH];
        let reply = registry
            .on_incoming_invite(incoming, "5001234567", "opus", 100)
            .unwrap();
        assert!(matches!(reply, CallSignal::Accept { .. }));
        registry.accept(&incoming, 105).unwrap();
        assert_eq!(registry.active_call().unwrap().state, CallState::Active);

        let second = [0x44; CALL_ID_LENGTH];
        let busy = registry
            .on_incoming_invite(second, "5007654321", "opus", 110)
            .unwrap();
        assert!(matches!(busy, CallSignal::Busy { .. }));
        assert!(
            registry.get(&second).is_none(),
            "busy reply does not register a call"
        );
    }

    #[test]
    fn reject_transitions_and_unknown_codecs_fail() {
        let mut registry = CallRegistry::new();
        let incoming = [0x55; CALL_ID_LENGTH];
        registry
            .on_incoming_invite(incoming, "5001234567", "opus", 0)
            .unwrap();
        let end = registry.end(&incoming, 10).unwrap();
        assert!(matches!(end, CallSignal::End { .. }));
        assert!(
            registry
                .on_incoming_invite([0x66; CALL_ID_LENGTH], "5000000002", "aac", 0)
                .is_err()
        );
    }
}
