use crate::storage::content::ContentId;

pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

pub const MIN_CHUNK_SIZE: usize = 16 * 1024;

pub const MAX_CHUNK_SIZE: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentChunk {
    pub id: ChunkId,

    pub content: ContentId,

    pub offset: u64,

    pub length: u32,
}

#[derive(Debug, Clone)]
pub struct ContentManifest {
    pub content: ContentId,

    pub total_size: u64,

    pub chunk_size: u32,

    pub chunks: Vec<ContentChunk>,
}

impl ContentManifest {
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn validate(&self) -> bool {
        if (self.chunk_size as usize) < MIN_CHUNK_SIZE {
            return false;
        }

        if (self.chunk_size as usize) > MAX_CHUNK_SIZE {
            return false;
        }

        let mut expected_offset = 0u64;

        for (index, chunk) in self.chunks.iter().enumerate() {
            if chunk.id.0 != index as u64 {
                return false;
            }

            if chunk.offset != expected_offset {
                return false;
            }

            expected_offset = expected_offset.saturating_add(chunk.length as u64);
        }

        expected_offset == self.total_size
    }
}
