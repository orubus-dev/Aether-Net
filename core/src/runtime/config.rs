use crate::security::SecurityProfile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub protocol_major: u8,
    pub protocol_minor: u8,
    pub relay_enabled: bool,
    pub default_hop_limit: u8,
    pub mesh_stop_relay_battery: u8,
    pub emergency_mode: bool,
    pub session_idle_timeout_seconds: u64,
    pub session_max_age_seconds: u64,
    pub security_profile: SecurityProfile,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            protocol_major: 1,
            protocol_minor: 0,
            relay_enabled: true,
            default_hop_limit: 16,
            mesh_stop_relay_battery: 10,
            emergency_mode: false,
            session_idle_timeout_seconds: 300,
            session_max_age_seconds: 3600,
            security_profile: SecurityProfile::Standard,
        }
    }
}
