use crate::identity::device::DeviceId;
use crate::link::crypto::{CryptoError, EphemeralKey, derive_session_keys, transcript_hash};
use crate::link::frame::LinkSession;
use crate::network::node::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    Initiator,
    Responder,
}

pub struct SessionNegotiation {
    pub role: SessionRole,
    pub local_ephemeral: EphemeralKey,
}

impl SessionNegotiation {
    pub fn create(role: SessionRole) -> Result<Self, CryptoError> {
        Ok(Self {
            role,
            local_ephemeral: EphemeralKey::generate()?,
        })
    }

    pub fn finish(
        self,
        remote_ephemeral: &x25519_dalek::PublicKey,
        shared_transcript: &[&[u8]],
        initiator: NodeId,
        responder: NodeId,
    ) -> Result<LinkSession, CryptoError> {
        let shared_secret = self.local_ephemeral.derive_shared_secret(remote_ephemeral);

        let shared_secret_bytes = *shared_secret;

        let transcript = transcript_hash(shared_transcript);

        let keys = derive_session_keys(
            &shared_secret_bytes,
            &transcript,
            initiator,
            responder,
            self.role == SessionRole::Initiator,
        )?;

        Ok(LinkSession::new(keys))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPeer {
    pub node_id: NodeId,
    pub device_id: DeviceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Negotiating,
    Established,
    Rekeying,
    Closing,
    Closed,
}
