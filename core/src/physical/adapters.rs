use super::capabilities::{PhysicalCapabilities, PhysicalTransport};
use super::engine::{PhysicalAdapterExt, UnderlayProperties};
use super::interface::{PhysicalAdapter, PhysicalError};
use super::link::{PhysicalLink, PhysicalLinkId};

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
    ) -> Result<Self, PhysicalError> {
        if properties.transport != transport || properties.mtu == 0 || properties.bandwidth_bps == 0
        {
            return Err(PhysicalError::Unsupported);
        }
        Ok(Self {
            transport,
            capabilities,
            properties,
            discovered_links: Vec::new(),
        })
    }

    pub fn with_link(mut self, link: PhysicalLink) -> Result<Self, PhysicalError> {
        self.add_link(link)?;
        Ok(self)
    }

    pub fn add_link(&mut self, link: PhysicalLink) -> Result<(), PhysicalError> {
        if link.transport != self.transport
            || link.packet_loss_ppm > 1_000_000
            || link.signal_quality > 100
            || link.estimated_bandwidth_bps == 0
        {
            return Err(PhysicalError::DiscoveryFailed);
        }
        if let Some(existing) = self
            .discovered_links
            .iter_mut()
            .find(|existing| existing.id == link.id)
        {
            *existing = link;
        } else {
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

pub fn ethernet_adapter(
    properties: UnderlayProperties,
) -> Result<ConfiguredAdapter, PhysicalError> {
    ConfiguredAdapter::new(
        PhysicalTransport::Ethernet,
        PhysicalCapabilities {
            ethernet: true,
            ..PhysicalCapabilities::empty()
        },
        UnderlayProperties {
            transport: PhysicalTransport::Ethernet,
            mtu: properties.mtu.min(9000),
            supports_broadcast: true,
            supports_multicast: true,
            ..properties
        },
    )
}

pub fn wifi_adapter(properties: UnderlayProperties) -> Result<ConfiguredAdapter, PhysicalError> {
    ConfiguredAdapter::new(
        PhysicalTransport::Wifi,
        PhysicalCapabilities {
            wifi: true,
            ..PhysicalCapabilities::empty()
        },
        UnderlayProperties {
            transport: PhysicalTransport::Wifi,
            mtu: properties.mtu.min(2304),
            supports_broadcast: true,
            supports_multicast: true,
            ..properties
        },
    )
}

pub fn wifi_direct_adapter(
    properties: UnderlayProperties,
) -> Result<ConfiguredAdapter, PhysicalError> {
    ConfiguredAdapter::new(
        PhysicalTransport::WifiDirect,
        PhysicalCapabilities {
            wifi_direct: true,
            ..PhysicalCapabilities::empty()
        },
        UnderlayProperties {
            transport: PhysicalTransport::WifiDirect,
            mtu: properties.mtu.min(2304),
            supports_broadcast: false,
            supports_multicast: false,
            ..properties
        },
    )
}

pub fn bluetooth_le_adapter(
    properties: UnderlayProperties,
) -> Result<ConfiguredAdapter, PhysicalError> {
    ConfiguredAdapter::new(
        PhysicalTransport::BluetoothLe,
        PhysicalCapabilities {
            bluetooth_le: true,
            ..PhysicalCapabilities::empty()
        },
        UnderlayProperties {
            transport: PhysicalTransport::BluetoothLe,
            mtu: properties.mtu.min(512),
            supports_broadcast: false,
            supports_multicast: false,
            ..properties
        },
    )
}

pub fn direct_link_adapter(
    properties: UnderlayProperties,
) -> Result<ConfiguredAdapter, PhysicalError> {
    ConfiguredAdapter::new(
        PhysicalTransport::Uwb,
        PhysicalCapabilities {
            uwb: true,
            ..PhysicalCapabilities::empty()
        },
        UnderlayProperties {
            transport: PhysicalTransport::Uwb,
            mtu: properties.mtu.min(4096),
            supports_broadcast: false,
            supports_multicast: false,
            ..properties
        },
    )
}

pub fn link_id(seed: u8) -> PhysicalLinkId {
    PhysicalLinkId([seed; 16])
}
