use hkdf::Hkdf;
use sha2::Sha256;
use std::fmt;

pub const PRIVACY_TOKEN_LENGTH: usize = 32;
pub const PRIVACY_EPOCH_SECONDS: u64 = 300;

/// Per-session, rotating privacy material for Aether control/data protocols.
///
/// The stable Aether identity must not be serialized into ordinary data-plane
/// traffic. A protocol can use this context to derive short-lived opaque tokens
/// and rotate them without changing the underlying identity.
#[derive(Clone)]
pub struct PrivacyContext {
    secret: [u8; PRIVACY_TOKEN_LENGTH],
    epoch: u64,
}

impl PrivacyContext {
    pub fn generate(epoch: u64) -> Result<Self, PrivacyError> {
        let mut secret = [0u8; PRIVACY_TOKEN_LENGTH];
        getrandom::fill(&mut secret).map_err(|_| PrivacyError::RandomnessUnavailable)?;
        Ok(Self { secret, epoch })
    }

    pub fn from_secret(secret: [u8; PRIVACY_TOKEN_LENGTH], epoch: u64) -> Self {
        Self { secret, epoch }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn rotate(&mut self, epoch: u64) -> Result<(), PrivacyError> {
        if epoch == self.epoch {
            return Ok(());
        }

        let mut next = [0u8; PRIVACY_TOKEN_LENGTH];
        getrandom::fill(&mut next).map_err(|_| PrivacyError::RandomnessUnavailable)?;
        self.secret = next;
        self.epoch = epoch;
        Ok(())
    }

    pub fn token(&self, purpose: &[u8]) -> PrivacyToken {
        PrivacyToken::derive(&self.secret, self.epoch, purpose)
    }

    pub fn token_for_epoch(&self, epoch: u64, purpose: &[u8]) -> PrivacyToken {
        PrivacyToken::derive(&self.secret, epoch, purpose)
    }
}

impl Drop for PrivacyContext {
    fn drop(&mut self) {
        self.secret.fill(0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyToken([u8; PRIVACY_TOKEN_LENGTH]);

impl PrivacyToken {
    pub fn derive(secret: &[u8; PRIVACY_TOKEN_LENGTH], epoch: u64, purpose: &[u8]) -> Self {
        let hkdf = Hkdf::<Sha256>::new(None, secret);
        let mut info = Vec::with_capacity(24 + purpose.len());
        info.extend_from_slice(b"aether:privacy:v1:");
        info.extend_from_slice(&epoch.to_be_bytes());
        info.extend_from_slice(purpose);

        let mut output = [0u8; PRIVACY_TOKEN_LENGTH];
        hkdf.expand(&info, &mut output)
            .expect("HKDF output length is fixed and valid");
        Self(output)
    }

    pub const fn as_bytes(&self) -> &[u8; PRIVACY_TOKEN_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyError {
    RandomnessUnavailable,
}

impl fmt::Display for PrivacyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RandomnessUnavailable => formatter.write_str("cryptographic randomness unavailable"),
        }
    }
}

impl std::error::Error for PrivacyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_context_is_deterministic_for_same_epoch_and_purpose() {
        let context = PrivacyContext::from_secret([7u8; PRIVACY_TOKEN_LENGTH], 10);
        assert_eq!(context.token(b"route"), context.token(b"route"));
    }

    #[test]
    fn purpose_and_epoch_change_the_token() {
        let context = PrivacyContext::from_secret([7u8; PRIVACY_TOKEN_LENGTH], 10);
        let route = context.token(b"route");
        let presence = context.token(b"presence");
        let next_epoch = context.token_for_epoch(11, b"route");
        assert_ne!(route, presence);
        assert_ne!(route, next_epoch);
    }
}
