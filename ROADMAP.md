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
- [x] Ethernet, Wi-Fi, Wi-Fi Direct, Bluetooth LE and UWB descriptors.
- [ ] OS-specific hardware drivers.
- [ ] Live L1/L2 transmission integration.
- [ ] Hardware interoperability tests.

## Phase 6 - Network Formation & Discovery [IMPLEMENTED - RUNTIME INTEGRATION PENDING]
- [x] Bounded bootstrap peers.
- [x] Isolated/forming/connected/partitioned state machine.
- [x] Bounded peer, service and resource discovery state.
- [x] Version monotonicity and expiry cleanup.
- [x] Partition generation tracking and recovery transitions.
- [ ] Live authenticated dissemination.
- [ ] Physical discovery integration.
- [ ] Routing convergence integration.
- [ ] Multi-node partition/heal tests.

## Phase 7 - Naming & Resource Resolution [IMPLEMENTED - DISTRIBUTED INTEGRATION PENDING]
- [x] Canonical Aether name normalization and validation.
- [x] Fixed-size hashed name identifiers.
- [x] Signed name-to-resource records.
- [x] Monotonic record-version convergence.
- [x] Bounded naming database and resolver cache.
- [x] TTL/expiry validation and cleanup.
- [ ] Distributed dissemination.
- [ ] Delegation and ownership transfer.
- [ ] Native resolver traffic over Aether L3/L4.
- [ ] Phase 6/8 integration.

## Phase 8 - Distributed Storage & CDN [IMPLEMENTED - DISTRIBUTED INTEGRATION PENDING]
- [x] Content-addressed chunk digests.
- [x] Bounded local chunk storage.
- [x] Read-time chunk integrity verification.
- [x] Deduplication and TTL garbage collection.
- [x] Versioned manifest convergence.
- [x] Signed provider advertisements.
- [x] Provider selection by capacity/reliability/cost.
- [x] Replica repair planning.
- [ ] Native storage request/response over Aether L3/L4.
- [ ] Live replica transfer and automatic repair.
- [ ] Phase 6/7 integration.
- [ ] Multi-node failure/recovery tests.
- [ ] Strong provider NodeId/signing-key binding.

## Phase 9 - Distributed Compute & Services [IMPLEMENTED - EXECUTION INTEGRATION PENDING]
- [x] Cryptographically-derived workload identifiers.
- [x] Bounded compute-node registry.
- [x] Capacity-aware workload requirements.
- [x] Versioned workload submission.
- [x] Deterministic replica placement.
- [x] Migration permission and target-capacity checks.
- [x] Placement bounds and expiry cleanup.
- [ ] Authenticated capacity advertisements.
- [ ] Sandboxed workload execution.
- [ ] CPU/memory/storage/network quota enforcement.
- [ ] Node attestation and workload provenance.
- [ ] Failure-triggered rescheduling and state migration.
- [ ] Live multi-node compute integration tests.

## Phase 10 - Aether Hosting [IMPLEMENTED - LIVE SERVING INTEGRATION PENDING]
- [x] Upload deployment input model.
- [x] Native domain canonicalization.
- [x] Deterministic artifact hashing.
- [x] Bounded file-size/count validation.
- [x] Artifact verification.
- [x] Site manifest/site identity generation.
- [x] Rollback target metadata.
- [x] GitHub source descriptor model.
- [ ] Live GitHub repository import.
- [ ] Isolated build workers.
- [ ] Signed deployment authorization.
- [ ] Phase 7 name publication and Phase 8 distributed artifact replication.
- [ ] Native HTTP/application serving over Aether L3/L4.
- [ ] Cache invalidation and automatic rollback.
- [ ] Multi-node hosting tests.

## Phase 11 - Aether Connect
- Native messaging, voice, video, presence, groups and offline delivery.

## Phase 12 - Internet Interoperability
- Optional Aether gateways and protocol translation.

## Phase 13 - AetherNet OS & Operatives
- Lightweight infrastructure OS, L0-L5 operation, provisioning, updates, recovery and monitoring.

## Phase 14 - Global Hardening
- Adversarial testing, large-scale routing/discovery validation, abuse resistance, protocol upgrade and production certification.
