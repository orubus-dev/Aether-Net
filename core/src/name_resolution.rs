use crate::naming::{canonicalize_name, NameError, NameHash, NameRecord, NamingEngine, ResourceId};
use crate::network::NodeId;

pub const DEFAULT_MAX_DELEGATIONS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionKind {
    Resource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionError {
    InvalidName(NameError),
    NameNotFound,
    DelegationCapacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameDelegation {
    pub name_hash: NameHash,
    pub parent_hash: NameHash,
    pub owner: NodeId,
    pub version: u64,
    pub expires_at: u64,
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

    pub fn resolve(&mut self, name: &str, now: u64) -> Result<ResourceId, ResolutionError> {
        let canonical = canonicalize_name(name).map_err(ResolutionError::InvalidName)?;
        self.naming.resolve(&canonical, now).map_err(|error| match error {
            NameError::NotFound | NameError::Stale => ResolutionError::NameNotFound,
            other => ResolutionError::InvalidName(other),
        })
    }

    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.delegations.len();
        self.delegations.retain(|item| now < item.expires_at);
        let delegation_expired = before.saturating_sub(self.delegations.len());
        delegation_expired + self.naming.expire(now)
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
        assert!(!engine
            .add_delegation(NameDelegation { version: 0, ..delegation }, 10)
            .unwrap());
        assert_eq!(engine.delegation_count(), 1);
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
