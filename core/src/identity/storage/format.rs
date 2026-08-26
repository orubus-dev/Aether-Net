use serde::{Deserialize, Serialize};

pub const VAULT_FORMAT_VERSION: u32 = 1;
pub const VAULT_SALT_LENGTH: usize = 32;
pub const VAULT_NONCE_LENGTH: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEnvelope {
    pub format_version: u32,
    pub algorithm: String,
    pub kdf: String,
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

impl VaultEnvelope {
    pub fn new(
        salt: Vec<u8>,
        nonce: Vec<u8>,
        ciphertext: Vec<u8>,
    ) -> Self {
        Self {
            format_version: VAULT_FORMAT_VERSION,
            algorithm: "ChaCha20-Poly1305".to_owned(),
            kdf: "Argon2id".to_owned(),
            salt,
            nonce,
            ciphertext,
        }
    }

    pub fn validate(&self) -> bool {
        self.format_version == VAULT_FORMAT_VERSION
            && self.algorithm == "ChaCha20-Poly1305"
            && self.kdf == "Argon2id"
            && self.salt.len() == VAULT_SALT_LENGTH
            && self.nonce.len() == VAULT_NONCE_LENGTH
            && !self.ciphertext.is_empty()
    }
}
