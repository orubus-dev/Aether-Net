use crate::network::NodeId;
use crate::storage::chunk::ChunkId;
use crate::storage::content::{ContentId, CONTENT_ID_LENGTH};
use crate::storage::distributed::{ChunkDigest, ProviderAdvertisement};
use crate::storage::object_store::{ObjectChunk, ObjectManifest};

pub const STORAGE_MAGIC: [u8; 4] = *b"AES1";
pub const STORAGE_VERSION: u8 = 1;
pub const MAX_STORAGE_MESSAGE: usize = 64 * 1024;
pub const MAX_MANIFEST_PAGE_ENTRIES: usize = 32;
pub const MAX_STORAGE_FRAGMENT: usize = 16 * 1024;

const KIND_GET_MANIFEST: u8 = 1;
const KIND_MANIFEST_PAGE: u8 = 2;
const KIND_GET_CHUNK: u8 = 3;
const KIND_CHUNK: u8 = 4;
const KIND_PUT_CHUNK: u8 = 5;
const KIND_ACK: u8 = 6;
const KIND_PROVIDER: u8 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageControl {
    GetManifest { request_id: [u8; 16], content: ContentId, start: u32, limit: u16 },
    ManifestPage {
        request_id: [u8; 16],
        content: ContentId,
        version: u64,
        total_size: u64,
        chunk_size: u32,
        total_chunks: u32,
        start: u32,
        entries: Vec<ObjectChunk>,
        created_at: u64,
        expires_at: u64,
    },
    GetChunk {
        request_id: [u8; 16],
        content: ContentId,
        chunk: ChunkId,
        offset: u32,
        length: u16,
        digest: ChunkDigest,
    },
    Chunk {
        request_id: [u8; 16],
        content: ContentId,
        chunk: ChunkId,
        offset: u32,
        total_len: u32,
        digest: ChunkDigest,
        payload: Vec<u8>,
    },
    PutChunk {
        request_id: [u8; 16],
        content: ContentId,
        chunk: ChunkId,
        offset: u32,
        total_len: u32,
        digest: ChunkDigest,
        payload: Vec<u8>,
        expires_at: u64,
    },
    Ack { request_id: [u8; 16], accepted: bool, code: u8 },
    ProviderAnnouncement(ProviderAdvertisement),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageCodecError {
    TooShort,
    InvalidMagic,
    UnsupportedVersion,
    UnknownMessage,
    InvalidLength,
    CapacityExceeded,
    InvalidManifestPage,
    InvalidChunkFrame,
}

impl StorageControl {
    pub fn encode(&self) -> Result<Vec<u8>, StorageCodecError> {
        let mut out = Vec::with_capacity(1024);
        out.extend_from_slice(&STORAGE_MAGIC);
        out.push(STORAGE_VERSION);
        match self {
            Self::GetManifest { request_id, content, start, limit } => {
                out.push(KIND_GET_MANIFEST);
                out.extend_from_slice(request_id);
                out.extend_from_slice(content.as_bytes());
                out.extend_from_slice(&start.to_be_bytes());
                out.extend_from_slice(&limit.min(MAX_MANIFEST_PAGE_ENTRIES as u16).to_be_bytes());
            }
            Self::ManifestPage { request_id, content, version, total_size, chunk_size, total_chunks, start, entries, created_at, expires_at } => {
                if entries.len() > MAX_MANIFEST_PAGE_ENTRIES { return Err(StorageCodecError::CapacityExceeded); }
                out.push(KIND_MANIFEST_PAGE);
                out.extend_from_slice(request_id);
                out.extend_from_slice(content.as_bytes());
                out.extend_from_slice(&version.to_be_bytes());
                out.extend_from_slice(&total_size.to_be_bytes());
                out.extend_from_slice(&chunk_size.to_be_bytes());
                out.extend_from_slice(&total_chunks.to_be_bytes());
                out.extend_from_slice(&start.to_be_bytes());
                out.extend_from_slice(&(entries.len() as u16).to_be_bytes());
                out.extend_from_slice(&created_at.to_be_bytes());
                out.extend_from_slice(&expires_at.to_be_bytes());
                for entry in entries {
                    out.extend_from_slice(&entry.id.0.to_be_bytes());
                    out.extend_from_slice(&entry.offset.to_be_bytes());
                    out.extend_from_slice(&entry.length.to_be_bytes());
                    out.extend_from_slice(entry.digest.as_bytes());
                }
            }
            Self::GetChunk { request_id, content, chunk, offset, length, digest } => {
                if *length as usize > MAX_STORAGE_FRAGMENT { return Err(StorageCodecError::InvalidChunkFrame); }
                out.push(KIND_GET_CHUNK);
                out.extend_from_slice(request_id);
                out.extend_from_slice(content.as_bytes());
                out.extend_from_slice(&chunk.0.to_be_bytes());
                out.extend_from_slice(&offset.to_be_bytes());
                out.extend_from_slice(&length.to_be_bytes());
                out.extend_from_slice(digest.as_bytes());
            }
            Self::Chunk { request_id, content, chunk, offset, total_len, digest, payload } => {
                validate_fragment(*offset, *total_len, payload.len())?;
                out.push(KIND_CHUNK);
                out.extend_from_slice(request_id);
                out.extend_from_slice(content.as_bytes());
                out.extend_from_slice(&chunk.0.to_be_bytes());
                out.extend_from_slice(&offset.to_be_bytes());
                out.extend_from_slice(&total_len.to_be_bytes());
                out.extend_from_slice(digest.as_bytes());
                out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
                out.extend_from_slice(payload);
            }
            Self::PutChunk { request_id, content, chunk, offset, total_len, digest, payload, expires_at } => {
                validate_fragment(*offset, *total_len, payload.len())?;
                out.push(KIND_PUT_CHUNK);
                out.extend_from_slice(request_id);
                out.extend_from_slice(content.as_bytes());
                out.extend_from_slice(&chunk.0.to_be_bytes());
                out.extend_from_slice(&offset.to_be_bytes());
                out.extend_from_slice(&total_len.to_be_bytes());
                out.extend_from_slice(digest.as_bytes());
                out.extend_from_slice(&expires_at.to_be_bytes());
                out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
                out.extend_from_slice(payload);
            }
            Self::Ack { request_id, accepted, code } => {
                out.push(KIND_ACK);
                out.extend_from_slice(request_id);
                out.push(u8::from(*accepted));
                out.push(*code);
            }
            Self::ProviderAnnouncement(advertisement) => {
                out.push(KIND_PROVIDER);
                encode_provider(&mut out, advertisement);
            }
        }
        if out.len() > MAX_STORAGE_MESSAGE { return Err(StorageCodecError::CapacityExceeded); }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, StorageCodecError> {
        if bytes.len() < 6 { return Err(StorageCodecError::TooShort); }
        if bytes[0..4] != STORAGE_MAGIC { return Err(StorageCodecError::InvalidMagic); }
        if bytes[4] != STORAGE_VERSION { return Err(StorageCodecError::UnsupportedVersion); }
        match bytes[5] {
            KIND_GET_MANIFEST => {
                let mut r = Reader::new(&bytes[6..]);
                let request_id = r.take_array::<16>()?;
                let content = ContentId::from_raw(r.take_array::<CONTENT_ID_LENGTH>()?);
                let start = r.u32()?;
                let limit = r.u16()?;
                if limit as usize > MAX_MANIFEST_PAGE_ENTRIES { return Err(StorageCodecError::CapacityExceeded); }
                r.finish()?;
                Ok(Self::GetManifest { request_id, content, start, limit })
            }
            KIND_MANIFEST_PAGE => decode_manifest_page(&bytes[6..]),
            KIND_GET_CHUNK => {
                let mut r = Reader::new(&bytes[6..]);
                let request_id = r.take_array::<16>()?;
                let content = ContentId::from_raw(r.take_array::<CONTENT_ID_LENGTH>()?);
                let chunk = ChunkId(r.u64()?);
                let offset = r.u32()?;
                let length = r.u16()?;
                let digest = ChunkDigest::from_raw(r.take_array::<32>()?);
                if length as usize > MAX_STORAGE_FRAGMENT { return Err(StorageCodecError::InvalidChunkFrame); }
                r.finish()?;
                Ok(Self::GetChunk { request_id, content, chunk, offset, length, digest })
            }
            KIND_CHUNK => decode_chunk(&bytes[6..], false),
            KIND_PUT_CHUNK => decode_chunk(&bytes[6..], true),
            KIND_ACK => {
                let mut r = Reader::new(&bytes[6..]);
                let request_id = r.take_array::<16>()?;
                let accepted = r.u8()? != 0;
                let code = r.u8()?;
                r.finish()?;
                Ok(Self::Ack { request_id, accepted, code })
            }
            KIND_PROVIDER => decode_provider(&bytes[6..]),
            _ => Err(StorageCodecError::UnknownMessage),
        }
    }
}

fn validate_fragment(offset: u32, total_len: u32, payload_len: usize) -> Result<(), StorageCodecError> {
    if payload_len > MAX_STORAGE_FRAGMENT || offset.saturating_add(payload_len as u32) > total_len || total_len as usize > 8 * 1024 * 1024 {
        return Err(StorageCodecError::InvalidChunkFrame);
    }
    Ok(())
}

fn decode_chunk(bytes: &[u8], put: bool) -> Result<StorageControl, StorageCodecError> {
    let mut r = Reader::new(bytes);
    let request_id = r.take_array::<16>()?;
    let content = ContentId::from_raw(r.take_array::<32>()?);
    let chunk = ChunkId(r.u64()?);
    let offset = r.u32()?;
    let total_len = r.u32()?;
    let digest = ChunkDigest::from_raw(r.take_array::<32>()?);
    let expires_at = if put { r.u64()? } else { 0 };
    let payload_len = r.u16()? as usize;
    validate_fragment(offset, total_len, payload_len)?;
    let payload = r.take(payload_len)?.to_vec();
    r.finish()?;
    if put { Ok(StorageControl::PutChunk { request_id, content, chunk, offset, total_len, digest, payload, expires_at }) }
    else { Ok(StorageControl::Chunk { request_id, content, chunk, offset, total_len, digest, payload }) }
}

fn decode_manifest_page(bytes: &[u8]) -> Result<StorageControl, StorageCodecError> {
    let mut r = Reader::new(bytes);
    let request_id = r.take_array::<16>()?;
    let content = ContentId::from_raw(r.take_array::<32>()?);
    let version = r.u64()?;
    let total_size = r.u64()?;
    let chunk_size = r.u32()?;
    let total_chunks = r.u32()?;
    let start = r.u32()?;
    let count = r.u16()? as usize;
    let created_at = r.u64()?;
    let expires_at = r.u64()?;
    if count > MAX_MANIFEST_PAGE_ENTRIES { return Err(StorageCodecError::CapacityExceeded); }
    if start.saturating_add(count as u32) > total_chunks { return Err(StorageCodecError::InvalidManifestPage); }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(ObjectChunk { id: ChunkId(r.u64()?), offset: r.u64()?, length: r.u32()?, digest: ChunkDigest::from_raw(r.take_array::<32>()?) });
    }
    r.finish()?;
    Ok(StorageControl::ManifestPage { request_id, content, version, total_size, chunk_size, total_chunks, start, entries, created_at, expires_at })
}

fn encode_provider(out: &mut Vec<u8>, ad: &ProviderAdvertisement) {
    out.extend_from_slice(ad.content.as_bytes());
    out.extend_from_slice(ad.provider.as_bytes());
    out.extend_from_slice(&ad.version.to_be_bytes());
    out.extend_from_slice(&ad.capacity_bytes.to_be_bytes());
    out.extend_from_slice(&ad.cached_bytes.to_be_bytes());
    out.extend_from_slice(&ad.cost_micros.to_be_bytes());
    out.extend_from_slice(&ad.reliability_ppm.to_be_bytes());
    out.extend_from_slice(&ad.observed_at.to_be_bytes());
    out.extend_from_slice(&ad.expires_at.to_be_bytes());
    out.extend_from_slice(&ad.owner_public_key);
    out.extend_from_slice(&ad.signature);
}

fn decode_provider(bytes: &[u8]) -> Result<StorageControl, StorageCodecError> {
    let mut r = Reader::new(bytes);
    let content = ContentId::from_raw(r.take_array::<32>()?);
    let provider = NodeId::from_bytes(r.take_array::<32>()?);
    let version = r.u64()?;
    let capacity_bytes = r.u64()?;
    let cached_bytes = r.u64()?;
    let cost_micros = r.u64()?;
    let reliability_ppm = r.u32()?;
    let observed_at = r.u64()?;
    let expires_at = r.u64()?;
    let owner_public_key = r.take_array::<32>()?;
    let signature = r.take_array::<64>()?;
    r.finish()?;
    Ok(StorageControl::ProviderAnnouncement(ProviderAdvertisement { content, provider, version, capacity_bytes, cached_bytes, cost_micros, reliability_ppm, observed_at, expires_at, owner_public_key, signature }))
}

struct Reader<'a> { bytes: &'a [u8], pos: usize }
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, pos: 0 } }
    fn take(&mut self, count: usize) -> Result<&'a [u8], StorageCodecError> {
        let end = self.pos.checked_add(count).ok_or(StorageCodecError::InvalidLength)?;
        if end > self.bytes.len() { return Err(StorageCodecError::TooShort); }
        let value = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(value)
    }
    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], StorageCodecError> {
        self.take(N)?.try_into().map_err(|_| StorageCodecError::InvalidLength)
    }
    fn u8(&mut self) -> Result<u8, StorageCodecError> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16, StorageCodecError> { Ok(u16::from_be_bytes(self.take_array()?)) }
    fn u32(&mut self) -> Result<u32, StorageCodecError> { Ok(u32::from_be_bytes(self.take_array()?)) }
    fn u64(&mut self) -> Result<u64, StorageCodecError> { Ok(u64::from_be_bytes(self.take_array()?)) }
    fn finish(&self) -> Result<(), StorageCodecError> { if self.pos == self.bytes.len() { Ok(()) } else { Err(StorageCodecError::InvalidLength) } }
}

impl ChunkDigest {
    pub(crate) fn from_raw(bytes: [u8; 32]) -> Self { Self(bytes) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::chunk::MIN_CHUNK_SIZE;

    #[test]
    fn get_chunk_round_trip() {
        let content = ContentId::from_bytes(b"content");
        let digest = ChunkDigest::of(content, ChunkId(2), &[4; MIN_CHUNK_SIZE]);
        let message = StorageControl::GetChunk { request_id: [8; 16], content, chunk: ChunkId(2), offset: 1024, length: 1024, digest };
        assert_eq!(StorageControl::decode(&message.encode().unwrap()).unwrap(), message);
    }
}
