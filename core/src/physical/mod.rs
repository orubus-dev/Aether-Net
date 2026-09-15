pub mod adapters;
pub mod capabilities;
pub mod driver;
pub mod engine;
pub mod interface;
pub mod link;
#[cfg(target_os = "linux")]
pub mod linux_ethernet;
pub mod runtime;

pub use adapters::{
    ConfiguredAdapter, bluetooth_le_adapter, direct_link_adapter, ethernet_adapter, link_id,
    wifi_adapter, wifi_direct_adapter,
};
pub use capabilities::{PhysicalCapabilities, PhysicalTransport};
pub use driver::LoopbackDriver;
pub use engine::{
    DEFAULT_MAX_ADAPTERS, DEFAULT_MAX_LINKS, PhysicalAdapterExt, PhysicalEngine, PhysicalSnapshot,
    UnderlayFrame, UnderlayProperties,
};
pub use interface::{PhysicalAdapter, PhysicalDriver, PhysicalError};
pub use link::{PhysicalLink, PhysicalLinkId};
#[cfg(target_os = "linux")]
pub use linux_ethernet::{AETHER_ETHERTYPE, LinuxEthernetDriver};
pub use runtime::{DEFAULT_MAX_DRIVERS, PhysicalRuntime};
