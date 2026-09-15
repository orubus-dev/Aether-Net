pub mod compat;
pub mod control_plane_runtime;
pub mod control_protocol;
pub mod distributed_fixed;
pub mod engine;
pub mod gateway;
pub mod linux_kernel;
pub mod metric;
pub mod path;
pub mod runtime;

pub use control_plane_runtime::{
    DEFAULT_MAX_DYNAMIC_ROUTES, DEFAULT_MAX_NEIGHBORS, DEFAULT_MAX_PEERS,
    ROUTING_ADVERTISEMENT_MAGIC, ROUTING_PUBLIC_KEY_LENGTH, ROUTING_SIGNATURE_LENGTH,
    RoutingControlError, RoutingControlPlane, RoutingPeer, SignedRouteAdvertisement,
};
pub use control_protocol::{
    ROUTE_CONTROL_ADVERTISEMENT, ROUTE_CONTROL_HEADER_LENGTH, ROUTE_CONTROL_MAGIC,
    ROUTE_CONTROL_MAX_NEIGHBORS, ROUTE_CONTROL_MAX_SIZE, ROUTE_CONTROL_VERSION,
    RouteControlCodecError, decode_advertisement, encode_advertisement,
};
pub use distributed_fixed as distributed;
pub use distributed_fixed::{
    DEFAULT_MAX_ADVERTISEMENT_NEIGHBORS, DEFAULT_MAX_LINKS, DEFAULT_MAX_NODES, DEFAULT_MAX_PATHS,
    DEFAULT_MAX_SEARCH_STATES, DEFAULT_ROUTE_TTL_SECONDS, DistributedRoutingEngine,
    RouteAdvertisement, RouteNeighbor, RoutingError,
};
pub use engine::RouteEngine;
pub use gateway::{GatewayAdvertisement, GatewayCapacity};
pub use linux_kernel::{
    KernelInterfaceAnalytics, KernelInterfaceConfig, LinuxKernelError, LinuxKernelInterface,
};
pub use metric::RouteMetrics;
pub use path::RoutePath;
pub use runtime::{
    DEFAULT_ROUTE_ADVERTISEMENT_INTERVAL_SECONDS, RoutingOutboundFrame, RoutingRuntime,
    RoutingRuntimeError, RoutingTickResult, node_id_from_signing_key,
};
