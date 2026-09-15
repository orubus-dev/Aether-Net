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
}
