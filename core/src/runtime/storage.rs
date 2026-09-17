use crate::identity::device::DeviceSigningKey;
use crate::identity::fingerprint::Fingerprint;
use crate::network::NodeId;
use crate::storage::chunk::ChunkId;
use crate::storage::content::ContentId;
use crate::storage::distributed::{ProviderAdvertisement, ProviderDirectory, DEFAULT_PROVIDER_TTL_SECONDS};
use crate::storage::object_store::{ObjectManifest, ObjectStore, ObjectStoreError};
use crate::storage::protocol::{StorageCodecError, StorageControl, MAX_MANIFEST_PAGE_ENTRIES, MAX_STORAGE_FRAGMENT};
use crate::storage::transfer::{AssemblyError, ChunkAssembler, TransferDirection, TransferGovernor};
use ed25519_dalek::Signer;

pub const STORAGE_QUERY_TTL_SECONDS: u64 = 15;
pub const STORAGE_TRANSFER_TTL_MS: u64 = 30_000;
pub const MAX_MANIFEST_ASSEMBLIES: usize = 8;
pub const MAX_ASSEMBLY_ENTRIES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageRuntimeError {
    Codec(StorageCodecError),
    Object(ObjectStoreError),
    Assembly(AssemblyError),
    ProviderRejected,
    RequestCapacity,
    InvalidPeer,
    InvalidManifestPage,
}

struct ManifestAssembly {
    request_id: [u8; 16],
    content: ContentId,
    version: u64,
    total_size: u64,
    chunk_size: u32,
    total_chunks: u32,
    created_at: u64,
    expires_at: u64,
    entries: Vec<Option<crate::storage::object_store::ObjectChunk>>,
    received: usize,
    expires_at_ms: u64,
}

struct PendingAssembler {
    request_id: [u8; 16],
    assembler: ChunkAssembler,
    expires_at_ms: u64,
}

pub struct StorageRuntime {
    node_id: NodeId,
    signing_key: DeviceSigningKey,
    store: ObjectStore,
    providers: ProviderDirectory,
    governor: TransferGovernor,
    manifest_assemblies: Vec<ManifestAssembly>,
    chunk_assemblers: Vec<PendingAssembler>,
    sequence: u64,
}

impl StorageRuntime {
    pub fn new(node_id: NodeId, signing_key: DeviceSigningKey) -> Self {
        Self {
            node_id,
            signing_key,
            store: ObjectStore::new(),
            providers: ProviderDirectory::new(),
            governor: TransferGovernor::new(),
            manifest_assemblies: Vec::new(),
            chunk_assemblers: Vec::new(),
            sequence: 1,
        }
    }

    pub fn node_id(&self) -> NodeId { self.node_id }
    pub fn store(&self) -> &ObjectStore { &self.store }
    pub fn store_mut(&mut self) -> &mut ObjectStore { &mut self.store }
    pub fn providers(&self) -> &ProviderDirectory { &self.providers }
    pub fn provider_count(&self) -> usize { self.providers.len() }
    pub fn stored_bytes(&self) -> u64 { self.store.used_bytes() }
    pub fn object_count(&self) -> usize { self.store.manifest_count() }
    pub fn chunk_count(&self) -> usize { self.store.chunk_count() }

    pub fn publish_bytes(&mut self, bytes: &[u8], now: u64) -> Result<(ObjectManifest, ProviderAdvertisement), StorageRuntimeError> {
        let manifest = self.store.put_object(bytes, crate::storage::chunk::DEFAULT_CHUNK_SIZE, self.sequence, now, crate::storage::object_store::DEFAULT_OBJECT_TTL_SECONDS).map_err(StorageRuntimeError::Object)?;
        self.sequence = self.sequence.wrapping_add(1).max(1);
        let advertisement = self.local_advertisement(manifest.content, now);
        self.providers.publish(advertisement.clone(), now).map_err(|_| StorageRuntimeError::ProviderRejected)?;
        Ok((manifest, advertisement))
    }

