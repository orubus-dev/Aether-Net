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

## Phase 3 - Native L4 Transport Engine [IMPLEMENTED - INTEGRATION PENDING]

- [x] Native L4 wire envelope with flow, sequence, cumulative acknowledgement, and receive-window fields.
- [x] Datagram transport mode.
- [x] Ordered stream transport mode.
- [x] Reliable stream mode with bounded in-flight state.
- [x] Cumulative acknowledgements.
- [x] Bounded receive buffering and advertised receive window.
- [x] Retransmission timers with bounded RTO.
- [x] AIMD congestion-window growth and loss response.
- [x] Realtime transport that drops stale packets instead of retransmitting them.
- [x] FIN/RESET lifecycle controls and flow validation.
- [ ] Native L4 session multiplexing integrated into the live runtime data path.
- [ ] Multi-node transport integration and interoperability tests.

## Phase 4 - Distributed Routing Engine

- Route discovery.
- Distributed topology.
- Alternate paths and repair.
- Multipath and adaptive routing.

## Phase 5 - Physical Underlay Engine

- Wi-Fi, Ethernet, Bluetooth, direct links and future radio adapters.
- Underlay abstraction and capability negotiation.

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
