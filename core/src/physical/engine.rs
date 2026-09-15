use super::{PhysicalAdapter, PhysicalCapabilities, PhysicalError, PhysicalLink, PhysicalLinkId, PhysicalTransport};

pub const DEFAULT_MAX_ADAPTERS: usize = 64;
pub const DEFAULT_MAX_LINKS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnderlayProperties {
    pub transport: PhysicalTransport,
    pub mtu: usize,
    pub bandwidth_bps: u64,
    pub latency_us: u64,
    pub energy_cost: u32,
    pub supports_broadcast: bool,
    pub supports_multicast: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnderlayFrame {
    pub link_id: PhysicalLinkId,
    pub payload_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSnapshot {
    pub adapter_count: usize,
    pub active_link_count: usize,
}

pub trait PhysicalAdapterExt: PhysicalAdapter {
    fn properties(&self) -> UnderlayProperties;
}

pub struct PhysicalEngine {
    max_adapters: usize,
    max_links: usize,
    adapters: Vec<Box<dyn PhysicalAdapterExt>>,
    links: Vec<PhysicalLink>,
}

impl PhysicalEngine {
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_ADAPTERS, DEFAULT_MAX_LINKS)
    }

    pub fn with_limits(max_adapters: usize, max_links: usize) -> Self {
        Self {
            max_adapters: max_adapters.max(1),
            max_links: max_links.max(1),
            adapters: Vec::new(),
            links: Vec::new(),
        }
    }

    pub fn register_adapter(
        &mut self,
        adapter: Box<dyn PhysicalAdapterExt>,
    ) -> Result<(), PhysicalError> {
        if self.adapters.len() >= self.max_adapters {
            return Err(PhysicalError::HardwareFailure);
        }
        self.adapters.push(adapter);
        Ok(())
    }

    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }

    pub fn links(&self) -> &[PhysicalLink] {
        &self.links
    }

    pub fn snapshot(&self) -> PhysicalSnapshot {
        PhysicalSnapshot {
            adapter_count: self.adapters.len(),
            active_link_count: self.links.iter().filter(|link| link.active).count(),
        }
    }

    pub fn discover_all(&mut self) -> Result<usize, PhysicalError> {
        let mut discovered = Vec::new();
        for adapter in &self.adapters {
            let links = adapter.discover()?;
            discovered.extend(links);
        }

        if discovered.len() > self.max_links {
            return Err(PhysicalError::DiscoveryFailed);
        }

        self.links.clear();
        for link in discovered {
            if link.packet_loss_ppm > 1_000_000 || link.signal_quality > 100 {
                return Err(PhysicalError::DiscoveryFailed);
            }
            if !self.links.iter().any(|existing| existing.id == link.id) {
                self.links.push(link);
            }
        }
        Ok(self.links.len())
    }

    pub fn refresh_adapter(&mut self, transport: PhysicalTransport) -> Result<usize, PhysicalError> {
        let adapter = self
            .adapters
            .iter()
            .find(|adapter| adapter.transport() == transport)
            .ok_or(PhysicalError::Unsupported)?;
        let links = adapter.discover()?;
        if links.len() > self.max_links {
            return Err(PhysicalError::DiscoveryFailed);
        }

        let ids: Vec<PhysicalLinkId> = links.iter().map(|link| link.id).collect();
        self.links.retain(|link| !(link.transport == transport && !ids.contains(&link.id)));
        for link in links {
            if link.packet_loss_ppm > 1_000_000 || link.signal_quality > 100 {
                return Err(PhysicalError::DiscoveryFailed);
            }
            if let Some(existing) = self.links.iter_mut().find(|existing| existing.id == link.id) {
                *existing = link;
            } else if self.links.len() < self.max_links {
                self.links.push(link);
            } else {
                return Err(PhysicalError::DiscoveryFailed);
            }
        }
        Ok(self.links.iter().filter(|link| link.transport == transport).count())
    }

    pub fn best_link(&self, transport: PhysicalTransport) -> Option<PhysicalLink> {
        self.links
            .iter()
            .filter(|link| link.active && link.transport == transport)
            .min_by_key(|link| (link.packet_loss_ppm, link.estimated_latency_us, u64::MAX - link.estimated_bandwidth_bps))
            .copied()
    }

    pub fn can_transmit(&self, link_id: PhysicalLinkId, payload_len: usize) -> bool {
        self.links.iter().any(|link| {
            link.id == link_id && link.active && payload_len <= self.mtu_for(link.transport)
        })
    }

    pub fn frame(&self, link_id: PhysicalLinkId, payload_len: usize) -> Result<UnderlayFrame, PhysicalError> {
        if !self.can_transmit(link_id, payload_len) {
            return Err(PhysicalError::ConnectionFailed);
        }
        Ok(UnderlayFrame { link_id, payload_len })
    }

    pub fn mtu_for(&self, transport: PhysicalTransport) -> usize {
        self.adapters
            .iter()
            .filter(|adapter| adapter.transport() == transport)
            .map(|adapter| adapter.properties().mtu)
            .min()
            .unwrap_or(0)
    }

    pub fn capabilities(&self) -> PhysicalCapabilities {
        let mut result = PhysicalCapabilities::empty();
        for adapter in &self.adapters {
            let caps = adapter.capabilities();
            result.ethernet |= caps.ethernet;
            result.wifi |= caps.wifi;
            result.wifi_direct |= caps.wifi_direct;
            result.wifi_aware |= caps.wifi_aware;
            result.bluetooth_le |= caps.bluetooth_le;
            result.uwb |= caps.uwb;
            result.lora |= caps.lora;
            result.cellular |= caps.cellular;
            result.satellite |= caps.satellite;
        }
        result
    }
}

impl Default for PhysicalEngine {
    fn default() -> Self {
        Self::new()
    }
}
