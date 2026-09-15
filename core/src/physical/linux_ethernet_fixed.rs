#![cfg(target_os = "linux")]

use super::capabilities::{PhysicalCapabilities, PhysicalTransport};
use super::engine::{PhysicalAdapterExt, UnderlayProperties};
use super::interface::{PhysicalAdapter, PhysicalAnalytics, PhysicalDriver, PhysicalError};
use super::link::{PhysicalLink, PhysicalLinkId};
use libc::{sockaddr, sockaddr_ll, AF_PACKET, FIONREAD, SOCK_CLOEXEC, SOCK_NONBLOCK, SOCK_RAW};
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::mem::{self, MaybeUninit};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::ptr;

pub const AETHER_ETHERTYPE: u16 = 0xA37E;
const HEADER: usize = 14;
const MAX_PAYLOAD: usize = 65_507;

pub struct LinuxEthernetDriver {
    fd: OwnedFd,
    ifindex: u32,
    name: String,
    transport: PhysicalTransport,
    link: PhysicalLink,
    properties: UnderlayProperties,
    source_mac: [u8; 6],
    mtu: usize,
}

impl LinuxEthernetDriver {
    pub fn open(name: &str, mtu: usize) -> Result<Self, PhysicalError> { Self::open_with_transport(name, mtu, PhysicalTransport::Ethernet) }
    pub fn open_with_transport(name: &str, mtu: usize, transport: PhysicalTransport) -> Result<Self, PhysicalError> {
        if !(576..=MAX_PAYLOAD).contains(&mtu) { return Err(PhysicalError::FrameTooLarge); }
        let cname=CString::new(name).map_err(|_|PhysicalError::Unsupported)?;
        let ifindex=unsafe{libc::if_nametoindex(cname.as_ptr())};
        if ifindex==0{return Err(PhysicalError::Unavailable);}
        let fd=unsafe{libc::socket(AF_PACKET,SOCK_RAW|SOCK_NONBLOCK|SOCK_CLOEXEC,AETHER_ETHERTYPE.to_be() as i32)};
        if fd<0{return Err(map_io(std::io::Error::last_os_error()));}
        let fd=unsafe{OwnedFd::from_raw_fd(fd)};
        let mut addr=sockaddr_ll{sll_family:AF_PACKET as u16,sll_protocol:AETHER_ETHERTYPE.to_be(),sll_ifindex:ifindex as i32,sll_hatype:0,sll_pkttype:0,sll_halen:0,sll_addr:[0;8]};
        let rc=unsafe{libc::bind(fd.as_raw_fd(),(&mut addr as *mut sockaddr_ll).cast::<sockaddr>(),mem::size_of::<sockaddr_ll>() as u32)};
        if rc<0{return Err(map_io(std::io::Error::last_os_error()));}
        let source_mac=mac(cname.as_ptr())?;
        let actual_mtu=mtu.min(interface_mtu(cname.as_ptr())?).max(576);
        let active=Path::new("/sys/class/net").join(name).join("operstate").exists() && fs_string(Path::new("/sys/class/net").join(name).join("operstate")).map(|v|v.trim()=="up").unwrap_or(false);
        let speed=interface_speed(name,transport);
        let link=PhysicalLink{id:link_id(name),transport,estimated_latency_us:if active{100}else{0},estimated_bandwidth_bps:speed.max(1),packet_loss_ppm:0,signal_quality:if active{100}else{0},active};
        let properties=UnderlayProperties{transport,mtu:actual_mtu,bandwidth_bps:speed.max(1),latency_us:link.estimated_latency_us,energy_cost:1,supports_broadcast:true,supports_multicast:true};
        Ok(Self{fd,ifindex,name:name.to_owned(),transport,link,properties,source_mac,mtu:actual_mtu})
    }
    pub fn interface_name(&self)->&str{&self.name}
    pub fn ifindex(&self)->u32{self.ifindex}
}

