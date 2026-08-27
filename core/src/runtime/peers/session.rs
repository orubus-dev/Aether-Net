use crate::link::LinkSession;
use crate::network::NodeId;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSessionState {
    Negotiating,
    Established,
    Rekeying,
    Closing,
    Closed,
}

pub struct RuntimePeerSession {
    pub node_id: NodeId,

    pub state: RuntimeSessionState,

    pub established_at: Instant,

    pub last_activity: Instant,

    pub session: LinkSession,
}

impl RuntimePeerSession {
    pub fn new(node_id: NodeId, session: LinkSession, now: Instant) -> Self {
        Self {
            node_id,
            state: RuntimeSessionState::Established,
            established_at: now,
            last_activity: now,
            session,
        }
    }

    pub fn touch(&mut self, now: Instant) {
        self.last_activity = now;
    }

    pub fn mark_rekeying(&mut self) {
        self.state = RuntimeSessionState::Rekeying;
    }

    pub fn mark_closing(&mut self) {
        self.state = RuntimeSessionState::Closing;
    }

    pub fn mark_closed(&mut self) {
        self.state = RuntimeSessionState::Closed;
    }
}
