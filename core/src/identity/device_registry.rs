use crate::identity::device::{DeviceId, DeviceState, DeviceStatus};
use std::collections::HashMap;

#[derive(Debug)]
pub struct RegisteredDevice {
    pub state: DeviceState,
}

#[derive(Debug, Default)]
pub struct DeviceRegistry {
    devices: HashMap<DeviceId, RegisteredDevice>,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, state: DeviceState) -> Result<DeviceId, RegistryError> {
        let device_id = state.device_id;

        if self.devices.contains_key(&device_id) {
            return Err(RegistryError::AlreadyRegistered);
        }

        self.devices.insert(device_id, RegisteredDevice { state });

        Ok(device_id)
    }

    pub fn get(&self, device_id: &DeviceId) -> Option<&RegisteredDevice> {
        self.devices.get(device_id)
    }

    pub fn get_mut(&mut self, device_id: &DeviceId) -> Option<&mut RegisteredDevice> {
        self.devices.get_mut(device_id)
    }

    pub fn status(&self, device_id: &DeviceId) -> Option<DeviceStatus> {
        self.devices
            .get(device_id)
            .map(|device| device.state.status)
    }

    pub fn revoke(&mut self, device_id: &DeviceId) -> Result<(), RegistryError> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or(RegistryError::NotFound)?;

        device.state.revoke();

        Ok(())
    }

    pub fn suspend(&mut self, device_id: &DeviceId) -> Result<(), RegistryError> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or(RegistryError::NotFound)?;

        device.state.suspend();

        Ok(())
    }

    pub fn restore(&mut self, device_id: &DeviceId) -> Result<(), RegistryError> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or(RegistryError::NotFound)?;

        device.state.restore();

        Ok(())
    }

    pub fn remove(&mut self, device_id: &DeviceId) -> Result<RegisteredDevice, RegistryError> {
        self.devices
            .remove(device_id)
            .ok_or(RegistryError::NotFound)
    }

    pub fn contains(&self, device_id: &DeviceId) -> bool {
        self.devices.contains_key(device_id)
    }

    pub fn len(&self) -> usize {
        self.devices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&DeviceId, &RegisteredDevice)> {
        self.devices.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    AlreadyRegistered,
    NotFound,
}
