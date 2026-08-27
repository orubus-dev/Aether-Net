use crate::identity::device::{DeviceSigningKey, DeviceState};
use crate::link::{LinkSession, SessionNegotiation, SessionRole};
use crate::network::NodeId;
use crate::runtime::discovery::{PeerChallengeMessage, PeerResponseMessage};
use crate::runtime::peers::{AuthenticationState, PeerAuthentication};

use std::collections::HashMap;
use std::time::Instant;

pub struct PendingInitiator {
    pub peer: NodeId,
    pub challenge: [u8; 32],
    pub negotiation: SessionNegotiation,
    pub created_at: Instant,
}

pub struct PendingResponder {
    pub peer: NodeId,
    pub challenge: [u8; 32],
    pub negotiation: SessionNegotiation,
    pub created_at: Instant,
}

#[derive(Default)]
pub struct HandshakeManager {
    authentication: HashMap<NodeId, PeerAuthentication>,

    initiators: HashMap<NodeId, PendingInitiator>,

    responders: HashMap<NodeId, PendingResponder>,
}

impl HandshakeManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_initiator(
        &mut self,
        local_node: NodeId,

        remote_node: NodeId,

        now: Instant,
    ) -> Result<PeerChallengeMessage, HandshakeError> {
        if self.initiators.contains_key(&remote_node) {
            return Err(HandshakeError::AlreadyPending);
        }

        let negotiation = SessionNegotiation::create(SessionRole::Initiator)
            .map_err(|_| HandshakeError::Crypto)?;

        let challenge = PeerChallengeMessage::new(
            local_node,
            remote_node,
            negotiation.local_ephemeral.public_key().to_bytes(),
            unix_timestamp(),
        )
        .map_err(|_| HandshakeError::Randomness)?;

        let pending = PendingInitiator {
            peer: remote_node,

            challenge: challenge.challenge,

            negotiation,

            created_at: now,
        };

        self.initiators.insert(remote_node, pending);

        self.authentication.insert(remote_node, {
            let mut state = PeerAuthentication::new(remote_node, now);

            state.challenge_sent(challenge.challenge, now);

            state
        });

        Ok(challenge)
    }

    pub fn respond(
        &mut self,
        challenge: &PeerChallengeMessage,

        local_device: &DeviceState,

        local_device_key: &DeviceSigningKey,

        now: Instant,
    ) -> Result<(PeerResponseMessage, LinkSession), HandshakeError> {
        let peer = challenge.initiator;

        if self.responders.contains_key(&peer) {
            return Err(HandshakeError::AlreadyPending);
        }

        let negotiation = SessionNegotiation::create(SessionRole::Responder)
            .map_err(|_| HandshakeError::Crypto)?;

        let response = PeerResponseMessage::sign(
            local_device_key.signing_key(),
            NodeId::from_device(local_device),
            local_device.device_id,
            challenge.challenge,
            challenge.initiator_ephemeral,
            negotiation.local_ephemeral.public_key().to_bytes(),
            unix_timestamp(),
        );

        let transcript: Vec<&[u8]> = vec![
            challenge.initiator.as_bytes(),
            challenge.responder.as_bytes(),
            &challenge.challenge,
            &challenge.initiator_ephemeral,
            response.responder_ephemeral.as_ref(),
        ];

        let session = negotiation
            .finish(
                &response.responder_key(),
                &transcript,
                challenge.initiator,
                challenge.responder,
            )
            .map_err(|_| HandshakeError::Crypto)?;

        self.authentication.insert(peer, {
            let mut state = PeerAuthentication::new(peer, now);

            state.challenge_sent(challenge.challenge, now);

            state.response_received(now);

            state.verified(now);

            state.established(now);

            state
        });

        Ok((response, session))
    }

    pub fn complete_initiator(
        &mut self,
        response: &PeerResponseMessage,

        responder_device: &DeviceState,

        local_node: NodeId,

        responder_node: NodeId,

        now: Instant,
    ) -> Result<LinkSession, HandshakeError> {
        let pending = self
            .initiators
            .remove(&responder_node)
            .ok_or(HandshakeError::UnknownPeer)?;

        if response.responder != responder_node {
            return Err(HandshakeError::WrongPeer);
        }

        if response.challenge != pending.challenge {
            return Err(HandshakeError::ChallengeMismatch);
        }

        if response.initiator_ephemeral
            != pending.negotiation.local_ephemeral.public_key().to_bytes()
        {
            return Err(HandshakeError::EphemeralMismatch);
        }

        if response.device_id != responder_device.device_id {
            return Err(HandshakeError::DeviceMismatch);
        }

        let public_key = responder_device
            .public_key()
            .map_err(|_| HandshakeError::InvalidIdentity)?;

        if !response.verify(&public_key) {
            return Err(HandshakeError::AuthenticationFailed);
        }

        let transcript: Vec<&[u8]> = vec![
            local_node.as_bytes(),
            responder_node.as_bytes(),
            &response.challenge,
            &response.initiator_ephemeral,
            &response.responder_ephemeral,
        ];

        let session = pending
            .negotiation
            .finish(
                &response.responder_key(),
                &transcript,
                local_node,
                responder_node,
            )
            .map_err(|_| HandshakeError::Crypto)?;

        let auth = self
            .authentication
            .entry(responder_node)
            .or_insert_with(|| PeerAuthentication::new(responder_node, now));

        auth.response_received(now);

        auth.verified(now);

        auth.established(now);

        Ok(session)
    }

    pub fn authentication(&self, peer: &NodeId) -> Option<&PeerAuthentication> {
        self.authentication.get(peer)
    }

    pub fn is_established(&self, peer: &NodeId) -> bool {
        self.authentication
            .get(peer)
            .map(|state| state.state == AuthenticationState::Established)
            .unwrap_or(false)
    }

    pub fn remove_peer(&mut self, peer: &NodeId) {
        self.authentication.remove(peer);

        self.initiators.remove(peer);

        self.responders.remove(peer);
    }

    pub fn pending_count(&self) -> usize {
        self.initiators.len() + self.responders.len()
    }
}

fn unix_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug)]
pub enum HandshakeError {
    AlreadyPending,
    UnknownPeer,
    WrongPeer,
    ChallengeMismatch,
    EphemeralMismatch,
    DeviceMismatch,
    InvalidIdentity,
    AuthenticationFailed,
    Crypto,
    Randomness,
}
