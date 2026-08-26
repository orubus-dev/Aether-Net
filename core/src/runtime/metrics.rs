use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct CoreMetrics {
    pub packets_sent: AtomicU64,
    pub packets_received: AtomicU64,
    pub packets_dropped: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub relay_packets: AtomicU64,
    pub active_sessions: AtomicU64,
}

impl CoreMetrics {
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            packets_sent: self.packets_sent.load(Ordering::Relaxed),

            packets_received: self.packets_received.load(Ordering::Relaxed),

            packets_dropped: self.packets_dropped.load(Ordering::Relaxed),

            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),

            bytes_received: self.bytes_received.load(Ordering::Relaxed),

            relay_packets: self.relay_packets.load(Ordering::Relaxed),

            active_sessions: self.active_sessions.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_dropped: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub relay_packets: u64,
    pub active_sessions: u64,
}
