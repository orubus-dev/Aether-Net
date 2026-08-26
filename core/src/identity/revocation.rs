use ed25519_dalek::{
    Signature,
    Signer,
    SigningKey,
    Verifier,
    VerifyingKey,
};

use super::device::DeviceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationReason {
    Lost,
    Stolen,
    Compromised,
    Retired,
    Administrative,
}

#[derive(Debug, Clone)]
pub struct DeviceRevocation {
    pub device_id: DeviceId,
    pub reason: RevocationReason,
    pub sequence: u64,
    pub signature: Signature,
}

impl DeviceRevocation {
    pub fn create(
        identity_key: &SigningKey,
        device_id: DeviceId,
        reason: RevocationReason,
        sequence: u64,
    ) -> Self {
        let mut payload = Vec::with_capacity(32 + 1 + 8);

        payload.extend_from_slice(device_id.as_bytes());

        payload.push(reason as u8);

        payload.extend_from_slice(&sequence.to_be_bytes());

        let signature = identity_key.sign(&payload);

        Self {
            device_id,
            reason,
            sequence,
            signature,
        }
    }

    pub fn verify(
        &self,
        identity_public_key: &VerifyingKey,
    ) -> bool {
        let mut payload = Vec::with_capacity(32 + 1 + 8);

        payload.extend_from_slice(self.device_id.as_bytes());

        payload.push(self.reason as u8);

        payload.extend_from_slice(
            &self.sequence.to_be_bytes(),
        );

        identity_public_key
            .verify(&payload, &self.signature)
            .is_ok()
    }
}