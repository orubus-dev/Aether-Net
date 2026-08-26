use crate::identity::fingerprint::Fingerprint;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const DEVICE_PUBLIC_KEY_SIZE: usize = 32;
pub const DEVICE_ID_SIZE: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId([u8; DEVICE_ID_SIZE]);

impl DeviceId {
    pub fn from_public_key(public_key: &VerifyingKey) -> Self {
        let fingerprint = Fingerprint::from_domain(b"aether/device-id/v1", public_key.as_bytes());

        Self(*fingerprint.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; DEVICE_ID_SIZE] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; DEVICE_ID_SIZE] {
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

pub struct DeviceSigningKey {
    signing_key: SigningKey,
}

impl DeviceSigningKey {
    pub fn generate() -> Result<Self, DeviceError> {
        let mut secret = [0u8; 32];

        getrandom::fill(&mut secret).map_err(|_| DeviceError::RandomnessUnavailable)?;

        Ok(Self {
            signing_key: SigningKey::from_bytes(&secret),
        })
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn device_id(&self) -> DeviceId {
        DeviceId::from_public_key(&self.verifying_key())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        use ed25519_dalek::Signer;
        self.signing_key.sign(message)
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceError {
    RandomnessUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub relay: bool,
    pub gateway: bool,
    pub storage: bool,
    pub compute: bool,
    pub emergency: bool,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            relay: true,
            gateway: false,
            storage: false,
            compute: false,
            emergency: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    Active,
    Suspended,
    Revoked,
    Recovery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub device_id: DeviceId,
    pub public_key: [u8; DEVICE_PUBLIC_KEY_SIZE],
    pub created_at: u64,
    pub status: DeviceStatus,
    pub capabilities: DeviceCapabilities,
}

impl DeviceState {
    pub fn from_signing_key(
        signing_key: &DeviceSigningKey,
        created_at: u64,
        capabilities: DeviceCapabilities,
    ) -> Self {
        let public_key = signing_key.verifying_key();

        Self {
            device_id: DeviceId::from_public_key(&public_key),
            public_key: public_key.to_bytes(),
            created_at,
            status: DeviceStatus::Active,
            capabilities,
        }
    }

    pub fn public_key(&self) -> Result<VerifyingKey, ed25519_dalek::SignatureError> {
        VerifyingKey::from_bytes(&self.public_key)
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, DeviceStatus::Active)
    }

    pub fn revoke(&mut self) {
        self.status = DeviceStatus::Revoked;
    }

    pub fn suspend(&mut self) {
        self.status = DeviceStatus::Suspended;
    }

    pub fn restore(&mut self) {
        self.status = DeviceStatus::Active;
    }
}

pub fn generate_device(created_at: u64) -> Result<(DeviceSigningKey, DeviceState), DeviceError> {
    let signing_key = DeviceSigningKey::generate()?;

    let state =
        DeviceState::from_signing_key(&signing_key, created_at, DeviceCapabilities::default());

    Ok((signing_key, state))
}
