use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

/// A signed authorization connecting a device key to an Aether identity.
///
/// The actual identity key signs the device's public key.
#[derive(Debug, Clone)]
pub struct DeviceAuthorization {
    pub device_public_key: [u8; 32],
    pub signature: Signature,
}

impl DeviceAuthorization {
    pub fn create(
        identity_key: &SigningKey,
        device_public_key: [u8; 32],
    ) -> Self {
        let signature = identity_key.sign(&device_public_key);

        Self {
            device_public_key,
            signature,
        }
    }

    pub fn verify(
        &self,
        identity_public_key: &VerifyingKey,
    ) -> bool {
        identity_public_key
            .verify(
                &self.device_public_key,
                &self.signature,
            )
            .is_ok()
    }
}