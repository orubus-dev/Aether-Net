use crate::identity::fingerprint::Fingerprint;
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Length of an Ed25519 public key.
pub const PUBLIC_KEY_LENGTH: usize = 32;

/// Length of a device identifier.
pub const DEVICE_ID_LENGTH: usize = 32;

/// Cryptographic identifier for an Aether device.
///
/// A DeviceId does not contain hardware identifiers or personally
/// identifiable information.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId([u8; DEVICE_ID_LENGTH]);

impl DeviceId {
    /// Create a DeviceId from cryptographic material.
    pub fn from_public_key(public_key: &VerifyingKey) -> Self {
        let fingerprint = Fingerprint::from_domain(
            b"aether/device-id/v1",
            public_key.as_bytes(),
        );

        Self(*fingerprint.as_bytes())
    }

    /// Return the raw identifier bytes.
    pub fn as_bytes(&self) -> &[u8; DEVICE_ID_LENGTH] {
        &self.0
    }

    /// Consume the identifier and return its raw bytes.
    pub fn into_bytes(self) -> [u8; DEVICE_ID_LENGTH] {
        self.0
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DeviceId(")?;

        for byte in &self.0 {
            write!(formatter, "{byte:02x}")?;
        }

        formatter.write_str(")")
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(formatter, "{byte:02x}")?;
        }

        Ok(())
    }
}

/// Public cryptographic information belonging to an Aether device.
///
/// Private key material is deliberately excluded.
#[derive(Clone, Serialize, Deserialize)]
pub struct DevicePublicIdentity {
    pub device_id: DeviceId,
    pub public_key: [u8; PUBLIC_KEY_LENGTH],
    pub created_at: u64,
}

/// Runtime device state.
///
/// Revocation is represented explicitly so a revoked device cannot continue
/// participating in authenticated Aether operations.
#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub identity: DevicePublicIdentity,
    pub revoked: bool,
    pub revoked_at: Option<u64>,
}

impl DeviceState {
    /// Create a new device state from a generated signing key.
    pub fn new(signing_key: &SigningKey, created_at: u64) -> Self {
        let verifying_key = signing_key.verifying_key();
        let device_id = DeviceId::from_public_key(&verifying_key);

        Self {
            identity: DevicePublicIdentity {
                device_id,
                public_key: verifying_key.to_bytes(),
                created_at,
            },
            revoked: false,
            revoked_at: None,
        }
    }

    /// Return the device identifier.
    pub fn device_id(&self) -> DeviceId {
        self.identity.device_id
    }

    /// Return the public key.
    pub fn public_key(&self) -> Result<VerifyingKey, ed25519_dalek::SignatureError> {
        VerifyingKey::from_bytes(&self.identity.public_key)
    }

    /// Check whether this device is currently active.
    pub fn is_active(&self) -> bool {
        !self.revoked
    }

    /// Revoke this device.
    pub fn revoke(&mut self, timestamp: u64) {
        self.revoked = true;
        self.revoked_at = Some(timestamp);
    }
}

/// Generate a fresh Ed25519 device key.
///
/// The private key remains in memory owned by the caller. It must be moved
/// into the Aether secure storage layer before long-term persistence.
pub fn generate_key() -> Result<SigningKey, getrandom::Error> {
    let mut secret = [0u8; 32];

    getrandom::fill(&mut secret)?;

    Ok(SigningKey::from_bytes(&secret))
}

/// Generate a new device identity.
pub fn generate_device(
    created_at: u64,
) -> Result<(SigningKey, DeviceState), getrandom::Error> {
    let signing_key = generate_key()?;
    let state = DeviceState::new(&signing_key, created_at);

    Ok((signing_key, state))
}
