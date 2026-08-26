use crate::identity::aether_id::AetherId;
use crate::identity::device::DeviceId;
use crate::identity::signature::{sign, verify};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Permissions that can be granted to an Aether identity or device.
///
/// These are protocol capabilities, not user roles. Higher-level services
/// decide which capabilities are appropriate for a particular operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    Connect,
    Relay,
    Send,
    Receive,
    Discover,
    Route,
    Operate,
    Gateway,
    Storage,
    Compute,
    ManageIdentity,
    ManageDevice,
}

/// A signed authorization grant.
///
/// The grant binds a capability to an identity/device for a bounded period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationGrant {
    pub subject: AetherId,
    pub device: DeviceId,
    pub capability: Capability,
    pub issued_at: u64,
    pub expires_at: Option<u64>,
    pub nonce: [u8; 32],
    pub signature: Vec<u8>,
}

/// Canonical bytes used when signing an authorization grant.
///
/// The signature is deliberately calculated over the complete semantic
/// contents rather than over an in-memory representation.
fn signing_bytes(
    subject: &AetherId,
    device: &DeviceId,
    capability: Capability,
    issued_at: u64,
    expires_at: Option<u64>,
    nonce: &[u8; 32],
) -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.extend_from_slice(subject.as_bytes());
    bytes.extend_from_slice(device.as_bytes());
    bytes.push(capability as u8);
    bytes.extend_from_slice(&issued_at.to_be_bytes());

    match expires_at {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }

    bytes.extend_from_slice(nonce);

    bytes
}

/// Create a signed authorization grant.
pub fn issue(
    signing_key: &SigningKey,
    subject: AetherId,
    device: DeviceId,
    capability: Capability,
    issued_at: u64,
    expires_at: Option<u64>,
    nonce: [u8; 32],
) -> AuthorizationGrant {
    let message = signing_bytes(
        &subject,
        &device,
        capability,
        issued_at,
        expires_at,
        &nonce,
    );

    let signature = sign(signing_key, &message);

    AuthorizationGrant {
        subject,
        device,
        capability,
        issued_at,
        expires_at,
        nonce,
        signature: signature.to_bytes().to_vec(),
    }
}

/// Verify an authorization grant against the identity public key.
pub fn verify_grant(
    public_key: &VerifyingKey,
    grant: &AuthorizationGrant,
    now: u64,
) -> bool {
    if grant.expires_at.is_some_and(|expiry| now >= expiry) {
        return false;
    }

    let message = signing_bytes(
        &grant.subject,
        &grant.device,
        grant.capability,
        grant.issued_at,
        grant.expires_at,
        &grant.nonce,
    );

    let Ok(signature_bytes) = <[u8; 64]>::try_from(grant.signature.as_slice()) else {
        return false;
    };

    let signature = Signature::from_bytes(&signature_bytes);

    verify(public_key, &message, &signature)
}
