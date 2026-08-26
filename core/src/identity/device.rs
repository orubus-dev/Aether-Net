use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

/// Stable cryptographic identifier for a device.
///
/// This is derived from the device's public identity key and does not
/// contain personally identifying information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId([u8; 32]);

impl DeviceId {
    pub fn from_public_key(public_key: &VerifyingKey) -> Self {
        let digest = Sha256::digest(public_key.as_bytes());

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&digest);

        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// A device's long-lived signing identity.
///
/// The private signing key must never be transmitted across the Aether
/// network.
pub struct DeviceSigningKey {
    signing_key: SigningKey,
}

impl DeviceSigningKey {
    /// Generate a new cryptographically random device signing key.
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];

        getrandom::fill(&mut seed)
            .expect("cryptographic random number generation failed");

        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn device_id(&self) -> DeviceId {
        DeviceId::from_public_key(&self.verifying_key())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}

/// An ephemeral/rotatable X25519 key used for secure transport.
///
/// This key is intentionally separate from the device signing identity.
pub struct DeviceTransportKey {
    secret: StaticSecret,
    public: X25519PublicKey,
}

impl DeviceTransportKey {
    /// Generate a fresh transport key.
    pub fn generate() -> Self {
        let mut secret_bytes = [0u8; 32];

        getrandom::fill(&mut secret_bytes)
            .expect("cryptographic random number generation failed");

        let secret = StaticSecret::from(secret_bytes);
        let public = X25519PublicKey::from(&secret);

        Self { secret, public }
    }

    pub fn public_key(&self) -> X25519PublicKey {
        self.public
    }

    /// Derive a shared secret with another X25519 public key.
    pub fn derive_shared_secret(
        &self,
        remote_public_key: &X25519PublicKey,
    ) -> [u8; 32] {
        self.secret
            .diffie_hellman(remote_public_key)
            .to_bytes()
    }
}

/// Capabilities advertised by an Aether device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Public device information.
///
/// This structure is safe to advertise to peers.
#[derive(Debug, Clone)]
pub struct DeviceDescriptor {
    pub device_id: DeviceId,
    pub signing_public_key: VerifyingKey,
    pub transport_public_key: X25519PublicKey,
    pub capabilities: DeviceCapabilities,
}

/// A signed statement proving possession of a device signing key.
#[derive(Debug, Clone)]
pub struct DeviceProof {
    pub device_id: DeviceId,
    pub signature: Signature,
}

impl DeviceProof {
    pub fn create(
        signing_key: &DeviceSigningKey,
        challenge: &[u8],
    ) -> Self {
        Self {
            device_id: signing_key.device_id(),
            signature: signing_key.sign(challenge),
        }
    }

    pub fn verify(
        &self,
        public_key: &VerifyingKey,
        challenge: &[u8],
    ) -> bool {
        public_key.verify(challenge, &self.signature).is_ok()
    }
}