use crate::storage::chunk::{ChunkId, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE};
use crate::storage::content::ContentId;
use crate::storage::distributed::ChunkDigest;

pub const DEFAULT_MAX_OBJECT_STORE_CHUNKS: usize = 65_536;
pub const DEFAULT_MAX_OBJECT_STORE_MANIFESTS: usize = 16_384;
pub const DEFAULT_MAX_OBJECT_STORE_BYTES: u64 = 512 * 1024 * 1024;
pub const DEFAULT_OBJECT_TTL_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectChunk {
    pub id: ChunkId,
    pub offset: u64,
    pub length: u32,
    pub digest: ChunkDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectManifest {
    pub content: ContentId,
    pub total_size: u64,
    pub chunk_size: u32,
    pub chunks: Vec<ObjectChunk>,
    pub version: u64,
    pub created_at: u64,
    pub expires_at: u64,
}

impl ObjectManifest {
    pub fn validate(&self, now: u64) -> Result<(), ObjectStoreError> {
        if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&(self.chunk_size as usize)) {
            return Err(ObjectStoreError::InvalidManifest);
        }
        if self.expires_at <= now || self.expires_at <= self.created_at {
            return Err(ObjectStoreError::InvalidManifest);
        }
        let mut offset = 0u64;
        for (index, chunk) in self.chunks.iter().enumerate() {
            if chunk.id.0 != index as u64 || chunk.offset != offset {
                return Err(ObjectStoreError::InvalidManifest);
            }
            offset = offset.saturating_add(chunk.length as u64);
        }
        if offset != self.total_size {
            return Err(ObjectStoreError::InvalidManifest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct StoredChunk {
    content: ContentId,
    id: ChunkId,
    digest: ChunkDigest,
    bytes: Vec<u8>,
    last_access: u64,
    expires_at: u64,
}

#[derive(Debug, Clone)]
struct StoredManifest {
    manifest: ObjectManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectStoreError {
    InvalidChunk,
    InvalidManifest,
    CapacityExceeded,
    ManifestCapacityExceeded,
    ChunkNotFound,
    IntegrityFailure,
    ObjectNotFound,
    ObjectIncomplete,
}

pub struct ObjectStore {
    max_chunks: usize,
    max_manifests: usize,
    max_bytes: u64,
    used_bytes: u64,
    chunks: Vec<StoredChunk>,
    manifests: Vec<StoredManifest>,
}

impl ObjectStore {
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_MAX_OBJECT_STORE_CHUNKS,
            DEFAULT_MAX_OBJECT_STORE_MANIFESTS,
            DEFAULT_MAX_OBJECT_STORE_BYTES,
        )
    }
    pub fn with_limits(max_chunks: usize, max_manifests: usize, max_bytes: u64) -> Self {
        Self {
            max_chunks: max_chunks.max(1),
            max_manifests: max_manifests.max(1),
            max_bytes: max_bytes.max(1),
            used_bytes: 0,
            chunks: Vec::new(),
            manifests: Vec::new(),
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
    pub fn manifest_count(&self) -> usize {
        self.manifests.len()
    }
    pub fn contents(&self, now: u64) -> Vec<ContentId> {
        self.manifests
            .iter()
            .filter(|item| now < item.manifest.expires_at)
            .map(|item| item.manifest.content)
            .collect()
    }

    pub fn put_object(
        &mut self,
        bytes: &[u8],
        chunk_size: usize,
        version: u64,
        now: u64,
        ttl_seconds: u64,
    ) -> Result<ObjectManifest, ObjectStoreError> {
        if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&chunk_size) {
            return Err(ObjectStoreError::InvalidChunk);
        }
        if bytes.len() as u64 > self.max_bytes {
            return Err(ObjectStoreError::CapacityExceeded);
        }
        let content = ContentId::from_bytes(bytes);
        let mut chunks = Vec::new();
        let mut offset = 0u64;
        let mut index = 0u64;
        while (offset as usize) < bytes.len() {
            let start = offset as usize;
            let end = start.saturating_add(chunk_size).min(bytes.len());
            let data = &bytes[start..end];
            chunks.push(ObjectChunk {
                id: ChunkId(index),
                offset,
                length: data.len() as u32,
                digest: ChunkDigest::of(content, ChunkId(index), data),
            });
            offset = end as u64;
            index += 1;
        }
        if bytes.is_empty() {
            chunks.push(ObjectChunk {
                id: ChunkId(0),
                offset: 0,
                length: 0,
                digest: ChunkDigest::of(content, ChunkId(0), &[]),
            });
        }
        let manifest = ObjectManifest {
            content,
            total_size: bytes.len() as u64,
            chunk_size: chunk_size as u32,
            chunks,
            version,
            created_at: now,
            expires_at: now.saturating_add(ttl_seconds.max(1)),
        };
        manifest.validate(now)?;
        self.ensure_capacity(bytes.len() as u64, manifest.chunks.len())?;
        for chunk in &manifest.chunks {
            let start = chunk.offset as usize;
            let end = start.saturating_add(chunk.length as usize);
            self.put_chunk(
                content,
                chunk.id,
                chunk.digest,
                &bytes[start..end],
                now,
                manifest.expires_at,
            )?;
        }
        self.publish_manifest(manifest.clone(), now)?;
        Ok(manifest)
    }

    pub fn publish_manifest(
        &mut self,
        manifest: ObjectManifest,
        now: u64,
    ) -> Result<bool, ObjectStoreError> {
        manifest.validate(now)?;
        if let Some(existing) = self
            .manifests
            .iter_mut()
            .find(|item| item.manifest.content == manifest.content)
        {
            if manifest.version < existing.manifest.version {
                return Ok(false);
            }
            existing.manifest = manifest;
            return Ok(true);
        }
        if self.manifests.len() >= self.max_manifests {
            self.evict_oldest_manifest();
        }
        if self.manifests.len() >= self.max_manifests {
            return Err(ObjectStoreError::ManifestCapacityExceeded);
        }
        self.manifests.push(StoredManifest { manifest });
        Ok(true)
    }

    pub fn put_chunk(
        &mut self,
        content: ContentId,
        id: ChunkId,
        expected: ChunkDigest,
        bytes: &[u8],
        now: u64,
        expires_at: u64,
    ) -> Result<(), ObjectStoreError> {
        let digest = ChunkDigest::of(content, id, bytes);
        if digest != expected || bytes.len() > MAX_CHUNK_SIZE {
            return Err(ObjectStoreError::IntegrityFailure);
        }
        let len = bytes.len() as u64;
        if let Some(index) = self
            .chunks
            .iter()
            .position(|item| item.content == content && item.id == id)
        {
            let old = self.chunks[index].bytes.len() as u64;
            let new_used = self.used_bytes.saturating_sub(old).saturating_add(len);
            if new_used > self.max_bytes {
                return Err(ObjectStoreError::CapacityExceeded);
            }
            let item = &mut self.chunks[index];
            item.digest = digest;
            item.bytes.clear();
            item.bytes.extend_from_slice(bytes);
            item.last_access = now;
            item.expires_at = expires_at;
            self.used_bytes = new_used;
            return Ok(());
        }
        self.ensure_capacity(len, 1)?;
        self.chunks.push(StoredChunk {
            content,
            id,
            digest,
            bytes: bytes.to_vec(),
            last_access: now,
            expires_at,
        });
        self.used_bytes = self.used_bytes.saturating_add(len);
        Ok(())
    }

    pub fn get_chunk(
        &mut self,
        content: ContentId,
        id: ChunkId,
        now: u64,
    ) -> Result<Vec<u8>, ObjectStoreError> {
        let item = self
            .chunks
            .iter_mut()
            .find(|item| item.content == content && item.id == id && now < item.expires_at)
            .ok_or(ObjectStoreError::ChunkNotFound)?;
        let actual = ChunkDigest::of(item.content, item.id, &item.bytes);
        if actual != item.digest {
            return Err(ObjectStoreError::IntegrityFailure);
        }
        item.last_access = now;
        Ok(item.bytes.clone())
    }

    pub fn manifest(
        &self,
        content: ContentId,
        now: u64,
    ) -> Result<ObjectManifest, ObjectStoreError> {
        self.manifests
            .iter()
            .find(|item| item.manifest.content == content && now < item.manifest.expires_at)
            .map(|item| item.manifest.clone())
            .ok_or(ObjectStoreError::ObjectNotFound)
    }

    pub fn assemble(&mut self, content: ContentId, now: u64) -> Result<Vec<u8>, ObjectStoreError> {
        let manifest = self.manifest(content, now)?;
        let mut output = Vec::with_capacity(manifest.total_size as usize);
        for chunk in &manifest.chunks {
            let bytes = self.get_chunk(content, chunk.id, now)?;
            if bytes.len() != chunk.length as usize
                || ChunkDigest::of(content, chunk.id, &bytes) != chunk.digest
            {
                return Err(ObjectStoreError::IntegrityFailure);
            }
            output.extend_from_slice(&bytes);
        }
        if output.len() as u64 != manifest.total_size || ContentId::from_bytes(&output) != content {
            return Err(ObjectStoreError::IntegrityFailure);
        }
        Ok(output)
    }

    pub fn remove_expired(&mut self, now: u64) -> (usize, usize) {
        let before_chunks = self.chunks.len();
        self.chunks.retain(|item| now < item.expires_at);
        self.used_bytes = self.chunks.iter().map(|item| item.bytes.len() as u64).sum();
        let before_manifests = self.manifests.len();
        self.manifests.retain(|item| now < item.manifest.expires_at);
        (
            before_chunks.saturating_sub(self.chunks.len()),
            before_manifests.saturating_sub(self.manifests.len()),
        )
    }
    pub fn has_object(&self, content: ContentId, now: u64) -> bool {
        self.manifest(content, now).is_ok()
    }
    fn ensure_capacity(&mut self, bytes: u64, chunks: usize) -> Result<(), ObjectStoreError> {
        while self.used_bytes.saturating_add(bytes) > self.max_bytes
            || self.chunks.len().saturating_add(chunks) > self.max_chunks
        {
            let Some(index) = self
                .chunks
                .iter()
                .enumerate()
                .min_by_key(|(_, item)| item.last_access)
                .map(|(i, _)| i)
            else {
                break;
            };
            let item = self.chunks.swap_remove(index);
            self.used_bytes = self.used_bytes.saturating_sub(item.bytes.len() as u64);
        }
        if self.used_bytes.saturating_add(bytes) > self.max_bytes
            || self.chunks.len().saturating_add(chunks) > self.max_chunks
        {
            return Err(ObjectStoreError::CapacityExceeded);
        }
        Ok(())
    }
    fn evict_oldest_manifest(&mut self) {
        if !self.manifests.is_empty() {
            self.manifests.remove(0);
        }
    }
}
impl Default for ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn object_round_trip_supports_small_final_chunk() {
        let data = vec![9u8; MIN_CHUNK_SIZE + 17];
        let mut store = ObjectStore::with_limits(8, 8, 4 * MIN_CHUNK_SIZE as u64);
        let manifest = store.put_object(&data, MIN_CHUNK_SIZE, 1, 10, 100).unwrap();
        assert_eq!(store.assemble(manifest.content, 20).unwrap(), data);
    }
    #[test]
    fn corrupt_chunk_is_rejected() {
        let mut store = ObjectStore::new();
        let content = ContentId::from_bytes(b"x");
        let id = ChunkId(0);
        let digest = ChunkDigest::of(content, id, b"good");
        assert_eq!(
            store.put_chunk(content, id, digest, b"bad", 1, 10),
            Err(ObjectStoreError::IntegrityFailure)
        );
    }
}
