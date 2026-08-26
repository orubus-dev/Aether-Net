use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::OsRng;
use std::fmt;

/// A cryptographic Aether identity.
///
/// The private signing key remains local to the identity owner.
/// Aether services should use the public key / fingerprint as the
/// externally identifiable portion of the identity.
pub struct AetherId {
    signing_key: SigningKey,
}

impl AetherId {
    /// Generate a completely new Aether identity.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Return the public identity key.
    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Return the raw public key bytes.
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public_key().to_bytes()
    }

    /// Return the private key bytes.
    ///
    /// This function exists for secure persistence/recovery code.
    /// Callers must never expose these bytes publicly.
    pub(crate) fn private_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
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