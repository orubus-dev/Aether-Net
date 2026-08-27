use crate::network::NodeId;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationState {
    Unauthenticated,
    ChallengeSent,
    ResponseReceived,
    Verified,
    Established,
    Failed,
}

#[derive(Debug, Clone)]
pub struct PeerAuthentication {
    pub node_id: NodeId,

    pub state: AuthenticationState,

    pub challenge: Option<[u8; 32]>,

    pub created_at: Instant,

    pub last_transition: Instant,

    pub attempts: u32,
}

impl PeerAuthentication {
    pub fn new(node_id: NodeId, now: Instant) -> Self {
        Self {
            node_id,
            state: AuthenticationState::Unauthenticated,
            challenge: None,
            created_at: now,
            last_transition: now,
            attempts: 0,
        }
    }

    pub fn challenge_sent(&mut self, challenge: [u8; 32], now: Instant) {
        self.challenge = Some(challenge);

        self.state = AuthenticationState::ChallengeSent;

        self.last_transition = now;

        self.attempts = self.attempts.saturating_add(1);
    }

    pub fn response_received(&mut self, now: Instant) {
        self.state = AuthenticationState::ResponseReceived;

        self.last_transition = now;
    }

    pub fn verified(&mut self, now: Instant) {
        self.state = AuthenticationState::Verified;

        self.last_transition = now;
    }

    pub fn established(&mut self, now: Instant) {
        self.state = AuthenticationState::Established;

        self.last_transition = now;
    }

    pub fn failed(&mut self, now: Instant) {
        self.state = AuthenticationState::Failed;

        self.last_transition = now;
    }
}
