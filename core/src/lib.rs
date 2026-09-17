pub mod compute;
pub mod connect;
pub mod core;
pub mod discovery;
pub mod errors;
pub mod gateway;
pub mod identity;
pub mod link;
pub mod mesh;
pub mod name_resolution;
pub mod naming;
pub mod naming_protocol;
pub mod network;
pub mod operative;
pub mod operative_os;
pub mod physical;
pub mod platform;
pub mod route;
pub mod runtime;
pub mod security;
pub mod services;
pub mod storage;
pub mod transport;

pub use compute::{
    ComputeCapacity, ComputeError, ComputeNode, DistributedCompute, Placement, Workload,
    WorkloadId, WorkloadSpec,
};
pub use connect::{
    AetherConnect, ConnectError, ConversationId, Group, MediaKind, MessageEnvelope, OfflineMessage,
    Presence,
};
pub use core::{AetherCore, CoreState};
pub use discovery::{
    BootstrapPeer, DEFAULT_MAX_ADVERTISEMENT_ORIGINS, DiscoveryError, DiscoveryGossipError,
    DiscoveryGossipState, DiscoveryKind, FormationState, NetworkFormationEngine, PeerRecord,
    ResourceRecord, ServiceRecord, SignedDiscoveryAdvertisement,
};
pub use errors::CoreError;
pub use gateway::{
    GatewayDescriptor, GatewayDirection, GatewayDirectory, GatewayError, GatewayProtocol,
};
pub use name_resolution::{
    NameDelegation, NameResolutionEngine, ResolutionError, ResolutionKind, SignedNameDelegation,
};
pub use naming::{
    NameError, NameHash, NameRecord, NamingEngine, ResourceId, SignedNameUpdate, canonicalize_name,
};
pub use naming_protocol::{
    DelegationAnnouncement, NAMING_MAGIC, NAMING_VERSION, NameAnnouncement, NameControl, NameQuery,
    NameResponse, NamingCodecError,
};
pub use operative::{
    AetherOperative, AetherPacketBudget, BandwidthGovernor, BandwidthLimits, DataPacket,
    OperativeError, OperativeProfile, TransferDirection, TransferId, TransferState,
};
pub use operative_os::{
    AetherNetOs, BootState, CapabilityLevel, HealthState, NodeResources, OperativeIdentity,
    OperativeStatus, OsError,
};
pub use platform::{
    HostPaths, HostPlatform, RestartPolicy, ServiceLifecycle, ServiceSpec, ServiceStatus,
    ServiceSupervisor,
};
pub use runtime::{STORAGE_QUERY_TTL_SECONDS, StorageRuntime, StorageRuntimeError};
pub use security::{PrivacyContext, PrivacyToken, ReplayWindow, SecurityPolicy, SecurityProfile};
pub use services::{
    BuildArtifact, DeploymentError, DeploymentPlan, SiteFile, prepare_upload, verify_artifact,
};
pub use storage::{
    AssemblyError, ChunkAssembler, ChunkDigest, ChunkId, ChunkRequest, ChunkStore,
    ChunkTransferRequest, ContentChunk, ContentId, ContentManifest, DistributedStorage,
    MAX_ASSEMBLY_BYTES, MAX_MANIFEST_PAGE_ENTRIES, MAX_STORAGE_FRAGMENT, MAX_STORAGE_MESSAGE,
    ManifestStore, ObjectChunk, ObjectManifest, ObjectStore, ObjectStoreError, PendingTransfer,
    ProviderAdvertisement, ProviderDirectory, RepairPlan, RequestDeduplicator, STORAGE_MAGIC,
    STORAGE_VERSION, StorageCodecError, StorageControl, StorageError, TransferGovernor,
    TransferId as StorageTransferId, TransferLimits, TransferQueue,
};
