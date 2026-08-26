use argon2::Argon2;
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit},
};
use getrandom;
use zeroize::Zeroizing;

use super::format::{VAULT_NONCE_LENGTH, VAULT_SALT_LENGTH, VaultEnvelope};

const VAULT_AAD: &[u8] = b"AETHER-IDENTITY-VAULT-V1";

#[derive(Debug)]
pub enum VaultError {
    RandomnessUnavailable,
    KeyDerivationFailed,
    EncryptionFailed,
    DecryptionFailed,
    InvalidEnvelope,
    SerializationFailed,
    DeserializationFailed,
    Io(std::io::Error),
}

impl From<std::io::Error> for VaultError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// A protected identity vault.
///
/// Private identity/device material should enter this layer only briefly,
/// be encrypted, and then be persisted as authenticated ciphertext.
pub struct IdentityVault;

impl IdentityVault {
    pub fn encrypt(plaintext: &[u8], secret: &[u8]) -> Result<Vec<u8>, VaultError> {
        let salt = generate_random(VAULT_SALT_LENGTH)?;
        let nonce = generate_random(VAULT_NONCE_LENGTH)?;

        let key = derive_key(secret, &salt)?;

        let cipher = ChaCha20Poly1305::new_from_slice(key.as_ref())
            .map_err(|_| VaultError::KeyDerivationFailed)?;

        let nonce_ref = Nonce::from_slice(&nonce);

        let ciphertext = cipher
            .encrypt(
                nonce_ref,
                chacha20poly1305::aead::Payload {
                    msg: plaintext,
                    aad: VAULT_AAD,
                },
            )
            .map_err(|_| VaultError::EncryptionFailed)?;

        let envelope = VaultEnvelope::new(salt, nonce, ciphertext);

        serde_json::to_vec(&envelope).map_err(|_| VaultError::SerializationFailed)
    }

    pub fn decrypt(encoded: &[u8], secret: &[u8]) -> Result<Vec<u8>, VaultError> {
        let envelope: VaultEnvelope =
            serde_json::from_slice(encoded).map_err(|_| VaultError::DeserializationFailed)?;

        if !envelope.validate() {
            return Err(VaultError::InvalidEnvelope);
        }

        let key = derive_key(secret, &envelope.salt)?;

        let cipher = ChaCha20Poly1305::new_from_slice(key.as_ref())
            .map_err(|_| VaultError::KeyDerivationFailed)?;

        let nonce = Nonce::from_slice(&envelope.nonce);

        cipher
            .decrypt(
                nonce,
                chacha20poly1305::aead::Payload {
                    msg: envelope.ciphertext.as_ref(),
                    aad: VAULT_AAD,
                },
            )
            .map_err(|_| VaultError::DecryptionFailed)
    }

    pub fn save(
        path: impl AsRef<std::path::Path>,
        plaintext: &[u8],
        secret: &[u8],
    ) -> Result<(), VaultError> {
        let encoded = Self::encrypt(plaintext, secret)?;

        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, encoded)?;

        Ok(())
    }

    pub fn load(path: impl AsRef<std::path::Path>, secret: &[u8]) -> Result<Vec<u8>, VaultError> {
        let encoded = std::fs::read(path)?;

        Self::decrypt(&encoded, secret)
    }
}

fn generate_random(length: usize) -> Result<Vec<u8>, VaultError> {
    let mut bytes = vec![0u8; length];

    getrandom::fill(&mut bytes).map_err(|_| VaultError::RandomnessUnavailable)?;

    Ok(bytes)
}

fn derive_key(secret: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let mut key = Zeroizing::new([0u8; 32]);

    Argon2::default()
        .hash_password_into(secret, salt, key.as_mut())
        .map_err(|_| VaultError::KeyDerivationFailed)?;

    Ok(key)
}
