use crate::errors::CoreError;
use crate::identity::{AetherId, DeviceSigningKey, DeviceState};
use crate::network::NodeRegistry;
use crate::route::RouteEngine;
use crate::runtime::{CoreConfig, CoreMetrics, HealthReport};
use crate::services::RuntimeServiceRegistry;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    Created,
    Initialized,
    Running,
    Stopping,
    Stopped,
}

pub struct AetherCore {
    config: CoreConfig,
    identity: Option<AetherId>,
    device_key: Option<DeviceSigningKey>,
    device_state: Option<DeviceState>,
    node_registry: NodeRegistry,
    route_engine: RouteEngine,
    service_registry: RuntimeServiceRegistry,
    metrics: Arc<CoreMetrics>,
    state: CoreState,
}

impl AetherCore {
    pub fn new(config: CoreConfig) -> Self {
        Self {
            config,
            identity: None,
            device_key: None,
            device_state: None,
            node_registry: NodeRegistry::new(),
            route_engine: RouteEngine::new(),
            service_registry: RuntimeServiceRegistry::new(),
            metrics: Arc::new(CoreMetrics::default()),
            state: CoreState::Created,
        }
    }

    pub fn initialize(&mut self) -> Result<(), CoreError> {
        if self.state != CoreState::Created {
            return Err(CoreError::AlreadyInitialized);
        }

        let identity = AetherId::generate().map_err(|_| CoreError::IdentityFailure)?;

        let device_key = DeviceSigningKey::generate().map_err(|_| CoreError::DeviceFailure)?;

        let device_state =
            DeviceState::from_signing_key(&device_key, current_timestamp(), Default::default());

        self.identity = Some(identity);

        self.device_key = Some(device_key);

        self.device_state = Some(device_state);

        self.state = CoreState::Initialized;

        Ok(())
    }

    pub fn start(&mut self) -> Result<(), CoreError> {
        if self.state != CoreState::Initialized {
            return Err(CoreError::InvalidState);
        }

        self.state = CoreState::Running;

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), CoreError> {
        if self.state != CoreState::Running {
            return Err(CoreError::InvalidState);
        }

        self.state = CoreState::Stopping;

        self.state = CoreState::Stopped;

        Ok(())
    }

    pub fn state(&self) -> CoreState {
        self.state
    }

    pub fn config(&self) -> &CoreConfig {
        &self.config
    }

    pub fn identity(&self) -> Option<&AetherId> {
        self.identity.as_ref()
    }

    pub fn device(&self) -> Option<&DeviceState> {
        self.device_state.as_ref()
    }

    pub fn device_key(&self) -> Option<&DeviceSigningKey> {
        self.device_key.as_ref()
    }

    pub fn node_registry(&self) -> &NodeRegistry {
        &self.node_registry
    }

    pub fn route_engine(&self) -> &RouteEngine {
        &self.route_engine
    }

    pub fn service_registry(&self) -> &RuntimeServiceRegistry {
        &self.service_registry
    }

    pub fn metrics(&self) -> &CoreMetrics {
        self.metrics.as_ref()
    }

    pub fn health(&self) -> HealthReport {
        let identity_ready = self.identity.is_some();

        let device_ready = self.device_state.is_some() && self.device_key.is_some();

        let runtime_ready = matches!(
            self.state,
            CoreState::Initialized | CoreState::Running | CoreState::Stopped
        );

        HealthReport {
            status: if identity_ready && device_ready && runtime_ready {
                crate::runtime::HealthStatus::Healthy
            } else {
                crate::runtime::HealthStatus::Unhealthy
            },

            identity_ready,
            network_ready: device_ready,
            link_ready: device_ready,
            runtime_ready,
        }
    }
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
