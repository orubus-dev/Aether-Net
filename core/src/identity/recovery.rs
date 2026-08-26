use crate::identity::device::DeviceId;
use crate::identity::fingerprint::Fingerprint;
use crate::identity::signature::{sign, verify};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const RECOVERY_GUARDIAN_COUNT: usize = 3;
pub const RECOVERY_THRESHOLD: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GuardianId([u8; 32]);

impl GuardianId {
    pub fn from_public_key(public_key: &VerifyingKey) -> Self {
        let fingerprint =
            Fingerprint::from_domain(b"aether/recovery-guardian/v1", public_key.as_bytes());

        Self(*fingerprint.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryGuardian {
    pub id: GuardianId,
    pub public_key: [u8; 32],
    pub added_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryConfiguration {
    pub guardians: Vec<RecoveryGuardian>,
    pub threshold: usize,
}

impl RecoveryConfiguration {
    pub fn new(guardians: Vec<RecoveryGuardian>) -> Result<Self, RecoveryError> {
        if guardians.len() != RECOVERY_GUARDIAN_COUNT {
            return Err(RecoveryError::InvalidGuardianCount);
        }

        Ok(Self {
            guardians,
            threshold: RECOVERY_THRESHOLD,
        })
    }

    pub fn find(&self, id: &GuardianId) -> Option<&RecoveryGuardian> {
        self.guardians.iter().find(|guardian| guardian.id == *id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryRequest {
    pub identity_fingerprint: [u8; 32],
    pub new_device: DeviceId,
    pub requested_at: u64,
    pub nonce: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryApproval {
    pub guardian_id: GuardianId,
    pub request: RecoveryRequest,
    pub signature: Vec<u8>,
}

impl RecoveryApproval {
    pub fn create(
        guardian_key: &SigningKey,
        guardian_id: GuardianId,
        request: RecoveryRequest,
    ) -> Self {
        let payload = request_payload(&request);

        let signature = sign(guardian_key, &payload);

        Self {
            guardian_id,
            request,
            signature: signature.to_bytes().to_vec(),
        }
    }

    pub fn verify(&self, configuration: &RecoveryConfiguration) -> Result<(), RecoveryError> {
        let guardian = configuration
            .find(&self.guardian_id)
            .ok_or(RecoveryError::GuardianNotAuthorized)?;

        let public_key = VerifyingKey::from_bytes(&guardian.public_key)
            .map_err(|_| RecoveryError::InvalidSignature)?;

        let Ok(signature_bytes) = <[u8; 64]>::try_from(self.signature.as_slice()) else {
            return Err(RecoveryError::InvalidSignature);
        };

        let signature = Signature::from_bytes(&signature_bytes);

        let payload = request_payload(&self.request);

        if !verify(&public_key, &payload, &signature) {
            return Err(RecoveryError::InvalidSignature);
        }

        Ok(())
    }
}

pub fn recoverable(
    configuration: &RecoveryConfiguration,
    approvals: &[RecoveryApproval],
) -> Result<bool, RecoveryError> {
    let mut guardians = Vec::<GuardianId>::new();

    for approval in approvals {
        approval.verify(configuration)?;

        if guardians.contains(&approval.guardian_id) {
            return Err(RecoveryError::DuplicateApproval);
        }

        guardians.push(approval.guardian_id);
    }

    Ok(guardians.len() >= configuration.threshold)
}

fn request_payload(request: &RecoveryRequest) -> Vec<u8> {
    let mut payload = Vec::with_capacity(32 + 32 + 8 + 32 + 32);

    payload.extend_from_slice(b"AETHER-RECOVERY-V1");

    payload.extend_from_slice(&request.identity_fingerprint);

    payload.extend_from_slice(request.new_device.as_bytes());

    payload.extend_from_slice(&request.requested_at.to_be_bytes());

    payload.extend_from_slice(&request.nonce);

    payload
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    InvalidGuardianCount,
    GuardianNotAuthorized,
    InvalidSignature,
    DuplicateApproval,
    InsufficientApprovals,
}
