use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhysicalTransport {
    Ethernet,
    Wifi,
    WifiDirect,
    WifiAware,
    BluetoothLe,
    Uwb,
    LoRa,
    Cellular,
    Satellite,
}

impl PhysicalTransport {
    #[allow(non_upper_case_globals)]
    pub const Bluetooth: Self = Self::BluetoothLe;
    #[allow(non_upper_case_globals)]
    pub const Virtual: Self = Self::Ethernet;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalCapabilities {
    pub ethernet: bool,
    pub wifi: bool,
    pub wifi_direct: bool,
    pub wifi_aware: bool,
    pub bluetooth_le: bool,
    pub uwb: bool,
    pub lora: bool,
    pub cellular: bool,
    pub satellite: bool,
}
impl PhysicalCapabilities {
    pub const fn empty() -> Self {
        Self {
            ethernet: false,
            wifi: false,
            wifi_direct: false,
            wifi_aware: false,
            bluetooth_le: false,
            uwb: false,
            lora: false,
            cellular: false,
            satellite: false,
        }
    }
    pub fn supports(&self, transport: PhysicalTransport) -> bool {
        match transport {
            PhysicalTransport::Ethernet => self.ethernet,
            PhysicalTransport::Wifi => self.wifi,
            PhysicalTransport::WifiDirect => self.wifi_direct,
            PhysicalTransport::WifiAware => self.wifi_aware,
            PhysicalTransport::BluetoothLe => self.bluetooth_le,
            PhysicalTransport::Uwb => self.uwb,
            PhysicalTransport::LoRa => self.lora,
            PhysicalTransport::Cellular => self.cellular,
            PhysicalTransport::Satellite => self.satellite,
        }
    }
}
