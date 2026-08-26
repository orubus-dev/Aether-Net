use crate::network::node::NodeId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceId(pub [u8; 16]);

#[derive(Debug, Clone)]
pub struct ServiceAdvertisement {
    pub service_id: ServiceId,
    pub provider: NodeId,
    pub name: String,
    pub version: String,
    pub priority: u8,
}

#[derive(Debug, Default)]
pub struct ServiceRegistry {
    services: HashMap<ServiceId, ServiceAdvertisement>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advertise(&mut self, service: ServiceAdvertisement) {
        self.services.insert(service.service_id, service);
    }

    pub fn withdraw(&mut self, service_id: &ServiceId) {
        self.services.remove(service_id);
    }

    pub fn get(&self, service_id: &ServiceId) -> Option<&ServiceAdvertisement> {
        self.services.get(service_id)
    }

    pub fn find_by_name(&self, name: &str) -> Vec<&ServiceAdvertisement> {
        self.services
            .values()
            .filter(|service| service.name == name)
            .collect()
    }
}
