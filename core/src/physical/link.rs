use super::capabilities::PhysicalTransport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PhysicalLinkId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalLink {
    pub id: PhysicalLinkId,
    pub transport: PhysicalTransport,
    pub estimated_latency_us: u64,
    pub estimated_bandwidth_bps: u64,
    pub packet_loss_ppm: u32,
    pub signal_quality: u8,
    pub active: bool,
}
