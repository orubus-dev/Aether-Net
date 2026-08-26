use crate::network::node::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayCapacity {
    pub upstream_bps: u64,
    pub available_bps: u64,
    pub monthly_bytes_remaining: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GatewayAdvertisement {
    pub node_id: NodeId,
    pub capacity: GatewayCapacity,
    pub latency_us: u64,
    pub reliability_ppm: u32,
    pub price_micros_per_gb: u64,
}

impl GatewayAdvertisement {
    pub fn available(&self) -> bool {
        self.capacity.available_bps > 0
            && self.capacity.monthly_bytes_remaining > 0
            && self.reliability_ppm > 0
    }
}
