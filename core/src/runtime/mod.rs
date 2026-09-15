pub mod config;
pub mod data_plane;
pub mod discovery;
pub mod events;
pub mod forwarding;
pub mod health;
pub mod metrics;
pub mod native_plane;
pub mod node;
pub mod peers;

pub use config::CoreConfig;

pub use data_plane::{DataPlaneError, EncryptedDataPlane};
pub use native_plane::{NativeAetherDataPlane, NativeDataPlaneError};

pub use discovery::{
    AuthMessageError, ControlError, DiscoveryControl, DiscoveryHello, DiscoveryMessageType,
    HelloAck, HelloError, PeerChallengeMessage, PeerResponseMessage,
};

pub use events::CoreEvent;

pub use forwarding::{
    ForwardCache, ForwardCacheEntry, ForwardCacheKey, ForwardCandidate, ForwardDecision,
    ForwardSelector, ForwardingEngine, ForwardingError, ForwardingPolicy,
};

pub use health::{HealthReport, HealthStatus};

pub use metrics::{CoreMetrics, MetricsSnapshot};

pub use node::{
    AetherNodeRuntime, NodeEvent, NodeEventLoop, NodeLoopError, NodeRuntimeError, NodeRuntimeState,
    NodeState,
};

pub use peers::{
    AuthenticationState, HandshakeError, HandshakeManager, PeerAuthentication, PeerEndpoint,
    PeerSession, PeerSessionRegistry, PeerTable, RuntimePeer, RuntimePeerSession, RuntimePeerState,
    RuntimeSessionState, SessionPolicy, SessionRegistryError,
};
