use crate::identity::device::DeviceId;
use crate::identity::signature::{sign, verify};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevocationReason {
    Lost = 1,
    Stolen = 2,
    Compromised = 3,
    Retired = 4,
    Administrative = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRevocation {
    pub device_id: DeviceId,

    pub reason: RevocationReason,

    pub issued_at: u64,

    pub sequence: u64,

    pub signature: Vec<u8>,
}

impl DeviceRevocation {
    pub fn create(
        identity_key: &SigningKey,

        device_id: DeviceId,

        reason: RevocationReason,

        issued_at: u64,

        sequence: u64,
    ) -> Self {
        let payload = canonical_payload(&device_id, reason, issued_at, sequence);

        let signature = sign(identity_key, &payload);

        Self {
            device_id,
            reason,
            issued_at,
            sequence,
            signature: signature.to_bytes().to_vec(),
        }
    }

    pub fn verify(&self, identity_public_key: &VerifyingKey) -> bool {
        let Ok(signature_bytes) = <[u8; 64]>::try_from(self.signature.as_slice()) else {
            return false;
        };

        let signature = Signature::from_bytes(&signature_bytes);

        let payload =
            canonical_payload(&self.device_id, self.reason, self.issued_at, self.sequence);

        verify(identity_public_key, &payload, &signature)
    }
}

fn canonical_payload(
    device_id: &DeviceId,

    reason: RevocationReason,

    issued_at: u64,

    sequence: u64,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32 + 1 + 8 + 8 + 32);

    bytes.extend_from_slice(b"AETHER-DEVICE-REVOCATION-V1");

    bytes.extend_from_slice(device_id.as_bytes());

    bytes.push(reason as u8);

    bytes.extend_from_slice(&issued_at.to_be_bytes());

    bytes.extend_from_slice(&sequence.to_be_bytes());

    bytes
}
