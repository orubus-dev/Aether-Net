# AetherNet Phase 6 — Network Formation & Discovery

## Purpose

Phase 6 turns the discovery protocol into a runtime network-formation subsystem.
Discovery is bounded and stateful: peers, services, and resources have lifetimes,
updates are monotonic where versions exist, and the runtime exposes partition/recovery
transitions rather than treating a peer table as permanent truth.

## Runtime flow

```text
Physical / bootstrap carrier
        |
        v
Discovery HELLO
        |
        v
Identity + protocol validation
        |
        v
NodeEventLoop
        |
        +--> peer table
        |
        +--> DiscoveryFormation
                 |
                 +--> NetworkFormationEngine
                 |      |- bounded peers
                 |      |- bounded services
                 |      |- bounded resources
                 |      |- bootstrap set
                 |      |- expiry / GC
                 |      `- partition generation
                 |
                 `--> FormationChanged event
```

## Formation states

- `Isolated`: no usable discovered peers and no active bootstrap path.
- `Forming`: the node has a path it can attempt to use to discover the network.
- `Connected`: at least one current peer observation exists.
- `Partitioned`: the runtime has explicitly detected or declared a partition.

A formation generation increments when partition lifecycle changes occur so dependent
components can invalidate stale topology assumptions.

## Runtime behavior

The node event loop begins formation when configured with advertised peers, imports a
validated peer observation when HELLO is received, removes the corresponding formation
record when the existing runtime peer expires, and periodically garbage-collects stale
records. Formation changes are emitted as `NodeEvent::FormationChanged` for direct
telemetry/control-plane integration.

The current UDP discovery loop is a bootstrap/control carrier. It does not redefine the
Aether L3/L4 semantics; native physical drivers remain the data-plane substrate.

## Bounds

The engine defaults to 16,384 peers, 16,384 services, 65,536 resources and 64 bootstrap
peers. Record expiry prevents unbounded stale state. These are safety bounds, not claims
about deployment-scale limits.

## Next integration stage

The next networking integration is to advertise and synchronize discovery through native
L2/L3 links so formation no longer depends on the UDP bootstrap carrier. Service/resource
records can then feed naming, storage, compute, hosting and routing automatically.