    pub fn local_advertisement(&self, content: ContentId, now: u64) -> ProviderAdvertisement {
        let mut ad = ProviderAdvertisement {
            content,
            provider: self.node_id,
            version: self.sequence,
            capacity_bytes: self.store.max_bytes(),
            cached_bytes: self.store.used_bytes(),
            cost_micros: 0,
            reliability_ppm: 1_000_000,
            observed_at: now,
            expires_at: now.saturating_add(DEFAULT_PROVIDER_TTL_SECONDS),
            owner_public_key: self.signing_key.verifying_key().to_bytes(),
            signature: [0; 64],
        };
        ad.signature = self.signing_key.sign(&ad.signing_bytes()).to_bytes();
        ad
    }

    pub fn accept_provider(&mut self, advertisement: ProviderAdvertisement, now: u64) -> Result<bool, StorageRuntimeError> {
        if advertisement.provider == self.node_id || advertisement.expires_at <= now {
            return Err(StorageRuntimeError::InvalidPeer);
        }
        let derived = NodeId::from_bytes(*Fingerprint::from_domain(b"aether/node-id/v1", &advertisement.owner_public_key).as_bytes());
        if derived != advertisement.provider || !advertisement.verify(now) {
            return Err(StorageRuntimeError::ProviderRejected);
        }
        self.providers.publish(advertisement, now).map_err(|_| StorageRuntimeError::ProviderRejected)
    }

    pub fn begin_manifest_query(&mut self, content: ContentId, start: u32) -> StorageControl {
        let request_id = self.request_id(content.as_bytes(), start as u64);
        StorageControl::GetManifest { request_id, content, start, limit: MAX_MANIFEST_PAGE_ENTRIES as u16 }
    }

    pub fn begin_chunk_query(&mut self, content: ContentId, chunk: ChunkId, offset: u32, length: u16, digest: crate::storage::distributed::ChunkDigest) -> StorageControl {
        let request_id = self.request_id(content.as_bytes(), chunk.0 ^ u64::from(offset));
        StorageControl::GetChunk { request_id, content, chunk, offset, length: length.min(MAX_STORAGE_FRAGMENT as u16), digest }
    }

