use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreError {
    NotInitialized,
    AlreadyInitialized,
    InvalidState,
    IdentityFailure,
    DeviceFailure,
    AuthorizationFailure,
    StorageFailure,
    NetworkFailure,
    LinkFailure,
    RoutingFailure,
    ServiceFailure,
    ConfigurationFailure,
    ProtocolFailure,
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotInitialized => "core is not initialized",

            Self::AlreadyInitialized => "core is already initialized",

            Self::InvalidState => "invalid core state",

            Self::IdentityFailure => "identity operation failed",

            Self::DeviceFailure => "device operation failed",

            Self::AuthorizationFailure => "authorization failed",

            Self::StorageFailure => "secure storage operation failed",

            Self::NetworkFailure => "network operation failed",

            Self::LinkFailure => "secure link operation failed",

            Self::RoutingFailure => "routing operation failed",

            Self::ServiceFailure => "service operation failed",

            Self::ConfigurationFailure => "configuration error",

            Self::ProtocolFailure => "protocol error",
        };

        write!(formatter, "{message}")
    }
}

impl std::error::Error for CoreError {}
