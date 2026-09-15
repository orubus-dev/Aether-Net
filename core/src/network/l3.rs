use super::address::{AETHER_ADDRESS_LENGTH, AetherAddress};
use super::protocol::TrafficPriority;

pub const L3_MAGIC: [u8; 4] = *b"A3TH";
pub const L3_VERSION: u8 = 1;
pub const FLOW_ID_LENGTH: usize = 16;
pub const L3_HEADER_LENGTH: usize = 4 + 1 + 1 + 1 + 1 + 32 + 32 + 16 + 8 + 4;
pub const MAX_L3_PAYLOAD: usize = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AetherFlowId([u8; FLOW_ID_LENGTH]);

impl AetherFlowId {
    pub fn generate() -> Result<Self, L3Error> {
        let mut bytes = [0u8; FLOW_ID_LENGTH];
        getrandom::fill(&mut bytes).map_err(|_| L3Error::RandomnessUnavailable)?;
        Ok(Self(bytes))
    }

    pub const fn from_bytes(bytes: [u8; FLOW_ID_LENGTH]) -> Self { Self(bytes) }

    pub const fn as_bytes(&self) -> &[u8; FLOW_ID_LENGTH] { &self.0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L3Flags(pub u8);

impl L3Flags {
    pub const NONE: Self = Self(0);
    pub const RELAYABLE: Self = Self(1 << 0);
    pub const ACK_REQUIRED: Self = Self(1 << 1);
    pub const FRAGMENT: Self = Self(1 << 2);
    pub const FINAL_FRAGMENT: Self = Self(1 << 3);

    pub const fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AetherL3Packet {
    pub source: AetherAddress,
    pub destination: AetherAddress,
    pub flow_id: AetherFlowId,
    pub sequence: u64,
    pub hop_limit: u8,
    pub priority: TrafficPriority,
    pub flags: L3Flags,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L3Error {
    PayloadTooLarge,
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    InvalidFlags,
    InvalidHopLimit,
    RandomnessUnavailable,
}

impl AetherL3Packet {
    pub fn new(
        source: AetherAddress,
        destination: AetherAddress,
        priority: TrafficPriority,
        flags: L3Flags,
        hop_limit: u8,
        payload: Vec<u8>,
    ) -> Result<Self, L3Error> {
        if payload.len() > MAX_L3_PAYLOAD { return Err(L3Error::PayloadTooLarge); }
        if hop_limit == 0 { return Err(L3Error::InvalidHopLimit); }
        Ok(Self {
            source,
            destination,
            flow_id: AetherFlowId::generate()?,
            sequence: 0,
            hop_limit,
            priority,
            flags,
            payload,
        })
    }

    pub fn forward(&mut self) -> Result<(), L3Error> {
        if self.hop_limit <= 1 { return Err(L3Error::InvalidHopLimit); }
        self.hop_limit -= 1;
        Ok(())
    }
}
