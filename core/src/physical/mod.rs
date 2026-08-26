pub mod capabilities;
pub mod interface;
pub mod link;

pub use capabilities::{PhysicalCapabilities, PhysicalTransport};

pub use interface::{PhysicalAdapter, PhysicalError};

pub use link::{PhysicalLink, PhysicalLinkId};
