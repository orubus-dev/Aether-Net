use crate::identity::device::DeviceId;
use crate::identity::signature::{sign, verify};
use crate::network::NodeId;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use x25519_dalek::PublicKey;

const MAGIC: [u8; 4] = *b"AEK1";
const VERSION: u8 = 1;

pub const CHALLENGE_LENGTH: usize = 32;
pub const EPHEMERAL_KEY_LENGTH: usize = 32;
pub const SIGNATURE_LENGTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerChallengeMessage {
    pub initiator: NodeId,
    pub responder: NodeId,
    pub challenge: [u8; CHALLENGE_LENGTH],
    pub initiator_ephemeral: [u8; EPHEMERAL_KEY_LENGTH],
    pub timestamp: u64,
}

impl PeerChallengeMessage {
    pub fn new(
        initiator: NodeId,
        responder: NodeId,
        initiator_ephemeral: [u8; EPHEMERAL_KEY_LENGTH],
        timestamp: u64,
    ) -> Result<Self, AuthMessageError> {
        let mut challenge = [0u8; CHALLENGE_LENGTH];

        getrandom::fill(&mut challenge).map_err(|_| AuthMessageError::RandomnessUnavailable)?;

        Ok(Self {
            initiator,
            responder,
            challenge,
            initiator_ephemeral,
            timestamp,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(142);

        output.extend_from_slice(&MAGIC);
        output.push(VERSION);
        output.push(1);

        output.extend_from_slice(self.initiator.as_bytes());

        output.extend_from_slice(self.responder.as_bytes());

        output.extend_from_slice(&self.challenge);

        output.extend_from_slice(&self.initiator_ephemeral);

        output.extend_from_slice(&self.timestamp.to_be_bytes());

        output
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, AuthMessageError> {
        if bytes.len() != 142 {
            return Err(AuthMessageError::InvalidLength);
        }

        validate_header(bytes, 1)?;

        let mut initiator = [0u8; 32];
        initiator.copy_from_slice(&bytes[6..38]);

        let mut responder = [0u8; 32];
        responder.copy_from_slice(&bytes[38..70]);

        let mut challenge = [0u8; 32];
        challenge.copy_from_slice(&bytes[70..102]);

        let mut initiator_ephemeral = [0u8; 32];
        initiator_ephemeral.copy_from_slice(&bytes[102..134]);

        let timestamp = u64::from_be_bytes(
            bytes[134..142]
                .try_into()
                .map_err(|_| AuthMessageError::InvalidLength)?,
        );

        Ok(Self {
            initiator: NodeId::from_bytes(initiator),
            responder: NodeId::from_bytes(responder),
            challenge,
            initiator_ephemeral,
            timestamp,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PeerResponseMessage {
    pub responder: NodeId,
    pub device_id: DeviceId,
    pub challenge: [u8; CHALLENGE_LENGTH],
    pub initiator_ephemeral: [u8; 32],
    pub responder_ephemeral: [u8; 32],
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

impl PeerResponseMessage {
    pub fn sign(
        signing_key: &SigningKey,
        responder: NodeId,
        device_id: DeviceId,
        challenge: [u8; CHALLENGE_LENGTH],
        initiator_ephemeral: [u8; 32],
        responder_ephemeral: [u8; 32],
        timestamp: u64,
    ) -> Self {
        let payload = response_payload(
            responder,
            device_id,
            &challenge,
            &initiator_ephemeral,
            &responder_ephemeral,
            timestamp,
        );

        let signature = sign(signing_key, &payload);

        Self {
            responder,
            device_id,
            challenge,
            initiator_ephemeral,
            responder_ephemeral,
            timestamp,
            signature: signature.to_bytes().to_vec(),
        }
    }

    pub fn verify(&self, public_key: &VerifyingKey) -> bool {
        let Ok(signature_bytes) = <[u8; 64]>::try_from(self.signature.as_slice()) else {
            return false;
        };

        let signature = Signature::from_bytes(&signature_bytes);

        let payload = response_payload(
            self.responder,
            self.device_id,
            &self.challenge,
            &self.initiator_ephemeral,
            &self.responder_ephemeral,
            self.timestamp,
        );

        verify(public_key, &payload, &signature)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(238);

        output.extend_from_slice(&MAGIC);
        output.push(VERSION);
        output.push(2);

        output.extend_from_slice(self.responder.as_bytes());

        output.extend_from_slice(self.device_id.as_bytes());

        output.extend_from_slice(&self.challenge);

        output.extend_from_slice(&self.initiator_ephemeral);

        output.extend_from_slice(&self.responder_ephemeral);

        output.extend_from_slice(&self.timestamp.to_be_bytes());

        output.extend_from_slice(&self.signature);

        output
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, AuthMessageError> {
        if bytes.len() != 238 {
            return Err(AuthMessageError::InvalidLength);
        }

        validate_header(bytes, 2)?;

        let mut responder = [0u8; 32];
        responder.copy_from_slice(&bytes[6..38]);

        let mut device_id = [0u8; 32];
        device_id.copy_from_slice(&bytes[38..70]);

        let mut challenge = [0u8; 32];
        challenge.copy_from_slice(&bytes[70..102]);

        let mut initiator_ephemeral = [0u8; 32];
        initiator_ephemeral.copy_from_slice(&bytes[102..134]);

        let mut responder_ephemeral = [0u8; 32];
        responder_ephemeral.copy_from_slice(&bytes[134..166]);

        let timestamp = u64::from_be_bytes(
            bytes[166..174]
                .try_into()
                .map_err(|_| AuthMessageError::InvalidLength)?,
        );

        let signature = bytes[174..238].to_vec();

        Ok(Self {
            responder: NodeId::from_bytes(responder),
            device_id: DeviceId::from_bytes(device_id),
            challenge,
            initiator_ephemeral,
            responder_ephemeral,
            timestamp,
            signature,
        })
    }

    pub fn initiator_key(&self) -> PublicKey {
        PublicKey::from(self.initiator_ephemeral)
    }

    pub fn responder_key(&self) -> PublicKey {
        PublicKey::from(self.responder_ephemeral)
    }
}

fn response_payload(
    responder: NodeId,
    device_id: DeviceId,
    challenge: &[u8; 32],
    initiator_ephemeral: &[u8; 32],
    responder_ephemeral: &[u8; 32],
    timestamp: u64,
) -> Vec<u8> {
    let mut output = Vec::with_capacity(32 + 32 + 32 + 32 + 32 + 8 + 24);

    output.extend_from_slice(b"AETHER-AUTH-RESPONSE-V2");

    output.extend_from_slice(responder.as_bytes());

    output.extend_from_slice(device_id.as_bytes());

    output.extend_from_slice(challenge);

    output.extend_from_slice(initiator_ephemeral);

    output.extend_from_slice(responder_ephemeral);

    output.extend_from_slice(&timestamp.to_be_bytes());

    output
}

fn validate_header(bytes: &[u8], expected: u8) -> Result<(), AuthMessageError> {
    if bytes.len() < 6 {
        return Err(AuthMessageError::InvalidLength);
    }

    if bytes[0..4] != MAGIC {
        return Err(AuthMessageError::InvalidMagic);
    }

    if bytes[4] != VERSION {
        return Err(AuthMessageError::UnsupportedVersion);
    }

    if bytes[5] != expected {
        return Err(AuthMessageError::UnknownMessage);
    }

    Ok(())
}

#[derive(Debug)]
pub enum AuthMessageError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    UnknownMessage,
    RandomnessUnavailable,
}
