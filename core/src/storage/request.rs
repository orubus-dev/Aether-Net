use crate::storage::chunk::ChunkId;
use crate::storage::content::ContentId;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkRequest {
    pub content: ContentId,

    pub chunk: ChunkId,
}

#[derive(Debug, Default)]
pub struct RequestDeduplicator {
    pending: HashSet<ChunkRequest>,
}

impl RequestDeduplicator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&mut self, request: ChunkRequest) -> bool {
        self.pending.insert(request)
    }

    pub fn complete(&mut self, request: &ChunkRequest) -> bool {
        self.pending.remove(request)
    }

    pub fn is_pending(&self, request: &ChunkRequest) -> bool {
        self.pending.contains(request)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}
