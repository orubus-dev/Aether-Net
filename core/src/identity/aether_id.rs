use ed25519_dalek::{SigningKey, VerifyingKey};
use std::fmt;

pub const AETHER_ID_PUBLIC_KEY_SIZE: usize = 32;
pub const AETHER_ID_PRIVATE_KEY_SIZE: usize = 32;

pub struct AetherId {
    signing_key: SigningKey,
}

impl AetherId {
    pub fn generate() -> Result<Self, AetherIdError> {
        let mut secret = [0u8; AETHER_ID_PRIVATE_KEY_SIZE];

        getrandom::fill(&mut secret).map_err(|_| AetherIdError::RandomnessUnavailable)?;

        Ok(Self::from_private_key(secret))
    }

    pub fn from_private_key(secret: [u8; AETHER_ID_PRIVATE_KEY_SIZE]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&secret),
        }
    }

    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn public_key_bytes(&self) -> [u8; AETHER_ID_PUBLIC_KEY_SIZE] {
        self.public_key().to_bytes()
    }

    pub(crate) fn private_key_bytes(&self) -> [u8; AETHER_ID_PRIVATE_KEY_SIZE] {
        self.signing_key.to_bytes()
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    pub fn as_bytes(&self) -> [u8; AETHER_ID_PUBLIC_KEY_SIZE] {
        self.public_key_bytes()
    }
}

impl fmt::Debug for AetherId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AetherId")
            .field("public_key", &self.public_key_bytes())
            .field("private_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AetherIdError {
    RandomnessUnavailable,
}

impl fmt::Display for AetherIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RandomnessUnavailable => {
                write!(formatter, "cryptographic randomness unavailable")
            }
        }
    }
}

impl std::error::Error for AetherIdError {}