impl PhysicalAdapter for LinuxEthernetDriver {
    fn transport(&self)->PhysicalTransport{self.transport}
    fn capabilities(&self)->PhysicalCapabilities{if self.transport==PhysicalTransport::Wifi{PhysicalCapabilities{wifi:true,..PhysicalCapabilities::empty()}}else{PhysicalCapabilities{ethernet:true,..PhysicalCapabilities::empty()}}}
    fn discover(&self)->Result<Vec<PhysicalLink>,PhysicalError>{let mut link=self.link;link.active=fs_string(Path::new("/sys/class/net").join(&self.name).join("operstate")).map(|v|v.trim()=="up").unwrap_or(false);link.estimated_bandwidth_bps=interface_speed(&self.name,self.transport).max(1);link.signal_quality=if link.active{100}else{0};Ok(vec![link])}
}
impl PhysicalAdapterExt for LinuxEthernetDriver{fn properties(&self)->UnderlayProperties{self.properties}}
impl PhysicalDriver for LinuxEthernetDriver{
    fn mtu(&self)->usize{self.mtu}
    fn transmit(&mut self,link_id:PhysicalLinkId,frame:&[u8])->Result<(),PhysicalError>{if link_id!=self.link.id||!self.discover()?.first().map(|l|l.active).unwrap_or(false){return Err(PhysicalError::ConnectionFailed)}if frame.len()>self.mtu{return Err(PhysicalError::FrameTooLarge)}let mut packet=Vec::with_capacity(HEADER+frame.len());packet.extend_from_slice(&[0xff;6]);packet.extend_from_slice(&self.source_mac);packet.extend_from_slice(&AETHER_ETHERTYPE.to_be_bytes());packet.extend_from_slice(frame);let mut addr=sockaddr_ll{sll_family:AF_PACKET as u16,sll_protocol:AETHER_ETHERTYPE.to_be(),sll_ifindex:self.ifindex as i32,sll_hatype:0,sll_pkttype:0,sll_halen:6,sll_addr:[0;8]};addr.sll_addr[..6].copy_from_slice(&[0xff;6]);let written=unsafe{libc::sendto(self.fd.as_raw_fd(),packet.as_ptr().cast(),packet.len(),0,(&addr as *const sockaddr_ll).cast(),mem::size_of::<sockaddr_ll>() as u32)};if written<0{return Err(map_io(std::io::Error::last_os_error()))}if written as usize!=packet.len(){return Err(PhysicalError::ConnectionFailed)}Ok(())}
    fn poll_receive(&mut self)->Result<Option<(PhysicalLinkId,Vec<u8>)>,PhysicalError>{let mut available=0i32;let rc=unsafe{libc::ioctl(self.fd.as_raw_fd(),FIONREAD,&mut available)};if rc<0{return Err(map_io(std::io::Error::last_os_error()))}if available<=0{return Ok(None)}let mut buffer=vec![0u8;self.mtu+HEADER];let n=unsafe{libc::recv(self.fd.as_raw_fd(),buffer.as_mut_ptr().cast(),buffer.len(),0)};if n<0{let e=std::io::Error::last_os_error();if e.kind()==std::io::ErrorKind::WouldBlock{return Ok(None)}return Err(map_io(e))}let n=n as usize;if n<HEADER{return Err(PhysicalError::DiscoveryFailed)}if u16::from_be_bytes([buffer[12],buffer[13]])!=AETHER_ETHERTYPE{return Ok(None)}let payload=n-HEADER;if payload>self.mtu{return Err(PhysicalError::FrameTooLarge)}buffer.copy_within(HEADER..n,0);buffer.truncate(payload);Ok(Some((self.link.id,buffer)))}
    fn analytics(&self,link_id:PhysicalLinkId)->Result<Option<PhysicalAnalytics>,PhysicalError>{if link_id!=self.link.id{return Err(PhysicalError::ConnectionFailed)}let s=Path::new("/sys/class/net").join(&self.name).join("statistics");Ok(Some(PhysicalAnalytics{rx_bytes:read_u64(s.join("rx_bytes"))?,rx_packets:read_u64(s.join("rx_packets"))?,rx_errors:read_u64(s.join("rx_errors"))?,rx_drops:read_u64(s.join("rx_dropped"))?,tx_bytes:read_u64(s.join("tx_bytes"))?,tx_packets:read_u64(s.join("tx_packets"))?,tx_errors:read_u64(s.join("tx_errors"))?,tx_drops:read_u64(s.join("tx_dropped"))?}))}
}

