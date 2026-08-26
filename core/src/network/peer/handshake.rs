use crate::identity::device::{DeviceId, DeviceState};
use crate::identity::signature::{sign, verify};
use crate::network::capabilities::NodeCapabilities;
use crate::network::node::{NodeDescriptor, NodeId, NodeRole};
use crate::network::peer::challenge::Challenge;
use crate::network::protocol::ProtocolVersion;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHello {
    pub node_id: NodeId,
    pub device_id: DeviceId,
    pub public_key: [u8; 32],
    pub protocol: ProtocolVersion,
    pub capabilities: NodeCapabilities,
    pub role: NodeRole,
    pub timestamp: u64,
}

impl NodeHello {
    pub fn from_device(
        device: &DeviceState,
        capabilities: NodeCapabilities,
        role: NodeRole,
        timestamp: u64,
    ) -> Result<Self, HandshakeError> {
        let public_key = device
            .public_key()
            .map_err(|_| HandshakeError::InvalidPublicKey)?;

        Ok(Self {
            node_id: NodeId::from_device(device),
            device_id: device.device_id,
            public_key: public_key.to_bytes(),
            protocol: ProtocolVersion::CURRENT,
            capabilities,
            role,
            timestamp,
        })
    }

    pub fn descriptor(&self) -> NodeDescriptor {
        NodeDescriptor {
            node_id: self.node_id,
            device_id: self.device_id,
            protocol: self.protocol,
            capabilities: self.capabilities,
            role: self.role,
            announced_at: self.timestamp,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHelloAck {
    pub accepted: bool,
    pub protocol: ProtocolVersion,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerChallenge {
    pub challenge: [u8; 32],
    pub issued_at: u64,
}

impl PeerChallenge {
    pub fn generate(issued_at: u64) -> Result<Self, HandshakeError> {
        let challenge = Challenge::generate().map_err(|_| HandshakeError::RandomnessUnavailable)?;

        Ok(Self {
            challenge: *challenge.as_bytes(),
            issued_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerResponse {
    pub node_id: NodeId,
    pub device_id: DeviceId,
    pub signature: Vec<u8>,
}

impl PeerResponse {
    pub fn sign(
        signing_key: &SigningKey,
        node_id: NodeId,
        device_id: DeviceId,
        challenge: &PeerChallenge,
    ) -> Self {
        let transcript = authentication_transcript(node_id, device_id, challenge);

        let signature = sign(signing_key, &transcript);

        Self {
            node_id,
            device_id,
            signature: signature.to_bytes().to_vec(),
        }
    }

    pub fn verify(&self, public_key: &VerifyingKey, challenge: &PeerChallenge) -> bool {
        let Ok(signature_bytes) = <[u8; 64]>::try_from(self.signature.as_slice()) else {
            return false;
        };

        let signature = Signature::from_bytes(&signature_bytes);

        let transcript = authentication_transcript(self.node_id, self.device_id, challenge);

        verify(public_key, &transcript, &signature)
    }
}

fn authentication_transcript(
    node_id: NodeId,
    device_id: DeviceId,
    challenge: &PeerChallenge,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32 + 32 + 32 + 16);

    bytes.extend_from_slice(b"AETHER-PEER-AUTH-V1");

    bytes.extend_from_slice(node_id.as_bytes());

    bytes.extend_from_slice(device_id.as_bytes());

    bytes.extend_from_slice(&challenge.challenge);

    bytes.extend_from_slice(&challenge.issued_at.to_be_bytes());

    bytes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeError {
    InvalidPublicKey,
    RandomnessUnavailable,
}
