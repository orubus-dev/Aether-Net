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

    pub const fn from_bytes(bytes: [u8; FLOW_ID_LENGTH]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; FLOW_ID_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L3Flags(pub u8);

impl L3Flags {
    pub const NONE: Self = Self(0);
    pub const RELAYABLE: Self = Self(1 << 0);
    pub const ACK_REQUIRED: Self = Self(1 << 1);
    pub const FRAGMENT: Self = Self(1 << 2);
    pub const FINAL_FRAGMENT: Self = Self(1 << 3);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
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
        if payload.len() > MAX_L3_PAYLOAD {
            return Err(L3Error::PayloadTooLarge);
        }
        if hop_limit == 0 {
            return Err(L3Error::InvalidHopLimit);
        }

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
        if self.hop_limit <= 1 {
            return Err(L3Error::InvalidHopLimit);
        }
        self.hop_limit -= 1;
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, L3Error> {
        if self.payload.len() > MAX_L3_PAYLOAD {
            return Err(L3Error::PayloadTooLarge);
        }
        if self.hop_limit == 0 {
            return Err(L3Error::InvalidHopLimit);
        }
        if self.flags.0
            & !(L3Flags::RELAYABLE.0
                | L3Flags::ACK_REQUIRED.0
                | L3Flags::FRAGMENT.0
                | L3Flags::FINAL_FRAGMENT.0)
            != 0
        {
            return Err(L3Error::InvalidFlags);
        }

        let mut output = Vec::with_capacity(L3_HEADER_LENGTH + self.payload.len());
        output.extend_from_slice(&L3_MAGIC);
        output.push(L3_VERSION);
        output.push(self.flags.0);
        output.push(self.priority as u8);
        output.push(self.hop_limit);
        output.extend_from_slice(self.source.as_bytes());
        output.extend_from_slice(self.destination.as_bytes());
        output.extend_from_slice(self.flow_id.as_bytes());
        output.extend_from_slice(&self.sequence.to_be_bytes());
        output.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        output.extend_from_slice(&self.payload);
        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, L3Error> {
        if bytes.len() < L3_HEADER_LENGTH {
            return Err(L3Error::InvalidLength);
        }
        if bytes[0..4] != L3_MAGIC {
            return Err(L3Error::InvalidMagic);
        }
        if bytes[4] != L3_VERSION {
            return Err(L3Error::UnsupportedVersion);
        }

        let flags = L3Flags(bytes[5]);
        let known_flags = L3Flags::RELAYABLE.0
            | L3Flags::ACK_REQUIRED.0
            | L3Flags::FRAGMENT.0
            | L3Flags::FINAL_FRAGMENT.0;
        if flags.0 & !known_flags != 0 {
            return Err(L3Error::InvalidFlags);
        }

        let priority = TrafficPriority::from_u8(bytes[6]).ok_or(L3Error::InvalidLength)?;
        let hop_limit = bytes[7];
        if hop_limit == 0 {
            return Err(L3Error::InvalidHopLimit);
        }

        let mut source = [0u8; AETHER_ADDRESS_LENGTH];
        source.copy_from_slice(&bytes[8..40]);
        let mut destination = [0u8; AETHER_ADDRESS_LENGTH];
        destination.copy_from_slice(&bytes[40..72]);
        let mut flow = [0u8; FLOW_ID_LENGTH];
        flow.copy_from_slice(&bytes[72..88]);
        let mut sequence_bytes = [0u8; 8];
        sequence_bytes.copy_from_slice(&bytes[88..96]);
        let sequence = u64::from_be_bytes(sequence_bytes);
        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&bytes[96..100]);
        let payload_length = u32::from_be_bytes(length_bytes) as usize;

        if payload_length > MAX_L3_PAYLOAD || bytes.len() != L3_HEADER_LENGTH + payload_length {
            return Err(L3Error::InvalidLength);
        }

        Ok(Self {
            source: AetherAddress::from_bytes(source),
            destination: AetherAddress::from_bytes(destination),
            flow_id: AetherFlowId::from_bytes(flow),
            sequence,
            hop_limit,
            priority,
            flags,
            payload: bytes[L3_HEADER_LENGTH..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(seed: u8) -> AetherAddress {
        AetherAddress::from_bytes([seed; AETHER_ADDRESS_LENGTH])
    }

    #[test]
    fn l3_round_trip_preserves_native_addresses() {
        let packet = AetherL3Packet::new(
            address(1),
            address(2),
            TrafficPriority::Interactive,
            L3Flags::RELAYABLE,
            32,
            b"hello-aether".to_vec(),
        )
        .unwrap();

        let decoded = AetherL3Packet::decode(&packet.encode().unwrap()).unwrap();
        assert_eq!(decoded.source, address(1));
        assert_eq!(decoded.destination, address(2));
        assert_eq!(decoded.payload, b"hello-aether");
    }

    #[test]
    fn forwarding_consumes_hop_budget() {
        let mut packet = AetherL3Packet::new(
            address(1),
            address(2),
            TrafficPriority::Browsing,
            L3Flags::RELAYABLE,
            2,
            vec![1, 2, 3],
        )
        .unwrap();
        packet.forward().unwrap();
        assert_eq!(packet.hop_limit, 1);
        assert!(packet.forward().is_err());
    }
}
