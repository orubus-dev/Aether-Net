use crate::network::{NodeCapabilities, NodeRole, ProtocolVersion};

const MAGIC: [u8; 4] = *b"AEH1";
const VERSION: u8 = 1;
const HELLO_LENGTH: usize = 4 + 1 + 2 + 32 + 32 + 8 + 1 + 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryHello {
    pub protocol: ProtocolVersion,
    pub node_id: [u8; 32],
    pub device_id: [u8; 32],
    pub capabilities: NodeCapabilities,
    pub role: NodeRole,
    pub timestamp: u64,
}

impl DiscoveryHello {
    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(HELLO_LENGTH);

        output.extend_from_slice(&MAGIC);
        output.push(VERSION);

        output.push(self.protocol.major);
        output.push(self.protocol.minor);

        output.extend_from_slice(&self.node_id);
        output.extend_from_slice(&self.device_id);

        output.extend_from_slice(&self.capabilities.bits().to_be_bytes());

        output.push(self.role as u8);

        output.extend_from_slice(&self.timestamp.to_be_bytes());

        output
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, HelloError> {
        if bytes.len() != HELLO_LENGTH {
            return Err(HelloError::InvalidLength);
        }

        if bytes[0..4] != MAGIC {
            return Err(HelloError::InvalidMagic);
        }

        if bytes[4] != VERSION {
            return Err(HelloError::UnsupportedVersion);
        }

        let protocol = ProtocolVersion::new(bytes[5], bytes[6]);

        if !ProtocolVersion::CURRENT.is_compatible_with(protocol) {
            return Err(HelloError::ProtocolMismatch);
        }

        let mut node_id = [0u8; 32];
        node_id.copy_from_slice(&bytes[7..39]);

        let mut device_id = [0u8; 32];
        device_id.copy_from_slice(&bytes[39..71]);

        let capabilities = NodeCapabilities::from_bits(u64::from_be_bytes(
            bytes[71..79]
                .try_into()
                .map_err(|_| HelloError::InvalidLength)?,
        ));

        let role = match bytes[79] {
            1 => NodeRole::Device,
            2 => NodeRole::Relay,
            3 => NodeRole::Gateway,
            4 => NodeRole::Operative,
            5 => NodeRole::Core,
            _ => return Err(HelloError::InvalidRole),
        };

        let timestamp = u64::from_be_bytes(
            bytes[80..88]
                .try_into()
                .map_err(|_| HelloError::InvalidLength)?,
        );

        Ok(Self {
            protocol,
            node_id,
            device_id,
            capabilities,
            role,
            timestamp,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelloError {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    ProtocolMismatch,
    InvalidRole,
}
