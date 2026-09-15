#![cfg(target_os = "linux")]

use super::capabilities::{PhysicalCapabilities, PhysicalTransport};
use super::engine::{PhysicalAdapterExt, UnderlayProperties};
use super::interface::{PhysicalAdapter, PhysicalDriver, PhysicalError};
use super::link::{PhysicalLink, PhysicalLinkId};
use libc::{
    AF_PACKET, ETH_P_ALL, ETH_P_IPV6, ETH_P_LOOP, ETH_P_IP, FIONREAD, IFNAMSIZ, SOCK_RAW,
    SOL_SOCKET, SO_RCVBUF, c_char, c_int, if_nametoindex, sockaddr, sockaddr_ll,
};
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::mem::{self, MaybeUninit};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::ptr;

pub const AETHER_ETHERTYPE: u16 = 0xA37E;
const ETHERNET_HEADER_LEN: usize = 14;
const MAX_ETHERNET_FRAME: usize = 65_535;

pub struct LinuxEthernetDriver {
    fd: OwnedFd,
    ifindex: u32,
    interface_name: String,
    link: PhysicalLink,
    properties: UnderlayProperties,
    source_mac: [u8; 6],
    max_frame: usize,
}

impl LinuxEthernetDriver {
    pub fn open(interface_name: &str, mtu: usize) -> Result<Self, PhysicalError> {
        let ifname = CString::new(interface_name).map_err(|_| PhysicalError::Unsupported)?;
        let ifindex = unsafe { if_nametoindex(ifname.as_ptr()) };
        if ifindex == 0 {
            return Err(PhysicalError::Unavailable);
        }

        let fd = unsafe {
            libc::socket(
                AF_PACKET,
                SOCK_RAW | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                (AETHER_ETHERTYPE as u16).to_be() as c_int,
            )
        };
        if fd < 0 {
            return Err(last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };

        let mut addr = sockaddr_ll {
            sll_family: AF_PACKET as u16,
            sll_protocol: (AETHER_ETHERTYPE).to_be(),
            sll_ifindex: ifindex as i32,
            sll_hatype: 0,
            sll_pkttype: 0,
            sll_halen: 0,
            sll_addr: [0; 8],
        };
        let rc = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&mut addr as *mut sockaddr_ll).cast::<sockaddr>(),
                mem::size_of::<sockaddr_ll>() as u32,
            )
        };
        if rc < 0 {
            return Err(last_os_error());
        }

        let source_mac = interface_mac(ifname.as_ptr())?;
        let interface_mtu = interface_mtu(ifname.as_ptr())?;
        let max_frame = mtu.min(interface_mtu.max(1));
        if max_frame == 0 || max_frame + ETHERNET_HEADER_LEN > MAX_ETHERNET_FRAME {
            return Err(PhysicalError::Unsupported);
        }

        let link = PhysicalLink {
            id: link_id(interface_name),
            transport: PhysicalTransport::Ethernet,
            estimated_latency_us: 100,
            estimated_bandwidth_bps: 1_000_000_000,
            packet_loss_ppm: 0,
            signal_quality: 100,
            active: true,
        };
        let properties = UnderlayProperties {
            transport: PhysicalTransport::Ethernet,
            mtu: max_frame,
            bandwidth_bps: link.estimated_bandwidth_bps,
            latency_us: link.estimated_latency_us,
            energy_cost: 1,
            supports_broadcast: true,
            supports_multicast: true,
        };

        Ok(Self {
            fd,
            ifindex,
            interface_name: interface_name.to_owned(),
            link,
            properties,
            source_mac,
            max_frame,
        })
    }

    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    pub fn ifindex(&self) -> u32 {
        self.ifindex
    }
}

impl PhysicalAdapter for LinuxEthernetDriver {
    fn transport(&self) -> PhysicalTransport {
        PhysicalTransport::Ethernet
    }

    fn capabilities(&self) -> PhysicalCapabilities {
        PhysicalCapabilities {
            ethernet: true,
            ..PhysicalCapabilities::empty()
        }
    }

    fn discover(&self) -> Result<Vec<PhysicalLink>, PhysicalError> {
        Ok(vec![self.link])
    }
}

impl PhysicalAdapterExt for LinuxEthernetDriver {
    fn properties(&self) -> UnderlayProperties {
        self.properties
    }
}

impl PhysicalDriver for LinuxEthernetDriver {
    fn mtu(&self) -> usize {
        self.max_frame
    }

