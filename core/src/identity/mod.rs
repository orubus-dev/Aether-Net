pub mod aether_id;
pub mod authorization;
pub mod device;
pub mod device_registry;
pub mod fingerprint;
pub mod key_manager;
pub mod recovery;
pub mod revocation;
pub mod signature;
pub mod storage;

pub use aether_id::{AetherId, AetherIdError};

pub use authorization::{AuthorizationGrant, Capability};

pub use device::{
    DeviceCapabilities, DeviceError, DeviceId, DeviceSigningKey, DeviceState, DeviceStatus,
};

pub use device_registry::{DeviceRegistry, RegisteredDevice, RegistryError};

pub use fingerprint::Fingerprint;

pub use key_manager::{IdentityKeyPair, KeyManagerError};

pub use recovery::{
    GuardianId, RecoveryApproval, RecoveryConfiguration, RecoveryError, RecoveryGuardian,
    RecoveryRequest,
};

pub use revocation::{DeviceRevocation, RevocationReason};

pub use signature::{sign, verify, verify_bytes};

pub use storage::{IdentityStore, IdentityVault, VaultError};
