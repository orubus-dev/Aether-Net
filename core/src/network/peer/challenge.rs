use getrandom;

pub const CHALLENGE_LENGTH: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Challenge([u8; CHALLENGE_LENGTH]);

impl Challenge {
    pub fn generate() -> Result<Self, ChallengeError> {
        let mut value = [0u8; CHALLENGE_LENGTH];

        getrandom::fill(&mut value)
            .map_err(|_| ChallengeError::RandomnessUnavailable)?;

        Ok(Self(value))
    }

    pub fn as_bytes(&self) -> &[u8; CHALLENGE_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeError {
    RandomnessUnavailable,
}
