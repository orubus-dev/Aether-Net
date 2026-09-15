use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const AETHER_ADDRESS_LENGTH: usize = 32;

/// A native Aether network-layer address.
///
/// Addresses are not IP addresses. They are stable 256-bit values derived
/// from an Aether identity's public material and can therefore remain valid
/// while a node moves between physical networks or underlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AetherAddress([u8; AETHER_ADDRESS_LENGTH]);

impl AetherAddress {
    pub const ZERO: Self = Self([0; AETHER_ADDRESS_LENGTH]);

    pub fn from_identity_bytes(identity: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"aether:l3:address:v1:");
        hasher.update(identity);
        Self(hasher.finalize().into())
    }

    pub const fn from_bytes(bytes: [u8; AETHER_ADDRESS_LENGTH]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; AETHER_ADDRESS_LENGTH] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; AETHER_ADDRESS_LENGTH]
    }

    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(AETHER_ADDRESS_LENGTH * 2);
        for byte in self.0 {
            use fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}

impl fmt::Display for AetherAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_derivation_is_stable() {
        let first = AetherAddress::from_identity_bytes(b"identity-a");
        let second = AetherAddress::from_identity_bytes(b"identity-a");
        assert_eq!(first, second);
        assert_ne!(first, AetherAddress::from_identity_bytes(b"identity-b"));
    }

    #[test]
    fn address_has_expected_size() {
        let address = AetherAddress::from_identity_bytes(b"test");
        assert_eq!(address.as_bytes().len(), AETHER_ADDRESS_LENGTH);
        assert!(!address.is_zero());
    }
}
