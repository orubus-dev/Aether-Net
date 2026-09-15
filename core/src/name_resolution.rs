use crate::identity::device::{DeviceId, DeviceSigningKey};
use crate::identity::fingerprint::Fingerprint;
use crate::naming::{NameError, NameHash, NameRecord, NamingEngine, ResourceId, canonicalize_name};
use crate::network::NodeId;
use ed25519_dalek::{Signature, VerifyingKey};

pub const DEFAULT_MAX_DELEGATIONS: usize = 16_384;
pub const DELEGATION_SIGNATURE_LENGTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionKind {
    Resource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionError {
    InvalidName(NameError),
    NameNotFound,
    DelegationCapacity,
    InvalidDelegation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameDelegation {
    pub name_hash: NameHash,
    pub parent_hash: NameHash,
    pub owner: NodeId,
    pub version: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedNameDelegation {
    pub delegation: NameDelegation,
    pub device_id: DeviceId,
    pub public_key: [u8; 32],
    pub signature: [u8; DELEGATION_SIGNATURE_LENGTH],
}

impl SignedNameDelegation {
    pub fn sign(
        signing_key: &DeviceSigningKey,
        delegation: NameDelegation,
    ) -> Result<Self, ResolutionError> {
        let verifying = signing_key.verifying_key();
        let public_key = verifying.to_bytes();
        let device_id = signing_key.device_id();
        let derived_owner = NodeId::from_bytes(
            *Fingerprint::from_domain(b"aether/node-id/v1", &public_key).as_bytes(),
        );
        if derived_owner != delegation.owner {
            return Err(ResolutionError::InvalidDelegation);
        }
        let unsigned = Self {
            delegation,
            device_id,
            public_key,
            signature: [0; DELEGATION_SIGNATURE_LENGTH],
        };
        let signature = signing_key.sign(&unsigned.signing_bytes());
        Ok(Self {
            signature: signature.to_bytes(),
            ..unsigned
        })
    }

    pub fn verify(&self, now: u64) -> Result<(), ResolutionError> {
        if now >= self.delegation.expires_at {
            return Err(ResolutionError::InvalidDelegation);
        }
        let key = VerifyingKey::from_bytes(&self.public_key)
            .map_err(|_| ResolutionError::InvalidDelegation)?;
        if DeviceId::from_public_key(&key) != self.device_id {
            return Err(ResolutionError::InvalidDelegation);
        }
        let owner = NodeId::from_bytes(
            *Fingerprint::from_domain(b"aether/node-id/v1", &self.public_key).as_bytes(),
        );
        if owner != self.delegation.owner {
            return Err(ResolutionError::InvalidDelegation);
        }
        let signature = Signature::from_bytes(&self.signature);
        key.verify_strict(&self.signing_bytes(), &signature)
            .map_err(|_| ResolutionError::InvalidDelegation)
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + 32 + 32 + 32 + 32 + 32 + 8 + 8);
        bytes.extend_from_slice(b"AETHER-NAME-DELEGATION-V1");
        bytes.extend_from_slice(self.delegation.name_hash.as_bytes());
        bytes.extend_from_slice(self.delegation.parent_hash.as_bytes());
        bytes.extend_from_slice(self.delegation.owner.as_bytes());
        bytes.extend_from_slice(self.device_id.as_bytes());
        bytes.extend_from_slice(&self.public_key);
        bytes.extend_from_slice(&self.delegation.version.to_be_bytes());
        bytes.extend_from_slice(&self.delegation.expires_at.to_be_bytes());
        bytes
    }
}

pub struct NameResolutionEngine {
    naming: NamingEngine,
    delegations: Vec<NameDelegation>,
    max_delegations: usize,
}

impl NameResolutionEngine {
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_MAX_DELEGATIONS,
            crate::naming::DEFAULT_MAX_RECORDS,
            crate::naming::DEFAULT_MAX_CACHE_ENTRIES,
        )
    }
    pub fn with_limits(
        max_delegations: usize,
        max_records: usize,
        max_cache_entries: usize,
    ) -> Self {
        Self {
            naming: NamingEngine::with_limits(max_records, max_cache_entries),
            delegations: Vec::new(),
            max_delegations: max_delegations.max(1),
        }
    }
    pub fn naming(&self) -> &NamingEngine {
        &self.naming
    }
    pub fn naming_mut(&mut self) -> &mut NamingEngine {
        &mut self.naming
    }
    pub fn delegation_count(&self) -> usize {
        self.delegations.len()
    }
    pub fn publish_name(&mut self, record: NameRecord, now: u64) -> Result<bool, ResolutionError> {
        self.naming
            .publish(record, now)
            .map_err(ResolutionError::InvalidName)
    }
    pub fn add_delegation(
        &mut self,
        delegation: NameDelegation,
        now: u64,
    ) -> Result<bool, ResolutionError> {
        if now >= delegation.expires_at {
            return Err(ResolutionError::InvalidName(NameError::Expired));
        }
        if let Some(existing) = self
            .delegations
            .iter_mut()
            .find(|item| item.name_hash == delegation.name_hash)
        {
            if delegation.version < existing.version {
                return Ok(false);
            }
            *existing = delegation;
            return Ok(true);
        }
        if self.delegations.len() >= self.max_delegations {
            return Err(ResolutionError::DelegationCapacity);
        }
        self.delegations.push(delegation);
        Ok(true)
    }
    pub fn accept_signed_delegation(
        &mut self,
        signed: &SignedNameDelegation,
        now: u64,
    ) -> Result<bool, ResolutionError> {
        signed.verify(now)?;
        self.add_delegation(signed.delegation, now)
    }
    pub fn resolve(&mut self, name: &str, now: u64) -> Result<ResourceId, ResolutionError> {
        let canonical = canonicalize_name(name).map_err(ResolutionError::InvalidName)?;
        self.naming
            .resolve(&canonical, now)
            .map_err(|error| match error {
                NameError::NotFound | NameError::Stale => ResolutionError::NameNotFound,
                other => ResolutionError::InvalidName(other),
            })
    }
    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.delegations.len();
        self.delegations.retain(|item| now < item.expires_at);
        before.saturating_sub(self.delegations.len()) + self.naming.expire(now)
    }
}
impl Default for NameResolutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegation_is_versioned_and_bounded() {
        let mut engine = NameResolutionEngine::with_limits(1, 4, 4);
        let delegation = NameDelegation {
            name_hash: NameHash::of("app.aether"),
            parent_hash: NameHash::of("aether"),
            owner: NodeId::from_bytes([1; 32]),
            version: 1,
            expires_at: 100,
        };
        assert!(engine.add_delegation(delegation, 10).unwrap());
        assert!(
            !engine
                .add_delegation(
                    NameDelegation {
                        version: 0,
                        ..delegation
                    },
                    10
                )
                .unwrap()
        );
        assert_eq!(engine.delegation_count(), 1);
    }
    #[test]
    fn signed_delegation_binds_identity() {
        let key = DeviceSigningKey::generate().unwrap();
        let device = key.device_id();
        let public = key.verifying_key().to_bytes();
        let owner =
            NodeId::from_bytes(*Fingerprint::from_domain(b"aether/node-id/v1", &public).as_bytes());
        let delegation = NameDelegation {
            name_hash: NameHash::of("app.aether"),
            parent_hash: NameHash::of("aether"),
            owner,
            version: 1,
            expires_at: 100,
        };
        let signed = SignedNameDelegation::sign(&key, delegation).unwrap();
        assert_eq!(signed.device_id, device);
        assert!(signed.verify(10).is_ok());
        let mut engine = NameResolutionEngine::new();
        assert!(engine.accept_signed_delegation(&signed, 10).unwrap());
    }
    #[test]
    fn expired_delegations_are_collected() {
        let mut engine = NameResolutionEngine::new();
        engine
            .add_delegation(
                NameDelegation {
                    name_hash: NameHash::of("old.aether"),
                    parent_hash: NameHash::of("aether"),
                    owner: NodeId::from_bytes([1; 32]),
                    version: 1,
                    expires_at: 10,
                },
                1,
            )
            .unwrap();
        assert_eq!(engine.expire(10), 1);
    }
}
