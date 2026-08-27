use crate::network::protocol::TrafficPriority;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandwidthLimit {
    pub bits_per_second: u64,
}

impl BandwidthLimit {
    pub const UNLIMITED: Self = Self {
        bits_per_second: u64::MAX,
    };

    pub const fn new(bits_per_second: u64) -> Self {
        Self { bits_per_second }
    }

    pub fn is_unlimited(&self) -> bool {
        self.bits_per_second == u64::MAX
    }

    pub fn bytes_per_second(&self) -> u64 {
        if self.is_unlimited() {
            return u64::MAX;
        }

        self.bits_per_second / 8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeBandwidthPolicy {
    pub ingress: BandwidthLimit,

    pub egress: BandwidthLimit,
}

impl Default for NodeBandwidthPolicy {
    fn default() -> Self {
        Self {
            ingress: BandwidthLimit::UNLIMITED,

            egress: BandwidthLimit::UNLIMITED,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrafficBudget {
    pub priority: TrafficPriority,

    pub ingress: BandwidthLimit,

    pub egress: BandwidthLimit,
}

impl TrafficBudget {
    pub fn unlimited(priority: TrafficPriority) -> Self {
        Self {
            priority,
            ingress: BandwidthLimit::UNLIMITED,
            egress: BandwidthLimit::UNLIMITED,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TransferWindow {
    pub started: Instant,

    pub bytes_sent: u64,

    pub bytes_received: u64,
}

impl TransferWindow {
    pub fn new(now: Instant) -> Self {
        Self {
            started: now,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent = self.bytes_sent.saturating_add(bytes);
    }

    pub fn record_received(&mut self, bytes: u64) {
        self.bytes_received = self.bytes_received.saturating_add(bytes);
    }

    pub fn elapsed_seconds(&self, now: Instant) -> f64 {
        now.duration_since(self.started)
            .as_secs_f64()
            .max(0.000_001)
    }

    pub fn send_rate_bps(&self, now: Instant) -> u64 {
        ((self.bytes_sent as f64 * 8.0) / self.elapsed_seconds(now)) as u64
    }

    pub fn receive_rate_bps(&self, now: Instant) -> u64 {
        ((self.bytes_received as f64 * 8.0) / self.elapsed_seconds(now)) as u64
    }
}
