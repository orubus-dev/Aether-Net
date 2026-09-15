use super::{
    PhysicalAdapter, PhysicalCapabilities, PhysicalError, PhysicalLink, PhysicalLinkId,
    PhysicalTransport,
};

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
            return Err(PhysicalError::CapacityExceeded);
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
    pub fn replace_links(&mut self, discovered: Vec<PhysicalLink>) -> Result<usize, PhysicalError> {
        if discovered.len() > self.max_links {
            return Err(PhysicalError::CapacityExceeded);
        }
        let mut unique = Vec::with_capacity(discovered.len());
        for link in discovered {
            validate_link(link)?;
            if let Some(index) = unique.iter().position(|existing| existing.id == link.id) {
                unique[index] = link;
            } else {
                unique.push(link);
            }
        }
        self.links = unique;
        Ok(self.links.len())
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
            if discovered.len().saturating_add(links.len()) > self.max_links {
                return Err(PhysicalError::CapacityExceeded);
            }
            discovered.extend(links);
        }
        self.replace_links(discovered)
    }
    pub fn refresh_adapter(
        &mut self,
        transport: PhysicalTransport,
    ) -> Result<usize, PhysicalError> {
        let links = {
            let adapter = self
                .adapters
                .iter()
                .find(|adapter| adapter.transport() == transport)
                .ok_or(PhysicalError::Unsupported)?;
            adapter.discover()?
        };
        if links.len() > self.max_links {
            return Err(PhysicalError::CapacityExceeded);
        }
        for link in &links {
            validate_link(*link)?;
        }
        self.links.retain(|link| {
            !(link.transport == transport && !links.iter().any(|new| new.id == link.id))
        });
        for link in links {
            if let Some(existing) = self
                .links
                .iter_mut()
                .find(|existing| existing.id == link.id)
            {
                *existing = link;
            } else if self.links.len() < self.max_links {
                self.links.push(link);
            } else {
                return Err(PhysicalError::CapacityExceeded);
            }
        }
        Ok(self
            .links
            .iter()
            .filter(|link| link.transport == transport)
            .count())
    }
    pub fn best_link(&self, transport: PhysicalTransport) -> Option<PhysicalLink> {
        self.links
            .iter()
            .filter(|link| link.active && link.transport == transport)
            .min_by(|a, b| {
                a.packet_loss_ppm
                    .cmp(&b.packet_loss_ppm)
                    .then(a.estimated_latency_us.cmp(&b.estimated_latency_us))
                    .then(b.estimated_bandwidth_bps.cmp(&a.estimated_bandwidth_bps))
                    .then(a.id.0.cmp(&b.id.0))
            })
            .copied()
    }
    pub fn can_transmit(&self, link_id: PhysicalLinkId, payload_len: usize) -> bool {
        self.links.iter().any(|link| {
            link.id == link_id && link.active && payload_len <= self.mtu_for(link.transport)
        })
    }
    pub fn frame(
        &self,
        link_id: PhysicalLinkId,
        payload_len: usize,
    ) -> Result<UnderlayFrame, PhysicalError> {
        if !self.can_transmit(link_id, payload_len) {
            return Err(PhysicalError::ConnectionFailed);
        }
        Ok(UnderlayFrame {
            link_id,
            payload_len,
        })
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
fn validate_link(link: PhysicalLink) -> Result<(), PhysicalError> {
    if link.packet_loss_ppm > 1_000_000
        || link.signal_quality > 100
        || link.estimated_bandwidth_bps == 0
    {
        return Err(PhysicalError::DiscoveryFailed);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::physical::adapters::{ethernet_adapter, link_id, wifi_adapter};
    fn ethernet_link(active: bool, latency: u64, bandwidth: u64) -> PhysicalLink {
        PhysicalLink {
            id: link_id(latency as u8),
            transport: PhysicalTransport::Ethernet,
            estimated_latency_us: latency,
            estimated_bandwidth_bps: bandwidth,
            packet_loss_ppm: 100,
            signal_quality: 100,
            active,
        }
    }
    fn properties(transport: PhysicalTransport, mtu: usize) -> UnderlayProperties {
        UnderlayProperties {
            transport,
            mtu,
            bandwidth_bps: 1_000_000_000,
            latency_us: 10,
            energy_cost: 1,
            supports_broadcast: false,
            supports_multicast: false,
        }
    }
    #[test]
    fn engine_discovers_and_selects_best_active_link() {
        let mut engine = PhysicalEngine::with_limits(4, 8);
        let adapter = ethernet_adapter(properties(PhysicalTransport::Ethernet, 1500))
            .unwrap()
            .with_link(ethernet_link(true, 50, 1_000_000))
            .unwrap()
            .with_link(PhysicalLink {
                estimated_latency_us: 10,
                estimated_bandwidth_bps: 10_000_000,
                ..ethernet_link(true, 10, 10_000_000)
            })
            .unwrap();
        engine.register_adapter(Box::new(adapter)).unwrap();
        assert_eq!(engine.discover_all().unwrap(), 2);
        assert_eq!(
            engine
                .best_link(PhysicalTransport::Ethernet)
                .unwrap()
                .estimated_latency_us,
            10
        );
        assert_eq!(engine.mtu_for(PhysicalTransport::Ethernet), 1500);
    }
    #[test]
    fn inactive_links_cannot_be_framed() {
        let mut engine = PhysicalEngine::with_limits(1, 4);
        let adapter = ethernet_adapter(properties(PhysicalTransport::Ethernet, 1500))
            .unwrap()
            .with_link(ethernet_link(false, 10, 1_000_000))
            .unwrap();
        let id = link_id(10);
        engine.register_adapter(Box::new(adapter)).unwrap();
        engine.discover_all().unwrap();
        assert!(!engine.can_transmit(id, 100));
        assert_eq!(
            engine.frame(id, 100).unwrap_err(),
            PhysicalError::ConnectionFailed
        );
    }
    #[test]
    fn adapter_capacity_is_bounded() {
        let mut engine = PhysicalEngine::with_limits(1, 4);
        engine
            .register_adapter(Box::new(
                ethernet_adapter(properties(PhysicalTransport::Ethernet, 1500)).unwrap(),
            ))
            .unwrap();
        assert_eq!(
            engine
                .register_adapter(Box::new(
                    wifi_adapter(properties(PhysicalTransport::Wifi, 1500)).unwrap()
                ))
                .unwrap_err(),
            PhysicalError::CapacityExceeded
        );
    }
}
