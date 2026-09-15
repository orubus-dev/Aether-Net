use hkdf::Hkdf;
use sha2_legacy::{Digest, Sha256};
use std::fmt;

pub const PRIVACY_TOKEN_LENGTH: usize = 32;
pub const PRIVACY_EPOCH_SECONDS: u64 = 300;

#[derive(Clone)]
pub struct PrivacyContext { secret: [u8; PRIVACY_TOKEN_LENGTH], epoch: u64 }
impl PrivacyContext {
    pub fn generate(epoch: u64) -> Result<Self, PrivacyError> {
        let mut secret = [0u8; PRIVACY_TOKEN_LENGTH]; getrandom::fill(&mut secret).map_err(|_| PrivacyError::RandomnessUnavailable)?; Ok(Self { secret, epoch })
    }
    pub const fn from_secret(secret: [u8; PRIVACY_TOKEN_LENGTH], epoch: u64) -> Self { Self { secret, epoch } }
    pub const fn epoch(&self) -> u64 { self.epoch }
    pub fn rotate(&mut self, epoch: u64) -> Result<(), PrivacyError> {
        if epoch == self.epoch { return Ok(()); }
        let mut next = [0u8; PRIVACY_TOKEN_LENGTH]; getrandom::fill(&mut next).map_err(|_| PrivacyError::RandomnessUnavailable)?; self.secret = next; self.epoch = epoch; Ok(())
    }
    pub fn token(&self, purpose: &[u8]) -> PrivacyToken { PrivacyToken::derive(&self.secret, self.epoch, purpose) }
}
impl Drop for PrivacyContext { fn drop(&mut self) { self.secret.fill(0); } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyToken([u8; PRIVACY_TOKEN_LENGTH]);
impl PrivacyToken {
    pub fn derive(secret: &[u8; PRIVACY_TOKEN_LENGTH], epoch: u64, purpose: &[u8]) -> Self {
        let mut purpose_hash = Sha256::new(); purpose_hash.update(purpose); let purpose_digest = purpose_hash.finalize();
        let hkdf = Hkdf::<Sha256>::new(None, secret);
        let mut info = [0u8; 58]; let prefix = b"aether:privacy:v1:";
        info[..prefix.len()].copy_from_slice(prefix); info[prefix.len()..prefix.len()+8].copy_from_slice(&epoch.to_be_bytes()); info[prefix.len()+8..].copy_from_slice(&purpose_digest);
        let mut output = [0u8; PRIVACY_TOKEN_LENGTH]; hkdf.expand(&info, &mut output).expect("HKDF output length is fixed and valid"); Self(output)
    }
    pub const fn as_bytes(&self) -> &[u8; PRIVACY_TOKEN_LENGTH] { &self.0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyError { RandomnessUnavailable }
impl fmt::Display for PrivacyError { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { match self { Self::RandomnessUnavailable => formatter.write_str("cryptographic randomness unavailable") } } }
impl std::error::Error for PrivacyError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_is_stable_for_same_epoch_and_purpose() {
        let context = PrivacyContext::from_secret([7; PRIVACY_TOKEN_LENGTH], 10);
        assert_eq!(context.token(b"routing"), context.token(b"routing"));
    }
    #[test]
    fn purpose_and_epoch_domain_separate_tokens() {
        let a = PrivacyContext::from_secret([7; PRIVACY_TOKEN_LENGTH], 10);
        let b = PrivacyContext::from_secret([7; PRIVACY_TOKEN_LENGTH], 11);
        assert_ne!(a.token(b"routing"), a.token(b"storage"));
        assert_ne!(a.token(b"routing"), b.token(b"routing"));
    }
}
