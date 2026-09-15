use super::PhysicalTransport;
use super::engine::PhysicalEngine;
use super::interface::{PhysicalDriver, PhysicalError};
use super::link::{PhysicalLink, PhysicalLinkId};

pub const DEFAULT_MAX_DRIVERS: usize = 64;

struct DriverBinding {
    transport: PhysicalTransport,
    driver: Box<dyn PhysicalDriver>,
}

/// Owns concrete platform carriers and keeps carrier I/O below the native
/// Aether L2/L3 boundary. This is the integration point for OS-specific
/// Ethernet/Wi-Fi/BLE/UWB drivers.
pub struct PhysicalRuntime {
    engine: PhysicalEngine,
    drivers: Vec<DriverBinding>,
    max_drivers: usize,
}

impl PhysicalRuntime {
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_MAX_DRIVERS,
            super::DEFAULT_MAX_ADAPTERS,
            super::DEFAULT_MAX_LINKS,
        )
    }

    pub fn with_limits(max_drivers: usize, max_adapters: usize, max_links: usize) -> Self {
        Self {
            engine: PhysicalEngine::with_limits(max_adapters, max_links),
            drivers: Vec::new(),
            max_drivers: max_drivers.max(1),
        }
    }

    pub fn engine(&self) -> &PhysicalEngine {
        &self.engine
    }
    pub fn engine_mut(&mut self) -> &mut PhysicalEngine {
        &mut self.engine
    }
    pub fn driver_count(&self) -> usize {
        self.drivers.len()
    }

    pub fn register_driver(
        &mut self,
        driver: Box<dyn PhysicalDriver>,
    ) -> Result<(), PhysicalError> {
        if self.drivers.len() >= self.max_drivers {
            return Err(PhysicalError::CapacityExceeded);
        }
        let transport = driver.transport();
        self.drivers.push(DriverBinding { transport, driver });
        Ok(())
    }

    pub fn discover(&mut self) -> Result<usize, PhysicalError> {
        let mut discovered = Vec::new();
        for binding in &self.drivers {
            discovered.extend(binding.driver.discover()?);
            if discovered.len() > super::DEFAULT_MAX_LINKS {
                return Err(PhysicalError::CapacityExceeded);
            }
        }
        self.engine.replace_links(discovered)
    }

    pub fn refresh_links(&mut self) -> Result<usize, PhysicalError> {
        self.discover()
    }
    pub fn links(&self) -> &[PhysicalLink] {
        self.engine.links()
    }

    pub fn transmit(&mut self, link_id: PhysicalLinkId, frame: &[u8]) -> Result<(), PhysicalError> {
        let link = self
            .engine
            .links()
            .iter()
            .find(|link| link.id == link_id && link.active)
            .copied()
            .ok_or(PhysicalError::ConnectionFailed)?;
        let driver = self
            .drivers
            .iter_mut()
            .find(|binding| binding.transport == link.transport)
            .ok_or(PhysicalError::Unavailable)?;
        if frame.len() > driver.driver.mtu() {
            return Err(PhysicalError::FrameTooLarge);
        }
        driver.driver.transmit(link_id, frame)
    }

    pub fn poll_receive(&mut self) -> Result<Option<(PhysicalLinkId, Vec<u8>)>, PhysicalError> {
        for binding in &mut self.drivers {
            if let Some(frame) = binding.driver.poll_receive()? {
                return Ok(Some(frame));
            }
        }
        Ok(None)
    }
}

impl Default for PhysicalRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physical::{LoopbackDriver, PhysicalCapabilities, UnderlayProperties};

    #[test]
    fn runtime_discovers_and_sends_through_selected_carrier() {
        let link = PhysicalLink {
            id: PhysicalLinkId([9; 16]),
            transport: PhysicalTransport::Ethernet,
            estimated_latency_us: 20,
            estimated_bandwidth_bps: 10_000_000,
            packet_loss_ppm: 0,
            signal_quality: 100,
            active: true,
        };
        let properties = UnderlayProperties {
            transport: PhysicalTransport::Ethernet,
            mtu: 256,
            bandwidth_bps: 10_000_000,
            latency_us: 20,
            energy_cost: 1,
            supports_broadcast: true,
            supports_multicast: true,
        };
        let caps = PhysicalCapabilities {
            ethernet: true,
            ..PhysicalCapabilities::empty()
        };
        let driver =
            LoopbackDriver::new(PhysicalTransport::Ethernet, caps, properties, link, 4).unwrap();
        let mut runtime = PhysicalRuntime::with_limits(4, 4, 8);
        runtime.register_driver(Box::new(driver)).unwrap();
        assert_eq!(runtime.refresh_links().unwrap(), 1);
        runtime.transmit(link.id, b"aether").unwrap();
        assert_eq!(runtime.poll_receive().unwrap().unwrap().1, b"aether");
    }

    #[test]
    fn runtime_rejects_oversized_frame() {
        let link = PhysicalLink {
            id: PhysicalLinkId([3; 16]),
            transport: PhysicalTransport::BluetoothLe,
            estimated_latency_us: 20,
            estimated_bandwidth_bps: 1_000_000,
            packet_loss_ppm: 0,
            signal_quality: 100,
            active: true,
        };
        let properties = UnderlayProperties {
            transport: PhysicalTransport::BluetoothLe,
            mtu: 8,
            bandwidth_bps: 1_000_000,
            latency_us: 20,
            energy_cost: 1,
            supports_broadcast: false,
            supports_multicast: false,
        };
        let caps = PhysicalCapabilities {
            bluetooth_le: true,
            ..PhysicalCapabilities::empty()
        };
        let driver =
            LoopbackDriver::new(PhysicalTransport::BluetoothLe, caps, properties, link, 4).unwrap();
        let mut runtime = PhysicalRuntime::new();
        runtime.register_driver(Box::new(driver)).unwrap();
        runtime.refresh_links().unwrap();
        assert_eq!(
            runtime.transmit(link.id, &[0; 9]).unwrap_err(),
            PhysicalError::FrameTooLarge
        );
    }
}