fn link_id(name:&str)->PhysicalLinkId{let mut h=Sha256::new();h.update(b"aether:physical:ethernet:v2:");h.update(name.as_bytes());let d=h.finalize();let mut id=[0u8;16];id.copy_from_slice(&d[..16]);PhysicalLinkId(id)}
fn fs_string(path:impl AsRef<Path>)->Result<String,PhysicalError>{std::fs::read_to_string(path).map_err(|_|PhysicalError::Unavailable)}
fn read_u64(path:impl AsRef<Path>)->Result<u64,PhysicalError>{fs_string(path)?.trim().parse().map_err(|_|PhysicalError::DiscoveryFailed)}
fn interface_speed(name:&str,transport:PhysicalTransport)->u64{match fs_string(Path::new("/sys/class/net").join(name).join("speed")).ok().and_then(|s|s.trim().parse::<i64>().ok()).filter(|v|*v>0){Some(v)=>v as u64*1_000_000,None=>if transport==PhysicalTransport::Wifi{100_000_000}else{1_000_000_000}}}
fn mac(name:*const std::ffi::c_char)->Result<[u8;6],PhysicalError>{let fd=unsafe{libc::socket(libc::AF_INET,libc::SOCK_DGRAM|SOCK_CLOEXEC,0)};if fd<0{return Err(map_io(std::io::Error::last_os_error()))}let mut ifr=unsafe{MaybeUninit::<libc::ifreq>::zeroed().assume_init()};copy_name(&mut ifr,name)?;let rc=unsafe{libc::ioctl(fd,libc::SIOCGIFHWADDR,&mut ifr)};unsafe{libc::close(fd)};if rc<0{return Err(map_io(std::io::Error::last_os_error()))}let mut out=[0u8;6];unsafe{ptr::copy_nonoverlapping(ifr.ifr_ifru.ifru_hwaddr.sa_data.as_ptr().cast::<u8>(),out.as_mut_ptr(),6)}Ok(out)}
fn interface_mtu(name:*const std::ffi::c_char)->Result<usize,PhysicalError>{let fd=unsafe{libc::socket(libc::AF_INET,libc::SOCK_DGRAM|SOCK_CLOEXEC,0)};if fd<0{return Err(map_io(std::io::Error::last_os_error()))}let mut ifr=unsafe{MaybeUninit::<libc::ifreq>::zeroed().assume_init()};copy_name(&mut ifr,name)?;let rc=unsafe{libc::ioctl(fd,libc::SIOCGIFMTU,&mut ifr)};unsafe{libc::close(fd)};if rc<0{return Err(map_io(std::io::Error::last_os_error()))}Ok(unsafe{ifr.ifr_ifru.ifru_mtu.max(1) as usize})}
fn copy_name(ifr:&mut libc::ifreq,name:*const std::ffi::c_char)->Result<(),PhysicalError>{let bytes=unsafe{std::ffi::CStr::from_ptr(name).to_bytes_with_nul()};if bytes.len()>libc::IFNAMSIZ as usize{return Err(PhysicalError::Unsupported)}unsafe{ptr::copy_nonoverlapping(bytes.as_ptr(),ifr.ifr_name.as_mut_ptr().cast::<u8>(),bytes.len())}Ok(())}
fn map_io(e:std::io::Error)->PhysicalError{match e.kind(){std::io::ErrorKind::PermissionDenied=>PhysicalError::PermissionDenied,std::io::ErrorKind::NotFound=>PhysicalError::Unavailable,std::io::ErrorKind::WouldBlock=>PhysicalError::Unavailable,_=>PhysicalError::HardwareFailure}}

#[cfg(test)]
mod tests{use super::*;#[test]fn stable_link_ids(){assert_eq!(link_id("eth0"),link_id("eth0"));assert_ne!(link_id("eth0"),link_id("wlan0"));}}
