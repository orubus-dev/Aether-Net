use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::{OsRng, TryRngCore};
use zeroize::Zeroizing;

#[derive(Debug)]
pub enum KeyManagerError {
    RandomnessUnavailable,
}

pub struct IdentityKeyPair {
    signing_key: Zeroizing<[u8; 32]>,
}

impl IdentityKeyPair {
    pub fn generate() -> Result<Self, KeyManagerError> {
        let mut secret = [0u8; 32];

        OsRng
            .try_fill_bytes(&mut secret)
            .map_err(|_| KeyManagerError::RandomnessUnavailable)?;

        Ok(Self {
            signing_key: Zeroizing::new(secret),
        })
    }

    pub fn from_secret(secret: [u8; 32]) -> Self {
        Self {
            signing_key: Zeroizing::new(secret),
        }
    }

    pub fn signing_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.signing_key)
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key().verifying_key()
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key().to_bytes()
    }
}