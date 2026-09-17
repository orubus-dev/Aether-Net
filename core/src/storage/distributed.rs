use crate::identity::device::DeviceSigningKey;
use crate::identity::fingerprint::Fingerprint;
use crate::identity::signature::verify_bytes;
use crate::network::NodeId;
use crate::storage::chunk::{ChunkId, ContentManifest, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE};
use crate::storage::content::ContentId;
use ed25519_dalek::{Signer, VerifyingKey};
use sha2::{Digest, Sha256};

pub const CHUNK_DIGEST_LENGTH: usize = 32;
pub const DEFAULT_MAX_CHUNKS: usize = 65_536;
pub const DEFAULT_MAX_STORAGE_BYTES: u64 = 512 * 1024 * 1024;
pub const DEFAULT_MAX_MANIFESTS: usize = 16_384;
pub const DEFAULT_MAX_PROVIDERS: usize = 16_384;
pub const DEFAULT_PROVIDER_TTL_SECONDS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkDigest([u8; CHUNK_DIGEST_LENGTH]);
impl ChunkDigest {
    pub fn of(content: ContentId, chunk: ChunkId, bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"aether:chunk:v1:");
        hasher.update(content.as_bytes());
        hasher.update(chunk.0.to_be_bytes());
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut output = [0u8; CHUNK_DIGEST_LENGTH];
        output.copy_from_slice(&digest);
        Self(output)
    }
    pub(crate) fn from_raw(bytes: [u8; CHUNK_DIGEST_LENGTH]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; CHUNK_DIGEST_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct StoredChunk {
    pub content: ContentId,
    pub chunk: ChunkId,
    pub digest: ChunkDigest,
    pub bytes: Vec<u8>,
    pub stored_at: u64,
    pub last_access: u64,
    pub expires_at: u64,
}
impl StoredChunk {
    pub fn is_valid_at(&self, now: u64) -> bool {
        now < self.expires_at
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    InvalidChunkSize,
    CapacityExceeded,
    InvalidManifest,
    ManifestCapacityExceeded,
    ChunkNotFound,
    ChunkIntegrityFailure,
    ProviderCapacityExceeded,
    InvalidProviderAdvertisement,
    ProviderSignatureInvalid,
    ProviderNotFound,
    InvalidProviderIdentity,
    ReplicaRequirementInvalid,
}

pub struct ChunkStore {
    max_chunks: usize,
    max_bytes: u64,
    used_bytes: u64,
    chunks: Vec<StoredChunk>,
}
impl ChunkStore {
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_CHUNKS, DEFAULT_MAX_STORAGE_BYTES)
    }
    pub fn with_limits(max_chunks: usize, max_bytes: u64) -> Self {
        Self {
            max_chunks: max_chunks.max(1),
            max_bytes: max_bytes.max(1),
            used_bytes: 0,
            chunks: Vec::new(),
        }
    }
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }
    pub fn used_bytes(&self) -> u64 {
        self.used_bytes
    }
    pub fn available_bytes(&self) -> u64 {
        self.max_bytes.saturating_sub(self.used_bytes)
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
    pub fn put(
        &mut self,
        content: ContentId,
        chunk: ChunkId,
        bytes: &[u8],
        now: u64,
        expires_at: u64,
    ) -> Result<ChunkDigest, StorageError> {
        if bytes.is_empty()
            || bytes.len() < MIN_CHUNK_SIZE
            || bytes.len() > MAX_CHUNK_SIZE
            || expires_at <= now
        {
            return Err(StorageError::InvalidChunkSize);
        };
        let digest = ChunkDigest::of(content, chunk, bytes);
        if let Some(index) = self
            .chunks
            .iter()
            .position(|item| item.content == content && item.chunk == chunk)
        {
            let old = self.chunks[index].bytes.len() as u64;
            let new_used = self
                .used_bytes
                .saturating_sub(old)
                .saturating_add(bytes.len() as u64);
            if new_used > self.max_bytes {
                return Err(StorageError::CapacityExceeded);
            };
            let item = &mut self.chunks[index];
            item.digest = digest;
            item.bytes.clear();
            item.bytes.extend_from_slice(bytes);
            item.stored_at = now;
            item.last_access = now;
            item.expires_at = expires_at;
            self.used_bytes = new_used;
            return Ok(digest);
        }
        if self.chunks.len() >= self.max_chunks
            || self.used_bytes.saturating_add(bytes.len() as u64) > self.max_bytes
        {
            return Err(StorageError::CapacityExceeded);
        };
        self.chunks.push(StoredChunk {
            content,
            chunk,
            digest,
            bytes: bytes.to_vec(),
            stored_at: now,
            last_access: now,
            expires_at,
        });
        self.used_bytes = self.used_bytes.saturating_add(bytes.len() as u64);
        Ok(digest)
    }
    pub fn get(
        &self,
        content: ContentId,
        chunk: ChunkId,
        expected: Option<ChunkDigest>,
        now: u64,
    ) -> Result<&[u8], StorageError> {
        let item = self
            .chunks
            .iter()
            .find(|item| item.content == content && item.chunk == chunk && item.is_valid_at(now))
            .ok_or(StorageError::ChunkNotFound)?;
        let actual = ChunkDigest::of(item.content, item.chunk, &item.bytes);
        if actual != item.digest || expected.is_some_and(|v| v != actual) {
            return Err(StorageError::ChunkIntegrityFailure);
        };
        Ok(&item.bytes)
    }
    pub fn touch(&mut self, content: ContentId, chunk: ChunkId, now: u64) -> bool {
        if let Some(item) = self
            .chunks
            .iter_mut()
            .find(|item| item.content == content && item.chunk == chunk && item.is_valid_at(now))
        {
            item.last_access = now;
            return true;
        }
        false
    }
    pub fn remove(&mut self, content: ContentId, chunk: ChunkId) -> bool {
        if let Some(i) = self
            .chunks
            .iter()
            .position(|item| item.content == content && item.chunk == chunk)
        {
            let item = self.chunks.swap_remove(i);
            self.used_bytes = self.used_bytes.saturating_sub(item.bytes.len() as u64);
            true
        } else {
            false
        }
    }
    pub fn contains(&self, content: ContentId, chunk: ChunkId, now: u64) -> bool {
        self.chunks
            .iter()
            .any(|item| item.content == content && item.chunk == chunk && item.is_valid_at(now))
    }
    pub fn evict_lru_to_fit(&mut self, required_bytes: u64) -> usize {
        let mut removed = 0;
        while self.used_bytes.saturating_add(required_bytes) > self.max_bytes
            && !self.chunks.is_empty()
        {
            let Some(i) = self
                .chunks
                .iter()
                .enumerate()
                .min_by_key(|(_, item)| item.last_access)
                .map(|(i, _)| i)
            else {
                break;
            };
            let item = self.chunks.swap_remove(i);
            self.used_bytes = self.used_bytes.saturating_sub(item.bytes.len() as u64);
            removed += 1
        }
        removed
    }
    pub fn remove_expired(&mut self, now: u64) -> usize {
        let before = self.chunks.len();
        self.chunks.retain(|item| item.is_valid_at(now));
        self.used_bytes = self.chunks.iter().map(|item| item.bytes.len() as u64).sum();
        before.saturating_sub(self.chunks.len())
    }
}
impl Default for ChunkStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct StoredManifest {
    pub manifest: ContentManifest,
    pub version: u64,
    pub published_at: u64,
    pub expires_at: u64,
}
impl StoredManifest {
    pub fn is_valid_at(&self, now: u64) -> bool {
        now < self.expires_at && self.expires_at > self.published_at && self.manifest.validate()
    }
}
pub struct ManifestStore {
    max_manifests: usize,
    manifests: Vec<StoredManifest>,
}
impl ManifestStore {
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_MAX_MANIFESTS)
    }
    pub fn with_limit(max_manifests: usize) -> Self {
        Self {
            max_manifests: max_manifests.max(1),
            manifests: Vec::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.manifests.len()
    }
    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }
    pub fn publish(
        &mut self,
        manifest: ContentManifest,
        version: u64,
        now: u64,
        expires_at: u64,
    ) -> Result<bool, StorageError> {
        if !manifest.validate() || expires_at <= now {
            return Err(StorageError::InvalidManifest);
        };
        if let Some(existing) = self
            .manifests
            .iter_mut()
            .find(|item| item.manifest.content == manifest.content)
        {
            if version < existing.version {
                return Ok(false);
            };
            *existing = StoredManifest {
                manifest,
                version,
                published_at: now,
                expires_at,
            };
            return Ok(true);
        }
        if self.manifests.len() >= self.max_manifests {
            return Err(StorageError::ManifestCapacityExceeded);
        };
        self.manifests.push(StoredManifest {
            manifest,
            version,
            published_at: now,
            expires_at,
        });
        Ok(true)
    }
    pub fn get(&self, content: ContentId, now: u64) -> Option<&StoredManifest> {
        self.manifests
            .iter()
            .find(|item| item.manifest.content == content && item.is_valid_at(now))
    }
    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.manifests.len();
        self.manifests.retain(|item| item.is_valid_at(now));
        before.saturating_sub(self.manifests.len())
    }
}
impl Default for ManifestStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAdvertisement {
    pub content: ContentId,
    pub provider: NodeId,
    pub version: u64,
    pub capacity_bytes: u64,
    pub cached_bytes: u64,
    pub cost_micros: u64,
    pub reliability_ppm: u32,
    pub observed_at: u64,
    pub expires_at: u64,
    pub owner_public_key: [u8; 32],
    pub signature: [u8; 64],
}
impl ProviderAdvertisement {
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(148);
        bytes.extend_from_slice(self.content.as_bytes());
        bytes.extend_from_slice(self.provider.as_bytes());
        bytes.extend_from_slice(&self.version.to_be_bytes());
        bytes.extend_from_slice(&self.capacity_bytes.to_be_bytes());
        bytes.extend_from_slice(&self.cached_bytes.to_be_bytes());
        bytes.extend_from_slice(&self.cost_micros.to_be_bytes());
        bytes.extend_from_slice(&self.reliability_ppm.to_be_bytes());
        bytes.extend_from_slice(&self.observed_at.to_be_bytes());
        bytes.extend_from_slice(&self.expires_at.to_be_bytes());
        bytes.extend_from_slice(&self.owner_public_key);
        bytes
    }
    pub fn verify(&self, now: u64) -> bool {
        if self.expires_at <= self.observed_at
            || now >= self.expires_at
            || self.cached_bytes > self.capacity_bytes
            || self.reliability_ppm > 1_000_000
        {
            return false;
        }
        let key = match VerifyingKey::from_bytes(&self.owner_public_key) {
            Ok(k) => k,
            Err(_) => return false,
        };
        verify_bytes(&key, &self.signing_bytes(), &self.signature)
    }
}

