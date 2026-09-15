# AetherNet OS host architecture

AetherNet OS is being built as a native host environment around the Aether protocol core.
The goal is operating-system-grade integration without moving platform-specific policy into
the network protocol itself.

## Current host layer

`core/src/platform.rs` provides:

- canonical state/config/runtime/log paths per host platform;
- lifecycle states from Created through Running, Degraded, Stopping, Stopped and Failed;
- restart policies and bounded restart attempts;
- isolated directory initialization suitable for service installation.

`aethernet-os/daemon` provides the long-running host process and consumes the same runtime
boundary on Unix-like systems. Linux has a systemd unit with the capabilities required for
raw packet/interface operations. Windows has a Service Control Manager entrypoint using the
`windows-service` crate.

## Production operating-system track

The next layers are platform integration, not protocol redesign:

1. Native service/process supervision with health checks, dependency ordering and crash recovery.
2. Persistent configuration and identity storage with atomic updates and recovery semantics.
3. Native network-interface control and route provisioning on Linux and Windows.
4. Hardware capability discovery and power-aware operation for laptops and phones.
5. Sandboxed service execution, resource limits and per-service permissions.
6. Secure signed update delivery with verified installation, rollback and boot-time recovery.
7. Structured telemetry, diagnostics, crash dumps and support bundles.
8. Installer/uninstaller, firewall integration and user-facing control surfaces.
9. Cross-platform interoperability tests and real-hardware soak testing.
10. Security audit and production certification.

The current code is a host foundation, not a finished replacement for Windows. “Windows-level”
capability here means reaching comparable service lifecycle, hardware integration, recovery,
security, diagnostics and deployment maturity through native adapters and extensive testing.
