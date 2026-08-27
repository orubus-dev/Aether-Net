use crate::network::{AetherPacket, TrafficPriority};

use super::policy::ForwardingPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardDecision {
    pub priority: TrafficPriority,

    pub next_hop: [u8; 32],

    pub remaining_hops: u8,
}

pub struct ForwardingEngine {
    policy: ForwardingPolicy,
}

impl ForwardingEngine {
    pub fn new(policy: ForwardingPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> ForwardingPolicy {
        self.policy
    }

    pub fn prepare(
        &self,
        packet: &AetherPacket,

        next_hop: [u8; 32],
    ) -> Result<ForwardDecision, ForwardingError> {
        if !self
            .policy
            .permits(packet.header.priority, packet.header.hop_limit)
        {
            return Err(ForwardingError::PolicyDenied);
        }

        if packet.header.hop_limit <= 1 {
            return Err(ForwardingError::HopLimitExpired);
        }

        Ok(ForwardDecision {
            priority: packet.header.priority,

            next_hop,

            remaining_hops: packet.header.hop_limit - 1,
        })
    }

    pub fn decrement(&self, packet: &mut AetherPacket) -> Result<(), ForwardingError> {
        packet
            .header
            .decrement_hop_limit()
            .map_err(|_| ForwardingError::HopLimitExpired)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardingError {
    PolicyDenied,
    HopLimitExpired,
}
