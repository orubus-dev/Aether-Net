use crate::identity::device::DeviceId;
use crate::identity::fingerprint::Fingerprint;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    Connect = 1,
    Send = 2,
    Receive = 3,
    Discover = 4,
    Relay = 5,
    Route = 6,
    Gateway = 7,
    Storage = 8,
    Compute = 9,
    DomainManagement = 10,
    OAuth = 11,
    Operative = 12,
    ManageIdentity = 13,
    ManageDevice = 14,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationGrant {
    pub subject_fingerprint: [u8; 32],
    pub device_id: DeviceId,
    pub capability: Capability,
    pub issued_at: u64,
    pub expires_at: Option<u64>,
    pub nonce: [u8; 32],
    pub signature: Vec<u8>,
}

impl AuthorizationGrant {
    pub fn issue(
        signing_key: &SigningKey,
        device_id: DeviceId,
        capability: Capability,
        issued_at: u64,
        expires_at: Option<u64>,
        nonce: [u8; 32],
    ) -> Self {
        let public_key = signing_key.verifying_key();

        let subject_fingerprint =
            *Fingerprint::from_domain(b"aether/authorization-subject/v1", public_key.as_bytes())
                .as_bytes();

        let payload = canonical_payload(
            &subject_fingerprint,
            &device_id,
            capability,
            issued_at,
            expires_at,
            &nonce,
        );

        let signature = signing_key.sign(&payload);

        Self {
            subject_fingerprint,
            device_id,
            capability,
            issued_at,
            expires_at,
            nonce,
            signature: signature.to_bytes().to_vec(),
        }
    }

    pub fn verify(&self, public_key: &VerifyingKey, now: u64) -> bool {
        if let Some(expiry) = self.expires_at
            && now >= expiry
        {
            return false;
        }

        let Ok(signature_bytes) = <[u8; 64]>::try_from(self.signature.as_slice()) else {
            return false;
        };

        let payload = canonical_payload(
            &self.subject_fingerprint,
            &self.device_id,
            self.capability,
            self.issued_at,
            self.expires_at,
            &self.nonce,
        );

        let signature = Signature::from_bytes(&signature_bytes);

        public_key.verify(&payload, &signature).is_ok()
    }
}

fn canonical_payload(
    subject_fingerprint: &[u8; 32],
    device_id: &DeviceId,
    capability: Capability,
    issued_at: u64,
    expires_at: Option<u64>,
    nonce: &[u8; 32],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64 + 1 + 8 + 9 + 32);

    bytes.extend_from_slice(b"AETHER-AUTHORIZATION-V1");

    bytes.extend_from_slice(subject_fingerprint);

    bytes.extend_from_slice(device_id.as_bytes());

    bytes.push(capability as u8);

    bytes.extend_from_slice(&issued_at.to_be_bytes());

    match expires_at {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }

        None => {
            bytes.push(0);
        }
    }

    bytes.extend_from_slice(nonce);

    bytes
}
