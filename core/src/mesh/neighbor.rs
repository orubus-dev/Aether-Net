use crate::network::node::NodeId;
use crate::physical::PhysicalTransport;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkMetrics {
    pub latency_us: u64,
    pub bandwidth_bps: u64,
    pub packet_loss_ppm: u32,
    pub jitter_us: u64,
    pub reliability_ppm: u32,
}

impl LinkMetrics {
    pub fn quality_score(&self) -> f64 {
        let latency = 1.0 / (1.0 + self.latency_us as f64);
        let bandwidth = (self.bandwidth_bps as f64).ln_1p();
        let loss = 1.0 - (self.packet_loss_ppm as f64 / 1_000_000.0);
        let reliability = self.reliability_ppm as f64 / 1_000_000.0;

        (latency * 0.35) + (bandwidth * 0.15) + (loss * 0.20) + (reliability * 0.30)
    }
}

#[derive(Debug, Clone)]
pub struct Neighbor {
    pub node_id: NodeId,
    pub transport: PhysicalTransport,
    pub metrics: LinkMetrics,
    pub battery_percent: Option<u8>,
    pub charging: bool,
    pub thermal_ok: bool,
    pub last_seen: Instant,
}

impl Neighbor {
    pub fn relay_eligible(&self) -> bool {
        if !self.thermal_ok {
            return false;
        }

        if self.charging {
            return true;
        }

        self.battery_percent
            .map(|battery| battery > 10)
            .unwrap_or(false)
    }
}
