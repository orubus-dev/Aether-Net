use ed25519_dalek::{SigningKey, VerifyingKey};
use zeroize::Zeroizing;

pub struct IdentityKeyPair {
    secret: Zeroizing<[u8; 32]>,
}

impl IdentityKeyPair {
    pub fn generate() -> Result<Self, KeyManagerError> {
        let mut secret = [0u8; 32];

        getrandom::fill(&mut secret).map_err(|_| KeyManagerError::RandomnessUnavailable)?;

        Ok(Self {
            secret: Zeroizing::new(secret),
        })
    }

    pub fn from_secret(secret: [u8; 32]) -> Self {
        Self {
            secret: Zeroizing::new(secret),
        }
    }

    pub fn signing_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.secret)
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key().verifying_key()
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key().to_bytes()
    }

    pub fn private_key_bytes(&self) -> [u8; 32] {
        *self.secret
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyManagerError {
    RandomnessUnavailable,
}
