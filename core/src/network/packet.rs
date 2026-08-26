use crate::network::protocol::{
    MessageType, PACKET_HEADER_VERSION, PROTOCOL_MAGIC, PacketFlags, ProtocolVersion,
    TrafficPriority,
};
use getrandom;

pub const PACKET_ID_LENGTH: usize = 16;
pub const FLOW_ID_LENGTH: usize = 16;
pub const ROUTE_TOKEN_LENGTH: usize = 16;
pub const MAX_PAYLOAD_SIZE: usize = 65_535;

/*
    Aether packet header:

    4   bytes  magic
    1   byte   header version
    1   byte   protocol major
    1   byte   protocol minor
    1   byte   message type
    1   byte   traffic priority
    2   bytes  flags
    1   byte   hop limit
    2   bytes  header length
    16  bytes  packet id
    16  bytes  flow id
    16  bytes  route token
    4   bytes  payload length

    Total = 66 bytes
*/
pub const HEADER_LENGTH: usize = 66;

#[derive(Debug)]
pub enum PacketError {
    TooLarge,
    TooShort,
    InvalidMagic,
    UnsupportedHeaderVersion,
    UnsupportedProtocolVersion,
    UnknownMessageType,
    UnknownPriority,
    InvalidFlags,
    InvalidHeaderLength,
    InvalidPayloadLength,
    HopLimitExpired,
    RandomnessUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketId([u8; PACKET_ID_LENGTH]);

impl PacketId {
    pub fn generate() -> Result<Self, PacketError> {
        let mut value = [0u8; PACKET_ID_LENGTH];

        getrandom::fill(&mut value).map_err(|_| PacketError::RandomnessUnavailable)?;

        Ok(Self(value))
    }

