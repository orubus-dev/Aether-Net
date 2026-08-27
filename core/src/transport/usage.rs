use crate::physical::PhysicalTransport;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct TransportUsage {
    pub bytes_sent: AtomicU64,

    pub bytes_received: AtomicU64,

    pub packets_sent: AtomicU64,

    pub packets_received: AtomicU64,
}

impl TransportUsage {
    pub fn record_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);

        self.packets_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);

        self.packets_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    pub fn received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Default)]
pub struct TransportUsageTable {
    transports: HashMap<PhysicalTransport, TransportUsage>,
}

impl TransportUsageTable {
    pub fn record_sent(&mut self, transport: PhysicalTransport, bytes: u64) {
        self.transports
            .entry(transport)
            .or_default()
            .record_sent(bytes);
    }

    pub fn record_received(&mut self, transport: PhysicalTransport, bytes: u64) {
        self.transports
            .entry(transport)
            .or_default()
            .record_received(bytes);
    }

    pub fn get(&self, transport: &PhysicalTransport) -> Option<&TransportUsage> {
        self.transports.get(transport)
    }
}
