use crate::naming::{NameHash, NameRecord, ResourceId};

const MAGIC: [u8; 4] = *b"AEN1";
const VERSION: u8 = 1;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameMessageType {
    Query = 1,
    Response = 2,
    Update = 3,
}

impl NameMessageType {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Query),
            2 => Some(Self::Response),
            3 => Some(Self::Update),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameQuery {
    pub request_id: u64,
    pub name_hash: NameHash,
}

impl NameQuery {
    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(6 + 8 + 32);
        output.extend_from_slice(&MAGIC);
        output.push(VERSION);
        output.push(NameMessageType::Query as u8);
        output.extend_from_slice(&self.request_id.to_be_bytes());
        output.extend_from_slice(self.name_hash.as_bytes());
        output
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, NameProtocolError> {
        validate_header(bytes, NameMessageType::Query, 46)?;
        Ok(Self {
            request_id: u64::from_be_bytes(bytes[6..14].try_into().unwrap()),
            name_hash: NameHash::from_bytes(bytes[14..46].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameResponse {
    pub request_id: u64,
    pub resource_id: ResourceId,
    pub version: u64,
    pub expires_at: u64,
}

impl NameResponse {
    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(6 + 8 + 32 + 8 + 8);
        output.extend_from_slice(&MAGIC);
        output.push(VERSION);
        output.push(NameMessageType::Response as u8);
        output.extend_from_slice(&self.request_id.to_be_bytes());
        output.extend_from_slice(self.resource_id.as_bytes());
        output.extend_from_slice(&self.version.to_be_bytes());
        output.extend_from_slice(&self.expires_at.to_be_bytes());
        output
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, NameProtocolError> {
        validate_header(bytes, NameMessageType::Response, 54)?;
        Ok(Self {
            request_id: u64::from_be_bytes(bytes[6..14].try_into().unwrap()),
            resource_id: ResourceId::from_bytes(bytes[14..46].try_into().unwrap()),
            version: u64::from_be_bytes(bytes[46..54].try_into().unwrap()),
            expires_at: u64::from_be_bytes(bytes[54..62].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameUpdate {
    pub record: NameRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProtocol {
    Query(NameQuery),
    Response(NameResponse),
    Update(NameUpdate),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProtocolError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    UnknownMessage,
}

fn validate_header(bytes: &[u8], expected: NameMessageType, minimum: usize) -> Result<(), NameProtocolError> {
    if bytes.len() < minimum {
        return Err(NameProtocolError::InvalidLength);
    }
    if bytes[0..4] != MAGIC {
        return Err(NameProtocolError::InvalidMagic);
    }
    if bytes[4] != VERSION {
        return Err(NameProtocolError::UnsupportedVersion);
    }
    if bytes[5] != expected as u8 {
        return Err(NameProtocolError::UnknownMessage);
    }
    Ok(())
}

impl NameHash {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self::from_bytes_internal(bytes)
    }
}
