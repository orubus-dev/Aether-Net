use aether_core::{AetherNodeRuntime, CoreConfig, HostPaths, RestartPolicy, ServiceSpec, ServiceSupervisor};
use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc as tokio_mpsc;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

pub const SERVICE_NAME: &str = "AetherNet";

pub fn start() -> windows_service::Result<()> {
    windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

windows_service::define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    if let Err(error) = run() {
        eprintln!("AetherNet Windows service failed: {error}");
    }
}

fn run() -> windows_service::Result<()> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let event_handler = move |control| -> ServiceControlHandlerResult {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = stop_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
    set_status(&status_handle, ServiceState::StartPending, 10, Duration::from_secs(10));

    let runtime = Runtime::new().map_err(|error| windows_service::Error::Winapi(error))?;
    let (shutdown_tx, shutdown_rx) = tokio_mpsc::channel(1);

    let host = HostPaths::discover();
    if let Err(error) = host.create_all() {
        set_status(&status_handle, ServiceState::Stopped, 0, Duration::default());
        eprintln!("AetherNet host directory initialization failed: {error}");
        return Ok(());
    }

    set_status(&status_handle, ServiceState::Running, 0, Duration::default());

    runtime.block_on(async move {
        let bind = std::env::var("AETHERNET_BIND")
            .unwrap_or_else(|_| "0.0.0.0:39000".to_owned())
            .parse()
            .map_err(|error| format!("invalid AETHERNET_BIND: {error}"))?;

        let peers = std::env::var("AETHERNET_PEERS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.parse().map_err(|error| format!("invalid peer {value}: {error}")))
            .collect::<Result<Vec<std::net::SocketAddr>, _>>()?;

        let node = AetherNodeRuntime::bind(CoreConfig::default(), bind)
            .await
            .map_err(|error| error.to_string())?;

        let runner = tokio::spawn(node.run_with_shutdown(peers, shutdown_rx));
        let _ = stop_rx.recv();
        let _ = shutdown_tx.send(()).await;
        runner
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())
    }).map_err(|error: String| windows_service::Error::Winapi(std::io::Error::other(error)))?;

    set_status(&status_handle, ServiceState::Stopped, 0, Duration::default());
    let _ = ServiceSupervisor::new(ServiceSpec {
        restart: RestartPolicy::OnFailure,
        max_restart_attempts: 10,
    });
    Ok(())
}

fn set_status(
    handle: &windows_service::service_control_handler::ServiceStatusHandle,
    state: ServiceState,
    checkpoint: u32,
    wait_hint: Duration,
) {
    let controls = if state == ServiceState::Running {
        ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
    } else {
        ServiceControlAccept::empty()
    };

    let _ = handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: controls,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint,
        wait_hint,
        process_id: None,
    });
}
