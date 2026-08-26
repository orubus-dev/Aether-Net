use sha2::{Digest, Sha256};

/// Length of an Aether cryptographic fingerprint in bytes.
pub const FINGERPRINT_LENGTH: usize = 32;

/// A cryptographic fingerprint used to identify a device or protocol object.
///
/// This is deliberately independent of hardware identifiers such as IMEI,
/// MAC addresses, serial numbers, or filesystem identifiers.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; FINGERPRINT_LENGTH]);

impl Fingerprint {
    /// Construct a fingerprint from arbitrary cryptographic material.
    ///
    /// SHA-256 is used as a domain-independent digest here. Higher-level
    /// protocols should provide domain separation in the input material.
    pub fn from_bytes(data: &[u8]) -> Self {
        let digest = Sha256::digest(data);

        let mut bytes = [0u8; FINGERPRINT_LENGTH];
        bytes.copy_from_slice(&digest);

        Self(bytes)
    }

    /// Construct a fingerprint using an explicit protocol domain.
    ///
    /// Domain separation prevents identical byte sequences used by different
    /// Aether subsystems from unintentionally producing the same identifier.
    pub fn from_domain(domain: &[u8], data: &[u8]) -> Self {
        let mut hasher = Sha256::new();

        hasher.update(domain);
        hasher.update([0u8]);
        hasher.update(data);

        let digest = hasher.finalize();

        let mut bytes = [0u8; FINGERPRINT_LENGTH];
        bytes.copy_from_slice(&digest);

        Self(bytes)
    }

    /// Return the raw fingerprint bytes.
    pub fn as_bytes(&self) -> &[u8; FINGERPRINT_LENGTH] {
        &self.0
    }

    /// Consume the fingerprint and return its raw bytes.
    pub fn into_bytes(self) -> [u8; FINGERPRINT_LENGTH] {
        self.0
    }
}

impl std::fmt::Debug for Fingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Fingerprint(")?;

        for byte in &self.0 {
            write!(formatter, "{byte:02x}")?;
        }

        formatter.write_str(")")
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(formatter, "{byte:02x}")?;
        }

        Ok(())
    }
}
