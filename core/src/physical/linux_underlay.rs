#![cfg(target_os = "linux")]

use super::capabilities::PhysicalTransport;
use super::interface::PhysicalError;
use super::link::{PhysicalLink, PhysicalLinkId};
use std::fs;
use std::path::{Path, PathBuf};

const SYS_NET: &str = "/sys/class/net";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxInterfaceAnalytics {
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_dropped: u64,
}

impl LinuxInterfaceAnalytics {
    pub fn total_bytes(self) -> u64 {
        self.rx_bytes.saturating_add(self.tx_bytes)
    }

    pub fn total_packets(self) -> u64 {
        self.rx_packets.saturating_add(self.tx_packets)
    }

    pub fn total_errors(self) -> u64 {
        self.rx_errors.saturating_add(self.tx_errors)
    }

    pub fn total_drops(self) -> u64 {
        self.rx_dropped.saturating_add(self.tx_dropped)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxInterface {
    pub name: String,
    pub index: u32,
    pub transport: PhysicalTransport,
    pub mtu: usize,
    pub up: bool,
    pub speed_bps: Option<u64>,
    pub link: PhysicalLink,
    pub analytics: LinuxInterfaceAnalytics,
}

pub struct LinuxUnderlay {
    root: PathBuf,
}

impl Default for LinuxUnderlay {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxUnderlay {
    pub fn new() -> Self {
        Self {
            root: PathBuf::from(SYS_NET),
        }
    }

    pub fn discover(&self) -> Result<Vec<LinuxInterface>, PhysicalError> {
        let entries = fs::read_dir(&self.root).map_err(map_io_error)?;
        let mut interfaces = Vec::new();
        for entry in entries {
            let entry = entry.map_err(map_io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(interface) = self.read_interface(&name)? {
                interfaces.push(interface);
            }
        }
        interfaces.sort_by(|a, b| a.index.cmp(&b.index).then_with(|| a.name.cmp(&b.name)));
        Ok(interfaces)
    }

    pub fn interface(&self, name: &str) -> Result<LinuxInterface, PhysicalError> {
        self.read_interface(name)?.ok_or(PhysicalError::Unavailable)
    }

    pub fn analytics(&self, name: &str) -> Result<LinuxInterfaceAnalytics, PhysicalError> {
        let root = self.interface_root(name);
        read_analytics(&root)
    }

    fn read_interface(&self, name: &str) -> Result<Option<LinuxInterface>, PhysicalError> {
        let root = self.interface_root(name);
        if !root.exists() {
            return Ok(None);
        }
        let index = read_u32(root.join("ifindex"))?;
        let mtu = read_u64(root.join("mtu"))? as usize;
        if mtu == 0 {
            return Err(PhysicalError::InvalidMtu);
        }
        let up = read_string(root.join("operstate"))?.trim() == "up";
        if name == "lo" {
            return Ok(None);
        }
        let transport = classify_transport(&root);
        let speed_bps = read_optional_speed(root.join("speed"));
        let analytics = read_analytics(&root)?;
        let link = PhysicalLink {
            id: interface_link_id(name),
            transport,
            estimated_latency_us: 100,
            estimated_bandwidth_bps: speed_bps.unwrap_or(0).max(1),
            packet_loss_ppm: 0,
            signal_quality: if up { 100 } else { 0 },
            active: up,
        };
        Ok(Some(LinuxInterface {
            name: name.to_owned(),
            index,
            transport,
            mtu,
            up,
            speed_bps,
            link,
            analytics,
        }))
    }

    fn interface_root(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

fn classify_transport(root: &Path) -> PhysicalTransport {
    if root.join("wireless").exists() {
        PhysicalTransport::Wifi
    } else {
        PhysicalTransport::Ethernet
    }
}

fn interface_link_id(name: &str) -> PhysicalLinkId {
    let mut hash = [0u8; 32];
    let digest = sha2_digest(name.as_bytes());
    hash.copy_from_slice(&digest);
    let mut id = [0u8; 16];
    id.copy_from_slice(&hash[..16]);
    PhysicalLinkId(id)
}

fn sha2_digest(input: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"aether:physical:linux-interface:v1:");
    hasher.update(input);
    hasher.finalize().into()
}

fn read_analytics(root: &Path) -> Result<LinuxInterfaceAnalytics, PhysicalError> {
    let stats = root.join("statistics");
    Ok(LinuxInterfaceAnalytics {
        rx_bytes: read_u64(stats.join("rx_bytes"))?,
        rx_packets: read_u64(stats.join("rx_packets"))?,
        rx_errors: read_u64(stats.join("rx_errors"))?,
        rx_dropped: read_u64(stats.join("rx_dropped"))?,
        tx_bytes: read_u64(stats.join("tx_bytes"))?,
        tx_packets: read_u64(stats.join("tx_packets"))?,
        tx_errors: read_u64(stats.join("tx_errors"))?,
        tx_dropped: read_u64(stats.join("tx_dropped"))?,
    })
}

fn read_u64(path: impl AsRef<Path>) -> Result<u64, PhysicalError> {
    read_string(path)?
        .trim()
        .parse()
        .map_err(|_| PhysicalError::DiscoveryFailed)
}

fn read_u32(path: impl AsRef<Path>) -> Result<u32, PhysicalError> {
    read_u64(path)
        .and_then(|value| u32::try_from(value).map_err(|_| PhysicalError::DiscoveryFailed))
}

fn read_string(path: impl AsRef<Path>) -> Result<String, PhysicalError> {
    fs::read_to_string(path).map_err(map_io_error)
}

fn read_optional_speed(path: impl AsRef<Path>) -> Option<u64> {
    read_string(path)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .and_then(|mbps| u64::try_from(mbps).ok())
        .map(|mbps| mbps.saturating_mul(1_000_000))
}

fn map_io_error(error: std::io::Error) -> PhysicalError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => PhysicalError::PermissionDenied,
        std::io::ErrorKind::NotFound => PhysicalError::Unavailable,
        _ => PhysicalError::DiscoveryFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytics_totals_saturate() {
        let value = LinuxInterfaceAnalytics {
            rx_bytes: u64::MAX,
            tx_bytes: 1,
            rx_packets: 2,
            tx_packets: 3,
            rx_errors: 4,
            tx_errors: 5,
            rx_dropped: 6,
            tx_dropped: 7,
        };
        assert_eq!(value.total_bytes(), u64::MAX);
        assert_eq!(value.total_packets(), 5);
        assert_eq!(value.total_errors(), 9);
        assert_eq!(value.total_drops(), 13);
    }

    #[test]
    fn interface_ids_are_domain_separated_and_stable() {
        assert_eq!(interface_link_id("eth0"), interface_link_id("eth0"));
        assert_ne!(interface_link_id("eth0"), interface_link_id("wlan0"));
    }
}
