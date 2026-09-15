pub mod address;
pub mod capabilities;
pub mod l3;
pub mod l3_engine;
pub mod l3_fragment;
pub mod node;
pub mod packet;
pub mod peer;
pub mod protocol;
pub mod registry;
pub mod service;

pub use address::{AETHER_ADDRESS_LENGTH, AetherAddress};
pub use capabilities::NodeCapabilities;
pub use l3::{
    AetherFlowId, AetherL3Packet, L3_HEADER_LENGTH, L3_MAGIC, L3_VERSION, L3Error, L3Flags,
};
pub use l3_engine::{
    AetherL3Engine, DEFAULT_MAX_ROUTES, L3Delivery, L3EngineError, L3ForwardDecision, L3Route,
};
pub use l3_fragment::{
    DEFAULT_MAX_REASSEMBLIES, DEFAULT_MAX_REASSEMBLY_BYTES, DEFAULT_REASSEMBLY_TIMEOUT,
    FragmentError, L3ReassemblyTable, fragment_packet,
};
pub use node::{NodeDescriptor, NodeId, NodeRole};
pub use packet::{AetherPacket, FlowId, PacketError, PacketHeader, PacketId, RouteToken};
pub use peer::{
    Challenge, NodeHello, NodeHelloAck, Peer, PeerChallenge, PeerFailure, PeerResponse, PeerState,
};
pub use protocol::{MessageType, PacketFlags, ProtocolVersion, TrafficPriority};
pub use registry::NodeRegistry;
pub use service::{ServiceAdvertisement, ServiceId, ServiceRegistry};
