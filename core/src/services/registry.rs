use crate::services::service::{ServiceDescriptor, ServiceId};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct RuntimeServiceRegistry {
    services: HashMap<ServiceId, ServiceDescriptor>,
}

impl RuntimeServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, service: ServiceDescriptor) -> bool {
        self.services.insert(service.id, service).is_some()
    }

    pub fn unregister(&mut self, id: &ServiceId) -> Option<ServiceDescriptor> {
        self.services.remove(id)
    }

    pub fn get(&self, id: &ServiceId) -> Option<&ServiceDescriptor> {
        self.services.get(id)
    }

    pub fn list(&self) -> impl Iterator<Item = &ServiceDescriptor> {
        self.services.values()
    }
}
