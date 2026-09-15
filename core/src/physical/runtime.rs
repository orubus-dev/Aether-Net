use super::PhysicalTransport;
use super::engine::PhysicalEngine;
use super::interface::{PhysicalAnalytics, PhysicalDriver, PhysicalError};
use super::link::{PhysicalLink, PhysicalLinkId};

pub const DEFAULT_MAX_DRIVERS: usize = 64;
pub const DEFAULT_POLL_BUDGET: usize = 64;

struct DriverBinding {
    transport: PhysicalTransport,
    driver: Box<dyn PhysicalDriver>,
}

pub struct PhysicalRuntime {
    engine: PhysicalEngine,
    drivers: Vec<DriverBinding>,
    max_drivers: usize,
}

impl PhysicalRuntime {
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_DRIVERS, super::DEFAULT_MAX_ADAPTERS, super::DEFAULT_MAX_LINKS)
    }
    pub fn with_limits(max_drivers: usize, max_adapters: usize, max_links: usize) -> Self {
        Self { engine: PhysicalEngine::with_limits(max_adapters, max_links), drivers: Vec::new(), max_drivers: max_drivers.max(1) }
    }
    pub fn engine(&self) -> &PhysicalEngine { &self.engine }
    pub fn engine_mut(&mut self) -> &mut PhysicalEngine { &mut self.engine }
    pub fn driver_count(&self) -> usize { self.drivers.len() }
    pub fn register_driver(&mut self, driver: Box<dyn PhysicalDriver>) -> Result<(), PhysicalError> {
        if self.drivers.len() >= self.max_drivers { return Err(PhysicalError::CapacityExceeded); }
        self.drivers.push(DriverBinding { transport: driver.transport(), driver });
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn discover_linux(&mut self, mtu_limit: usize) -> Result<usize, PhysicalError> {
        let interfaces = super::linux_underlay::LinuxUnderlay::new().discover()?;
        for interface in interfaces {
            if !interface.up { continue; }
            let mtu = mtu_limit.min(interface.mtu);
            if mtu < 576 { continue; }
            let driver = super::linux_ethernet::LinuxEthernetDriver::open_with_transport(&interface.name, mtu, interface.transport)?;
            let link_known = self.drivers.iter_mut().any(|binding| {
                binding.driver.discover().map(|links| links.iter().any(|link| link.id == interface.link.id)).unwrap_or(false)
            });
            if !link_known { self.register_driver(Box::new(driver))?; }
        }
        self.discover()
    }

    pub fn discover(&mut self) -> Result<usize, PhysicalError> {
        let mut discovered = Vec::new();
        for binding in &self.drivers {
            let links = binding.driver.discover()?;
            if discovered.len().saturating_add(links.len()) > super::DEFAULT_MAX_LINKS { return Err(PhysicalError::CapacityExceeded); }
            discovered.extend(links);
        }
        self.engine.replace_links(discovered)
    }
    pub fn refresh_links(&mut self) -> Result<usize, PhysicalError> { self.discover() }
    pub fn links(&self) -> &[PhysicalLink] { self.engine.links() }

    pub fn analytics(&self, link_id: PhysicalLinkId) -> Result<Option<PhysicalAnalytics>, PhysicalError> {
        if !self.links().iter().any(|link| link.id == link_id) { return Err(PhysicalError::ConnectionFailed); }
        for binding in &self.drivers {
            if binding.driver.discover()?.iter().any(|link| link.id == link_id) {
                return binding.driver.analytics(link_id);
            }
        }
        Ok(None)
    }

    pub fn transmit(&mut self, link_id: PhysicalLinkId, frame: &[u8]) -> Result<(), PhysicalError> {
        let link = self.engine.links().iter().find(|link| link.id == link_id && link.active).copied().ok_or(PhysicalError::ConnectionFailed)?;
        let driver = self.drivers.iter_mut().find(|binding| {
            binding.transport == link.transport && binding.driver.discover().map(|links| links.iter().any(|item| item.id == link_id)).unwrap_or(false)
        }).ok_or(PhysicalError::Unavailable)?;
        if frame.len() > driver.driver.mtu() { return Err(PhysicalError::FrameTooLarge); }
        driver.driver.transmit(link_id, frame)
    }

    pub fn poll_receive(&mut self) -> Result<Option<(PhysicalLinkId, Vec<u8>)>, PhysicalError> { self.poll_receive_budgeted(DEFAULT_POLL_BUDGET) }
    pub fn poll_receive_budgeted(&mut self, budget: usize) -> Result<Option<(PhysicalLinkId, Vec<u8>)>, PhysicalError> {
        let passes = budget.max(1);
        for pass in 0..passes {
            if self.drivers.is_empty() { return Ok(None); }
            let index = pass % self.drivers.len();
            if let Some(frame) = self.drivers[index].driver.poll_receive()? { return Ok(Some(frame)); }
        }
        Ok(None)
    }
}

impl Default for PhysicalRuntime { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physical::{LoopbackDriver, PhysicalCapabilities, UnderlayProperties};
    fn link(transport: PhysicalTransport, id: u8, mtu: usize) -> (PhysicalLink, UnderlayProperties, PhysicalCapabilities) {
        let capabilities = match transport { PhysicalTransport::Ethernet => PhysicalCapabilities { ethernet: true, ..PhysicalCapabilities::empty() }, PhysicalTransport::Wifi => PhysicalCapabilities { wifi: true, ..PhysicalCapabilities::empty() }, _ => PhysicalCapabilities::empty() };
        (PhysicalLink { id: PhysicalLinkId([id;16]), transport, estimated_latency_us: 20, estimated_bandwidth_bps: 10_000_000, packet_loss_ppm: 0, signal_quality: 100, active: true }, UnderlayProperties { transport, mtu, bandwidth_bps: 10_000_000, latency_us: 20, energy_cost: 1, supports_broadcast: true, supports_multicast: true }, capabilities)
    }
    #[test]
    fn runtime_routes_to_exact_driver_link() {
        let (a, props, caps) = link(PhysicalTransport::Ethernet, 9, 256);
        let (b, props2, caps2) = link(PhysicalTransport::Ethernet, 10, 256);
        let d1 = LoopbackDriver::new(PhysicalTransport::Ethernet, caps, props, a, 4).unwrap();
        let d2 = LoopbackDriver::new(PhysicalTransport::Ethernet, caps2, props2, b, 4).unwrap();
        let mut runtime = PhysicalRuntime::with_limits(4, 4, 8);
        runtime.register_driver(Box::new(d1)).unwrap(); runtime.register_driver(Box::new(d2)).unwrap();
        assert_eq!(runtime.refresh_links().unwrap(), 2);
        runtime.transmit(b.id, b"second").unwrap();
        assert_eq!(runtime.poll_receive().unwrap().unwrap().1, b"second");
    }
    #[test]
    fn runtime_rejects_oversized_frame() {
        let (link, properties, caps) = link(PhysicalTransport::BluetoothLe, 3, 8);
        let driver = LoopbackDriver::new(PhysicalTransport::BluetoothLe, caps, properties, link, 4).unwrap();
        let mut runtime = PhysicalRuntime::new(); runtime.register_driver(Box::new(driver)).unwrap(); runtime.refresh_links().unwrap();
        assert_eq!(runtime.transmit(link.id, &[0;9]).unwrap_err(), PhysicalError::FrameTooLarge);
    }
}
