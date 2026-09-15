pub mod config;
pub mod data_plane;
pub mod discovery;
pub mod events;
pub mod forwarding;
pub mod health;
pub mod metrics;
pub mod naming;
pub mod native_plane;
pub mod native_stack;
pub mod node;
pub mod peers;

pub use config::CoreConfig;
pub use data_plane::{DataPlaneError, EncryptedDataPlane};
pub use discovery::{
    AuthMessageError, ControlError, DiscoveryControl, DiscoveryFormation, DiscoveryHello,
    DiscoveryMessageType, HelloAck, HelloError, PeerChallengeMessage, PeerResponseMessage,
};
pub use events::CoreEvent;
pub use forwarding::{
    ForwardCache, ForwardCacheEntry, ForwardCacheKey, ForwardCandidate, ForwardDecision,
    ForwardSelector, ForwardingEngine, ForwardingError, ForwardingPolicy,
};
pub use health::{HealthReport, HealthStatus};
pub use metrics::{CoreMetrics, MetricsSnapshot};
pub use naming::{NAME_QUERY_TTL_SECONDS, NameRuntime, NameRuntimeError};
pub use native_plane::{NativeAetherDataPlane, NativeDataPlaneError};
pub use native_stack::{
    DEFAULT_MAX_L4_SESSIONS, DEFAULT_MAX_PEER_SESSIONS, NativeAetherStack, NativeIngress,
    NativeStackError,
};
pub use node::{
    AetherNodeRuntime, NodeEvent, NodeEventLoop, NodeLoopError, NodeRuntimeError, NodeRuntimeState,
    NodeState,
};
pub use peers::{
    AuthenticationState, HandshakeError, HandshakeManager, PeerAuthentication, PeerEndpoint,
    PeerSession, PeerSessionRegistry, PeerTable, RuntimePeer, RuntimePeerSession, RuntimePeerState,
    RuntimeSessionState, SessionPolicy, SessionRegistryError,
};
