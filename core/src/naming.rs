use crate::identity::signature::verify_bytes;
use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};

pub const NAME_MAX_LEN: usize = 253;
pub const LABEL_MAX_LEN: usize = 63;
pub const RESOURCE_ID_LENGTH: usize = 32;
pub const SIGNATURE_LENGTH: usize = 64;
pub const DEFAULT_MAX_RECORDS: usize = 65_536;
pub const DEFAULT_MAX_CACHE_ENTRIES: usize = 16_384;
pub const DEFAULT_RECORD_TTL_SECONDS: u64 = 300;
pub const NAME_HASH_LENGTH: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceId([u8; RESOURCE_ID_LENGTH]);
impl ResourceId {
    pub const fn from_bytes(bytes: [u8; RESOURCE_ID_LENGTH]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; RESOURCE_ID_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameHash([u8; NAME_HASH_LENGTH]);
impl NameHash {
    pub fn of(canonical_name: &str) -> Self {
        let mut h = Sha256::new();
        h.update(b"aether:name:v1:");
        h.update(canonical_name.as_bytes());
        let d = h.finalize();
        let mut b = [0u8; 32];
        b.copy_from_slice(&d);
        Self(b)
    }
    pub const fn as_bytes(&self) -> &[u8; NAME_HASH_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    InvalidLabel,
    InvalidCharacter,
    InvalidRecord,
    Expired,
    InvalidSignature,
    RecordCapacity,
    CacheCapacity,
    NotFound,
    Stale,
}

pub fn canonicalize_name(name: &str) -> Result<String, NameError> {
    let trimmed = name.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return Err(NameError::Empty);
    }
    if trimmed.len() > NAME_MAX_LEN {
        return Err(NameError::TooLong);
    }
    let lower = trimmed.to_ascii_lowercase();
    for label in lower.split('.') {
        if label.is_empty() || label.len() > LABEL_MAX_LEN {
            return Err(NameError::InvalidLabel);
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(NameError::InvalidLabel);
        }
        for b in label.bytes() {
            if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_') {
                return Err(NameError::InvalidCharacter);
            }
        }
    }
    Ok(lower)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameRecord {
    pub name_hash: NameHash,
    pub resource_id: ResourceId,
    pub owner_public_key: [u8; 32],
    pub version: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub signature: [u8; SIGNATURE_LENGTH],
}
impl NameRecord {
    pub fn unsigned(
        name: &str,
        resource_id: ResourceId,
        owner_public_key: [u8; 32],
        version: u64,
        issued_at: u64,
        expires_at: u64,
    ) -> Result<(String, Self), NameError> {
        let canonical = canonicalize_name(name)?;
        if expires_at <= issued_at {
            return Err(NameError::InvalidRecord);
        };
        Ok((
            canonical.clone(),
            Self {
                name_hash: NameHash::of(&canonical),
                resource_id,
                owner_public_key,
                version,
                issued_at,
                expires_at,
                signature: [0; 64],
            },
        ))
    }
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(120);
        b.extend_from_slice(self.name_hash.as_bytes());
        b.extend_from_slice(self.resource_id.as_bytes());
        b.extend_from_slice(&self.owner_public_key);
        b.extend_from_slice(&self.version.to_be_bytes());
        b.extend_from_slice(&self.issued_at.to_be_bytes());
        b.extend_from_slice(&self.expires_at.to_be_bytes());
        b
    }
    pub fn verify(&self, now: u64) -> Result<(), NameError> {
        if self.expires_at <= self.issued_at || now >= self.expires_at {
            return Err(NameError::Expired);
        };
        let key = VerifyingKey::from_bytes(&self.owner_public_key)
            .map_err(|_| NameError::InvalidSignature)?;
        if !verify_bytes(&key, &self.signing_bytes(), &self.signature) {
            return Err(NameError::InvalidSignature);
        };
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedNameUpdate {
    pub record: NameRecord,
}

pub struct NamingEngine {
    max_records: usize,
    max_cache_entries: usize,
    records: Vec<NameRecord>,
    cache: Vec<(NameHash, ResourceId, u64, u64)>,
}
impl NamingEngine {
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_RECORDS, DEFAULT_MAX_CACHE_ENTRIES)
    }
    pub fn with_limits(max_records: usize, max_cache_entries: usize) -> Self {
        Self {
            max_records: max_records.max(1),
            max_cache_entries: max_cache_entries.max(1),
            records: Vec::new(),
            cache: Vec::new(),
        }
    }
    pub fn record_count(&self) -> usize {
        self.records.len()
    }
    pub fn cache_count(&self) -> usize {
        self.cache.len()
    }
    pub fn publish(&mut self, record: NameRecord, now: u64) -> Result<bool, NameError> {
        record.verify(now)?;
        if let Some(existing) = self
            .records
            .iter_mut()
            .find(|r| r.name_hash == record.name_hash)
        {
            if record.version < existing.version {
                return Ok(false);
            };
            *existing = record;
            return Ok(true);
        };
        if self.records.len() >= self.max_records {
            return Err(NameError::RecordCapacity);
        };
        self.records.push(record);
        Ok(true)
    }
    pub fn resolve(&mut self, name: &str, now: u64) -> Result<ResourceId, NameError> {
        let canonical = canonicalize_name(name)?;
        let key = NameHash::of(&canonical);
        if let Some((_, resource, version, expiry)) = self
            .cache
            .iter()
            .find(|(h, _, _, e)| *h == key && now < *e)
            .copied()
        {
            if self
                .records
                .iter()
                .any(|r| r.name_hash == key && r.version == version && now < r.expires_at)
            {
                self.refresh_cache(key, resource, version, expiry)?;
                return Ok(resource);
            }
        }
        let record = self
            .records
            .iter()
            .find(|r| r.name_hash == key)
            .copied()
            .ok_or(NameError::NotFound)?;
        if now >= record.expires_at {
            return Err(NameError::Stale);
        };
        record.verify(now)?;
        self.refresh_cache(key, record.resource_id, record.version, record.expires_at)?;
        Ok(record.resource_id)
    }
    pub fn invalidate(&mut self, name: &str) -> Result<bool, NameError> {
        let key = NameHash::of(&canonicalize_name(name)?);
        let before = self.records.len();
        self.records.retain(|r| r.name_hash != key);
        self.cache.retain(|e| e.0 != key);
        Ok(before != self.records.len())
    }
    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.records.len() + self.cache.len();
        self.records.retain(|r| now < r.expires_at);
        self.cache.retain(|e| now < e.3);
        let after = self.records.len() + self.cache.len();
        before.saturating_sub(after)
    }
    fn refresh_cache(
        &mut self,
        key: NameHash,
        resource: ResourceId,
        version: u64,
        expires_at: u64,
    ) -> Result<(), NameError> {
        if let Some(existing) = self.cache.iter_mut().find(|e| e.0 == key) {
            *existing = (key, resource, version, expires_at);
            return Ok(());
        }
        if self.cache.len() >= self.max_cache_entries {
            return Err(NameError::CacheCapacity);
        }
        self.cache.push((key, resource, version, expires_at));
        Ok(())
    }
}
impl Default for NamingEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::device::DeviceSigningKey;
    use ed25519_dalek::Signer;
    fn resource(seed: u8) -> ResourceId {
        ResourceId::from_bytes([seed; 32])
    }
    #[test]
    fn canonicalization_is_stable() {
        assert_eq!(
            canonicalize_name(" School.AEETHER. ").unwrap(),
            "school.aeether"
        );
        assert_eq!(
            NameHash::of("school.aeether"),
            NameHash::of("school.aeether")
        );
    }
    #[test]
    fn signed_record_verifies_and_resolves() {
        let key = DeviceSigningKey::generate().unwrap();
        let public = key.verifying_key().to_bytes();
        let (canonical, mut record) =
            NameRecord::unsigned("Example.Aether", resource(7), public, 1, 10, 100).unwrap();
        assert_eq!(canonical, "example.aether");
        record.signature = key.sign(&record.signing_bytes()).to_bytes();
        assert!(record.verify(20).is_ok());
        let mut engine = NamingEngine::new();
        assert!(engine.publish(record, 20).unwrap());
        assert_eq!(engine.resolve("EXAMPLE.AETHER", 20).unwrap(), resource(7));
        assert_eq!(engine.cache_count(), 1);
    }
    #[test]
    fn stale_versions_cannot_replace_newer_record() {
        let key = DeviceSigningKey::generate().unwrap();
        let public = key.verifying_key().to_bytes();
        let (_, mut newer) =
            NameRecord::unsigned("service.aether", resource(9), public, 2, 0, 100).unwrap();
        newer.signature = key.sign(&newer.signing_bytes()).to_bytes();
        let (_, mut older) =
            NameRecord::unsigned("service.aether", resource(8), public, 1, 0, 100).unwrap();
        older.signature = key.sign(&older.signing_bytes()).to_bytes();
        let mut engine = NamingEngine::new();
        assert!(engine.publish(newer, 10).unwrap());
        assert!(!engine.publish(older, 10).unwrap());
        assert_eq!(engine.resolve("service.aether", 10).unwrap(), resource(9));
    }
    #[test]
    fn expired_records_are_removed() {
        let key = DeviceSigningKey::generate().unwrap();
        let public = key.verifying_key().to_bytes();
        let (_, mut record) =
            NameRecord::unsigned("old.aether", resource(1), public, 1, 0, 10).unwrap();
        record.signature = key.sign(&record.signing_bytes()).to_bytes();
        let mut engine = NamingEngine::new();
        engine.publish(record, 1).unwrap();
        assert_eq!(engine.expire(10), 1);
        assert_eq!(
            engine.resolve("old.aether", 10).unwrap_err(),
            NameError::NotFound
        );
    }
    #[test]
    fn cache_capacity_is_bounded() {
        let engine = NamingEngine::with_limits(4, 1);
        assert_eq!(engine.cache_count(), 0);
    }
}
