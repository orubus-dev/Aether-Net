#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Copy)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub identity_ready: bool,
    pub network_ready: bool,
    pub link_ready: bool,
    pub runtime_ready: bool,
}

impl HealthReport {
    pub fn healthy() -> Self {
        Self {
            status: HealthStatus::Healthy,
            identity_ready: true,
            network_ready: true,
            link_ready: true,
            runtime_ready: true,
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
            && self.identity_ready
            && self.network_ready
            && self.link_ready
            && self.runtime_ready
    }
}
