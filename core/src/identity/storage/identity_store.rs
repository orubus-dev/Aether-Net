use super::vault::{IdentityVault, VaultError};
use crate::identity::aether_id::AetherId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct StoredIdentity {
    version: u32,
    private_key: [u8; 32],
}

const STORED_IDENTITY_VERSION: u32 = 1;

pub struct IdentityStore;

impl IdentityStore {
    pub fn serialize_identity(identity: &AetherId, secret: &[u8]) -> Result<Vec<u8>, VaultError> {
        let record = StoredIdentity {
            version: STORED_IDENTITY_VERSION,
            private_key: identity.private_key_bytes(),
        };

        let plaintext = serde_json::to_vec(&record).map_err(|_| VaultError::SerializationFailed)?;

        IdentityVault::encrypt(&plaintext, secret)
    }

    pub fn deserialize_identity(encrypted: &[u8], secret: &[u8]) -> Result<AetherId, VaultError> {
        let plaintext = IdentityVault::decrypt(encrypted, secret)?;

        let record: StoredIdentity =
            serde_json::from_slice(&plaintext).map_err(|_| VaultError::DeserializationFailed)?;

        if record.version != STORED_IDENTITY_VERSION {
            return Err(VaultError::InvalidEnvelope);
        }

        Ok(AetherId::from_private_key(record.private_key))
    }
}