pub struct ProviderDirectory {
    max_providers: usize,
    providers: Vec<ProviderAdvertisement>,
}
impl ProviderDirectory {
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_MAX_PROVIDERS)
    }
    pub fn with_limit(max_providers: usize) -> Self {
        Self {
            max_providers: max_providers.max(1),
            providers: Vec::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.providers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
    pub fn publish(
        &mut self,
        advertisement: ProviderAdvertisement,
        now: u64,
    ) -> Result<bool, StorageError> {
        if !advertisement.verify(now) {
            return Err(StorageError::InvalidProviderAdvertisement);
        }
        if let Some(existing) = self.providers.iter_mut().find(|item| {
            item.content == advertisement.content && item.provider == advertisement.provider
        }) {
            if advertisement.version < existing.version {
                return Ok(false);
            }
            *existing = advertisement;
            return Ok(true);
        }
        if self.providers.len() >= self.max_providers {
            return Err(StorageError::ProviderCapacityExceeded);
        }
        self.providers.push(advertisement);
        Ok(true)
    }
    pub fn providers_for(
        &self,
        content: ContentId,
        now: u64,
    ) -> impl Iterator<Item = &ProviderAdvertisement> {
        self.providers
            .iter()
            .filter(move |item| item.content == content && item.verify(now))
    }
    pub fn provider(
        &self,
        content: ContentId,
        node: NodeId,
        now: u64,
    ) -> Option<&ProviderAdvertisement> {
        self.providers
            .iter()
            .find(|item| item.content == content && item.provider == node && item.verify(now))
    }
    pub fn select(&self, content: ContentId, now: u64, limit: usize) -> Vec<NodeId> {
        let mut candidates: Vec<&ProviderAdvertisement> =
            self.providers_for(content, now).collect();
        candidates.sort_by(|a, b| {
            provider_score(b)
                .cmp(&provider_score(a))
                .then_with(|| a.provider.as_bytes().cmp(b.provider.as_bytes()))
        });
        candidates
            .into_iter()
            .take(limit.max(1))
            .map(|item| item.provider)
            .collect()
    }
    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.providers.len();
        self.providers.retain(|item| item.verify(now));
        before.saturating_sub(self.providers.len())
    }
}
impl Default for ProviderDirectory {
    fn default() -> Self {
        Self::new()
    }
}
fn provider_score(provider: &ProviderAdvertisement) -> u64 {
    let free = provider
        .capacity_bytes
        .saturating_sub(provider.cached_bytes);
    let reliability = provider.reliability_ppm as u64;
    let cost_penalty = provider.cost_micros.min(1_000_000_000);
    free.saturating_div(1024)
        .saturating_add(reliability)
        .saturating_sub(cost_penalty)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairPlan {
    pub content: ContentId,
    pub required_replicas: usize,
    pub selected_providers: Vec<NodeId>,
    pub needs_repair: bool,
}
pub struct DistributedStorage {
    pub chunks: ChunkStore,
    pub manifests: ManifestStore,
    pub providers: ProviderDirectory,
}
impl DistributedStorage {
    pub fn new() -> Self {
        Self {
            chunks: ChunkStore::new(),
            manifests: ManifestStore::new(),
            providers: ProviderDirectory::new(),
        }
    }
    pub fn repair_plan(
        &self,
        content: ContentId,
        now: u64,
        required_replicas: usize,
    ) -> Result<RepairPlan, StorageError> {
        if required_replicas == 0 {
            return Err(StorageError::ReplicaRequirementInvalid);
        }
        let selected = self.providers.select(content, now, required_replicas);
        Ok(RepairPlan {
            content,
            required_replicas,
            needs_repair: selected.len() < required_replicas,
            selected_providers: selected,
        })
    }
    pub fn gc(&mut self, now: u64) -> (usize, usize, usize) {
        (
            self.chunks.remove_expired(now),
            self.manifests.expire(now),
            self.providers.expire(now),
        )
    }
}
impl Default for DistributedStorage {
    fn default() -> Self {
        Self::new()
    }
}
