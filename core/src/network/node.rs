use crate::identity::device::{DeviceId, DeviceState};
use crate::identity::fingerprint::Fingerprint;
use crate::network::capabilities::NodeCapabilities;
use crate::network::protocol::ProtocolVersion;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const NODE_ID_LENGTH: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; NODE_ID_LENGTH]);

impl NodeId {
    pub fn from_device(
        device: &DeviceState,
    ) -> Self {
        let fingerprint = Fingerprint::from_domain(
            b"aether/node-id/v1",
            &device.identity.public_key,
        );

        Self(*fingerprint.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; NODE_ID_LENGTH] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; NODE_ID_LENGTH] {
        self.0
    }
}

impl fmt::Debug for NodeId {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str("NodeId(")?;

        for byte in &self.0 {
            write!(formatter, "{byte:02x}")?;
        }

        formatter.write_str(")")
    }
}

impl fmt::Display for NodeId {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        for byte in &self.0 {
            write!(formatter, "{byte:02x}")?;
        }

        Ok(())
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Device = 1,
    Relay = 2,
    Gateway = 3,
    Operative = 4,
    Core = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDescriptor {
    pub node_id: NodeId,
    pub device_id: DeviceId,
    pub protocol: ProtocolVersion,
    pub capabilities: NodeCapabilities,
    pub role: NodeRole,
    pub announced_at: u64,
}

impl NodeDescriptor {
    pub fn from_device(
        device: &DeviceState,
        capabilities: NodeCapabilities,
        role: NodeRole,
        announced_at: u64,
    ) -> Self {
        Self {
            node_id: NodeId::from_device(device),
            device_id: device.identity.device_id,
            protocol: ProtocolVersion::CURRENT,
            capabilities,
            role,
            announced_at,
        }
    }

    pub fn can_relay(&self) -> bool {
        self.capabilities.contains(NodeCapabilities::RELAY)
    }

    pub fn can_gateway(&self) -> bool {
        self.capabilities.contains(NodeCapabilities::GATEWAY)
    }
}
