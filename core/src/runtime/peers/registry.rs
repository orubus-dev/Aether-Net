use crate::link::LinkSession;
use crate::network::NodeId;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct PeerSession {
    pub peer: NodeId,
    pub session: LinkSession,
    pub established_at: Instant,
    pub last_activity: Instant,
}

impl PeerSession {
    pub fn new(peer: NodeId, session: LinkSession, now: Instant) -> Self {
        Self {
            peer,
            session,
            established_at: now,
            last_activity: now,
        }
    }

    pub fn touch(&mut self, now: Instant) {
        self.last_activity = now;
    }

    pub fn idle_for(&self, now: Instant) -> Duration {
        now.duration_since(self.last_activity)
    }
}

#[derive(Default)]
pub struct PeerSessionRegistry {
    sessions: HashMap<NodeId, PeerSession>,
}

impl PeerSessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        peer: NodeId,
        session: LinkSession,
        now: Instant,
    ) -> Result<(), SessionRegistryError> {
        if self.sessions.contains_key(&peer) {
            return Err(SessionRegistryError::AlreadyExists);
        }

        self.sessions
            .insert(peer, PeerSession::new(peer, session, now));

        Ok(())
    }

    pub fn replace(&mut self, peer: NodeId, session: LinkSession, now: Instant) {
        self.sessions
            .insert(peer, PeerSession::new(peer, session, now));
    }

    pub fn get(&self, peer: &NodeId) -> Option<&PeerSession> {
        self.sessions.get(peer)
    }

    pub fn get_mut(&mut self, peer: &NodeId) -> Option<&mut PeerSession> {
        self.sessions.get_mut(peer)
    }

    pub fn remove(&mut self, peer: &NodeId) -> Option<PeerSession> {
        self.sessions.remove(peer)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn expire_idle(&mut self, now: Instant, timeout: Duration) -> Vec<NodeId> {
        let expired = self
            .sessions
            .iter()
            .filter_map(|(node_id, session)| {
                if session.idle_for(now) >= timeout {
                    Some(*node_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for peer in &expired {
            self.sessions.remove(peer);
        }

        expired
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRegistryError {
    AlreadyExists,
}
