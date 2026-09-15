use super::capabilities::{PhysicalCapabilities, PhysicalTransport};
use super::interface::{PhysicalAdapter, PhysicalAdapterExt, PhysicalError};
use super::link::{PhysicalLink, PhysicalLinkId};
use super::engine::UnderlayProperties;

#[derive(Debug, Clone)]
pub struct ConfiguredAdapter {
    transport: PhysicalTransport,
    capabilities: PhysicalCapabilities,
    properties: UnderlayProperties,
    discovered_links: Vec<PhysicalLink>,
}

impl ConfiguredAdapter {
    pub fn new(
        transport: PhysicalTransport,
        capabilities: PhysicalCapabilities,
        properties: UnderlayProperties,
    ) -> Self {
        Self {
            transport,
            capabilities,
            properties,
            discovered_links: Vec::new(),
        }
    }

    pub fn with_link(mut self, link: PhysicalLink) -> Result<Self, PhysicalError> {
        if link.transport != self.transport {
            return Err(PhysicalError::DiscoveryFailed);
        }
        if link.packet_loss_ppm > 1_000_000 || link.signal_quality > 100 {
            return Err(PhysicalError::DiscoveryFailed);
        }
        self.discovered_links.push(link);
        Ok(self)
    }

    pub fn add_link(&mut self, link: PhysicalLink) -> Result<(), PhysicalError> {
        if link.transport != self.transport {
            return Err(PhysicalError::DiscoveryFailed);
        }
        if link.packet_loss_ppm > 1_000_000 || link.signal_quality > 100 {
            return Err(PhysicalError::DiscoveryFailed);
        }
        if !self.discovered_links.iter().any(|existing| existing.id == link.id) {
            self.discovered_links.push(link);
        }
        Ok(())
    }
}

impl PhysicalAdapter for ConfiguredAdapter {
    fn transport(&self) -> PhysicalTransport {
        self.transport
    }

    fn capabilities(&self) -> PhysicalCapabilities {
        self.capabilities
    }

    fn discover(&self) -> Result<Vec<PhysicalLink>, PhysicalError> {
        Ok(self.discovered_links.clone())
    }
}

impl PhysicalAdapterExt for ConfiguredAdapter {
    fn properties(&self) -> UnderlayProperties {
        self.properties
    }
}

macro_rules! carrier_adapter {
    ($name:ident, $transport:expr, $caps:expr, $mtu:expr, $broadcast:expr, $multicast:expr) => {
        pub type $name = ConfiguredAdapter;

        #[allow(dead_code)]
        pub fn $name##_new(properties: UnderlayProperties) -> ConfiguredAdapter {
            ConfiguredAdapter::new(
                $transport,
                $caps,
                UnderlayProperties {
                    transport: $transport,
                    mtu: $mtu,
                    bandwidth_bps: properties.bandwidth_bps,
                    latency_us: properties.latency_us,
                    energy_cost: properties.energy_cost,
                    supports_broadcast: $broadcast,
                    supports_multicast: $multicast,
                },
            )
        }
    };
}

pub fn ethernet_adapter(properties: UnderlayProperties) -> ConfiguredAdapter {
    ConfiguredAdapter::new(
        PhysicalTransport::Ethernet,
        PhysicalCapabilities { ethernet: true, ..PhysicalCapabilities::empty() },
        UnderlayProperties { transport: PhysicalTransport::Ethernet, mtu: properties.mtu.min(9000), supports_broadcast: true, supports_multicast: true, ..properties },
    )
}

pub fn wifi_adapter(properties: UnderlayProperties) -> ConfiguredAdapter {
    ConfiguredAdapter::new(
        PhysicalTransport::Wifi,
        PhysicalCapabilities { wifi: true, ..PhysicalCapabilities::empty() },
        UnderlayProperties { transport: PhysicalTransport::Wifi, mtu: properties.mtu.min(2304), supports_broadcast: true, supports_multicast: true, ..properties },
    )
}

pub fn wifi_direct_adapter(properties: UnderlayProperties) -> ConfiguredAdapter {
    ConfiguredAdapter::new(
        PhysicalTransport::WifiDirect,
        PhysicalCapabilities { wifi_direct: true, ..PhysicalCapabilities::empty() },
        UnderlayProperties { transport: PhysicalTransport::WifiDirect, mtu: properties.mtu.min(2304), supports_broadcast: false, supports_multicast: false, ..properties },
    )
}

pub fn bluetooth_le_adapter(properties: UnderlayProperties) -> ConfiguredAdapter {
    ConfiguredAdapter::new(
        PhysicalTransport::BluetoothLe,
        PhysicalCapabilities { bluetooth_le: true, ..PhysicalCapabilities::empty() },
        UnderlayProperties { transport: PhysicalTransport::BluetoothLe, mtu: properties.mtu.min(512), supports_broadcast: false, supports_multicast: false, ..properties },
    )
}

pub fn direct_link_adapter(properties: UnderlayProperties) -> ConfiguredAdapter {
    ConfiguredAdapter::new(
        PhysicalTransport::Uwb,
        PhysicalCapabilities { uwb: true, ..PhysicalCapabilities::empty() },
        UnderlayProperties { transport: PhysicalTransport::Uwb, mtu: properties.mtu.min(4096), supports_broadcast: false, supports_multicast: false, ..properties },
    )
}

pub fn link_id(seed: u8) -> PhysicalLinkId {
    PhysicalLinkId([seed; 16])
}
