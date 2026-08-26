#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshPowerPolicy {
    pub stop_relay_below_percent: u8,
    pub allow_relay_while_charging: bool,
    pub emergency_override: bool,
}

impl Default for MeshPowerPolicy {
    fn default() -> Self {
        Self {
            stop_relay_below_percent: 10,
            allow_relay_while_charging: true,
            emergency_override: true,
        }
    }
}

impl MeshPowerPolicy {
    pub fn relay_allowed(&self, battery_percent: u8, charging: bool, emergency: bool) -> bool {
        if emergency && self.emergency_override {
            return true;
        }

        if charging && self.allow_relay_while_charging {
            return true;
        }

        battery_percent > self.stop_relay_below_percent
    }
}
