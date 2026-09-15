use super::capabilities::PhysicalTransport;
use super::interface::PhysicalError;
use super::link::{PhysicalLink, PhysicalLinkId};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxInterfaceAnalytics {
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_drops: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_drops: u64,
}

impl LinuxInterfaceAnalytics {
    pub const fn empty() -> Self {
        Self {
            rx_bytes: 0,
            rx_packets: 0,
            rx_errors: 0,
            rx_drops: 0,
            tx_bytes: 0,
            tx_packets: 0,
            tx_errors: 0,
            tx_drops: 0,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.rx_bytes.saturating_add(self.tx_bytes)
    }

    pub fn total_packets(&self) -> u64 {
        self.rx_packets.saturating_add(self.tx_packets)
    }

    pub fn total_errors(&self) -> u64 {
        self.rx_errors.saturating_add(self.tx_errors)
    }

    pub fn total_drops(&self) -> u64 {
        self.rx_drops.saturating_add(self.tx_drops)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxInterface {
    pub name: String,
    pub ifindex: u32,
    pub transport: PhysicalTransport,
    pub mtu: usize,
    pub up: bool,
    pub speed_bps: Option<u64>,
    pub link: PhysicalLink,
    pub analytics: LinuxInterfaceAnalytics,
}

#[derive(Debug, Clone)]
pub struct LinuxUnderlay {
    sysfs_root: PathBuf,
}

impl Default for LinuxUnderlay {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxUnderlay {
    pub fn new() -> Self {
        Self {
            sysfs_root: PathBuf::from("/sys/class/net"),
        }
    }

    pub fn discover(&self) -> Result<Vec<LinuxInterface>, PhysicalError> {
        let entries = fs::read_dir(&self.sysfs_root).map_err(|_| PhysicalError::DiscoveryFailed)?;
        let mut interfaces = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| PhysicalError::DiscoveryFailed)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(interface) = self.read_interface(&name)? {
                interfaces.push(interface);
            }
        }
        interfaces.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(interfaces)
    }

    pub fn interface(&self, name: &str) -> Result<Option<LinuxInterface>, PhysicalError> {
        self.read_interface(name)
    }

    pub fn analytics(&self, name: &str) -> Result<LinuxInterfaceAnalytics, PhysicalError> {
        let root = self.interface_root(name);
        if !root.exists() {
            return Err(PhysicalError::Unavailable);
        }
        read_analytics(&root)
    }

    fn interface_root(&self, name: &str) -> PathBuf {
        self.sysfs_root.join(name)
    }

    fn read_interface(&self, name: &str) -> Result<Option<LinuxInterface>, PhysicalError> {
        let root = self.interface_root(name);
        if !root.exists() || name == "lo" {
            return Ok(None);
        }
        let index = read_u32(root.join("ifindex"))?;
        let mtu = read_u64(root.join("mtu"))? as usize;
        if mtu == 0 {
            return Err(PhysicalError::DiscoveryFailed);
        }
        let up = read_string(root.join("operstate"))?.trim() == "up";
        let transport = classify_transport(&root);
        let speed_bps = read_optional_speed(root.join("speed"));
        let analytics = read_analytics(&root)?;
        let link = PhysicalLink {
            id: stable_link_id(name, index),
            transport,
            estimated_latency_us: 0,
            estimated_bandwidth_bps: speed_bps.unwrap_or(1),
            packet_loss_ppm: 0,
            signal_quality: if up { 255 } else { 0 },
            active: up,
        };
        Ok(Some(LinuxInterface {
            name: name.to_owned(),
            ifindex: index,
            transport,
            mtu,
            up,
            speed_bps,
            link,
            analytics,
        }))
    }
}

fn classify_transport(root: &Path) -> PhysicalTransport {
    if root.join("wireless").exists() {
        PhysicalTransport::Wifi
    } else {
        PhysicalTransport::Ethernet
    }
}

fn stable_link_id(name: &str, ifindex: u32) -> PhysicalLinkId {
    let mut hasher = Sha256::new();
    hasher.update(b"aether:physical:linux-interface:v1:");
    hasher.update(name.as_bytes());
    hasher.update(ifindex.to_be_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    PhysicalLinkId(bytes)
}

fn read_analytics(root: &Path) -> Result<LinuxInterfaceAnalytics, PhysicalError> {
    let stats = root.join("statistics");
    Ok(LinuxInterfaceAnalytics {
        rx_bytes: read_u64(stats.join("rx_bytes"))?,
        rx_packets: read_u64(stats.join("rx_packets"))?,
        rx_errors: read_u64(stats.join("rx_errors"))?,
        rx_drops: read_u64(stats.join("rx_dropped"))?,
        tx_bytes: read_u64(stats.join("tx_bytes"))?,
        tx_packets: read_u64(stats.join("tx_packets"))?,
        tx_errors: read_u64(stats.join("tx_errors"))?,
        tx_drops: read_u64(stats.join("tx_dropped"))?,
    })
}

fn read_string(path: impl AsRef<Path>) -> Result<String, PhysicalError> {
    fs::read_to_string(path).map_err(|_| PhysicalError::Unavailable)
}

fn read_u64(path: impl AsRef<Path>) -> Result<u64, PhysicalError> {
    read_string(path)?
        .trim()
        .parse()
        .map_err(|_| PhysicalError::DiscoveryFailed)
}

fn read_u32(path: impl AsRef<Path>) -> Result<u32, PhysicalError> {
    read_string(path)?
        .trim()
        .parse()
        .map_err(|_| PhysicalError::DiscoveryFailed)
}

fn read_optional_speed(path: impl AsRef<Path>) -> Option<u64> {
    let value = fs::read_to_string(path).ok()?.trim().parse::<i64>().ok()?;
    (value > 0).then_some(value as u64 * 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytics_aggregate_saturating() {
        let analytics = LinuxInterfaceAnalytics {
            rx_bytes: u64::MAX,
            tx_bytes: 1,
            ..LinuxInterfaceAnalytics::empty()
        };
        assert_eq!(analytics.total_bytes(), u64::MAX);
    }

    #[test]
    fn stable_ids_are_domain_separated() {
        assert_ne!(stable_link_id("eth0", 2), stable_link_id("eth1", 2));
        assert_eq!(stable_link_id("eth0", 2), stable_link_id("eth0", 2));
    }
}
