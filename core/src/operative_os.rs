use crate::identity::DeviceId;
use crate::network::NodeId;
use crate::operative::BandwidthLimits;
use crate::physical::PhysicalCapabilities;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapabilityLevel { L0=0, L1=1, L2=2, L3=3, L4=4, L5=5 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeResources {
    pub ram_bytes: u64,
    pub storage_bytes: u64,
    pub cpu_millis: u32,
    pub bandwidth: BandwidthLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperativeIdentity { pub node_id: NodeId, pub device_id: DeviceId }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootState { Cold, Starting, Ready, Degraded, Recovery }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState { Healthy, Warning, Critical }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperativeStatus {
    pub boot: BootState,
    pub health: HealthState,
    pub level: CapabilityLevel,
    pub uptime_seconds: u64,
    pub active_transfers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsError { InvalidResources, InvalidLevel, RecoveryRequired }

pub struct AetherNetOs {
    identity: OperativeIdentity,
    resources: NodeResources,
    physical: PhysicalCapabilities,
    status: OperativeStatus,
}
impl AetherNetOs {
    pub fn new(identity: OperativeIdentity, resources: NodeResources, physical: PhysicalCapabilities, level: CapabilityLevel) -> Result<Self, OsError> {
        if resources.ram_bytes == 0 || resources.storage_bytes == 0 || resources.cpu_millis == 0 || resources.bandwidth.upload_bps == 0 || resources.bandwidth.download_bps == 0 { return Err(OsError::InvalidResources); }
        Ok(Self { identity, resources, physical, status: OperativeStatus { boot: BootState::Cold, health: HealthState::Healthy, level, uptime_seconds: 0, active_transfers: 0 } })
    }
    pub fn identity(&self) -> OperativeIdentity { self.identity }
    pub fn resources(&self) -> NodeResources { self.resources }
    pub fn physical(&self) -> PhysicalCapabilities { self.physical }
    pub fn status(&self) -> OperativeStatus { self.status }
    pub fn boot(&mut self) { self.status.boot = BootState::Starting; self.status.boot = BootState::Ready; }
    pub fn tick(&mut self, seconds: u64) { self.status.uptime_seconds = self.status.uptime_seconds.saturating_add(seconds); }
    pub fn set_degraded(&mut self) { self.status.boot = BootState::Degraded; self.status.health = HealthState::Warning; }
    pub fn enter_recovery(&mut self) { self.status.boot = BootState::Recovery; self.status.health = HealthState::Critical; }
    pub fn recover(&mut self) { self.status.boot = BootState::Ready; self.status.health = HealthState::Healthy; }
    pub fn set_active_transfers(&mut self, count: usize) { self.status.active_transfers = count; }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(x:u8)->NodeId{NodeId::from_bytes([x;32])}
    fn d(x:u8)->DeviceId{DeviceId::from_bytes([x;32])}
    #[test] fn boot_and_recovery_lifecycle(){let resources=NodeResources{ram_bytes:4*1024*1024*1024,storage_bytes:512*1024*1024,cpu_millis:1000,bandwidth:BandwidthLimits::new(250_000,1_250_000)};let mut os=AetherNetOs::new(OperativeIdentity{node_id:n(1),device_id:d(1)},resources,PhysicalCapabilities::default(),CapabilityLevel::L5).unwrap();os.boot();assert_eq!(os.status().boot,BootState::Ready);os.enter_recovery();assert_eq!(os.status().health,HealthState::Critical);os.recover();assert_eq!(os.status().health,HealthState::Healthy);}
}
