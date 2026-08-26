use crate::link::frame::LinkSession;
use crate::link::session::SessionState;
use crate::network::node::NodeId;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const DEFAULT_SESSION_IDLE_TIMEOUT: Duration =
    Duration::from_secs(300);

pub const DEFAULT_SESSION_MAX_AGE: Duration =
    Duration::from_secs(3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub [u8; 16]);

pub struct ManagedSession {
    pub peer: NodeId,
    pub state: SessionState,
    pub link: LinkSession,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub idle_timeout: Duration,
    pub max_age: Duration,
}

impl ManagedSession {
    pub fn new(
        peer: NodeId,
        link: LinkSession,
        now: Instant,
    ) -> Self {
        Self {
            peer,
            state: SessionState::Established,
            link,
            created_at: now,
            last_activity: now,
            idle_timeout: DEFAULT_SESSION_IDLE_TIMEOUT,
            max_age: DEFAULT_SESSION_MAX_AGE,
        }
    }

    pub fn session_id(&self) -> SessionId {
        SessionId(*self.link.session_id())
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        now.duration_since(self.last_activity)
            >= self.idle_timeout
            ||
        now.duration_since(self.created_at)
            >= self.max_age
    }

    pub fn touch(&mut self, now: Instant) {
        self.last_activity = now;
    }

    pub fn close(&mut self) {
        self.state = SessionState::Closed;
    }
}

#[derive(Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, ManagedSession>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        session: ManagedSession,
    ) -> Result<SessionId, SessionManagerError> {
        let id = session.session_id();

        if self.sessions.contains_key(&id) {
            return Err(SessionManagerError::AlreadyExists);
        }

        self.sessions.insert(id, session);

        Ok(id)
    }

    pub fn get(
        &self,
        id: &SessionId,
    ) -> Option<&ManagedSession> {
        self.sessions.get(id)
    }

    pub fn get_mut(
        &mut self,
        id: &SessionId,
    ) -> Option<&mut ManagedSession> {
        self.sessions.get_mut(id)
    }

    pub fn remove(
        &mut self,
        id: &SessionId,
    ) -> Option<ManagedSession> {
        self.sessions.remove(id)
    }

    pub fn expire(
        &mut self,
        now: Instant,
    ) -> usize {
        let expired: Vec<SessionId> = self
            .sessions
            .iter()
            .filter_map(|(id, session)| {
                session
                    .is_expired(now)
                    .then_some(*id)
            })
            .collect();

        let count = expired.len();

        for id in expired {
            self.sessions.remove(&id);
        }

        count
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionManagerError {
    AlreadyExists,
}
