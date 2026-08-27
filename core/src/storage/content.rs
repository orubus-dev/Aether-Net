use sha2::{Digest, Sha256};

pub const CONTENT_ID_LENGTH: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentId([u8; CONTENT_ID_LENGTH]);

impl ContentId {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);

        let mut output = [0u8; CONTENT_ID_LENGTH];

        output.copy_from_slice(&digest);

        Self(output)
    }

    pub fn as_bytes(&self) -> &[u8; CONTENT_ID_LENGTH] {
        &self.0
    }
}
