pub mod address;
pub mod capabilities;
pub mod l3;
pub mod l3_engine;
pub mod node;
pub mod packet;
pub mod peer;
pub mod protocol;
pub mod registry;
pub mod service;

pub use address::{AetherAddress, AETHER_ADDRESS_LENGTH};

pub use capabilities::NodeCapabilities;

pub use l3::{AetherFlowId, AetherL3Packet, L3Error, L3Flags, L3_HEADER_LENGTH, L3_MAGIC, L3_VERSION};
pub use l3_engine::{
    AetherL3Engine, L3Delivery, L3EngineError, L3ForwardDecision, L3Route, DEFAULT_MAX_ROUTES,
};

pub use node::{NodeDescriptor, NodeId, NodeRole};

pub use packet::{AetherPacket, FlowId, PacketError, PacketHeader, PacketId, RouteToken};

pub use peer::{
    Challenge, NodeHello, NodeHelloAck, Peer, PeerChallenge, PeerFailure, PeerResponse, PeerState,
};

pub use protocol::{MessageType, PacketFlags, ProtocolVersion, TrafficPriority};

pub use registry::NodeRegistry;

pub use service::{ServiceAdvertisement, ServiceId, ServiceRegistry};
