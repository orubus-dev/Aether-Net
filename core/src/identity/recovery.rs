use crate::identity::device::DeviceId;
use crate::identity::fingerprint::Fingerprint;
use crate::identity::signature::{sign, verify};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Maximum number of recovery guardians supported by the protocol.
pub const MAX_GUARDIANS: usize = 3;

/// Number of guardian approvals required for recovery.
pub const REQUIRED_APPROVALS: usize = 2;

/// Cryptographic identifier for a recovery guardian.
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

/// A guardian authorized to participate in identity recovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryGuardian {
    pub id: GuardianId,
    pub public_key: [u8; 32],
    pub added_at: u64,
}

/// Recovery configuration associated with an Aether identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryConfiguration {
    pub guardians: Vec<RecoveryGuardian>,
    pub threshold: usize,
}

impl RecoveryConfiguration {
    /// Create a new recovery configuration.
    ///
    /// The production protocol currently uses a 2-of-3 model.
    pub fn new(guardians: Vec<RecoveryGuardian>) -> Result<Self, RecoveryError> {
        if guardians.len() != MAX_GUARDIANS {
            return Err(RecoveryError::InvalidGuardianCount);
        }

        Ok(Self {
            guardians,
            threshold: REQUIRED_APPROVALS,
        })
    }

    /// Determine whether a guardian belongs to this recovery configuration.
    pub fn contains(&self, guardian_id: &GuardianId) -> bool {
        self.guardians.iter().any(|guardian| &guardian.id == guardian_id)
    }
}

/// A signed request to recover an Aether identity onto another device.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryRequest {
    pub identity_fingerprint: [u8; 32],
    pub new_device: DeviceId,
    pub requested_at: u64,
    pub nonce: [u8; 32],
}

/// Guardian authorization of a recovery request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryApproval {
    pub guardian_id: GuardianId,
    pub request: RecoveryRequest,
    pub signature: Vec<u8>,
}

/// Recovery errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    InvalidGuardianCount,
    GuardianNotAuthorized,
    InvalidSignature,
    DuplicateApproval,
    InsufficientApprovals,
}

/// Produce canonical bytes for a recovery request.
fn request_bytes(request: &RecoveryRequest) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32 + 32 + 8 + 32);

    bytes.extend_from_slice(&request.identity_fingerprint);
    bytes.extend_from_slice(request.new_device.as_bytes());
    bytes.extend_from_slice(&request.requested_at.to_be_bytes());
    bytes.extend_from_slice(&request.nonce);

    bytes
}

/// Create a signed guardian approval.
pub fn approve(
    guardian_key: &SigningKey,
    guardian_id: GuardianId,
    request: RecoveryRequest,
) -> RecoveryApproval {
    let message = request_bytes(&request);
    let signature = sign(guardian_key, &message);

    RecoveryApproval {
        guardian_id,
        request,
        signature: signature.to_bytes().to_vec(),
    }
}

/// Verify one guardian approval.
pub fn verify_approval(
    configuration: &RecoveryConfiguration,
    approval: &RecoveryApproval,
) -> Result<(), RecoveryError> {
    let guardian = configuration
        .guardians
        .iter()
        .find(|guardian| guardian.id == approval.guardian_id)
        .ok_or(RecoveryError::GuardianNotAuthorized)?;

    let signature_bytes: [u8; 64] = approval
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| RecoveryError::InvalidSignature)?;

    let signature = Signature::from_bytes(&signature_bytes);

    let public_key =
        VerifyingKey::from_bytes(&guardian.public_key)
            .map_err(|_| RecoveryError::InvalidSignature)?;

    let message = request_bytes(&approval.request);

    if !verify(&public_key, &message, &signature) {
        return Err(RecoveryError::InvalidSignature);
    }

    Ok(())
}

/// Determine whether enough unique guardian approvals exist.
pub fn is_recoverable(
    configuration: &RecoveryConfiguration,
    approvals: &[RecoveryApproval],
) -> Result<bool, RecoveryError> {
    let mut unique_guardians = Vec::new();

    for approval in approvals {
        verify_approval(configuration, approval)?;

        if unique_guardians.contains(&approval.guardian_id) {
            return Err(RecoveryError::DuplicateApproval);
        }

        unique_guardians.push(approval.guardian_id);
    }

    Ok(unique_guardians.len() >= configuration.threshold)
}

/// Validate and finalize a recovery operation.
///
/// This function does not transfer private keys. The identity owner must
/// provision the newly authorized device through the secure identity layer.
pub fn authorize_recovery(
    configuration: &RecoveryConfiguration,
    approvals: &[RecoveryApproval],
) -> Result<RecoveryRequest, RecoveryError> {
    if !is_recoverable(configuration, approvals)? {
        return Err(RecoveryError::InsufficientApprovals);
    }

    Ok(approvals[0].request.clone())
}
