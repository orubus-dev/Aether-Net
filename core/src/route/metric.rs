use crate::network::protocol::TrafficPriority;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteMetrics {
    pub latency_us: u64,
    pub jitter_us: u64,
    pub packet_loss_ppm: u32,
    pub bandwidth_bps: u64,
    pub reliability_ppm: u32,
    pub monetary_cost_micros: u64,
    pub energy_cost: u32,
    pub hop_count: u16,
}

impl RouteMetrics {
    pub fn score(&self, priority: TrafficPriority) -> f64 {
        let latency = 1.0 / (1.0 + self.latency_us as f64);
        let jitter = 1.0 / (1.0 + self.jitter_us as f64);
        let loss = 1.0 - (self.packet_loss_ppm as f64 / 1_000_000.0);
        let reliability = self.reliability_ppm as f64 / 1_000_000.0;
        let bandwidth = (self.bandwidth_bps as f64).ln_1p();
        let cost = 1.0 / (1.0 + self.monetary_cost_micros as f64);
        let energy = 1.0 / (1.0 + self.energy_cost as f64);
        let hops = 1.0 / (1.0 + self.hop_count as f64);

        match priority {
            TrafficPriority::Emergency => {
                reliability * 0.35 + loss * 0.25 + latency * 0.20 + hops * 0.20
            }
            TrafficPriority::Voice => {
                latency * 0.35 + jitter * 0.30 + loss * 0.20 + reliability * 0.15
            }
            TrafficPriority::Interactive => {
                latency * 0.30
                    + jitter * 0.25
                    + loss * 0.20
                    + reliability * 0.15
                    + bandwidth * 0.10
            }
            TrafficPriority::Messaging => {
                reliability * 0.30 + loss * 0.25 + latency * 0.15 + cost * 0.20 + hops * 0.10
            }
            TrafficPriority::Browsing => {
                bandwidth * 0.30 + latency * 0.15 + reliability * 0.20 + cost * 0.25 + hops * 0.10
            }
            TrafficPriority::Background => {
                cost * 0.40 + bandwidth * 0.25 + reliability * 0.20 + energy * 0.15
            }
            TrafficPriority::Prefetch => {
                cost * 0.45 + energy * 0.25 + bandwidth * 0.20 + hops * 0.10
            }
        }
    }
}
