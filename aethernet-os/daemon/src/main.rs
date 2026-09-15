#[cfg(windows)]
mod windows_service;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_service::start().map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
}

#[cfg(not(windows))]
mod unix_host {
    use aether_core::{
        AetherNodeRuntime, CoreConfig, HostPaths, RestartPolicy, ServiceLifecycle, ServiceSpec,
        ServiceSupervisor,
    };
    use std::env;
    use std::net::SocketAddr;
    use std::path::Path;
    use tokio::sync::mpsc;

    const DEFAULT_BIND: &str = "0.0.0.0:39000";

    #[tokio::main(flavor = "multi_thread")]
    pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
        let paths = HostPaths::discover();
        paths.create_all()?;

        let bind = parse_bind()?;
        let peers = parse_peers()?;

        let mut supervisor = ServiceSupervisor::new(ServiceSpec {
            restart: RestartPolicy::OnFailure,
            max_restart_attempts: 10,
        });
        supervisor.begin_start();
        write_status(&paths.state_dir, supervisor.status().lifecycle, peers.len())?;

        let runtime = AetherNodeRuntime::bind(CoreConfig::default(), bind).await?;
        supervisor.mark_running();
        write_status(&paths.state_dir, supervisor.status().lifecycle, peers.len())?;

        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        let task = tokio::spawn(runtime.run_with_shutdown(peers, shutdown_rx));

        let result = tokio::select! {
            result = task => {
                result.map_err(|_| aether_core::runtime::node::NodeRuntimeError::Runtime)?
            }
            signal = tokio::signal::ctrl_c() => {
                signal?;
                supervisor.mark_stopping();
                write_status(&paths.state_dir, supervisor.status().lifecycle, 0)?;
                let _ = shutdown_tx.send(()).await;
                supervisor.mark_stopped();
                write_status(&paths.state_dir, supervisor.status().lifecycle, 0)?;
                Ok(())
            }
        };

        if result.is_err() {
            supervisor.mark_failure(unix_timestamp());
            write_status(&paths.state_dir, supervisor.status().lifecycle, 0)?;
        }

        result.map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
    }

    fn parse_bind() -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let value = env::var("AETHERNET_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_owned());
        Ok(value.parse()?)
    }

    fn parse_peers() -> Result<Vec<SocketAddr>, Box<dyn std::error::Error>> {
        let value = match env::var("AETHERNET_PEERS") {
            Ok(value) => value,
            Err(_) => return Ok(Vec::new()),
        };

        value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                value
                    .parse()
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            })
            .collect()
    }

    fn write_status(
        state_dir: &Path,
        lifecycle: ServiceLifecycle,
        peer_count: usize,
    ) -> std::io::Result<()> {
        let target = state_dir.join("service.status");
        let temporary = state_dir.join("service.status.tmp");
        let content = format!(
            "lifecycle={lifecycle:?}\nplatform={:?}\nconfigured_peers={peer_count}\npid={}\nunix_time={}\n",
            aether_core::HostPlatform::current(),
            std::process::id(),
            unix_timestamp()
        );
        std::fs::write(&temporary, content)?;
        std::fs::rename(temporary, target)
    }

    fn unix_timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    unix_host::main()
}
