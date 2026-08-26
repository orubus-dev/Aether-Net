use serde::{Deserialize, Serialize};

pub const PROTOCOL_MAGIC: [u8; 4] = *b"AETH";

pub const PROTOCOL_MAJOR: u8 = 1;
pub const PROTOCOL_MINOR: u8 = 0;

pub const PACKET_HEADER_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
}

impl ProtocolVersion {
    pub const CURRENT: Self = Self {
        major: PROTOCOL_MAJOR,
        minor: PROTOCOL_MINOR,
    };

    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }

    /// Compatible means the major protocol is identical and the remote
    /// minor version does not require unsupported semantics.
    pub const fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    NodeHello = 1,
    NodeHelloAck = 2,
    PeerChallenge = 3,
    PeerResponse = 4,
    RouteAdvertisement = 5,
    RouteProbe = 6,
    RouteProbeResponse = 7,
    Data = 8,
    Acknowledgement = 9,
    KeepAlive = 10,
    ServiceAdvertisement = 11,
    Disconnect = 12,
    Error = 13,
}

impl MessageType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::NodeHello),
            2 => Some(Self::NodeHelloAck),
            3 => Some(Self::PeerChallenge),
            4 => Some(Self::PeerResponse),
            5 => Some(Self::RouteAdvertisement),
            6 => Some(Self::RouteProbe),
            7 => Some(Self::RouteProbeResponse),
            8 => Some(Self::Data),
            9 => Some(Self::Acknowledgement),
            10 => Some(Self::KeepAlive),
            11 => Some(Self::ServiceAdvertisement),
            12 => Some(Self::Disconnect),
            13 => Some(Self::Error),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrafficPriority {
    Emergency = 0,
    Voice = 1,
    Interactive = 2,
    Messaging = 3,
    Browsing = 4,
    Background = 5,
    Prefetch = 6,
}

impl TrafficPriority {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Emergency),
            1 => Some(Self::Voice),
            2 => Some(Self::Interactive),
            3 => Some(Self::Messaging),
            4 => Some(Self::Browsing),
            5 => Some(Self::Background),
            6 => Some(Self::Prefetch),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketFlags(pub u16);

impl PacketFlags {
    pub const NONE: Self = Self(0);
    pub const ENCRYPTED: Self = Self(1 << 0);
    pub const RELAYABLE: Self = Self(1 << 1);
    pub const ACK_REQUIRED: Self = Self(1 << 2);
    pub const FRAGMENT: Self = Self(1 << 3);
    pub const FINAL_FRAGMENT: Self = Self(1 << 4);

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn insert(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
