use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPlatform {
    Linux,
    Windows,
    Other,
}

impl HostPlatform {
    pub const fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            return Self::Linux;
        }
        #[cfg(target_os = "windows")]
        {
            return Self::Windows;
        }
        Self::Other
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPaths {
    pub state_dir: PathBuf,
    pub config_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl HostPaths {
    pub fn discover() -> Self {
        match HostPlatform::current() {
            HostPlatform::Linux => Self {
                state_dir: path_from_env_or("AETHERNET_STATE_DIR", "/var/lib/aethernet"),
                config_dir: path_from_env_or("AETHERNET_CONFIG_DIR", "/etc/aethernet"),
                runtime_dir: path_from_env_or("AETHERNET_RUNTIME_DIR", "/run/aethernet"),
                log_dir: path_from_env_or("AETHERNET_LOG_DIR", "/var/log/aethernet"),
            },
            HostPlatform::Windows => {
                let root = env::var_os("PROGRAMDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
                let root = root.join("AetherNet");
                Self {
                    state_dir: root.join("state"),
                    config_dir: root.join("config"),
                    runtime_dir: root.join("runtime"),
                    log_dir: root.join("logs"),
                }
            }
            HostPlatform::Other => {
                let home = env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                let root = home.join(".aethernet");
                Self {
                    state_dir: root.join("state"),
                    config_dir: root.join("config"),
                    runtime_dir: root.join("runtime"),
                    log_dir: root.join("logs"),
                }
            }
        }
    }

    pub fn create_all(&self) -> io::Result<()> {
        fs::create_dir_all(&self.state_dir)?;
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(&self.runtime_dir)?;
        fs::create_dir_all(&self.log_dir)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceLifecycle {
    Created,
    Starting,
    Running,
    Degraded,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceSpec {
    pub restart: RestartPolicy,
    pub max_restart_attempts: u32,
}

impl Default for ServiceSpec {
    fn default() -> Self {
        Self {
            restart: RestartPolicy::OnFailure,
            max_restart_attempts: 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceStatus {
    pub lifecycle: ServiceLifecycle,
    pub restart_count: u32,
    pub last_failure_unix: u64,
}

impl Default for ServiceStatus {
    fn default() -> Self {
        Self {
            lifecycle: ServiceLifecycle::Created,
            restart_count: 0,
            last_failure_unix: 0,
        }
    }
}

pub struct ServiceSupervisor {
    spec: ServiceSpec,
    status: ServiceStatus,
}

impl ServiceSupervisor {
    pub fn new(spec: ServiceSpec) -> Self {
        Self {
            spec,
            status: ServiceStatus::default(),
        }
    }

    pub fn status(&self) -> ServiceStatus {
        self.status
    }

    pub fn begin_start(&mut self) {
        self.status.lifecycle = ServiceLifecycle::Starting;
    }

    pub fn mark_running(&mut self) {
        self.status.lifecycle = ServiceLifecycle::Running;
    }

    pub fn mark_degraded(&mut self) {
        self.status.lifecycle = ServiceLifecycle::Degraded;
    }

    pub fn mark_stopping(&mut self) {
        self.status.lifecycle = ServiceLifecycle::Stopping;
    }

    pub fn mark_stopped(&mut self) {
        self.status.lifecycle = ServiceLifecycle::Stopped;
    }

    pub fn mark_failure(&mut self, now_unix: u64) -> bool {
        self.status.lifecycle = ServiceLifecycle::Failed;
        self.status.last_failure_unix = now_unix;
        if self.spec.restart == RestartPolicy::Never {
            return false;
        }
        if self.status.restart_count >= self.spec.max_restart_attempts {
            return false;
        }
        self.status.restart_count = self.status.restart_count.saturating_add(1);
        true
    }
}

fn path_from_env_or(variable: &str, fallback: &str) -> PathBuf {
    env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(fallback))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_paths_can_be_created_in_an_isolated_tree() {
        let root = std::env::temp_dir().join(format!("aethernet-platform-{}", std::process::id()));
        let paths = HostPaths {
            state_dir: root.join("state"),
            config_dir: root.join("config"),
            runtime_dir: root.join("runtime"),
            log_dir: root.join("logs"),
        };
        paths.create_all().unwrap();
        assert!(paths.state_dir.is_dir());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn supervisor_respects_restart_limit() {
        let mut supervisor = ServiceSupervisor::new(ServiceSpec {
            restart: RestartPolicy::OnFailure,
            max_restart_attempts: 2,
        });
        assert!(supervisor.mark_failure(1));
        assert!(supervisor.mark_failure(2));
        assert!(!supervisor.mark_failure(3));
        assert_eq!(supervisor.status().restart_count, 2);
    }
}