    pub fn handle(&mut self, message: StorageControl, peer: NodeId, now_ms: u64, now: u64) -> Result<Vec<StorageControl>, StorageRuntimeError> {
        if peer == self.node_id { return Err(StorageRuntimeError::InvalidPeer); }
        match message {
            StorageControl::GetManifest { request_id, content, start, limit } => {
                let manifest = self.store.manifest(content, now).map_err(StorageRuntimeError::Object)?;
                let begin = start as usize;
                if begin > manifest.chunks.len() { return Ok(vec![StorageControl::Ack { request_id, accepted: false, code: 4 }]); }
                let count = (limit as usize).clamp(1, MAX_MANIFEST_PAGE_ENTRIES);
                let end = begin.saturating_add(count).min(manifest.chunks.len());
                Ok(vec![StorageControl::ManifestPage {
                    request_id,
                    content,
                    version: manifest.version,
                    total_size: manifest.total_size,
                    chunk_size: manifest.chunk_size,
                    total_chunks: manifest.chunks.len() as u32,
                    start,
                    entries: manifest.chunks[begin..end].to_vec(),
                    created_at: manifest.created_at,
                    expires_at: manifest.expires_at,
                }])
            }
            StorageControl::GetChunk { request_id, content, chunk, offset, length, digest } => {
                let bytes = self.store.get_chunk(content, chunk, now).map_err(StorageRuntimeError::Object)?;
                let start = offset as usize;
                if start > bytes.len() || length == 0 {
                    return Ok(vec![StorageControl::Ack { request_id, accepted: false, code: 5 }]);
                }
                let end = start.saturating_add(length as usize).min(bytes.len());
                let payload = &bytes[start..end];
                if !self.governor.allow(peer, TransferDirection::Upload, payload.len(), now_ms) {
                    return Ok(vec![StorageControl::Ack { request_id, accepted: false, code: 6 }]);
                }
                Ok(vec![StorageControl::Chunk { request_id, content, chunk, offset, total_len: bytes.len() as u32, digest, payload: payload.to_vec() }])
            }
            StorageControl::PutChunk { request_id, content, chunk, offset, total_len, digest, payload, expires_at } => {
                if !self.governor.allow(peer, TransferDirection::Download, payload.len(), now_ms) {
                    return Ok(vec![StorageControl::Ack { request_id, accepted: false, code: 6 }]);
                }
                let index = match self.chunk_assemblers.iter().position(|item| item.request_id == request_id) {
                    Some(index) => index,
                    None => {
                        if self.chunk_assemblers.len() >= 1024 { return Err(StorageRuntimeError::RequestCapacity); }
                        let assembler = ChunkAssembler::new(content, chunk, total_len as usize, digest).map_err(StorageRuntimeError::Assembly)?;
                        self.chunk_assemblers.push(PendingAssembler { request_id, assembler, expires_at_ms: now_ms.saturating_add(STORAGE_TRANSFER_TTL_MS) });
                        self.chunk_assemblers.len() - 1
                    }
                };
                let pending = &mut self.chunk_assemblers[index];
                pending.expires_at_ms = now_ms.saturating_add(STORAGE_TRANSFER_TTL_MS);
                pending.assembler.write(offset as usize, &payload).map_err(StorageRuntimeError::Assembly)?;
                if pending.assembler.complete() {
                    let finished = self.chunk_assemblers.swap_remove(index);
                    let bytes = finished.assembler.finish().map_err(StorageRuntimeError::Assembly)?;
                    self.store.put_chunk(content, chunk, digest, &bytes, now, expires_at.max(now.saturating_add(1))).map_err(StorageRuntimeError::Object)?;
                    Ok(vec![StorageControl::Ack { request_id, accepted: true, code: 0 }])
                } else {
                    Ok(vec![StorageControl::Ack { request_id, accepted: true, code: 1 }])
                }
            }
            StorageControl::Chunk { request_id, content, chunk, offset, total_len, digest, payload } => {
                if !self.governor.allow(peer, TransferDirection::Download, payload.len(), now_ms) {
                    return Ok(vec![StorageControl::Ack { request_id, accepted: false, code: 6 }]);
                }
                self.accept_chunk_fragment(request_id, content, chunk, offset, total_len, digest, payload, now, now_ms)?;
                Ok(Vec::new())
            }
            StorageControl::ManifestPage { request_id, content, version, total_size, chunk_size, total_chunks, start, entries, created_at, expires_at } => {
                self.accept_manifest_page(request_id, content, version, total_size, chunk_size, total_chunks, start, entries, created_at, expires_at, now, now_ms)?;
                Ok(Vec::new())
            }
            StorageControl::ProviderAnnouncement(advertisement) => {
                let accepted = self.accept_provider(advertisement, now)?;
                Ok(vec![StorageControl::Ack { request_id: [0; 16], accepted, code: if accepted { 0 } else { 7 } }])
            }
            StorageControl::Ack { .. } => Ok(Vec::new()),
        }
    }

    fn accept_chunk_fragment(&mut self, request_id: [u8; 16], content: ContentId, chunk: ChunkId, offset: u32, total_len: u32, digest: crate::storage::distributed::ChunkDigest, payload: Vec<u8>, now: u64, now_ms: u64) -> Result<(), StorageRuntimeError> {
        let index = match self.chunk_assemblers.iter().position(|item| item.request_id == request_id) {
            Some(index) => index,
            None => {
                if self.chunk_assemblers.len() >= 1024 { return Err(StorageRuntimeError::RequestCapacity); }
                let assembler = ChunkAssembler::new(content, chunk, total_len as usize, digest).map_err(StorageRuntimeError::Assembly)?;
                self.chunk_assemblers.push(PendingAssembler { request_id, assembler, expires_at_ms: now_ms.saturating_add(STORAGE_TRANSFER_TTL_MS) });
                self.chunk_assemblers.len() - 1
            }
        };
        self.chunk_assemblers[index].assembler.write(offset as usize, &payload).map_err(StorageRuntimeError::Assembly)?;
        self.chunk_assemblers[index].expires_at_ms = now_ms.saturating_add(STORAGE_TRANSFER_TTL_MS);
        if self.chunk_assemblers[index].assembler.complete() {
            let pending = self.chunk_assemblers.swap_remove(index);
            let bytes = pending.assembler.finish().map_err(StorageRuntimeError::Assembly)?;
            self.store.put_chunk(content, chunk, digest, &bytes, now, now.saturating_add(crate::storage::object_store::DEFAULT_OBJECT_TTL_SECONDS)).map_err(StorageRuntimeError::Object)?;
        }
        Ok(())
    }

