//! Linux kernel boundary for the Aether routing datapath.
//!
//! The kernel owns the network interface and low-level packet accounting. Aether keeps
//! the control surface deliberately small: create/configure a TAP interface and read
//! kernel-maintained interface counters for analytics.

use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelInterfaceConfig {
    pub mtu: u32,
}
impl Default for KernelInterfaceConfig {
    fn default() -> Self {
        Self { mtu: 1500 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KernelInterfaceAnalytics {
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_dropped: u64,
}
impl KernelInterfaceAnalytics {
    pub fn total_packets(self) -> u64 {
        self.rx_packets.saturating_add(self.tx_packets)
    }
    pub fn total_bytes(self) -> u64 {
        self.rx_bytes.saturating_add(self.tx_bytes)
    }
    pub fn total_drops(self) -> u64 {
        self.rx_dropped.saturating_add(self.tx_dropped)
    }
    pub fn total_errors(self) -> u64 {
        self.rx_errors.saturating_add(self.tx_errors)
    }
}

#[derive(Debug)]
pub enum LinuxKernelError {
    Unsupported,
    InvalidName,
    InvalidMtu,
    Io(io::Error),
}
impl From<io::Error> for LinuxKernelError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Minimal Linux TAP boundary. Linux owns the interface lifecycle and standard counters;
/// the Aether runtime supplies/consumes Ethernet frames through this single descriptor.
pub struct LinuxKernelInterface {
    name: String,
    #[cfg(target_os = "linux")]
    device: std::fs::File,
}

impl LinuxKernelInterface {
    /// Create and bring up a TAP interface. Linux normally requires root or CAP_NET_ADMIN.
    pub fn create(name: &str, config: KernelInterfaceConfig) -> Result<Self, LinuxKernelError> {
        validate_name(name)?;
        validate_mtu(config.mtu)?;
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            let device = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/net/tun")?;
            let mut req = IfReq::new(name)?;
            req.flags = (libc::IFF_TAP | libc::IFF_NO_PI) as libc::c_short;
            ioctl_checked(
                device.as_raw_fd(),
                libc::TUNSETIFF as libc::c_ulong,
                &mut req,
            )?;
            let mut interface = Self {
                name: name.to_owned(),
                device,
            };
            interface.set_mtu(config.mtu)?;
            interface.set_up(true)?;
            Ok(interface)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (name, config);
            Err(LinuxKernelError::Unsupported)
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    #[cfg(target_os = "linux")]
    pub fn read_frame(&mut self, buffer: &mut [u8]) -> Result<usize, LinuxKernelError> {
        use std::io::Read;
        Ok(self.device.read(buffer)?)
    }
    #[cfg(not(target_os = "linux"))]
    pub fn read_frame(&mut self, _buffer: &mut [u8]) -> Result<usize, LinuxKernelError> {
        Err(LinuxKernelError::Unsupported)
    }

    #[cfg(target_os = "linux")]
    pub fn write_frame(&mut self, frame: &[u8]) -> Result<usize, LinuxKernelError> {
        use std::io::Write;
        Ok(self.device.write(frame)?)
    }
    #[cfg(not(target_os = "linux"))]
    pub fn write_frame(&mut self, _frame: &[u8]) -> Result<usize, LinuxKernelError> {
        Err(LinuxKernelError::Unsupported)
    }

    /// Read counters maintained directly by the Linux networking stack.
    pub fn analytics(&self) -> Result<KernelInterfaceAnalytics, LinuxKernelError> {
        #[cfg(target_os = "linux")]
        {
            let root = Path::new("/sys/class/net")
                .join(&self.name)
                .join("statistics");
            Ok(KernelInterfaceAnalytics {
                rx_bytes: read_counter(root.join("rx_bytes"))?,
                rx_packets: read_counter(root.join("rx_packets"))?,
                rx_errors: read_counter(root.join("rx_errors"))?,
                rx_dropped: read_counter(root.join("rx_dropped"))?,
                tx_bytes: read_counter(root.join("tx_bytes"))?,
                tx_packets: read_counter(root.join("tx_packets"))?,
                tx_errors: read_counter(root.join("tx_errors"))?,
                tx_dropped: read_counter(root.join("tx_dropped"))?,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(LinuxKernelError::Unsupported)
        }
    }

    #[cfg(target_os = "linux")]
    fn set_mtu(&mut self, mtu: u32) -> Result<(), LinuxKernelError> {
        let fd = open_control_socket()?;
        let mut req = IfReq::new(&self.name)?;
        req.mtu = mtu as libc::c_int;
        let result = ioctl_checked(fd, libc::SIOCSIFMTU as libc::c_ulong, &mut req);
        close_fd(fd);
        result
    }

    #[cfg(target_os = "linux")]
    fn set_up(&mut self, up: bool) -> Result<(), LinuxKernelError> {
        let fd = open_control_socket()?;
        let mut req = IfReq::new(&self.name)?;
        let read_flags = ioctl_checked(fd, libc::SIOCGIFFLAGS as libc::c_ulong, &mut req);
        if let Err(error) = read_flags {
            close_fd(fd);
            return Err(error);
        }
        if up {
            req.flags |= libc::IFF_UP as libc::c_short;
        } else {
            req.flags &= !(libc::IFF_UP as libc::c_short);
        }
        let result = ioctl_checked(fd, libc::SIOCSIFFLAGS as libc::c_ulong, &mut req);
        close_fd(fd);
        result
    }
}

fn validate_name(name: &str) -> Result<(), LinuxKernelError> {
    if name.is_empty() || name.len() >= libc::IFNAMSIZ {
        return Err(LinuxKernelError::InvalidName);
    }
    if name
        .as_bytes()
        .iter()
        .any(|byte| *byte == b'/' || *byte == 0)
    {
        return Err(LinuxKernelError::InvalidName);
    }
    Ok(())
}
fn validate_mtu(mtu: u32) -> Result<(), LinuxKernelError> {
    if !(576..=65_535).contains(&mtu) {
        return Err(LinuxKernelError::InvalidMtu);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct IfReq {
    name: [libc::c_char; libc::IFNAMSIZ],
    flags: libc::c_short,
    mtu: libc::c_int,
}
#[cfg(target_os = "linux")]
impl IfReq {
    fn new(name: &str) -> Result<Self, LinuxKernelError> {
        validate_name(name)?;
        let mut value = Self {
            name: [0; libc::IFNAMSIZ],
            flags: 0,
            mtu: 0,
        };
        for (index, byte) in name.as_bytes().iter().enumerate() {
            value.name[index] = *byte as libc::c_char;
        }
        Ok(value)
    }
}

#[cfg(target_os = "linux")]
fn open_control_socket() -> Result<std::os::fd::RawFd, LinuxKernelError> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(fd)
    }
}
#[cfg(target_os = "linux")]
fn close_fd(fd: std::os::fd::RawFd) {
    unsafe {
        libc::close(fd);
    }
}
#[cfg(target_os = "linux")]
fn ioctl_checked(
    fd: std::os::fd::RawFd,
    request: libc::c_ulong,
    req: &mut IfReq,
) -> Result<(), LinuxKernelError> {
    let result = unsafe { libc::ioctl(fd, request, req as *mut IfReq) };
    if result < 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}
#[cfg(target_os = "linux")]
fn read_counter(path: impl AsRef<Path>) -> Result<u64, LinuxKernelError> {
    let text = std::fs::read_to_string(path)?;
    text.trim().parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Linux interface counter",
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_config_is_conventional() {
        assert_eq!(KernelInterfaceConfig::default().mtu, 1500);
    }
    #[test]
    fn analytics_aggregate_without_overflow() {
        let analytics = KernelInterfaceAnalytics {
            rx_bytes: u64::MAX,
            tx_bytes: 1,
            rx_packets: 4,
            tx_packets: 5,
            rx_dropped: 2,
            tx_dropped: 3,
            rx_errors: 1,
            tx_errors: 2,
        };
        assert_eq!(analytics.total_bytes(), u64::MAX);
        assert_eq!(analytics.total_packets(), 9);
        assert_eq!(analytics.total_drops(), 5);
        assert_eq!(analytics.total_errors(), 3);
    }
    #[test]
    fn validates_interface_names_and_mtu() {
        assert!(validate_name("aether0").is_ok());
        assert!(matches!(
            validate_name(""),
            Err(LinuxKernelError::InvalidName)
        ));
        assert!(matches!(
            validate_mtu(100),
            Err(LinuxKernelError::InvalidMtu)
        ));
        assert!(validate_mtu(1500).is_ok());
    }
}
