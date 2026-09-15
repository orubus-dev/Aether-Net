use super::capabilities::{PhysicalCapabilities, PhysicalTransport};
use super::link::PhysicalLink;

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

/// A concrete platform-facing carrier. Native Aether code hands this driver
/// an already-formed link frame without depending on a specific radio/wire.
pub trait PhysicalDriver: PhysicalAdapter + Send + Sync {
    fn transmit(
        &mut self,
        link_id: super::link::PhysicalLinkId,
        frame: &[u8],
    ) -> Result<(), PhysicalError>;

    fn poll_receive(
        &mut self,
    ) -> Result<Option<(super::link::PhysicalLinkId, Vec<u8>)>, PhysicalError>;
}