    fn accept_manifest_page(&mut self, request_id: [u8; 16], content: ContentId, version: u64, total_size: u64, chunk_size: u32, total_chunks: u32, start: u32, entries: Vec<crate::storage::object_store::ObjectChunk>, created_at: u64, expires_at: u64, now: u64, now_ms: u64) -> Result<(), StorageRuntimeError> {
        if total_chunks == 0 || total_chunks as usize > MAX_ASSEMBLY_ENTRIES || start as usize + entries.len() > total_chunks as usize {
            return Err(StorageRuntimeError::InvalidManifestPage);
        }
        let index = match self.manifest_assemblies.iter().position(|item| item.request_id == request_id) {
            Some(index) => index,
            None => {
                if self.manifest_assemblies.len() >= MAX_MANIFEST_ASSEMBLIES { return Err(StorageRuntimeError::RequestCapacity); }
                self.manifest_assemblies.push(ManifestAssembly { request_id, content, version, total_size, chunk_size, total_chunks, created_at, expires_at, entries: vec![None; total_chunks as usize], received: 0, expires_at_ms: now_ms.saturating_add(STORAGE_TRANSFER_TTL_MS) });
                self.manifest_assemblies.len() - 1
            }
        };
        let assembly = &mut self.manifest_assemblies[index];
        if assembly.content != content || assembly.version != version || assembly.total_size != total_size || assembly.chunk_size != chunk_size {
            return Err(StorageRuntimeError::InvalidManifestPage);
        }
        for (offset, entry) in entries.into_iter().enumerate() {
            let slot = start as usize + offset;
            if assembly.entries[slot].is_none() {
                if entry.id.0 != slot as u64 { return Err(StorageRuntimeError::InvalidManifestPage); }
                assembly.entries[slot] = Some(entry);
                assembly.received += 1;
            }
        }
        assembly.expires_at_ms = now_ms.saturating_add(STORAGE_TRANSFER_TTL_MS);
        if assembly.received == assembly.total_chunks as usize {
            let complete = self.manifest_assemblies.swap_remove(index);
            let chunks = complete.entries.into_iter().map(|item| item.ok_or(StorageRuntimeError::InvalidManifestPage)).collect::<Result<Vec<_>, _>>()?;
            let manifest = ObjectManifest { content: complete.content, total_size: complete.total_size, chunk_size: complete.chunk_size, chunks, version: complete.version, created_at: complete.created_at, expires_at: complete.expires_at };
            self.store.publish_manifest(manifest, now).map_err(StorageRuntimeError::Object)?;
        }
        Ok(())
    }

    pub fn gc(&mut self, now_ms: u64, now: u64) {
        self.store.remove_expired(now);
        self.providers.expire(now);
        self.chunk_assemblers.retain(|item| now_ms < item.expires_at_ms);
        self.manifest_assemblies.retain(|item| now_ms < item.expires_at_ms);
    }

    fn request_id(&self, content: &[u8; 32], salt: u64) -> [u8; 16] {
        let mut seed = Vec::with_capacity(40);
        seed.extend_from_slice(content);
        seed.extend_from_slice(&salt.to_be_bytes());
        let digest = Fingerprint::from_domain(b"aether:storage-request:v1", &seed);
        let mut output = [0u8; 16];
        output.copy_from_slice(&digest.as_bytes()[..16]);
        output
    }
}
