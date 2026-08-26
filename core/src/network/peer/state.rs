use crate::network::node::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    Discovered,
    Connecting,
    Challenging,
    Authenticating,
    Authenticated,
    Established,
    Closing,
    Closed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerFailure {
    InvalidIdentity,
    InvalidChallenge,
    ProtocolMismatch,
    UnauthorizedDevice,
    Timeout,
    MalformedMessage,
}

#[derive(Debug, Clone)]
pub struct Peer {
    node_id: NodeId,
    state: PeerState,
    last_activity: u64,
}

impl Peer {
    pub fn discovered(node_id: NodeId, timestamp: u64) -> Self {
        Self {
            node_id,
            state: PeerState::Discovered,
            last_activity: timestamp,
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn state(&self) -> PeerState {
        self.state
    }

    pub fn last_activity(&self) -> u64 {
        self.last_activity
    }

    pub fn transition(&mut self, next: PeerState, timestamp: u64) {
        self.state = next;
        self.last_activity = timestamp;
    }

    pub fn fail(&mut self, timestamp: u64) {
        self.transition(PeerState::Failed, timestamp);
    }
}
