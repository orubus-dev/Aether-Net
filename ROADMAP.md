# AetherNet Roadmap

## Phase 1 - Core Protocol Foundation [COMPLETE]

- Native Aether L3/L4 boundary defined.
- Native 256-bit Aether address primitive.
- Native L3 packet envelope and protocol invariants.
- Secure-by-default runtime security profiles.
- Rotating privacy-token primitive separated from stable identity.
- Allocation-free replay protection window.
- Security/privacy/performance acceptance gate documented.

## Phase 2 - Native L3 Network Engine [IN PROGRESS]

- [x] End-to-end native addressing.
- [x] Native route state with bounded forwarding table.
- [x] Destination lookup and next-hop selection.
- [x] Hop-limit enforcement during forwarding.
- [x] Bounded fragmentation and reassembly.
- [x] Duplicate/conflicting fragment rejection.
- [x] Reassembly memory and timeout bounds.
- [ ] Real multi-node forwarding integrated into the authenticated runtime data path.

## Phase 3 - Native L4 Transport Engine [IN PROGRESS]

- [x] Native L4 segment format with explicit sequence, cumulative ACK, and receive window.
- [x] Datagram transport mode.
- [x] Ordered stream transport.
- [x] Reliable stream retransmission and bounded in-flight state.
- [x] Receive-window flow control.
- [x] AIMD congestion control and bounded retransmission timing.
- [x] Realtime freshness-oriented delivery.
- [x] FIN/RESET session lifecycle.
- [ ] Live L4 session multiplexing over the authenticated native L3 runtime.
- [ ] Multi-node transport integration tests.

## Phase 4 - Distributed Routing Engine [IMPLEMENTED - INTEGRATION PENDING]

- [x] Distributed topology database with explicit node/link bounds.
- [x] Versioned route advertisements with monotonic sequence convergence.
- [x] Advertisement expiry and topology garbage collection.
- [x] Native metric aggregation across paths.
- [x] Priority-aware shortest-path selection.
- [x] Bounded alternate-path / multipath discovery.
- [x] Loop-free path construction.
- [x] Search-computation budget to prevent unbounded routing work.
- [ ] Signed/authenticated route-advertisement integration with the peer security plane.
- [ ] Runtime dissemination and convergence across live Aether nodes.
- [ ] Failure-repair integration with the Phase 2 forwarding engine.

## Phase 5 - Physical Underlay Engine [IMPLEMENTED - PLATFORM INTEGRATION PENDING]

- [x] Transport-independent physical adapter contract.
- [x] Explicit underlay properties including MTU, bandwidth, latency, energy, broadcast and multicast capabilities.
- [x] Bounded physical adapter registry.
- [x] Bounded physical-link registry and validation.
- [x] Discovery and per-transport refresh.
- [x] Deterministic active-link selection.
- [x] MTU-aware frame validation before transmission.
- [x] Aggregated device physical-capability reporting.
- [x] Ethernet adapter descriptor.
- [x] Wi-Fi adapter descriptor.
- [x] Wi-Fi Direct adapter descriptor.
- [x] Bluetooth LE adapter descriptor.
- [x] UWB direct-link adapter descriptor.
- [ ] OS-specific hardware drivers for each supported carrier.
- [ ] Live L1/L2 transmission integration with native Aether links.
- [ ] Hardware interoperability and multi-underlay integration tests.

## Phase 6 - Network Formation & Discovery

- Distributed peer/service/resource discovery.
- Bootstrap and partition recovery.

## Phase 7 - Naming & Resource Resolution

- Aether names/domains.
- Signed records.
- Distributed resolution and caching.

## Phase 8 - Distributed Storage & CDN

- Chunking, replication, repair, provider selection and caching.

## Phase 9 - Distributed Compute & Services

- Workload placement, service execution, migration and resource policies.

## Phase 10 - Aether Hosting

- GitHub import.
- Upload deployment.
- Build/package/verify.
- Domain assignment.
- Distributed hosting, replicas, caching and rollback.

## Phase 11 - Aether Connect

- Native messaging, voice, video, presence, groups and offline delivery.

## Phase 12 - Internet Interoperability

- Optional Aether gateways and protocol translation.

## Phase 13 - AetherNet OS & Operatives

- Lightweight infrastructure OS.
- L0-L5 capability operation.
- Provisioning, updates, recovery and monitoring.

## Phase 14 - Global Hardening

- Adversarial testing.
- Large-scale routing/discovery validation.
- Abuse resistance.
- Protocol upgrade and production certification process.
