use crate::identity::signature::verify_bytes;
use crate::network::NodeId;
use crate::storage::chunk::{ChunkId, ContentManifest, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE};
use crate::storage::content::ContentId;
use ed25519_dalek::VerifyingKey;
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
        if bytes.is_empty() || bytes.len() < MIN_CHUNK_SIZE || bytes.len() > MAX_CHUNK_SIZE {
            return Err(StorageError::InvalidChunkSize);
        }
        if expires_at <= now {
            return Err(StorageError::InvalidChunkSize);
        }

        let digest = ChunkDigest::of(content, chunk, bytes);
        if let Some(existing) = self
            .chunks
            .iter_mut()
            .find(|item| item.content == content && item.chunk == chunk)
        {
            self.used_bytes = self.used_bytes.saturating_sub(existing.bytes.len() as u64);
            if self.used_bytes.saturating_add(bytes.len() as u64) > self.max_bytes {
                return Err(StorageError::CapacityExceeded);
            }
            existing.digest = digest;
            existing.bytes.clear();
            existing.bytes.extend_from_slice(bytes);
            existing.stored_at = now;
            existing.expires_at = expires_at;
            self.used_bytes = self.used_bytes.saturating_add(bytes.len() as u64);
            return Ok(digest);
        }

        if self.chunks.len() >= self.max_chunks
            || self.used_bytes.saturating_add(bytes.len() as u64) > self.max_bytes
        {
            return Err(StorageError::CapacityExceeded);
        }

        self.chunks.push(StoredChunk {
            content,
            chunk,
            digest,
            bytes: bytes.to_vec(),
            stored_at: now,
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
        if actual != item.digest || expected.is_some_and(|value| value != actual) {
            return Err(StorageError::ChunkIntegrityFailure);
        }
        Ok(&item.bytes)
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
        }
        if let Some(existing) = self
            .manifests
            .iter_mut()
            .find(|item| item.manifest.content == manifest.content)
        {
            if version < existing.version {
                return Ok(false);
            }
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
        }
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

#[derive(Debug, Clone)]
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
        let mut bytes = Vec::with_capacity(32 + 32 + 8 * 6 + 4 + 32);
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
            Ok(key) => key,
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
        let required = required_replicas.max(1);
        let selected = self.providers.select(content, now, required);
        Ok(RepairPlan {
            content,
            required_replicas: required,
            needs_repair: selected.len() < required,
            selected_providers: selected,
        })
    }
}

impl Default for DistributedStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn content(seed: u8) -> ContentId {
        ContentId::from_bytes(&[seed; 8])
    }
    fn node(seed: u8) -> NodeId {
        NodeId::from_bytes([seed; 32])
    }
    fn chunk() -> Vec<u8> {
        vec![7u8; MIN_CHUNK_SIZE]
    }

    fn provider(seed: u8, signing: &SigningKey, now: u64, version: u64) -> ProviderAdvertisement {
        let mut advertisement = ProviderAdvertisement {
            content: content(1),
            provider: node(seed),
            version,
            capacity_bytes: 10_000_000,
            cached_bytes: 1_000_000,
            cost_micros: seed as u64,
            reliability_ppm: 990_000,
            observed_at: now,
            expires_at: now + DEFAULT_PROVIDER_TTL_SECONDS,
            owner_public_key: signing.verifying_key().to_bytes(),
            signature: [0; 64],
        };
        advertisement.signature = signing.sign(&advertisement.signing_bytes()).to_bytes();
        advertisement
    }

    #[test]
    fn chunk_store_verifies_and_deduplicates() {
        let mut store = ChunkStore::with_limits(4, 2 * MIN_CHUNK_SIZE as u64);
        let bytes = chunk();
        let id = content(1);
        let digest = store.put(id, ChunkId(0), &bytes, 10, 100).unwrap();
        assert_eq!(store.chunk_count(), 1);
        assert_eq!(store.put(id, ChunkId(0), &bytes, 20, 120).unwrap(), digest);
        assert_eq!(store.chunk_count(), 1);
        assert_eq!(
            store.get(id, ChunkId(0), Some(digest), 30).unwrap(),
            bytes.as_slice()
        );
    }

    #[test]
    fn manifest_versions_converge_monotonically() {
        let mut store = ManifestStore::new();
        let manifest = ContentManifest {
            content: content(2),
            total_size: MIN_CHUNK_SIZE as u64,
            chunk_size: MIN_CHUNK_SIZE as u32,
            chunks: vec![crate::storage::chunk::ContentChunk {
                id: ChunkId(0),
                content: content(2),
                offset: 0,
                length: MIN_CHUNK_SIZE as u32,
            }],
        };
        assert!(store.publish(manifest.clone(), 2, 10, 100).unwrap());
        assert!(!store.publish(manifest, 1, 10, 100).unwrap());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn provider_directory_rejects_bad_signature_and_selects_verified_nodes() {
        let mut directory = ProviderDirectory::with_limit(4);
        let key = SigningKey::from_bytes(&[3; 32]);
        let mut bad = provider(2, &key, 10, 1);
        bad.signature[0] ^= 1;
        assert_eq!(
            directory.publish(bad, 20).unwrap_err(),
            StorageError::InvalidProviderAdvertisement
        );
        assert!(directory.publish(provider(2, &key, 10, 1), 20).unwrap());
        assert_eq!(directory.select(content(1), 20, 2), vec![node(2)]);
    }

    #[test]
    fn repair_plan_detects_missing_replicas() {
        let storage = DistributedStorage::new();
        let plan = storage.repair_plan(content(9), 10, 3).unwrap();
        assert!(plan.needs_repair);
        assert!(plan.selected_providers.is_empty());
    }
}
