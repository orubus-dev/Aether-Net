use crate::network::AetherFlowId;

pub const L4_MAGIC: [u8; 4] = *b"A4TH";
pub const L4_VERSION: u8 = 1;
pub const L4_HEADER_LENGTH: usize = 4 + 1 + 1 + 1 + 1 + 16 + 8 + 4;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AetherTransportMode {
    Datagram = 0,
    Stream = 1,
    ReliableStream = 2,
    Realtime = 3,
}

impl AetherTransportMode {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Datagram),
            1 => Some(Self::Stream),
            2 => Some(Self::ReliableStream),
            3 => Some(Self::Realtime),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AetherL4Segment {
    pub flow_id: AetherFlowId,
    pub mode: AetherTransportMode,
    pub sequence: u64,
    pub flags: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L4Error {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    PayloadTooLarge,
    InvalidFlags,
}

pub const L4_FLAG_FIN: u8 = 1 << 0;
pub const L4_FLAG_ACK: u8 = 1 << 1;
pub const L4_FLAG_SYN: u8 = 1 << 2;
pub const L4_FLAG_RESET: u8 = 1 << 3;
const KNOWN_FLAGS: u8 = L4_FLAG_FIN | L4_FLAG_ACK | L4_FLAG_SYN | L4_FLAG_RESET;
pub const MAX_L4_PAYLOAD: usize = 60_000;

impl AetherL4Segment {
    pub fn new(
        flow_id: AetherFlowId,
        mode: AetherTransportMode,
        sequence: u64,
        flags: u8,
        payload: Vec<u8>,
    ) -> Result<Self, L4Error> {
        if payload.len() > MAX_L4_PAYLOAD {
            return Err(L4Error::PayloadTooLarge);
        }
        if flags & !KNOWN_FLAGS != 0 {
            return Err(L4Error::InvalidFlags);
        }
        Ok(Self {
            flow_id,
            mode,
            sequence,
            flags,
            payload,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, L4Error> {
        if self.payload.len() > MAX_L4_PAYLOAD {
            return Err(L4Error::PayloadTooLarge);
        }
        if self.flags & !KNOWN_FLAGS != 0 {
            return Err(L4Error::InvalidFlags);
        }
        let mut output = Vec::with_capacity(L4_HEADER_LENGTH + self.payload.len());
        output.extend_from_slice(&L4_MAGIC);
        output.push(L4_VERSION);
        output.push(self.mode as u8);
        output.push(self.flags);
        output.push(0);
        output.extend_from_slice(self.flow_id.as_bytes());
        output.extend_from_slice(&self.sequence.to_be_bytes());
        output.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        output.extend_from_slice(&self.payload);
        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, L4Error> {
        if bytes.len() < L4_HEADER_LENGTH {
            return Err(L4Error::InvalidLength);
        }
        if bytes[0..4] != L4_MAGIC {
            return Err(L4Error::InvalidMagic);
        }
        if bytes[4] != L4_VERSION {
            return Err(L4Error::UnsupportedVersion);
        }
        let mode = AetherTransportMode::from_u8(bytes[5]).ok_or(L4Error::InvalidLength)?;
        let flags = bytes[6];
        if flags & !KNOWN_FLAGS != 0 {
            return Err(L4Error::InvalidFlags);
        }
        let mut flow_bytes = [0u8; 16];
        flow_bytes.copy_from_slice(&bytes[8..24]);
        let mut sequence_bytes = [0u8; 8];
        sequence_bytes.copy_from_slice(&bytes[24..32]);
        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&bytes[32..36]);
        let payload_length = u32::from_be_bytes(length_bytes) as usize;
        if payload_length > MAX_L4_PAYLOAD || bytes.len() != L4_HEADER_LENGTH + payload_length {
            return Err(L4Error::InvalidLength);
        }
        Ok(Self {
            flow_id: AetherFlowId::from_bytes(flow_bytes),
            mode,
            sequence: u64::from_be_bytes(sequence_bytes),
            flags,
            payload: bytes[L4_HEADER_LENGTH..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l4_round_trip() {
        let flow = AetherFlowId::from_bytes([7; 16]);
        let segment = AetherL4Segment::new(
            flow,
            AetherTransportMode::Realtime,
            42,
            L4_FLAG_SYN,
            b"voice-frame".to_vec(),
        )
        .unwrap();
        let decoded = AetherL4Segment::decode(&segment.encode().unwrap()).unwrap();
        assert_eq!(decoded, segment);
    }
}