    pub fn as_bytes(&self) -> &[u8; PACKET_ID_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowId([u8; FLOW_ID_LENGTH]);

impl FlowId {
    pub fn generate() -> Result<Self, PacketError> {
        let mut value = [0u8; FLOW_ID_LENGTH];

        getrandom::fill(&mut value).map_err(|_| PacketError::RandomnessUnavailable)?;

        Ok(Self(value))
    }

    pub fn from_bytes(value: [u8; FLOW_ID_LENGTH]) -> Self {
        Self(value)
    }

    pub fn as_bytes(&self) -> &[u8; FLOW_ID_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteToken([u8; ROUTE_TOKEN_LENGTH]);

impl RouteToken {
    pub fn generate() -> Result<Self, PacketError> {
        let mut value = [0u8; ROUTE_TOKEN_LENGTH];

        getrandom::fill(&mut value).map_err(|_| PacketError::RandomnessUnavailable)?;

        Ok(Self(value))
    }

    pub fn as_bytes(&self) -> &[u8; ROUTE_TOKEN_LENGTH] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    pub protocol: ProtocolVersion,

    pub message_type: MessageType,

    pub priority: TrafficPriority,

    pub flags: PacketFlags,

    pub hop_limit: u8,

    pub packet_id: PacketId,

    pub flow_id: FlowId,

    pub route_token: RouteToken,

    pub payload_length: u32,
}

impl PacketHeader {
    pub fn new(
        message_type: MessageType,

        priority: TrafficPriority,

        flags: PacketFlags,

        hop_limit: u8,

        flow_id: FlowId,

        route_token: RouteToken,

        payload_length: usize,
    ) -> Result<Self, PacketError> {
        if payload_length > MAX_PAYLOAD_SIZE {
            return Err(PacketError::TooLarge);
        }

        if hop_limit == 0 {
            return Err(PacketError::HopLimitExpired);
        }

        Ok(Self {
            protocol: ProtocolVersion::CURRENT,

            message_type,

            priority,

            flags,

            hop_limit,

            packet_id: PacketId::generate()?,

            flow_id,

            route_token,

            payload_length: payload_length as u32,
        })
    }

    pub fn decrement_hop_limit(&mut self) -> Result<(), PacketError> {
        if self.hop_limit <= 1 {
            return Err(PacketError::HopLimitExpired);
        }

        self.hop_limit -= 1;

        Ok(())
    }

    pub fn validate(&self) -> Result<(), PacketError> {
        if self.protocol.major != ProtocolVersion::CURRENT.major {
            return Err(PacketError::UnsupportedProtocolVersion);
        }

        if self.payload_length as usize > MAX_PAYLOAD_SIZE {
            return Err(PacketError::TooLarge);
        }

        if self.hop_limit == 0 {
            return Err(PacketError::HopLimitExpired);
        }

        Ok(())
    }

    fn encode_into(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&PROTOCOL_MAGIC);

        output.push(PACKET_HEADER_VERSION);

        output.push(self.protocol.major);

        output.push(self.protocol.minor);

        output.push(self.message_type as u8);

        output.push(self.priority as u8);

        output.extend_from_slice(&self.flags.bits().to_be_bytes());

        output.push(self.hop_limit);

        output.extend_from_slice(&(HEADER_LENGTH as u16).to_be_bytes());

        output.extend_from_slice(self.packet_id.as_bytes());

        output.extend_from_slice(self.flow_id.as_bytes());

        output.extend_from_slice(self.route_token.as_bytes());

        output.extend_from_slice(&self.payload_length.to_be_bytes());
    }
}

pub struct AetherPacket {
    pub header: PacketHeader,

    pub payload: Vec<u8>,
}

impl AetherPacket {
    pub fn new(
        message_type: MessageType,

        priority: TrafficPriority,

        flags: PacketFlags,

        hop_limit: u8,

        flow_id: FlowId,

        route_token: RouteToken,

        payload: Vec<u8>,
    ) -> Result<Self, PacketError> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(PacketError::TooLarge);
        }

        let header = PacketHeader::new(
            message_type,
            priority,
            flags,
            hop_limit,
            flow_id,
            route_token,
            payload.len(),
        )?;

        Ok(Self { header, payload })
    }

    pub fn encode(&self) -> Result<Vec<u8>, PacketError> {
        self.header.validate()?;

        if self.payload.len() != self.header.payload_length as usize {
            return Err(PacketError::InvalidPayloadLength);
        }

        let mut output = Vec::with_capacity(HEADER_LENGTH + self.payload.len());

        self.header.encode_into(&mut output);

        output.extend_from_slice(&self.payload);

        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() < HEADER_LENGTH {
            return Err(PacketError::TooShort);
        }

        if bytes[0..4] != PROTOCOL_MAGIC {
            return Err(PacketError::InvalidMagic);
        }

        if bytes[4] != PACKET_HEADER_VERSION {
            return Err(PacketError::UnsupportedHeaderVersion);
        }

        let protocol = ProtocolVersion::new(bytes[5], bytes[6]);

        if protocol.major != ProtocolVersion::CURRENT.major {
            return Err(PacketError::UnsupportedProtocolVersion);
        }

        let message_type = MessageType::from_u8(bytes[7]).ok_or(PacketError::UnknownMessageType)?;

        let priority = TrafficPriority::from_u8(bytes[8]).ok_or(PacketError::UnknownPriority)?;

        let flags = PacketFlags(u16::from_be_bytes([bytes[9], bytes[10]]));

        let known_flags = PacketFlags::ENCRYPTED.0
            | PacketFlags::RELAYABLE.0
            | PacketFlags::ACK_REQUIRED.0
            | PacketFlags::FRAGMENT.0
            | PacketFlags::FINAL_FRAGMENT.0;

        if flags.bits() & !known_flags != 0 {
            return Err(PacketError::InvalidFlags);
        }

        let hop_limit = bytes[11];

        if hop_limit == 0 {
            return Err(PacketError::HopLimitExpired);
        }

        let header_length = u16::from_be_bytes([bytes[12], bytes[13]]) as usize;

        if header_length != HEADER_LENGTH {
            return Err(PacketError::InvalidHeaderLength);
        }

        let mut packet_id = [0u8; PACKET_ID_LENGTH];

        packet_id.copy_from_slice(&bytes[14..30]);

        let mut flow_id = [0u8; FLOW_ID_LENGTH];

        flow_id.copy_from_slice(&bytes[30..46]);

        let mut route_token = [0u8; ROUTE_TOKEN_LENGTH];

        route_token.copy_from_slice(&bytes[46..62]);

        let payload_length = u32::from_be_bytes([bytes[62], bytes[63], bytes[64], bytes[65]]);

        if payload_length as usize > MAX_PAYLOAD_SIZE {
            return Err(PacketError::TooLarge);
        }

        let expected_total = HEADER_LENGTH + payload_length as usize;

        if bytes.len() != expected_total {
            return Err(PacketError::InvalidPayloadLength);
        }

        let header = PacketHeader {
            protocol,
            message_type,
            priority,
            flags,
            hop_limit,

            packet_id: PacketId(packet_id),

            flow_id: FlowId(flow_id),

            route_token: RouteToken(route_token),

            payload_length,
        };

        let payload = bytes[HEADER_LENGTH..].to_vec();

        Ok(Self { header, payload })
    }
}
