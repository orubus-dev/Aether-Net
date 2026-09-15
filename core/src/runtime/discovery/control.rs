use super::gossip::{DiscoveryGossipError, SignedDiscoveryAdvertisement};
use super::hello::DiscoveryHello;
use crate::network::NodeId;

const MAGIC: [u8; 4] = *b"AED1";
const VERSION: u8 = 1;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryMessageType {
    Hello = 1,
    HelloAck = 2,
    Advertisement = 3,
}

impl DiscoveryMessageType {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Hello),
            2 => Some(Self::HelloAck),
            3 => Some(Self::Advertisement),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelloAck {
    pub node_id: NodeId,
    pub timestamp: u64,
}

impl HelloAck {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + 1 + 1 + 32 + 8);
        bytes.extend_from_slice(&MAGIC);
        bytes.push(VERSION);
        bytes.push(DiscoveryMessageType::HelloAck as u8);
        bytes.extend_from_slice(self.node_id.as_bytes());
        bytes.extend_from_slice(&self.timestamp.to_be_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ControlError> {
        if bytes.len() != 46 {
            return Err(ControlError::InvalidLength);
        }
        validate_header(bytes, DiscoveryMessageType::HelloAck)?;
        let mut node_id = [0u8; 32];
        node_id.copy_from_slice(&bytes[6..38]);
        let timestamp = u64::from_be_bytes(
            bytes[38..46]
                .try_into()
                .map_err(|_| ControlError::InvalidLength)?,
        );
        Ok(Self {
            node_id: NodeId::from_bytes(node_id),
            timestamp,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryControl {
    Hello(DiscoveryHello),
    HelloAck(HelloAck),
    Advertisement(SignedDiscoveryAdvertisement),
}

impl DiscoveryControl {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Hello(hello) => {
                let payload = hello.encode();
                let mut bytes = Vec::with_capacity(6 + payload.len());
                bytes.extend_from_slice(&MAGIC);
                bytes.push(VERSION);
                bytes.push(DiscoveryMessageType::Hello as u8);
                bytes.extend_from_slice(&payload);
                bytes
            }
            Self::HelloAck(ack) => ack.encode(),
            Self::Advertisement(advertisement) => {
                let payload = advertisement
                    .encode()
                    .expect("validated discovery advertisement exceeds codec bounds");
                let mut bytes = Vec::with_capacity(6 + payload.len());
                bytes.extend_from_slice(&MAGIC);
                bytes.push(VERSION);
                bytes.push(DiscoveryMessageType::Advertisement as u8);
                bytes.extend_from_slice(&payload);
                bytes
            }
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ControlError> {
        if bytes.len() < 6 {
            return Err(ControlError::InvalidLength);
        }
        if bytes[0..4] != MAGIC {
            return Err(ControlError::InvalidMagic);
        }
        if bytes[4] != VERSION {
            return Err(ControlError::UnsupportedVersion);
        }
        let message_type =
            DiscoveryMessageType::from_u8(bytes[5]).ok_or(ControlError::UnknownMessage)?;
        match message_type {
            DiscoveryMessageType::Hello => {
                let hello = DiscoveryHello::decode(&bytes[6..]).map_err(ControlError::Hello)?;
                Ok(Self::Hello(hello))
            }
            DiscoveryMessageType::HelloAck => Ok(Self::HelloAck(HelloAck::decode(bytes)?)),
            DiscoveryMessageType::Advertisement => {
                let advertisement = SignedDiscoveryAdvertisement::decode(&bytes[6..])
                    .map_err(ControlError::Advertisement)?;
                Ok(Self::Advertisement(advertisement))
            }
        }
    }
}

fn validate_header(bytes: &[u8], expected: DiscoveryMessageType) -> Result<(), ControlError> {
    if bytes.len() < 6 {
        return Err(ControlError::InvalidLength);
    }
    if bytes[0..4] != MAGIC {
        return Err(ControlError::InvalidMagic);
    }
    if bytes[4] != VERSION {
        return Err(ControlError::UnsupportedVersion);
    }
    if bytes[5] != expected as u8 {
        return Err(ControlError::UnknownMessage);
    }
    Ok(())
}

#[derive(Debug)]
pub enum ControlError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    UnknownMessage,
    Hello(super::hello::HelloError),
    Advertisement(DiscoveryGossipError),
}
