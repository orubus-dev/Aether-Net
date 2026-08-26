use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::{OsRng, TryRngCore};
use std::fmt;

pub const PUBLIC_KEY_SIZE: usize = 32;

/// The cryptographic root identity of an Aether participant.
///
/// The private signing key remains local and must never be transmitted
/// through Aether networking protocols.
pub struct AetherId {
    signing_key: SigningKey,
}

impl AetherId {
    /// Generate a new cryptographic Aether identity.
    pub fn generate() -> Result<Self, AetherIdentityError> {
        let mut secret = [0u8; 32];

        OsRng
            .try_fill_bytes(&mut secret)
            .map_err(|_| AetherIdentityError::RandomnessUnavailable)?;

        Ok(Self {
            signing_key: SigningKey::from_bytes(&secret),
        })
    }

    /// Construct an identity from existing private key material.
    ///
    /// The caller must obtain that material from the secure identity vault.
    pub fn from_private_key(secret: [u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&secret),
        }
    }

    /// Return the public cryptographic identity key.
    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Return the public key in raw form.
    pub fn public_key_bytes(&self) -> [u8; PUBLIC_KEY_SIZE] {
        self.public_key().to_bytes()
    }

    /// Return the raw private key.
    ///
    /// This is restricted to the identity subsystem so it can be passed
    /// directly to the secure vault. It must never be serialized into
    /// network packets or logs.
    pub(crate) fn private_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Return the canonical public identity material.
    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_SIZE] {
        let bytes = self.signing_key.verifying_key().as_bytes();
        // The verifying key is stored inside the signing key, so this
        // reference is valid for the lifetime of this identity.
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AetherIdentityError {
    RandomnessUnavailable,
}

impl fmt::Display for AetherIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RandomnessUnavailable => {
                write!(formatter, "cryptographic randomness unavailable")
            }
        }
    }
}

impl std::error::Error for AetherIdentityError {}
