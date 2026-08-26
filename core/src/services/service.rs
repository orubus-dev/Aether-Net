use crate::network::protocol::TrafficPriority;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    Core,
    Communication,
    Storage,
    Compute,
    Gaming,
    Identity,
    Infrastructure,
    ThirdParty,
}

#[derive(Debug, Clone)]
pub struct ServiceDescriptor {
    pub id: ServiceId,
    pub name: String,
    pub version: String,
    pub kind: ServiceKind,
    pub minimum_priority: TrafficPriority,
}
