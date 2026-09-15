use super::capabilities::{PhysicalCapabilities, PhysicalTransport};
use super::link::PhysicalLink;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhysicalAnalytics {
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_drops: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_drops: u64,
}

impl PhysicalAnalytics {
    pub fn total_bytes(self) -> u64 {
        self.rx_bytes.saturating_add(self.tx_bytes)
    }
    pub fn total_packets(self) -> u64 {
        self.rx_packets.saturating_add(self.tx_packets)
    }
    pub fn total_errors(self) -> u64 {
        self.rx_errors.saturating_add(self.tx_errors)
    }
    pub fn total_drops(self) -> u64 {
        self.rx_drops.saturating_add(self.tx_drops)
    }
}

pub trait PhysicalAdapter: Send + Sync {
    fn transport(&self) -> PhysicalTransport;
    fn capabilities(&self) -> PhysicalCapabilities;
    fn discover(&self) -> Result<Vec<PhysicalLink>, PhysicalError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalError {
    Unsupported,
    Unavailable,
    PermissionDenied,
    HardwareFailure,
    DiscoveryFailed,
    ConnectionFailed,
    CapacityExceeded,
    FrameTooLarge,
}

pub trait PhysicalDriver: PhysicalAdapter + Send + Sync {
    fn mtu(&self) -> usize;
    fn transmit(
        &mut self,
        link_id: super::link::PhysicalLinkId,
        frame: &[u8],
    ) -> Result<(), PhysicalError>;
    fn poll_receive(
        &mut self,
    ) -> Result<Option<(super::link::PhysicalLinkId, Vec<u8>)>, PhysicalError>;
    fn analytics(
        &self,
        _link_id: super::link::PhysicalLinkId,
    ) -> Result<Option<PhysicalAnalytics>, PhysicalError> {
        Ok(None)
    }
}
