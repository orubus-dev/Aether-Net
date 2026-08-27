use crate::network::{FlowId, PacketId};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForwardCacheKey {
    pub packet: PacketId,

    pub flow: FlowId,
}

#[derive(Debug, Clone, Copy)]
pub struct ForwardCacheEntry {
    pub next_hop: [u8; 32],

    pub inserted: Instant,
}

#[derive(Debug)]
pub struct ForwardCache {
    entries: HashMap<ForwardCacheKey, ForwardCacheEntry>,

    ttl: Duration,
}

impl ForwardCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    pub fn lookup(&self, key: &ForwardCacheKey, now: Instant) -> Option<ForwardCacheEntry> {
        let entry = self.entries.get(key)?;

        if now.duration_since(entry.inserted) >= self.ttl {
            return None;
        }

        Some(*entry)
    }

    pub fn insert(&mut self, key: ForwardCacheKey, next_hop: [u8; 32], now: Instant) {
        self.entries.insert(
            key,
            ForwardCacheEntry {
                next_hop,
                inserted: now,
            },
        );
    }

    pub fn expire(&mut self, now: Instant) {
        self.entries
            .retain(|_key, entry| now.duration_since(entry.inserted) < self.ttl);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
