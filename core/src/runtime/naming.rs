use crate::identity::fingerprint::Fingerprint;
use crate::name_resolution::{NameResolutionEngine, ResolutionError};
use crate::naming::{NameError, NameHash, NameRecord, ResourceId, canonicalize_name};
use crate::naming_protocol::{NameQuery, NameResponse};

pub const DEFAULT_MAX_PENDING_QUERIES: usize = 1024;
pub const NAME_QUERY_TTL_SECONDS: u64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameRuntimeError {
    Name(NameError),
    Resolution(ResolutionError),
    PendingCapacity,
    UnknownRequest,
    RequestMismatch,
    ExpiredRequest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingQuery {
    request_id: [u8; 16],
    name_hash: NameHash,
    expires_at: u64,
}

pub struct NameRuntime {
    resolution: NameResolutionEngine,
    local: Vec<(String, NameRecord)>,
    pending: Vec<PendingQuery>,
    next_request: u64,
    max_pending: usize,
}

impl NameRuntime {
    pub fn new() -> Self {
        Self::with_limits(
            crate::naming::DEFAULT_MAX_RECORDS,
            crate::naming::DEFAULT_MAX_CACHE_ENTRIES,
            crate::name_resolution::DEFAULT_MAX_DELEGATIONS,
            DEFAULT_MAX_PENDING_QUERIES,
        )
    }
    pub fn with_limits(
        max_records: usize,
        max_cache: usize,
        max_delegations: usize,
        max_pending: usize,
    ) -> Self {
        Self {
            resolution: NameResolutionEngine::with_limits(max_delegations, max_records, max_cache),
            local: Vec::new(),
            pending: Vec::new(),
            next_request: 0,
            max_pending: max_pending.max(1),
        }
    }
    pub fn resolution(&self) -> &NameResolutionEngine {
        &self.resolution
    }
    pub fn resolution_mut(&mut self) -> &mut NameResolutionEngine {
        &mut self.resolution
    }
    pub fn local_count(&self) -> usize {
        self.local.len()
    }
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    pub fn publish_local(
        &mut self,
        name: &str,
        record: NameRecord,
        now: u64,
    ) -> Result<bool, NameRuntimeError> {
        let canonical = canonicalize_name(name).map_err(NameRuntimeError::Name)?;
        if NameHash::of(&canonical) != record.name_hash {
            return Err(NameRuntimeError::Name(NameError::InvalidRecord));
        }
        let changed = self
            .resolution
            .publish_name(record, now)
            .map_err(NameRuntimeError::Resolution)?;
        if let Some(existing) = self.local.iter_mut().find(|(n, _)| *n == canonical) {
            if record.version >= existing.1.version {
                *existing = (canonical, record);
            }
        } else {
            self.local.push((canonical, record));
        }
        Ok(changed)
    }
    pub fn remember(
        &mut self,
        name: &str,
        record: NameRecord,
        now: u64,
    ) -> Result<bool, NameRuntimeError> {
        let canonical = canonicalize_name(name).map_err(NameRuntimeError::Name)?;
        if NameHash::of(&canonical) != record.name_hash {
            return Err(NameRuntimeError::Name(NameError::InvalidRecord));
        }
        self.resolution
            .publish_name(record, now)
            .map_err(NameRuntimeError::Resolution)
    }
    pub fn resolve(&mut self, name: &str, now: u64) -> Result<ResourceId, NameRuntimeError> {
        self.resolution
            .resolve(name, now)
            .map_err(NameRuntimeError::Resolution)
    }
    pub fn begin_query(&mut self, name: &str, now: u64) -> Result<NameQuery, NameRuntimeError> {
        let canonical = canonicalize_name(name).map_err(NameRuntimeError::Name)?;
        if self.pending.len() >= self.max_pending {
            return Err(NameRuntimeError::PendingCapacity);
        }
        self.next_request = self.next_request.saturating_add(1);
        let hash = NameHash::of(&canonical);
        let mut seed = [0u8; 24];
        seed[0..8].copy_from_slice(&self.next_request.to_be_bytes());
        seed[8..24].copy_from_slice(&hash.as_bytes()[0..16]);
        let digest = Fingerprint::from_domain(b"aether:name-query:v1", &seed);
        let mut request_id = [0u8; 16];
        request_id.copy_from_slice(&digest.as_bytes()[0..16]);
        self.pending.push(PendingQuery {
            request_id,
            name_hash: hash,
            expires_at: now.saturating_add(NAME_QUERY_TTL_SECONDS),
        });
        Ok(NameQuery {
            request_id,
            name_hash: hash,
        })
    }
    pub fn consume_response(
        &mut self,
        response: &NameResponse,
        now: u64,
    ) -> Result<Option<ResourceId>, NameRuntimeError> {
        let pos = self
            .pending
            .iter()
            .position(|p| p.request_id == response.request_id)
            .ok_or(NameRuntimeError::UnknownRequest)?;
        let pending = self.pending[pos];
        self.pending.swap_remove(pos);
        if now >= pending.expires_at {
            return Err(NameRuntimeError::ExpiredRequest);
        }
        match (&response.name, &response.record) {
            (None, None) => Ok(None),
            (Some(name), Some(record)) => {
                let canonical = canonicalize_name(name).map_err(NameRuntimeError::Name)?;
                if NameHash::of(&canonical) != pending.name_hash
                    || record.name_hash != pending.name_hash
                {
                    return Err(NameRuntimeError::RequestMismatch);
                }
                self.remember(&canonical, *record, now)?;
                Ok(Some(record.resource_id))
            }
            _ => Err(NameRuntimeError::RequestMismatch),
        }
    }
    pub fn local_records(&self) -> impl Iterator<Item = (&str, &NameRecord)> {
        self.local.iter().map(|(n, r)| (n.as_str(), r))
    }
    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.pending.len();
        self.pending.retain(|p| now < p.expires_at);
        before.saturating_sub(self.pending.len()) + self.resolution.expire(now)
    }
}
impl Default for NameRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::device::DeviceSigningKey;
    #[test]
    fn local_publish_and_query() {
        let k = DeviceSigningKey::generate().unwrap();
        let (_, mut r) = NameRecord::unsigned(
            "app.aether",
            ResourceId::from_bytes([3; 32]),
            k.verifying_key().to_bytes(),
            1,
            1,
            100,
        )
        .unwrap();
        r.signature = k.sign(&r.signing_bytes()).to_bytes();
        let mut rt = NameRuntime::new();
        assert!(rt.publish_local("app.aether", r, 10).unwrap());
        assert_eq!(
            rt.resolve("APP.AETHER", 10).unwrap(),
            ResourceId::from_bytes([3; 32])
        );
        let q = rt.begin_query("app.aether", 10).unwrap();
        let resp = NameResponse {
            request_id: q.request_id,
            name: Some("app.aether".into()),
            record: Some(r),
        };
        assert_eq!(
            rt.consume_response(&resp, 10).unwrap(),
            Some(ResourceId::from_bytes([3; 32]))
        );
    }
}