    fn transmit(&mut self, link_id: PhysicalLinkId, frame: &[u8]) -> Result<(), PhysicalError> {
        if link_id != self.link.id || !self.link.active {
            return Err(PhysicalError::ConnectionFailed);
        }
        if frame.len() > self.max_frame {
            return Err(PhysicalError::FrameTooLarge);
        }

        let mut ethernet = Vec::with_capacity(ETHERNET_HEADER_LEN + frame.len());
        ethernet.extend_from_slice(&[0xff; 6]);
        ethernet.extend_from_slice(&self.source_mac);
        ethernet.extend_from_slice(&AETHER_ETHERTYPE.to_be_bytes());
        ethernet.extend_from_slice(frame);

        let mut addr = sockaddr_ll {
            sll_family: AF_PACKET as u16,
            sll_protocol: AETHER_ETHERTYPE.to_be(),
            sll_ifindex: self.ifindex as i32,
            sll_hatype: 0,
            sll_pkttype: 0,
            sll_halen: 6,
            sll_addr: [0; 8],
        };
        addr.sll_addr[..6].copy_from_slice(&[0xff; 6]);

        let written = unsafe {
            libc::sendto(
                self.fd.as_raw_fd(),
                ethernet.as_ptr().cast::<libc::c_void>(),
                ethernet.len(),
                0,
                (&addr as *const sockaddr_ll).cast::<sockaddr>(),
                mem::size_of::<sockaddr_ll>() as u32,
            )
        };
        if written < 0 {
            return Err(last_os_error());
        }
        if written as usize != ethernet.len() {
            return Err(PhysicalError::ConnectionFailed);
        }
        Ok(())
    }

    fn poll_receive(&mut self) -> Result<Option<(PhysicalLinkId, Vec<u8>)>, PhysicalError> {
        let mut available: c_int = 0;
        let rc = unsafe { libc::ioctl(self.fd.as_raw_fd(), FIONREAD, &mut available) };
        if rc < 0 {
            return Err(last_os_error());
        }
        if available <= 0 {
            return Ok(None);
        }

        let mut buffer = vec![0u8; self.max_frame + ETHERNET_HEADER_LEN + 256];
        let received = unsafe {
            libc::recv(
                self.fd.as_raw_fd(),
                buffer.as_mut_ptr().cast::<libc::c_void>(),
                buffer.len(),
                0,
            )
        };
        if received < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::WouldBlock {
                return Ok(None);
            }
            return Err(map_io_error(error));
        }
        let received = received as usize;
        if received < ETHERNET_HEADER_LEN {
            return Err(PhysicalError::DiscoveryFailed);
        }
        let ether_type = u16::from_be_bytes([buffer[12], buffer[13]]);
        if ether_type != AETHER_ETHERTYPE {
            return Ok(None);
        }
        let payload_len = received - ETHERNET_HEADER_LEN;
        if payload_len > self.max_frame {
            return Err(PhysicalError::FrameTooLarge);
        }
        buffer.drain(..ETHERNET_HEADER_LEN);
        buffer.truncate(payload_len);
        Ok(Some((self.link.id, buffer)))
    }
}

fn link_id(interface_name: &str) -> PhysicalLinkId {
    let mut hasher = Sha256::new();
    hasher.update(b"aether:physical:ethernet:v1:");
    hasher.update(interface_name.as_bytes());
    let digest = hasher.finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    PhysicalLinkId(id)
}

fn interface_mac(name: *const c_char) -> Result<[u8; 6], PhysicalError> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(last_os_error());
    }

    let mut ifr = unsafe { MaybeUninit::<libc::ifreq>::zeroed().assume_init() };
    copy_ifname(&mut ifr, name)?;
    let rc = unsafe { libc::ioctl(fd, libc::SIOCGIFHWADDR, &mut ifr) };
    let close_result = unsafe { libc::close(fd) };
    if close_result < 0 {
        return Err(last_os_error());
    }
    if rc < 0 {
        return Err(last_os_error());
    }

    let mut mac = [0u8; 6];
    unsafe {
        ptr::copy_nonoverlapping(
            ifr.ifru_hwaddr.sa_data.as_ptr().cast::<u8>(),
            mac.as_mut_ptr(),
            6,
        );
    }
    Ok(mac)
}

fn interface_mtu(name: *const c_char) -> Result<usize, PhysicalError> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(last_os_error());
    }

    let mut ifr = unsafe { MaybeUninit::<libc::ifreq>::zeroed().assume_init() };
    copy_ifname(&mut ifr, name)?;
    let rc = unsafe { libc::ioctl(fd, libc::SIOCGIFMTU, &mut ifr) };
    let close_result = unsafe { libc::close(fd) };
    if close_result < 0 {
        return Err(last_os_error());
    }
    if rc < 0 {
        return Err(last_os_error());
    }

    Ok(unsafe { ifr.ifr_ifru.ifru_mtu.max(1) as usize })
}

fn copy_ifname(ifr: &mut libc::ifreq, name: *const c_char) -> Result<(), PhysicalError> {
    let bytes = unsafe { std::ffi::CStr::from_ptr(name).to_bytes_with_nul() };
    if bytes.len() > IFNAMSIZ as usize {
        return Err(PhysicalError::Unsupported);
    }
    unsafe {
        ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            ifr.ifr_name.as_mut_ptr().cast::<u8>(),
            bytes.len(),
        );
    }
    Ok(())
}

fn last_os_error() -> PhysicalError {
    map_io_error(std::io::Error::last_os_error())
}

fn map_io_error(error: std::io::Error) -> PhysicalError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => PhysicalError::PermissionDenied,
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NetworkUnreachable => {
            PhysicalError::Unavailable
        }
        std::io::ErrorKind::WouldBlock => PhysicalError::Unavailable,
        _ => PhysicalError::HardwareFailure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_ethernet_link_id_is_stable() {
        assert_eq!(link_id("eth0"), link_id("eth0"));
        assert_ne!(link_id("eth0"), link_id("wlan0"));
    }
}
