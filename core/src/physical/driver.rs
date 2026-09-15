use super::adapters::ConfiguredAdapter;
use super::capabilities::{PhysicalCapabilities, PhysicalTransport};
use super::engine::UnderlayProperties;
use super::interface::{PhysicalAdapter, PhysicalDriver, PhysicalError};
use super::link::{PhysicalLink, PhysicalLinkId};
use std::collections::VecDeque;

/// A bounded in-memory carrier used for integration tests, emulation and
/// platforms that bind the Aether driver interface to another event loop.
pub struct LoopbackDriver {
    adapter: ConfiguredAdapter,
    rx: VecDeque<(PhysicalLinkId, Vec<u8>)>,
    max_frame: usize,
    max_queue: usize,
}

impl LoopbackDriver {
    pub fn new(
        transport: PhysicalTransport,
        capabilities: PhysicalCapabilities,
        properties: UnderlayProperties,
        link: PhysicalLink,
        max_queue: usize,
    ) -> Result<Self, PhysicalError> {
        let adapter = ConfiguredAdapter::new(transport, capabilities, properties)?.with_link(link)?;
        Ok(Self {
            adapter,
            rx: VecDeque::new(),
            max_frame: properties.mtu,
            max_queue: max_queue.max(1),
        })
    }

    pub fn inject_receive(&mut self, link_id: PhysicalLinkId, frame: Vec<u8>) -> Result<(), PhysicalError> {
        if frame.len() > self.max_frame { return Err(PhysicalError::FrameTooLarge); }
        if self.rx.len() >= self.max_queue { return Err(PhysicalError::CapacityExceeded); }
        self.rx.push_back((link_id, frame));
        Ok(())
    }
}

impl PhysicalAdapter for LoopbackDriver {
    fn transport(&self) -> PhysicalTransport { self.adapter.transport() }
    fn capabilities(&self) -> PhysicalCapabilities { self.adapter.capabilities() }
    fn discover(&self) -> Result<Vec<PhysicalLink>, PhysicalError> { self.adapter.discover() }
}

impl PhysicalDriver for LoopbackDriver {
    fn transmit(&mut self, link_id: PhysicalLinkId, frame: &[u8]) -> Result<(), PhysicalError> {
        if frame.len() > self.max_frame { return Err(PhysicalError::FrameTooLarge); }
        if !self.adapter.discover()?.iter().any(|link| link.id == link_id && link.active) {
            return Err(PhysicalError::ConnectionFailed);
        }
        self.inject_receive(link_id, frame.to_vec())
    }

    fn poll_receive(&mut self) -> Result<Option<(PhysicalLinkId, Vec<u8>)>, PhysicalError> {
        Ok(self.rx.pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_transmit_and_receive_are_bounded() {
        let link = PhysicalLink {
            id: PhysicalLinkId([7; 16]),
            transport: PhysicalTransport::Ethernet,
            estimated_latency_us: 10,
            estimated_bandwidth_bps: 1_000_000,
            packet_loss_ppm: 0,
            signal_quality: 100,
            active: true,
        };
        let properties = UnderlayProperties {
            transport: PhysicalTransport::Ethernet,
            mtu: 128,
            bandwidth_bps: 1_000_000,
            latency_us: 10,
            energy_cost: 1,
            supports_broadcast: true,
            supports_multicast: true,
        };
        let caps = PhysicalCapabilities { ethernet: true, ..PhysicalCapabilities::empty() };
        let mut driver = LoopbackDriver::new(PhysicalTransport::Ethernet, caps, properties, link, 1).unwrap();
        driver.transmit(link.id, b"hello").unwrap();
        assert_eq!(driver.poll_receive().unwrap().unwrap().1, b"hello");
        assert_eq!(driver.poll_receive().unwrap(), None);
    }
}
