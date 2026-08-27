use crate::network::TrafficPriority;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardingPolicy {
    pub allow_relay: bool,

    pub max_hop_limit: u8,

    pub minimum_priority: TrafficPriority,
}

impl Default for ForwardingPolicy {
    fn default() -> Self {
        Self {
            allow_relay: true,
            max_hop_limit: 64,
            minimum_priority: TrafficPriority::Prefetch,
        }
    }
}

impl ForwardingPolicy {
    pub fn permits(&self, priority: TrafficPriority, hop_limit: u8) -> bool {
        self.allow_relay
            && hop_limit > 0
            && hop_limit <= self.max_hop_limit
            && priority as u8 <= self.minimum_priority as u8
    }
}
