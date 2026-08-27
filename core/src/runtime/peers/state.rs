use crate::network::NodeId;
use std::net::SocketAddr;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePeerState {
    Discovered,
    Alive,
    Stale,
    Expired,
}

#[derive(Debug, Clone)]
pub struct RuntimePeer {
    pub node_id: NodeId,

    pub address: SocketAddr,

    pub state: RuntimePeerState,

    pub first_seen: Instant,

    pub last_seen: Instant,

    pub hello_count: u64,

    pub ack_count: u64,
}

impl RuntimePeer {
    pub fn new(node_id: NodeId, address: SocketAddr, now: Instant) -> Self {
        Self {
            node_id,
            address,
            state: RuntimePeerState::Discovered,
            first_seen: now,
            last_seen: now,
            hello_count: 0,
            ack_count: 0,
        }
    }

    pub fn seen_hello(&mut self, now: Instant) {
        self.last_seen = now;

        self.hello_count = self.hello_count.saturating_add(1);

        self.state = RuntimePeerState::Alive;
    }

    pub fn seen_ack(&mut self, now: Instant) {
        self.last_seen = now;

        self.ack_count = self.ack_count.saturating_add(1);

        self.state = RuntimePeerState::Alive;
    }

    pub fn mark_stale(&mut self) {
        self.state = RuntimePeerState::Stale;
    }

    pub fn mark_expired(&mut self) {
        self.state = RuntimePeerState::Expired;
    }
}
